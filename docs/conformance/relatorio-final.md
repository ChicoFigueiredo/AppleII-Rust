# Relatório final de conformidade

Relatório do §21 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md), em português, com os 10 itens na ordem do plano. Todos os números vêm de execuções feitas em 21/10/2026 sobre o SHA do item 1; nada vem de memória. Onde um número vem de um relatório gerado por uma frente anterior, o relatório é citado. A nota do item 9 é **autoavaliação; a revisão independente vem depois**.

## 1. Baseline com SHAs e versões

| Repositório | Referência | SHA completo |
|---|---|---|
| AppleII-Rust (código avaliado) | `main` de 21/10/2026 (`git rev-parse HEAD`) | `22dbf2b7a4c57d9b6ede7be39a577e93196c1a3f` |
| AppleII-Rust (ponto de partida da auditoria) | `main` de 20/10/2026 | `6da60488a2c61b82099d6a7aabece21125eb0963` |
| AppleII-Rust (última versão publicada) | tag `v0.1.0` (workspace `0.1.0`) | `87110ff15ef35f58db882ab46c08f69296f7250d` |
| AppleWin (Oráculo de código) | commit `a8f0d1d`, 17/10/2026 | `a8f0d1d40f551ad9e010522c29c2891e33c88275` |
| AppleWin (release de referência) | tag `v1.32.1.0` (5 commits antes de `a8f0d1d`) | `daf813dcaced7469bbbc16f23905cfd75abe8496` |
| AppleWin-Test (corpus, sem licença; cache local) | HEAD de `master` em 20/10/2026 | `4b113ce8cff1426bda5595cac9cb04776adfbb5d` |
| audetto/AppleWin (Oráculo executável, Linux) | HEAD de `master` em 20/10/2026; núcleo AppleWin `3e8054b` | `a1e20c0362f392dc25f1a3b218d2f84d4f1a25da` |

Versões: AppleII-Rust `0.1.0` (tag) e `0.2.0-alpha.1` (preparada na branch local `release/v0.2.0-alpha.1`, sem tag nem publicação). AppleWin `1.32.1.0` mais 5 commits. Detalhes e a comparação de commits: [baseline.md](baseline.md).

Ambiente de execução: Ubuntu 26.04.1 LTS, kernel 7.0.0-38, x86_64, AMD Ryzen 7 3700X (16 threads, 125 GiB), `rustc 1.98.1 (48a229cea 2026-09-01)`, Wine 10.0 para o Windows via `cargo-xwin`. A máquina estava compartilhada com outros processos (carga média de 28 a 31 durante as medidas), o que afeta tempos, não resultados.

## 2. Tabela-resumo

Versão condensada. A tabela completa, com uma linha por componente e Modelo, está em [capability-matrix.md](capability-matrix.md).

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| CPU 6502 / 65C02 | ][, ][+, //e, //e Enhanced | PASS | 7 traces de 0,5 a 5 milhões de ciclos idênticos ao Oráculo; 10,22 milhões de vetores SingleStepTests; determinismo 10 x 2M | Diferenças AppleWin x hardware catalogadas (ciclo fantasma, RMB/SMB, WAI/STP) copiadas do AppleWin | #17 | P0 |
| Memória (LC, Saturn, RamWorks, AUX, floating bus) | ][+, //e, //e Enhanced | PASS (configuração e traces); FAIL (2 estados Saturn reais) | `audit_memoria.rs`, a2audit, estados `LC-*`, `RamWorks_r4`, `Saturn128-Bank3-2` | `Saturn64` e `Saturn128` reais divergem no ciclo +406 | #18, #19 | P0 |
| Vídeo | ][, ][+, //e, //e Enhanced | PASS (1.573 de 1.573 comparações pixel a pixel); FAIL (1 cena PAL) | 411 casos `video-*` | #241: 1 pixel no Color Monitor PAL | #20, #21, #46 | P0 |
| Disco (Disk II, WOZ, 2MG, gz/zip, HDD/SmartPort) | ][+, //e, //e Enhanced | PASS | 56 casos `disk-*`, 103.146.754 instruções comparadas | HDC no ][+ não comparável; NIB real e INIT/FORMAT sem Oráculo | #22, #23, #24 | P0 |
| Save-state | vários | PASS (79 de 115 reais; atômico; fuzz); FAIL (9); BLOCKED (27) | `savestate-import.md`; 78 casos `savestate-*` | 9 estados reais divergem (item 7) | #25, #26 | P0 |
| Áudio: eventos | ][+, //e | PASS | 27 casos de eventos e de fixtures | nenhuma | #27, #28 | P1 |
| Áudio: PCM do AY e do alto-falante, SSI263 +666, #1176 | ][+, //e | FAIL (6 casos) | `audio-*-pcm`, `audio-ssi263-phonemes-p333`, `audio-issue-1176-last-cycle` | blocos do AppleWin; 23 ciclos por amostra; fase do evento | #7, #27, #28 | P1 |
| Z80 SoftCard | ][+, //e | PASS (vetores de terceiros); BLOCKED (sistema/CP/M) | 1.604.000 vetores; ZEXDOC/ZEXALL 65/65 | F3/F5 de blocos repetindo segue o hardware (candidata) | #29 | P1 |
| Cartas (mouse, 4Play, SNES MAX, relógios, SSC, Uthernet) | ][+, //e | PASS (parcial); FAIL (mouse ROM, SNES MAX); BLOCKED (rede/serial) | `entrada-cartas.csv` 18 PASS, 5 BLOCKED, 1 N/A | IRQ de VBL do mouse +7 ciclos | #30, #31, #32, #33 | P1 |
| Depurador | ][+, //e Enhanced | PASS 62; FAIL 10; BLOCKED 6; N/A 8 | `results/depurador.csv` | Os 10 FAIL são comportamentos do AppleWin reproduzidos | #34 | P1 |
| Interface, linha de comando, configuração | todos | IMPLEMENTED (testes próprios); UNVERIFIED contra o AppleWin | `hu0702_input.rs`, `audit_persistence.rs`, `cli.rs` | 122 issues de interface UNVERIFIED | #35 | P1 |
| Linux x86_64 | Linux | PASS (smoke, pacote extraído) | [relatorio-plataformas.md](platform/relatorio-plataformas.md) | Wayland nativo e dispositivos: manual | #36 | P0 |
| Windows (Wine) / Windows nativo | Windows | PASS (Wine) / BLOCKED (nativo) | [windows-wine.md](platform/windows-wine.md) | Wine não é Windows | #36 | P0 |
| macOS | macOS | BLOCKED | sem hardware | tudo | #36 | P0 |
| Extensões: rewind, DAP, laboratório, WASM | Enhanced //e | IMPLEMENTED | crates próprios | sem equivalente no AppleWin | #40 a #44 | P2 a P3 |

## 3. Sumário por status das issues upstream

Coleta de 21/10/2026 (00:47 UTC) no HEAD do AppleWin `a8f0d1d`: 1.516 itens (1.115 issues e 401 PRs). Issues: **358 abertas e 757 fechadas**. Fonte: [applewin-issue-matrix.csv](applewin-issue-matrix.csv) e [issue-audit.md](issue-audit.md). `python3 tools/conformance/apply_results.py --check` confirma "Matriz em dia" (325 issues com resultado aplicado, 0 linhas por alterar).

| Estado de verificação | Abertas | Fechadas | Total |
|---|---:|---:|---:|
| PASS | 33 | 172 | **205** |
| FAIL | 11 | 2 | **13** |
| BLOCKED | 26 | 61 | **87** |
| NOT_APPLICABLE | 54 | 232 | **286** |
| UNVERIFIED | 234 | 290 | **524** |
| Total | 358 | 757 | **1.115** |

Taxa de PASS sobre as aplicáveis (todas menos NOT_APPLICABLE): 205 de 829 = 24,7 %.

Por prioridade:

| Prioridade | Total | PASS | FAIL | BLOCKED | NOT_APPLICABLE | UNVERIFIED |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 30 | 23 | 0 | 7 | 0 | 0 |
| P1 | 216 | 127 | 3 | 74 | 12 | 0 |
| P2 | 276 | 22 | 5 | 2 | 1 | 246 |
| P3 | 327 | 33 | 5 | 4 | 7 | 278 |
| sem prioridade | 266 | 0 | 0 | 0 | 266 | 0 |

NOT_APPLICABLE por aplicabilidade: pergunta/suporte 118, plataforma Windows 61, build 33, duplicada 28, invalida 26 (a soma é 266; mais 20 issues P1 a P3 marcadas N/A com justificativa por issue). Os 13 FAIL: **#241, #381, #1176** (P1) e **10 do depurador** (#114, #245, #466, #468, #713, #1145, #1377, #1381, #1426, #1431; P2 e P3). Os 7 P0 BLOCKED: **#4, #70, #400, #422, #564, #640, #1126**. Auditoria independente da triagem: [issue-audit-review.md](issue-audit-review.md) (amostra de 465 issues, concordância de 91 % em qualquer campo na parte aleatória; 168 correções; 334 linhas reexaminadas por classe). 323 linhas continuam com a triagem original sem revisão individual.

## 4. Top 10 problemas críticos

Os oito primeiros foram achados pela auditoria e corrigidos, cada um com teste de regressão. Os dois últimos estão abertos. SHA em que cada um foi observado: `6da6048` (início), salvo indicação.

| # | Problema | Passos para reproduzir | Causa raiz | Estado |
|---|---|---|---|---|
| 1 | **AUD-C-060 (P0)**: importar um save-state real do AppleWin travava a emulação | Importar o arquivo real anexado à issue #1520 do AppleWin (Merlin Pro, //e Enhanced, `Cycles This Frame: 26190`) e chamar `Machine::run_frame()`: não termina (cerca de 346 s de CPU até estourar `u32`; em debug, `attempt to add with overflow`). `cargo test -p apple2-savestate --release --test audit_cycles_this_frame` | O carregador copiava o contador cru (26190 > 17030 ciclos por quadro) e `tick()` só fechava o quadro com `==` | **Corrigido**: reduzido módulo o quadro ao carregar (`machine/state.rs`) e `>=` no `tick()`. Regressão `audit_cycles_this_frame.rs`; o arquivo da #1520 faz parte dos casos de importação |
| 2 | **AUD-C-010 (P0)**: arquivo de save-state hostil derruba o carregador | YAML com `Length: -1` no bloco `AppleII-Rust Extension` (ou bomba zlib): `attempt to multiply with overflow` em debug, alocação gigante/abort em release. `cargo test -p apple2-savestate --release --test fuzz_loader` | `Length` lido como `u64` sem teto e usado em `vec![0u8; len.div_ceil(8)*8]`; descompressão sem limite | **Corrigido**: teto de 64 MiB, validação contra as linhas de `Data`, descompressão limitada a 256 MiB. `fuzz_loader.rs`, `audit_robustness.rs` |
| 3 | **Saturn (AUD-C-001/003, #1486 e #1495, P0)**: a contagem de bancos configurada se perdia | Configurar `-s0 saturn64` (ou `Saturn128K { banks: 4 }`) e usar bancos 5 a 8; salvar e recarregar: o estado sempre gravava `Num Saturn Banks: 0x08` e o arquivo de 4 bancos voltava com 8 | `Saturn128K { banks }` virava `CardKind::Saturn128K` sem campo; a carta tinha sempre 8 bancos | **Corrigido** (`7e5c46f`): `MachineConfig.saturn_banks` até a carta e o save-state, banco ≥ total vira total-1 (`LanguageCard.cpp:424-430`). Testes `audit_saturn_banks.rs`, `audit_saturn_state.rs`. **Resta o problema 10** |
| 4 | **Gravação não atômica de save-state e de imagens de disco (AUD-C-022, AUD-C-033, P1; perda de dados)** | Interromper a escrita no meio (erro de E/S injetado): `std::fs::write` truncava o arquivo no lugar e destruía o último save válido ou a imagem `.gz/.zip` | `set_len(0)` seguido de `write_all` no mesmo arquivo | **Corrigido**: temporário no mesmo diretório, `sync_all`, `rename`, `fsync` do diretório; imagens até 2 MiB regravadas assim; HDD maior atualizado no lugar, bloco a bloco com `sync_data`. Testes `atomic_write.rs`, `crash_safety.rs`, `store.rs`. Queda real de energia: não testada |
| 5 | **`write_block` de 2 TB (AUD-C-035, P0 de robustez)** | `DiskImage::write_block(0xFFFFFFFF, ..)`: `memory allocation of 2199023255552 bytes failed`. Achado pelo fuzzing (`fuzz_parsers.rs`) | Um bloco após o fim crescia a imagem sem limite | **Corrigido**: recusa blocos a partir de 65.536 (volume ProDOS de 32 MB); limites de tamanho de arquivo antes de ler e orçamento de descompressão do `.zip` |
| 6 | **Escrita pendente de disco perdida (AUD-C-030/031, P1)** | Gravar um setor, aplicar configurações (`ApplySettings`) ou carregar um save-state antes do motor parar: a trilha suja era descartada | `rebuild`/`load_machine` trocavam a máquina sem `disks.flush`; o Disk II não tinha `Drop` | **Corrigido**: flush antes de reconstruir e de carregar; também no SIGTERM/SIGINT (AUD-C-034). `audit_persistence.rs`, `sigterm.rs` |
| 7 | **Tecla presa (AUD-P-028, P1)** | Segurar uma letra, apertar F7, soltar a letra, apertar F7 de novo: `$C010` continua com o bit 7 ligado; idem com o joystick no teclado numérico | O evento de soltar não chegava ao teclado quando o depurador ou o egui consumia o evento | **Corrigido**: soltar sempre chega; F7 e tela cheia soltam tudo. `hu0702_input.rs` (4 testes). Reprodução manual no [wave0-peripherals.md](audit/wave0-peripherals.md) |
| 8 | **Panics e alocação gigante no depurador** | Comandos com Unicode; intervalo com fim abaixo do início (`M`/`DA`), que virava cópia gigante; texto com Unicode no NTSC | Fatiamento por byte em cadeias UTF-8 e intervalo invertido não validado | **Corrigido** (`4cd5132`, `d2e9962`). Fuzz da linha de comando (`fuzz_cmdline.rs`) |
| 9 | **Aberto, P1: fase dos eventos agendados** (#1176, #381 e três estados reais) | `conformance run --case audio-issue-1176-last-cycle`; `savestate-compare.py` sobre `v2/Mockingboard/6522-one-shot.aws.yaml`, `SSI263-test.aws.yaml` e os três `dazzledraw1`: o IRQ do 6522 é tomado um opcode depois (#1176) e o evento de VBL do mouse entra 7 ciclos antes depois de carregar | O Rust marca eventos "no último ciclo do opcode" e reagenda a fase do evento ao carregar; o AppleWin tem outra ordem. Duas hipóteses testadas sem êxito | **Aberto** |
| 10 | **Aberto, P0/P1: estados reais que ainda divergem** (`Saturn64`, `Saturn128`, `SmartPort-firmware`, `SNES_MAX-slot4`) | `savestate-compare.py` sobre os arquivos de `v2/Memory/` e outros: a 1ª diferença está no ciclo +406 (`LDA` lê `$32`/`$46` no AppleWin e `$00` no Rust), +4.373 e +6 | Saturn: byte de banco restaurado diferente (causa não investigada; a contagem de bancos já está certa); SmartPort: status em `$C0nX`; SNES MAX: leitura do controle | **Aberto**. Aparece como FAIL no [savestate-import.md](../../tests/conformance/reports/savestate-import.md) |

Risco que não é um defeito achado, mas pesa na recomendação: **Windows e macOS nativos nunca foram executados** (BLOCKED).

## 5. Backlog importável

- [backlog.md](backlog.md) e [backlog.json](backlog.json): 13 épicos (EP-00 a EP-12) e 30 histórias (HU-0001 a HU-1205), com prioridade, onda, Gate e número da issue no GitHub, gerados de `docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md` por `tools/conformance/backlog.py` (sincronização idempotente com `gh`).
- Issues no repositório ChicoFigueiredo/AppleII-Rust: **#2 a #14** (épicos), **#15 a #44** (histórias HU) e **#45 a #80** (36 histórias do EP-11, derivadas das 358 issues abertas do AppleWin: [upstream-open-selected.md](upstream-open-selected.md), triagem completa em [upstream-open-issues-triage.md](upstream-open-issues-triage.md)). Cada história tem Critérios de aceite, dependências e rótulos.
- Fila da onda 1 (246 issues P0/P1 de runtime, formato e depurador): [wave1-candidates.md](wave1-candidates.md).
- Pendências novas a abrir como histórias: causa raiz de `Saturn64/128` e `SmartPort-firmware`; #1176 e fase do evento de VBL do mouse; PCM (política v3 do PCM); #241 PAL; fixtures dos 7 P0 BLOCKED; execução em Windows e macOS nativos. Não foram criadas issues novas nesta entrega.

## 6. Testes executados e não executados

### Executados (Linux x86_64 nativo, SHA do item 1, mais as alterações desta entrega)

| Execução | Comando | Resultado |
|---|---|---|
| Testes do workspace | `cargo test --workspace --release --no-fail-fast` | **1.400 passam, 0 falham, 20 ignorados** (162 suítes: 145 executáveis de teste e 17 doctests; exit 0). Os 20 ignorados: vetores pesados que exigem download de 2,5 GB (65x02) e 1,3 GB (Z80, ZEX), Klaus estendido, 9 testes que reproduzem FAIL do depurador (comportamento do AppleWin), medidas de tempo (`emulated_speed`, `disk_boot_speed`, `frames_per_second`), a carga do corpus AppleWin-Test, um gerador manual, um reprodutor de SNES MAX e um doctest |
| Casos de conformidade | `cargo build --release -p apple2-conformance && ./target/release/conformance run --all --report target/conformance-report` | **591 casos: 585 PASS, 6 FAIL, 0 BLOCKED** (com o corpus AppleWin-Test no cache; sem ele, 97 casos ficam BLOCKED). Por área: vídeo 411, save-state 78, disco 56, áudio 33 (27 PASS e 6 FAIL), CPU/boot/determinismo/framebuffer 13. Os 6 FAIL são os casos com divergência conhecida: `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333` e `audio-issue-1176-last-cycle`. O comando sai com código 3 enquanto houver FAIL |
| Matriz de issues | `python3 tools/conformance/apply_results.py --check` | "Matriz em dia": FAIL 13, PASS 205, BLOCKED 87, NOT_APPLICABLE 286, UNVERIFIED 524 |
| Importação de save-states reais | `savestate-compare.py` (115 arquivos, 100.000 ciclos cada) e `savestate-report.py` | 79 PASS, 9 FAIL, 27 BLOCKED |
| Vetores 65x02 | `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored` (relatório versionado) | 10.220.000 de vetores, 0 inesperados ([65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md)) |
| Vetores Z80 e ZEXALL | `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored` (relatório versionado) | 1.604.000 de 1.604.000; ZEXDOC e ZEXALL 65/65 ([z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md)) |
| Corpus AppleWin-Test (carga de 1 quadro) | `applewin_test_corpus.rs` | 94 OK, 18 OK com avisos, de 112 ([applewin-test-load.md](../../tests/conformance/reports/applewin-test-load.md)); carregar não é PASS de fidelidade |
| Windows via Wine 10.0 | `tools/package/smoke-wine.sh` | PASS (relatório de 20/10/2026, [windows-wine.md](platform/windows-wine.md)) |
| Linux: pacote e smoke | `tools/package/smoke-linux.sh`, `test-package.sh` | PASS, 17 verificações (relatório de 20/10/2026, [relatorio-plataformas.md](platform/relatorio-plataformas.md)) |
| Desempenho | `apple2-bench` | [performance.md](../../tests/conformance/reports/performance.md), em máquina compartilhada |

`cargo clippy --workspace --all-targets -- -D warnings`: sem avisos. `cargo fmt --all -- --check`: sem diferenças.

Relatórios por sistema operacional: Linux nativo, [relatorio-plataformas.md](platform/relatorio-plataformas.md); Windows (Wine), [windows-wine.md](platform/windows-wine.md); Windows nativo e macOS, roteiros em [manual-tests/plataformas.md](manual-tests/plataformas.md) (não executados). Os smokes de Linux e Wine são de 20/10/2026 (commit base `516c123`); os do SHA atual não foram refeitos, só a suíte de testes e de conformidade.

### Não executados

- **Windows nativo e macOS (Intel e Apple Silicon)**: build, smoke, GUI, áudio, DPI, gamepad, permissões, Gatekeeper, SmartScreen. BLOCKED.
- **CI do GitHub e do GitLab**: nenhuma execução foi acionada (cota esgotada; só o dono autoriza). `release.yml` nunca rodou.
- **Hardware físico**: gamepad, captura de mouse, som audível, serial real, pcap/Npcap, impressora, teclado regional ([manual-tests/](manual-tests/)).
- Fuzzing de longa duração (`cargo-fuzz`); queda real de energia; Wayland nativo.
- Jogos e software de terceiros sem fixture (60 issues BLOCKED).
- 27 save-states do corpus (imagens ausentes, rede, hora real, Shift Key Mod, Z80).
- 524 issues UNVERIFIED (P2 e P3).
- Fixtures de AppleWin real em Windows (só o AppleWin 1.32.1.0 sob Wine gerou save-states).

## 7. Comparação diferencial AppleWin x Rust

Oráculo executável: fork audetto no SHA `a1e20c0` (item 1). Política de comparação definida antes das execuções em [test-oracles.md](test-oracles.md): estado digital comparado de forma exata; áudio e vídeo analógicos com limiares registrados antes.

| Área | Resultado | Registro |
|---|---|---|
| **CPU: traces** | 7 traces idênticos em todos os campos (`cycle, pc, op, a, x, y, sp, p`): ][+ (0,5 e 2 milhões de ciclos), //e Enhanced (0,5 e 2 milhões), ][ original (2 milhões), //e (2 milhões) e //e Enhanced com boot do DOS 3.3 (5 milhões de ciclos, 1.821.912 instruções) | [cpu-trace-differential.md](audit/cpu-trace-differential.md). Sete divergências de ambiente e convenção (P com bits 4/5, RAM de power-on, entrada em interrupção, relógio de vídeo parado no Oráculo, Disk II ausente, `rand()` da glibc, atraso do motor), todas resolvidas; nenhuma era erro do núcleo 6502 |
| **CPU: vetores** | 10.220.000 de vetores SingleStepTests/65x02 (6502 NMOS, Rockwell, Synertek e WDC 65C02), 0 inesperados: 3.521.157 estritos e as diferenças AppleWin x hardware catalogadas (D0 a D10) | [65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md) |
| **Vídeo** | **1.573 telas** comparadas com o framebuffer inteiro (600x420 ou 680x436 com VidHD), diferença máxima de canal 0, Oráculo calibrado contra ele mesmo: 1.573 PASS, 0 FAIL, 0 BLOCKED. 411 casos `video-*` no harness (305 de matriz modo x tipo e 106 de issues). 59 cenas determinísticas. Três divergências do Rust (DIV-001 a DIV-003) e uma de RAM de power-on (DIV-004), todas resolvidas. A exceção que permanece é a cena #241 em Color Monitor PAL (1 pixel), que não está nas 1.573 | [video-differential.md](audit/video-differential.md) |
| **Disco** | 56 casos `disk-*`; **103.146.754 instruções** comparadas (ciclo, PC, A, X, Y, SP, P de cada) e SHA-256 das imagens ao final; 9 WOZ reais, DSK, `.do.gz`, `.zip`, 2MG, HDD de 32 MB. Nenhuma divergência de comportamento sobrou depois de alinhar o ambiente. O defeito do Rust achado no caminho foi o `write_block` de 2 TB | [disk-differential.md](audit/disk-differential.md) |
| **Save-states reais** | 115 arquivos (112 do AppleWin-Test e 3 do AppleWin 1.32.1.0 sob Wine), 100.000 ciclos por arquivo: **79 PASS, 9 FAIL, 27 BLOCKED**. O estado salvo pelo Rust e carregado no Oráculo também bate (`oracle_roundtrip.rs`, 30.000 ciclos) | [savestate-import.md](../../tests/conformance/reports/savestate-import.md); esquema em [savestate-schema.md](savestate-schema.md) |
| **Áudio (eventos)** | Escritas 6522/AY, timers, IRQ, cliques do alto-falante, soquetes e `6522a-bad`: eventos idênticos incluindo o ciclo (225, 148, 155, 110, 150, 76 e 524 eventos nos 7 programas); SSI263 e SC-01: duração de fonema dentro da tolerância medida no Oráculo (J_ssi = 1.670 ciclos); núcleo do AY contra `AY8910.cpp`: ~330 mil amostras idênticas | [audio-differential.md](audit/audio-differential.md) |
| **Áudio (PCM)** | **5 FAIL**: PCM do AY (tom, envelope, ruído; D-2), PCM do alto-falante (D-3) e SSI263 fase +666 (D-4). Mais o FAIL do #1176 | idem |
| **Z80** | 1.604.000 de 1.604.000 vetores SingleStepTests/z80; ZEXDOC e ZEXALL 65/65. É comparação com vetores de terceiros, não com o Z80VICE do AppleWin | [z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md), [z80-zex.md](../../tests/conformance/reports/z80-zex.md) |
| **Depurador** | 86 issues: 62 PASS, 10 FAIL (bugs do AppleWin reproduzidos), 6 BLOCKED, 8 N/A; 251 comandos (249 IMPLEMENTED, 2 N/A); fuzz da linha de comando | [results/depurador.csv](results/depurador.csv), [debugger-commands.md](audit/debugger-commands.md) |

Artefatos de diferença: o harness deixa o RGBA dos dois lados e o diagnóstico (pixels, caixa envolvente, MAE) em `target/video-differential` quando algo falha; os traces de referência estão em `tests/conformance/reference-traces/`, os eventos e PCM do Oráculo em `tests/conformance/reference-audio/`. As imagens de referência dos 3 save-states de Wine estão em `tests/conformance/inputs/applewin-real/*.png`.

## 8. Matriz de risco de corrupção de disco e de save-state

Impacto: perda ou corrupção de dados do usuário. Os riscos "residuais" são os que a mitigação não elimina.

| Cenário | Impacto | Antes (`6da6048`) | Mitigação | Evidência | Risco residual |
|---|---|---|---|---|---|
| Queda no meio da regravação de disquete (DSK/DO/PO/NIB/WOZ/2MG, `.gz`, `.zip`) | Alto | Truncava a imagem | Temporário + `sync_all` + `rename` + `fsync` do diretório, links seguidos, permissões preservadas | `store.rs`, `crash_safety.rs`, `integrity.rs` | Baixo: erro de E/S injetado, sem queda real de energia |
| Queda no meio da gravação do HDD (imagens maiores que 2 MiB, até 32 MB) | Alto | Truncava | Atualização no lugar, bloco a bloco com `sync_data`; sem renomear | `hdd.rs` testes | **Médio**: o arquivo nunca é substituído atomicamente; uma queda pode deixar um bloco escrito pela metade |
| Queda no meio da gravação do save-state | Alto | Destruía o último save válido | `write_atomic` (temporário no mesmo diretório, `sync_all`, `rename`, sync do diretório; `MOVEFILE_REPLACE_EXISTING` no Windows) | `atomic_write.rs`, `audit_persistence.rs` | Baixo; Windows nativo não testado |
| Save-state hostil (`Length`, bomba zlib, YAML truncado ou mutado) | Alto (travar ou abortar) | Panic/alocação gigante | Tetos de 64 MiB e 256 MiB, validação de linhas, fuzz | `fuzz_loader.rs`, `audit_robustness.rs` | Baixo; `cargo-fuzz` de longa duração não executado |
| Save-state de terceiros aponta imagens e as abre para leitura e escrita (AUD-C-037) | Médio (altera arquivo local) | Igual ao AppleWin | Nenhuma ainda | `state_io.rs` | **Médio** (UNVERIFIED): proposta é abrir somente leitura fora do diretório do estado |
| Imagens malformadas ou gigantes (WOZ, gzip, zip, 2MG, nomes com `../`) | Alto | `write_block` de 2 TB, bombas | Limites antes de ler, orçamento de descompressão, recusa de blocos > 32 MB, fuzz | `audit_images.rs`, `fuzz_parsers.rs` | Baixo |
| Trocar configuração ou carregar estado com trilha suja | Alto | Perdia a escrita | `disks.flush` antes de `rebuild` e `load_machine`; flush ao parar o motor, ejetar, sair | `audit_persistence.rs`, `audit_disk_flush.rs` | Baixo; o frontend atualiza os cartões por quadro, não por lote de 1 ms (até 16 ms de diferença no atraso do motor) |
| Encerramento por sinal (SIGTERM, SIGINT, SIGHUP) | Médio | Sem tratamento | Handler pede saída pela via normal; segundo sinal encerra na hora | `sigterm.rs` | Baixo; Windows nativo não testado |
| Strings do YAML (`\\`, `#`, Unicode, espaços) mudando a cada save | Médio | Perdia uma contrabarra por ciclo | Escape sempre; round-trip determinístico | `audit_robustness.rs`, `roundtrip_determinism.rs`, `oracle_roundtrip.rs` | Baixo |
| Duas instâncias abrindo a mesma imagem | Alto | Sem trava de arquivo | Nenhuma | não testado | **UNVERIFIED** |
| Estado importado com o Rust divergindo do AppleWin (9 arquivos) | Baixo para dados; médio para fidelidade | n/a | Divergência documentada | `savestate-import.md` | Aberto (problema 10 do item 4) |

## 9. Nota e Gates

**Autoavaliação; a revisão independente vem depois.** Cálculo e justificativa por dimensão em [release-readiness.md](release-readiness.md).

| Dimensão | Pontos | Nota |
|---|---:|---:|
| Especificação e critérios de aceite | 25 | 20 |
| Corretude e evidência | 25 | 18 |
| Testes e regressão | 15 | 12 |
| Arquitetura e manutenção | 10 | 8 |
| Segurança, dados e resiliência | 10 | 8 |
| Higiene da entrega | 5 | 4 |
| Eficiência/retrabalho | 10 | 6 |
| **Total** | **100** | **76** |

| Gate | Estado | Motivo |
|---|---|---|
| G0 Inventário | FAIL | 1.115 issues coletadas e classificadas, amostra auditada; falta evidência ou justificativa em 509 linhas UNVERIFIED (P2/P3) |
| G1 Alpha técnica | BLOCKED | Smoke em Windows nativo e macOS e CI nunca executados; o resto é PASS |
| G2 Beta funcional | FAIL (e BLOCKED em parte) | 7 P0 BLOCKED sem aceite formal, 9 estados reais com FAIL (incluindo Saturn), matriz incompleta; evidência de áudio/entrada/GUI nos 3 SO BLOCKED |
| G3 Stable candidata | FAIL | FAIL P1 (#241, #381, #1176), sem pacote instalado nos 3 SO, sem parecer independente |

Todos os critérios de cada Gate, com PASS, FAIL ou BLOCKED e evidência: [release-readiness.md](release-readiness.md).

## 10. Recomendação de release

**alpha.** Publicar, se o dono decidir, apenas como **pré-lançamento `v0.2.0-alpha.1`**, com a seção "Limitações conhecidas" do CHANGELOG, e sem chamá-la de beta nem de candidata a estável. Não é beta: o G2 não está atingido (FAIL P0 e P1 no corpus real, 7 P0 BLOCKED sem aceite, evidência dos 3 SO ausente). Antes de usar o rótulo "alpha técnica" sem ressalva, o G1 pede o smoke em Windows e macOS nativos (o que o dono pode executar pelos roteiros ou autorizando um `workflow_dispatch` do `release.yml`). A versão `0.2.0-alpha.1` está preparada na branch local `release/v0.2.0-alpha.1`; nenhuma tag, PR ou push foi feito.
