# Diferencial de disco: Rust x AppleWin (HU-0301, HU-0303)

Comparação passo a passo do Disk II (DSK/DO/PO, NIB, WOZ, 2MG, gz, zip) e do controlador de HDD (SmartPort e modo bloco) do AppleII-Rust com o oráculo executável (`audetto-oracle`, fork audetto/AppleWin `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da`, upstream `3e8054b`). Formato e convenções do Trace v1: `docs/conformance/test-oracles.md`. Registro paralelo do diferencial de CPU: `docs/conformance/audit/cpu-trace-differential.md`.

## Resultado

- **53 casos `disk-*` com `authority = "applewin"`**, todos PASS: 103.146.754 instruções comparadas (ciclo, PC, A, X, Y, SP, P de **cada** uma) e o SHA-256 de cada imagem ao final da execução (o oráculo grava nas imagens; o Rust grava nos arquivos, e os dois conteúdos têm de coincidir).
- **Nenhuma divergência de comportamento do Disk II** (tempo de nibble, motor e atraso de 1 s, passo do braço, quarter tracks, sensor de write-protect, latch por endereço, nibble parcial, WOZ com bits aleatórios e jitter) nem do HDC sobrou depois de alinhar o ambiente (itens D1 a D3). Os três itens eram diferenças de ambiente que o diferencial de CPU também achou e já corrigiu em `main` (`cpu-trace-differential.md` D4, D6, D7). Eu os vi como sintomas antes de mesclar `main` (D1 e D2 com a causa; D3 só como "o floating bus devolve 00 onde o oráculo devolve A0") e adotei as correções de `main`.
- Divergências que **não** viraram caso: o HDC no ][+ (item L1, o oráculo é mais antigo que a base do port).
- Defeitos do Rust achados no caminho (sem relação com o oráculo): `DiskImage::write_block` aceitava o bloco `0xFFFFFFFF` e pedia 2 TB de memória (achado pelo fuzzing, corrigido; ver AUD-C-035).

| Grupo | Casos | Cobertura |
|---|---:|---|
| Boot do DOS 3.3 System Master (][+ e //e Enhanced, `memclear` 0 e 2) | 4 | 2,1 a 2,9 milhões de instruções cada: varredura de slots, recalibração, leitura de nibble, `$BD34` ("o disco está girando?"), atraso de motor, seeks do RWTS |
| Boot do ProDOS 2.4.3 (][+, //e Enhanced, `memclear` 0 e 2) e em 2IMG com ordem ProDOS | 4 | 3,7 milhões de instruções cada |
| Boot de imagens reais do AppleWin-Test (WOZ: Gruds in Space, Wasteland, Legacy of the Ancients, Balance of Power, Accolade's Comics, Run For It, Wizardry III, Buzzard Bait e Glutton dentro de `.zip`; DSK: Gemstone Warrior; `.do.gz`: Ultima V com e sem "Enhance Disk Speed") | 12 | jitter T$00/T$21+, bit timing 0x20, WOZ protegido, contêiner zip e gz, velocidade autêntica |
| Micro-programas de boot (`tests/conformance/inputs/disk-micro`) | 25 | latch com motor desligado, endereços pares, write-protect, nibble parcial (DSK, autêntico, WOZ), cabeça além da trilha 35, período de rotação (DSK, autêntico, WOZ), gravação de setor (DO, GZ, WOZ, trilha 1, motor ligado, drive 2 com WOZ no drive 1), trilha WOZ grande, trilha sem fluxo, quarter tracks, dois drives 2MG, WOZ inválido, arquivo de 0 bytes |
| HDD (`hdd-*`) | 8 | status, leitura sequencial e aleatória, escrita, bloco fora do limite, DMA em `$Cnnn`/ROM e wrap de 64 KiB, SmartPort em `$08FD`, firmware SmartPort e modo bloco, imagem de 32 MB (`GNR-FINAL.2mg.gz`), `HDD-small`, `HDD-small(write-protected)`, `HDD-RW-IO-Boundary` |

Os 16 casos com imagens do AppleWin-Test (corpus, não versionado: 9 WOZ, Gemstone Warrior, Ultima V x2 e 4 HDD) ficam BLOCKED ("... ausente") onde o corpus não foi baixado; os 37 restantes rodam em qualquer checkout. `target/release/conformance run --all` e os testes do crate dão 0 FAIL.

## Como funciona

1. **Oráculo.** `tools/conformance/oracle/run-audetto-oracle.sh --model M --cycles N --memclear K --disk A [--disk2 B] [--hdd H] [--authentic]` sobre **cópias** das imagens. O `main.cpp` ganhou `--disk2`, `--hdd`, `--hdd2` e `--authentic` (registro `Enhance Disk Speed`); sem elas o resto do oráculo não mudou. O caminho `--disk`, que o roteiro dizia nunca ter rodado, funciona (gz, zip, zero bytes e 2MG inclusos).
2. **Rust.** `apple2_conformance::disk::build` monta a máquina do caso (modelo, cartões, `memclear` com a RAM determinística do oráculo, Disk II no slot 6, HDC no slot 7, imagens copiadas para um diretório temporário e abertas para escrita) e `Stepper` executa como o oráculo: passos de instrução, entradas de interrupção sem linha e `update_cards` a cada 1020 ciclos.
3. **Referência compacta.** Um trace de 12 milhões de ciclos passa de 100 MB; o caso guarda um `.ckpt` (`tests/conformance/reference-traces/disk/`): um checkpoint a cada 2000 instruções e o hash FNV-1a encadeado de todas as linhas (ciclo, PC, A, X, Y, SP, P|0x30), mais o SHA-256 do trace completo, o comando do oráculo e o SHA-256 de cada imagem ao final. `.gz` é comparado descomprimido.
4. **Geração.** `tools/conformance/disk/gen-disk-cases.sh [filtro]` lê `disk-cases.txt`, roda o oráculo, compara com `cargo run --release -p apple2-storage --example disk_follow` (lock-step linha a linha: PC, ciclo e registradores precisam bater, e aceita *opcionalmente* sincronizar A/X/Y/P do Rust com os do oráculo depois de uma leitura de barramento flutuante divergente; nenhum dos 53 casos precisou) e grava `.toml` e `.ckpt`. Os micro-programas são montados por `--example make_micro` (montador 6502 de ~200 linhas) e ficam em `tests/conformance/inputs/disk-micro` (688 KB; `.do.gz` de 3,4 KB, WOZ de 15 a 61 KB).
5. **Casos.** `oracle.kind = "trace-checkpoints"`, novos campos de caso `fixture2`, `hdd`, `hdd2`, `authentic` e `readonly`.

## Divergências do ambiente (já registradas em `cpu-trace-differential.md`)

### D1. `rand()` do Disk II

O oráculo usa a `rand()` da glibc com `srand(1)`; o Rust usava um xorshift. Qualquer bit aleatório de WOZ (MC3470, jitter T$00 e T$21+, latch de drive vazio) divergia: com sete boots WOZ reais a primeira divergência estava entre 256 mil e 1,7 milhão de instruções (por exemplo A=`0A` x `02` em `$C6xx`). Com o gerador TYPE_3 da glibc os sete passaram a bater. Corrigido em `main` (`cdf6832`, D6); minha primeira versão, equivalente, foi descartada na mesclagem.

### D2. Atraso de 1 s do motor

`Card::update` só era chamado por `run_cycles`/`run_frame`; um tracer que chama `Machine::step` nunca expirava o `m_spinning` do drive e o `$BD34` do DOS 3.3 lia dois nibbles diferentes (`EB` x `AA`) em vez de um latch parado. Corrigido em `main` (`Machine::update_cards`, D7). Os casos de disco fazem o mesmo que o oráculo: `update_cards` a cada 1020 ciclos.

### D3. Relógio de vídeo do oráculo parado

`CpuExecute(slice, false)` deixava o relógio NTSC em (0,0): todo floating bus lia o mesmo byte. Visto aqui como A=`A0` x `00` em `LDA ($C7),Y` da varredura de slots ($C707) e em leituras ímpares de `$C0E1..E7`; 84 a 921 leituras por boot. Corrigido em `main` (`CpuExecute(slice, true)`, D4). Depois disso: **zero** sincronizações em todos os casos.

## Limites e itens abertos

- **L1. HDC no ][+ (GH#1234).** O firmware do HDC patcha `BIT $C061 / BMI` para ler a tecla Shift (`BIT $C063 / BPL`) no ][ e no ][+ (`Harddisk.cpp:267`). A base do port (`a8f0d1d`) tem isso e o fork do oráculo (22 commits atrás) não; o boot do HDD diverge no ciclo 132.401 (`FLOW`, PC `C74F` x `C747`). Não é defeito do Rust: o caso de modo bloco usa o //e não Enhanced (`disk-hdd-seq-iie-block`), que não tem o patch.
- **L2. Cache do oráculo compartilhado.** `target/conformance-cache/oracle/build` é um só para todas as worktrees. Outro agente reconstruiu o binário com o `main.cpp` da worktree dele (sem `--disk2/--hdd/--authentic`, e em outro momento sem o relógio de vídeo corrigido); aparecem como `usage` do oráculo ou como falsas sincronizações de floating bus. Use `ORACLE_CACHE` privado (copie `audetto-AppleWin` para ele). Registrado em `tools/conformance/oracle/README.md`.
- **L3. Janela de ciclos.** A recalibração do ROM do Disk II leva ~1,75 milhão de ciclos antes do `JMP $0801`: um micro-programa precisa de pelo menos 2 a 3 milhões de ciclos (os meus primeiros rodavam 1 milhão e "bootavam" no laço de espera do monitor, idênticos nos dois lados e sem testar nada).
- **L4. Escopo.** Não há entrada de teclado nem troca de disco em execução no oráculo (por isso #640, #831, #1078 e os jogos que pedem lado B ficam BLOCKED); INIT/FORMAT de discos e a firmware de 13 setores não foram executados; Disk II em outro slot além do 6 e mais de uma carta não foram comparados; o caminho `g_bFullSpeed` (CPU a toda velocidade com o motor ligado) nunca roda no oráculo (a variável é sempre falsa lá), então o que o AppleWin faz nele não foi comparado; o `FrameDrawDiskStatus` e a interface não existem no oráculo.
- **L5. Escrita WOZ com cadência.** As escritas dos micro-programas usam `JSR/RTS` por nibble (cerca de 36 ciclos, não 32); a gravação em WOZ bate com o oráculo mesmo assim, mas não é a cadência exata do RWTS.
- **L6. Frontend.** O app atualiza os cartões por quadro (17.030 ciclos); o AppleWin, por lote de 1 ms. Só afeta o instante exato do fim do atraso do motor (até 16 ms de diferença em 1 s). Não foi alterado (`emu.rs` é de outro agente).

## Reproduzir

```sh
tools/conformance/oracle/build-audetto-oracle.sh                    # ou com ORACLE_CACHE privado
tools/conformance/fetch-applewin-test.sh                            # corpus (opcional)
cargo build --release -p apple2-storage --example disk_follow --example make_micro
target/release/examples/make_micro tests/conformance/inputs/disk-micro
tools/conformance/disk/gen-disk-cases.sh [filtro-de-id]             # regenera .toml e .ckpt
target/release/conformance run --all --report /tmp/rel              # roda os 53 (e os demais) casos
```
