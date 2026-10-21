# Oráculos e estratégia de testes

Este documento aplica ao projeto a estratégia de oráculos do §17 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Ele define contra o que cada teste compara, como a comparação é decidida e o que cada execução registra.

## Oráculos

| Oráculo | Quando se usa | Observação |
|---|---|---|
| AppleWin em `a8f0d1d` (padrão) | Qualquer comportamento que o AppleWin tem | Executáveis: AppleWin v1.32.1.0 no Wine 10 e o fork Linux do audetto com SHA fixo. Veja [baseline.md](baseline.md) |
| Especificação de hardware | Onde o AppleWin não cobre o caso | Só prevalece sobre o AppleWin como Divergência intencional, com prova de hardware |
| Fixture validada | Entrada com resultado conhecido, de proveniência documentada | Identificada por SHA-256 |
| Resultado conhecido | Valores publicados (por exemplo, a suíte de CPU de Klaus Dormann) | A fonte e a versão entram no Manifesto |

Quando o AppleWin e o hardware real divergem, o AppleWin vence (ADR 0004).

## Pirâmide de testes

Do mais barato ao mais caro. Cada nível tem seu lugar, e um nível baixo verde não substitui um nível alto.

1. **Unitários.** Registradores, opcodes, memória, Soft switches, relógios, analisadores de WOZ e YAML. Incluem casos de borda e propriedades invariantes (nenhum panic, integridade, ida e volta). Já existem em cada crate; seu resultado, sozinho, é IMPLEMENTED.
2. **Integração.** Boot da ROM, montagem e gravação de disco, expansão de RAM, vídeo, áudio, depurador e salvar e restaurar, sob sequências de entrada fixas. Rodam sem janela.
3. **Diferencial.** O AppleWin e o AppleII-Rust executam sob o mesmo Modelo, a mesma ROM, a mesma entrada e o mesmo Ciclo; comparam-se CPU, memória, barramento e Quadros. Na divergência, o relatório mostra a primeira diferença. Os Traces de referência gerados pelo AppleWin são versionados com a origem e o SHA (formato abaixo).
4. **Corpus real.** Fixtures geradas pelo próprio AppleWin (AppleWin-Test), em várias versões e Modelos. Nunca depende só de fixture escrita pela equipe. O corpus é baixado para `target/conformance-cache/` e não é redistribuído; sem rede o resultado é BLOCKED.
5. **Ponta a ponta entre plataformas.** Instalar o executável distribuído, abrir a janela, digitar, montar um disco, salvar e reabrir, gerar áudio e encerrar. Registra o que foi automatizado e o que foi validação humana. Linux roda local; Windows nativo e macOS ficam BLOCKED até o dono autorizar uma execução de CI.
6. **Robustez.** Entradas WOZ, NIB, DSK, YAML, configuração, serial e rede inválidas; limites de tamanho, tempo e memória. Nenhum panic por arquivo malformado do usuário e nenhuma mutação parcial de dados.
7. **Desempenho.** Benchmarks fixos. Não se sacrifica temporização correta por velocidade.

Testes que exigem hardware físico (gamepad, porta serial, captura de pacotes com pcap, escuta de áudio) têm roteiro manual e ficam BLOCKED até o dono executar o roteiro e registrar o resultado.

## Política de comparação

### Estado digital: igualdade exata

Compara-se valor a valor, sem tolerância: PC, A, X, Y, SP, flags, bancos de memória, a RAM relevante, eventos, a ordem das operações de E/S, o contador de Ciclos e o Quadro lógico antes de efeitos analógicos. Qualquer diferença é FAIL.

### NTSC, PAL e áudio: métricas definidas antes

A simulação analógica de vídeo e a síntese de áudio não têm igualdade bit a bit garantida. Nesses casos:

- Compara-se primeiro o que for digital: eventos e temporização exatos (toques do alto-falante, escritas nos registradores do AY, trocas de modo de vídeo).
- Para o que sobrar, a **métrica, o espaço de cor (ou a janela de áudio), o alinhamento temporal, as regiões críticas e o limiar são escritos no manifesto antes da primeira execução** e calibrados com a referência (por exemplo, o AppleWin contra ele mesmo em duas execuções). Candidatas de métrica: erro absoluto médio e SSIM para imagem; frequências, envelopes e eventos para áudio.
- O limiar não é ajustado depois para fazer um teste passar. Fora do limiar é FAIL, e o julgamento visual ou auditivo não o transforma em PASS. Mudar um limiar exige um novo registro com justificativa e vale só para as execuções seguintes.
- Todo diff gera um artefato (imagem de diferença, histograma, trecho de áudio) guardado em `tests/conformance/reports/`.

### Software de terceiros e testes instáveis

Jogos e software de terceiros só entram com licença adequada e procedimento reproduzível para obtê-los. Resultados que dependem de firmware registram o SHA-256 da ROM. Teste instável não é removido por conveniência: investiga-se a causa, isola-se o ambiente e registra-se a evidência.

## Trace de referência v1

É um arquivo CSV em UTF-8 com cabeçalho `cycle,pc,op,a,x,y,sp,p`. Há uma linha por instrução, gravada **antes** de executá-la.

| Coluna | Formato |
|---|---|
| `cycle` | Decimal. Ciclos acumulados desde o power-on |
| `pc` | 4 dígitos hexadecimais em maiúsculas |
| `op` | 2 dígitos hexadecimais em maiúsculas (o opcode a executar) |
| `a`, `x`, `y`, `sp`, `p` | 2 dígitos hexadecimais em maiúsculas |

Linhas começadas por `#` são metadados, por exemplo `# model=apple2e-enhanced` e `# rom_sha256=...`. Sugere-se também registrar `applewin_sha`, a origem do Trace (executável e opções) e a versão do formato nos metadados.

### Convenções do Trace v1

O gerador Rust (`conformance trace`) e o oráculo (`audetto-oracle`) seguem as mesmas convenções; sem elas os dois traces não coincidem mesmo com a CPU correta.

1. **`p` com os bits 4 e 5 ligados.** O AppleWin mantém B (bit 4) e o bit reservado (bit 5) sempre em 1 em `regs.ps`; o P após o reset é `34`. O registrador do núcleo Rust não muda (`24` após o reset); é só a representação: `TraceRow::from_machine` grava `p | 0x30` (`trace::P_FIXED_BITS`).
2. **Interrupção não gera linha.** No oráculo o gancho roda depois do Fetch do opcode; IRQ e NMI tomadas não fazem Fetch, logo não geram linha, e a linha seguinte é a primeira instrução do tratador. No Rust a entrada na interrupção é um `Machine::step`; o `TraceWriter` consulta `Machine::next_step_is_interrupt_entry()` (consulta pura, sem custo quando não usada) e não grava linha para esse passo.
3. **`op` é o opcode buscado.** Em `$C100-$CFFF` (ROM de cartão) o `peek` não lê a ROM sem efeito colateral; o `TraceWriter` usa o opcode que a CPU de fato buscou.
4. **RAM de power-on.** O padrão do AppleWin (`g_nMemoryClearType` = `MIP_FF_FF_00_00`, opção `-memclear N`, 0 a 7, mais -1 para sorteado) vale nos dois lados, com o sorteio de `getRandomTime()` fixo em 0 (`xx28/xx29/xx68/xx69` = `00`, `$4E`=`$4F`=`20`). No Rust: `MachineConfig::ram_init` (`RamInit`, `RandomSource::Zero` ou `Seed(n)`; o núcleo nunca lê o relógio, ADR 0001). `conformance trace --memclear N` e `--deterministic-ram` (padrão); `--ram-seed N` sorteia. A RAM auxiliar do //e fica zerada nos dois (o AppleWin só preenche `memmain`).
5. **Relógio de vídeo.** O oráculo avança o relógio de vídeo NTSC a cada opcode (`CpuExecute(..., bVideoUpdate=true)`), como o AppleWin do Windows. Com `false` (padrão dos frontends headless do fork) o relógio fica em (0,0) para sempre e todo floating bus lê o mesmo endereço do scanner, o que o AppleWin real nunca faz (HU-0103).
6. **Lotes de 1 ms.** O oráculo roda `CpuExecute(1020)` e chama `Card::Update(ciclos)` ao fim de cada lote; o `TraceWriter` chama `Machine::update_cards` a cada 1020 ciclos (`trace::BATCH_CYCLES`). O Disk II conta nesse `Update` o atraso de 1 s do motor desligado.
7. **Disk II.** `--cards disk2` (slot 6) e `--disk IMAGEM` (drive 1, protegido contra escrita) no Rust; o oráculo usa o mesmo cartão (`--cards disk2`, padrão). Os bytes "aleatórios" do Disk II vêm do `rand()` da glibc após `srand(1)`, igual nos dois.

Casos `authority = "applewin"`: `tests/conformance/cases/trace-applewin-*.toml`. Os de 500 mil ciclos têm o trace versionado; os maiores (2 e 5 milhões) ficam em `target/conformance-cache/reference-traces/` e o caso guarda o SHA-256 (`trace_sha256`) e o comando para regenerá-los em `source`; sem o arquivo, o resultado é BLOCKED com esse comando. O `applewin_sha` do manifesto vem do cabeçalho `#applewin_sha` do trace de referência. Registro das divergências encontradas: `docs/conformance/audit/cpu-trace-differential.md`.

Casos de disco (`tests/conformance/cases/disk-*.toml`, 53, `oracle.kind = "trace-checkpoints"`, `authority = "applewin"`): o Disk II (drives 1 e 2, `fixture`/`fixture2`), o controlador de HDD no slot 7 (`hdd`/`hdd2`), a velocidade autêntica (`authentic = true`) e imagens somente leitura (`readonly = ["fixture"]`) vêm do próprio caso. Um trace de 12 milhões de ciclos passa de 100 MB, então a referência é um arquivo `.ckpt` (`tests/conformance/reference-traces/disk/`): metadados (modelo, memclear, comando do oráculo, SHA-256 do trace completo, SHA-256 de cada imagem ao final), um checkpoint a cada 2000 instruções (`cycle,pc,a,x,y,sp,p`) e o hash FNV-1a encadeado de **todas** as linhas anteriores (`cycle`, `pc`, A, X, Y, SP, P com bits 4 e 5). Passar significa que a sequência inteira, ciclo a ciclo, e o conteúdo final das imagens são os do oráculo. As imagens do corpus AppleWin-Test (não versionadas) deixam o caso BLOCKED (`... ausente`) até serem baixadas. O gerador é `cargo run --release -p apple2-storage --example disk_follow -- --case CASO.toml --oracle TRACE.csv --emit-ckpt X.ckpt [--oracle-final fixture=SHA256]`; os micro-programas de boot vêm de `--example make_micro`. Registro e resultados: `docs/conformance/audit/disk-differential.md`.

Exemplo (valores ilustrativos do formato, não de uma execução real):

```csv
# model=apple2e-enhanced
# rom_sha256=aab38a03ca8deabbb2f868733148c2efd6f655a59cd9c5d058ef3e0b7aa86a1a
cycle,pc,op,a,x,y,sp,p
0,FA62,D8,00,00,00,FF,34
2,FA63,A9,00,00,00,FF,34
```

A comparação para na **primeira linha diferente** e relata essa linha e as N anteriores (N configurável), para mostrar a sequência que levou à divergência. Os metadados (`#`) não entram na comparação linha a linha, mas diferenças entre eles (modelo, ROM) são avisadas, porque tornam o resultado inválido.

## Manifesto de execução

Cada execução de teste produz um Manifesto de execução: o registro de tudo que a reproduz. Os campos são os do §17 do plano, todos obrigatórios:

| Campo | Conteúdo |
|---|---|
| `test_id` | Identificador estável do teste |
| `user_story` | História (HU) a que o teste pertence |
| `acceptance_criterion` | Critério de aceite (CA) avaliado |
| `applewin_sha` | SHA do AppleWin usado como Oráculo |
| `rust_sha` | SHA do AppleII-Rust testado |
| `fixture_sha256` | SHA-256 das Fixtures de entrada |
| `rom_sha256` | SHA-256 das ROMs usadas |
| `machine_model` | Modelo da Máquina |
| `cpu_model` | 6502 ou 65C02 |
| `cards_slots` | Cartas e Slots configurados |
| `host_os` | Sistema operacional hospedeiro |
| `host_arch` | Arquitetura do hospedeiro |
| `toolchain` | Versões de rustc, cargo e do Wine, quando usado |
| `input_trace` | Sequência de entradas aplicada |
| `command` | Comando exato executado |
| `expected` | Resultado esperado, com a origem |
| `actual` | Resultado obtido |
| `diff_artifact` | Caminho ou hash do artefato de diferença |
| `result` | `PASS`, `FAIL`, `BLOCKED`, `UNVERIFIED` ou `NOT_APPLICABLE` |
| `timestamp_utc` | Data e hora da execução em UTC |
| `reviewer` | Quem revisou o resultado (não pode ser quem implementou a alteração) |

O formato é validado pelo esquema `tests/conformance/manifests/schema.json`, escrito por outra frente de trabalho. Este documento só o referencia: em caso de divergência entre a lista acima e o esquema, vale o esquema, e esta página deve ser corrigida. Os manifestos ficam em `tests/conformance/manifests/` e os relatórios em `tests/conformance/reports/`.

Um resultado `BLOCKED` também gera Manifesto, com a causa em `actual`.
