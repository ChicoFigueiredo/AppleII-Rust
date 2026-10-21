# Matriz de capacidades

Item 2 do §21 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Consolida as linhas de [audit/wave0-core-capability.md](audit/wave0-core-capability.md) e [audit/wave0-peripherals-capability.md](audit/wave0-peripherals-capability.md) (estado de 20/10/2026, no SHA `6da6048`) com todos os resultados posteriores, até a onda final do G0 (21/10/2026): os diferenciais de [CPU](audit/cpu-trace-differential.md), [vídeo](audit/video-differential.md), [disco](audit/disk-differential.md) e [áudio](audit/audio-differential.md), o [relatório de importação de save-states](../../tests/conformance/reports/savestate-import.md), os vetores de CPU e Z80, os resultados por issue em [results/](results/) e os relatórios de plataforma.

Estado de execução: SHA da `main` `469bf941deab932ab228169a7ffebddae1fc6854` (reavaliação final; as versões anteriores desta matriz foram medidas em `bb01e34` e `22dbf2b`), AppleWin `a8f0d1d` como referência de código e o fork Linux do audetto (`a1e20c0`, núcleo `3e8054b`) como Oráculo executável. A tabela `baseline.md` tem os SHAs completos.

## Como ler

- **PASS**: um teste executado concorda com o Oráculo (executável ou código C++ lido, conforme a coluna Evidência). **FAIL**: um teste executado discorda. **BLOCKED**: o teste não pôde ser executado, com a causa. **UNVERIFIED**: ninguém comparou. **NOT_APPLICABLE**: não se aplica, com justificativa. **IMPLEMENTED**: existe código e testes próprios, sem comparação com o Oráculo; nunca vale PASS.
- Uma linha só diz PASS para o que a evidência cobre. Quando o mesmo componente tem partes em estados diferentes, ele aparece em linhas separadas (por exemplo, eventos do AY em PASS e PCM do AY em FAIL).
- "Oráculo" quer dizer o fork do audetto rodando headless, salvo onde a evidência diz Wine (AppleWin 1.32.1.0 oficial, só para gerar save-states) ou vetores de terceiros (SingleStepTests, ZEXALL), que são medidas de hardware ou de simuladores exatos e não do AppleWin.
- "Issue Rust" aponta para as histórias do backlog ([backlog.md](backlog.md)): #2 a #14 são os épicos EP-00 a EP-12, #15 a #44 as histórias HU e #45 a #80 as 36 histórias de issues abertas do AppleWin (EP-11, [upstream-open-selected.md](upstream-open-selected.md)). Prioridade é a do épico.
- O resultado de um Modelo não vale para outro. Modelos sem linha própria não foram comparados.
- Contagem total de casos do harness no SHA acima: veja [relatorio-final.md](relatorio-final.md), item 6.

## Núcleo: CPU, temporização e barramento

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| CPU 6502 NMOS: trace instrução a instrução (ciclo, PC, opcode, A, X, Y, SP, P) | Apple ][, ][+, //e (CPU 6502) | PASS | Casos `trace-applewin-ii-boot-2m`, `trace-applewin-iiplus-boot-2m/500k`, `trace-applewin-iie-boot-2m`: 2.000.000 de ciclos idênticos ao Oráculo (547 a 776 mil instruções por caso); [cpu-trace-differential.md](audit/cpu-trace-differential.md) | Nenhuma de CPU; seis divergências do ambiente (D1 a D7), todas corrigidas ou do lado do Oráculo | #3, #17 | P0 |
| CPU 65C02: trace | //e Enhanced | PASS | Casos `trace-applewin-iie-enhanced-boot-2m/500k` e `trace-applewin-iie-enhanced-dos33-5m` (5 milhões de ciclos, 1.821.912 instruções com o DOS 3.3) | Nenhuma | #3, #17 | P0 |
| CPU 6502/65C02: vetores SingleStepTests/65x02 (hardware) | NMOS 6502, Rockwell, Synertek e WDC 65C02 | PASS (estrito + diferenças catalogadas) | 10.220.000 de vetores, 0 inesperados ([65x02-singlestep.md](../../tests/conformance/reports/65x02-singlestep.md)); `#[ignore]` por ocupar 2,5 GB | AppleWin x hardware catalogadas D0 a D10 (ciclo fantasma, HLT, XAA, ARR decimal, ISC $FF, RMB/SMB/BBR/BBS como NOP, WAI/STP como NOP, `$5C`, `$DC/$FC`, `JMP (abs)` 65C02). O Rust segue o AppleWin; veja [gaps-and-known-limitations.md](gaps-and-known-limitations.md) | #17 | P0 |
| CPU: testes de Klaus Dormann | NMOS e 65C02 | PASS (funcional); IMPLEMENTED (`extended_cmos`, `#[ignore]`) | `apple2-cpu/tests/klaus.rs` | Instruções Rockwell/WDC ficam fora, como no AppleWin | #17 | P0 |
| CPU: tabela de ciclos e regressões de issues do AppleWin (`TestCPU6502.cpp`, GH#264/271/278/282/292/1257/1360 e outras) | NMOS e 65C02 | PASS | `apple2-cpu/tests/applewin_cpu.rs`, `opcode_semantics.rs`; 19 issues de CPU PASS em [results/cpu.csv](results/cpu.csv) | Nenhuma | #17 | P0 |
| CPU: determinismo | //e Enhanced | PASS | Casos `determinism-iie-enhanced-x10` e `determinism-trace-iie-enhanced-2m-x10` (10 repetições, mesmo hash) | Nenhuma | #17 | P0 |
| IRQ/NMI da CPU | todos | PASS (IRQ, BRK, RESET); NMI igual ao AppleWin | `opcode_semantics.rs:449-515`, `reset.rs`; AUD-C-053; NMI não chega à CPU como no AppleWin (AUD-C-054, `machine.rs::nmi_line_never_reaches_the_cpu_as_in_applewin`) | O hardware real trata NMI; o AppleWin não (código morto). O Rust copia o AppleWin | #17 | P0 |
| Ciclo fantasma (leitura/escrita espúria de alguns modos de endereçamento) | todos | FAIL contra hardware; igual ao AppleWin | D0 do relatório 65x02 (5.482.901 vetores) | O hardware faz o acesso, o AppleWin e o Rust não. Efeito colateral em `$C0xx` (AppleWin GH#967, aberta) | #17 | P1 |
| Floating bus: endereço do scanner e leituras em ciclos exatos | ][, ][+ (HBL + `$1000`), //e / Enhanced (NTSC e PAL) | PASS | Casos de trace com 4 modelos (qualquer leitura de `$C0xx` sem dono diverge sem o relógio de vídeo correto); `apple2-core/tests/video_scanner.rs`; cenas `issue-0241-*` | Nenhuma no endereço | #19 | P0 |
| Floating bus com `RAMRD=1` | //e | PASS (segue o AppleWin) | AUD-C-050, `audit_floating_bus_irq.rs`; importação de estados reais | No hardware lê a RAM principal; o AppleWin lê a imagem de leitura (aux). O Rust segue o AppleWin (ADR 0004) | #19 | P3 |
| Floating bus com Color Monitor PAL, `TEXT/GR` por linha e texto na página 2 | //e | PASS | #241: `tests/conformance/inputs/video/issue-0241-floatbus-dlores-text80.scene` (casos `video-issue-0241-*`: NTSC, Color TV PAL e Color Monitor PAL) | Nenhuma desde `de3ac3b` (histórico de sinal do primeiro quadro; antes, 1 pixel de borda em x=18, y=18) | #46 | P1 |
| Estado inicial da RAM (`MemReset`, `MIP_*`) | todos | PASS | `apple2-core/src/ram_init.rs`; boots do vídeo com "RAM padrão do AppleWin" e "RAM zerada" (DIV-004) | O sorteio de `$4E/$4F` é fixo no Oráculo (`$20`), aleatório no AppleWin real | #19 | P1 |
| Controle de velocidade ("Fastest") e pacing | todos | BLOCKED | #4, #290; vive no frontend (`applewin/src/pacing.rs`) e depende do relógio real; sem Oráculo headless | Nenhuma medida | #36 | P1 |
| Disco rápido com som ativo (GH#1340) e "Fastest" em todos os Modelos (#1448) | todos | PASS | `crates/applewin/tests/gh1340_full_speed_sound.rs` (8 testes: o disco só acelera sem alto-falante nem Mockingboard ativos, como `AppleWin.cpp:183`); `p23_ui.rs::p23_fastest_speed_on_every_model` | #1448: o AppleWin não acelera no ][ e ][+ e o port sim (a favor do usuário, sem decisão) | #36 | P2 |

## Memória e cartas de RAM

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| RAM principal, AUX 80 colunas, Extended 80 colunas | ][, ][+, //e, //e Enhanced | PASS (comportamento); IMPLEMENTED (soft switches isolados) | Trace dos 4 modelos; 22 issues de memória em [results/memoria.csv](results/memoria.csv): 16 PASS, 6 BLOCKED (software de terceiros); `apple2-core/tests/paging.rs`; importação de estados `audit-A2e-enhanced-aux-*` | Nenhuma | #3, #18 | P0 |
| a2audit (MMU, ROM de slot, LC) | //e Enhanced | PASS | `tools/conformance/fetch-a2audit.sh`; `apple2-conformance/tests/a2audit.rs` | Nenhuma | #18 | P0 |
| RamWorks III (127 bancos, reset x power-cycle) | //e, //e Enhanced | PASS | #158, #520, #1414 em `audit_memoria.rs`; estados `RamWorks_r4` (PASS) e `RamWorksIII-256KB` (trace PASS, formato FAIL 1 campo) | Nenhuma no trace; 1 campo de formato não modelado | #18, #52 | P0 |
| Language Card 16 K | ][, ][+ (slot 0), //e (embutido) | PASS | `apple2-core/tests/language_card.rs` (16 chaves, UTAIIe 5-23, GH#392/#700); estados `LC-RamWrite-Test*`, `audit-A2-LC`, `audit-A2+-LC` (traces idênticos) | Nenhuma | #18 | P0 |
| Saturn 64 K / 128 K (contagem de bancos configurada) | ][, ][+, //e | PASS | AUD-C-001/002/003/005/006 corrigidos (`apple2-config/tests/audit_saturn_banks.rs`, `apple2-savestate/tests/audit_saturn_state.rs`); estados reais `Saturn64`, `Saturn128`, `Saturn128-Bank3-2`, `II+_s0(Saturn)_s3(Saturn)-test1/2` e `audit-A2+-Saturn` PASS no trace de 100.000 ciclos | Os FAIL antigos de `Saturn64` e `Saturn128` (ciclo +406) eram do Oráculo, que lia lixo de heap nos bancos sem zerar (`MALLOC_PERTURB_=255` no `run-audetto-oracle.sh`); o port já estava certo | #18, #25 | P0 |
| Slot vazio / ROM de slot (`$Cn00`, `$C800`) | //e, ][+ | PASS | #149, #164, #190, #418, #423 em `audit_memoria.rs`; a2audit | Nenhuma | #18 | P1 |
| Fixtures de terceiros (Pooyan, Castle Wolfenstein, ProDrive, Quark Catalyst, Bejeweled, Total Replay) | vários | BLOCKED | #206, #222, #225, #297, #400, #422, #1126 (memória: 6 BLOCKED no total) | Sem fixture licenciada; os mecanismos sintéticos passam | #18 | P0 (#400, #422, #1126) |

## Vídeo

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Texto 40/80, LORES, DLORES, HIRES, DHIRES, misto, troca de página, bordas, flash | ][, ][+, //e, //e Enhanced | PASS | 1.573 comparações de framebuffer completo (diferença máxima de canal 0) nos 9 tipos de vídeo, NTSC 60 Hz e PAL 50 Hz; 583 casos `video-*` no harness, depois da onda final ([video-differential.md](audit/video-differential.md)) | Nenhuma. DIV-001 a DIV-004 corrigidas | #4, #20 | P0 |
| Troca de modo no meio da linha e do quadro (#1443, #1131, #650) | //e Enhanced | PASS | casos `video-midline-*`, `video-pageflip-*`; `apple2-video/tests/pal_1443.rs` | #650: o deslocamento de 1 pixel é comportamento atual do AppleWin e é reproduzido (NOT_APPLICABLE) | #4, #20, #56 | P0 |
| Tipos de vídeo: monocromático, idealizado, monitor NTSC/PAL, Color TV, B&W TV, âmbar, verde, branco | todos | PASS | idem | Nenhuma | #20 | P0 |
| Cartas RGB (Video-7 SL7, Le Chat Mauve EVE e Feline, AppleColor) | //e, //e Enhanced | PASS (renderização AN3, 140/160/misto/560); PASS (estado no save-state) | 42 + 14 comparações por grupo; `eec53b3` (estado RGB com round-trip) | Nenhuma | #20 | P1 |
| VidHD: modos legados centrados (640x400) e Super Hi-Res | //e | PASS | 9 + 4 comparações por tipo; DIV-002/DIV-003 corrigidas; `apple2-video/tests/vidhd_roundtrip.rs` | VidHD em ][/][+, SHR em PAL e modos 80x45/cor de borda não foram comparados (o AppleWin também não os suporta) | #4, #21 | P0 |
| Color Monitor PAL com floating bus (#241) | //e | PASS | Veja a linha de floating bus | Nenhuma (corrigida em `de3ac3b`) | #46 | P1 |
| Modelos Pravets, TK3000, Base64A, J-Plus no vídeo | clones | UNVERIFIED | Fora da bateria diferencial; os estados reais `Pravets82/8M/8A`, `Base64A`, `TK3000e`, `Video ROM/A2JPlus` batem no trace de CPU | Sem comparação de framebuffer | #4 | P1 |
| Fixtures de terceiros e apresentação (Karateka, cintilação em velocidade máxima, depurador na tela) | //e | BLOCKED | 12 issues de vídeo em [results/video.csv](results/video.csv) (apresentação do frontend, sem Oráculo headless; jogos sem fixture) | Sem medida | #4 | P1 |
| Desempenho do renderizador | todos | IMPLEMENTED | [performance.md](../../tests/conformance/reports/performance.md): `ntsc_tv` 12,8 MHz, `pal` 14,4 MHz, `hgr` 12,1 MHz (mediana, máquina compartilhada) | Não é comparação com o AppleWin | #38 | P1 |

## Disco

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Disk II: DSK/DO/PO, boot do DOS 3.3 e do ProDOS 2.4.3 | ][+, //e Enhanced | PASS | 56 casos `disk-*`, 103.146.754 de instruções comparadas (ciclo, PC, A, X, Y, SP, P de cada uma) e SHA-256 final das imagens ([disk-differential.md](audit/disk-differential.md)) | Nenhuma | #5, #22 | P0 |
| Disk II: WOZ1/WOZ2 reais, jitter, bit timing, quarter tracks, bits sem fluxo | //e Enhanced | PASS (WOZ2 e 7 WOZ + 2 em zip do AppleWin-Test); IMPLEMENTED (WOZ1) | casos `disk-woz-*`, `disk-micro-*-woz` | Sem WOZ1 real; proteção de cópia (E7, bit-slip) só até o boot | #5, #22 | P0 |
| Disk II: escrita de setor (DO, GZ, WOZ), INIT, nibble parcial, write-protect, rotação | ][+, //e Enhanced | PASS | casos `disk-micro-write-sector*`, `disk-micro-partial-nibble*`, `disk-micro-rotation*` | INIT/FORMAT não executados no Oráculo; firmware de 13 setores não comparado | #22 | P0 |
| Disk II: NIB | //e Enhanced | IMPLEMENTED | `apple2-storage/tests/images.rs`, fuzz | Sem NIB real; `DiskFormat/*` BLOCKED (imagens `test0.nib/dsk` e `wozv*.woz` ausentes do corpus) | #22 | P0 |
| Disk II: 2MG, gz, zip (contêiner) | //e Enhanced | PASS | `disk-boot-prodos243-2mg-po`, `disk-micro-dual-2mg`, `disk-micro-write-sector-gz`, boots Ultima V `.do.gz`, Buzzard Bait/Glutton `.zip`; `audit_images.rs`, `fuzz_parsers.rs` | AUD-C-033 e AUD-C-035 corrigidos | #22 | P0 |
| HDD / SmartPort: boot, E/S de bloco, status, DMA, imagem de 32 MB, firmware do DIB 132.1 | //e, //e Enhanced | PASS | casos `disk-hdd-*` (8); `apple2-storage/tests/hdd.rs`; estado real `SmartPort-firmware` PASS (a versão do firmware passou de 130.0 para 132.1, como o `version.h` do `a8f0d1d`) | HDC no ][+ não comparável: o Oráculo (22 commits mais antigo) não tem o patch de Shift (GH#1234). Escrita atômica só até 2 MiB; HDD de até 32 MB é gravado no lugar | #24 | P0 |
| Gravação atômica e queda simulada | todos | PASS | `apple2-storage/src/store.rs` (`Fault`), `crash_safety.rs`; AUD-C-033 | Sem teste de queda real de energia ou de disco | #22, #26 | P0 |
| Troca de disco em execução (hot swap), drive vazio, motor, SIGTERM | todos | IMPLEMENTED (testes Rust); BLOCKED (contra Oráculo) | `apple2-storage/tests/hot_insert.rs`; `applewin/tests/sigterm.rs`; #640, #831, #1078 | O Oráculo não insere nem troca disco com a máquina ligada e não aceita entrada | #23 | P0 |
| Atualização dos cartões por quadro (17.030 ciclos) contra lote de 1 ms | todos | NÃO IGUAL (documentado) | disk-differential.md, L6 | Até 16 ms de diferença no fim do atraso de 1 s do motor | #23 | P2 |
| Imagens de terceiros (Ultima I, jogos) | vários | BLOCKED | 28 issues de disco BLOCKED em [results/disco.csv](results/disco.csv) | Sem fixture com proveniência | #5 | P1 |

## Save-state

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Importar `.aws.yaml` real: trace de 100.000 ciclos igual ao Oráculo | vários Modelos (][, ][+, //e, //e Enhanced, clones) | PASS (88 de 115) | [savestate-import.md](../../tests/conformance/reports/savestate-import.md): **88 PASS, 0 FAIL, 27 BLOCKED** (3 estados do AppleWin 1.32.1.0 sob Wine + 112 do AppleWin-Test); 87 casos `savestate-*`; o runner compara `trace_rows_sha256` | O teste de formato (`check-import`) acusa 1 a 8 campos na maioria dos arquivos; o mais comum é o bit B do P (`0x37` no arquivo, `0x27` na máquina), normalizado de propósito | #6, #25 | P0 |
| Importar: os 9 estados que estavam em FAIL | //e Enhanced, ][+ | PASS | `SNES_MAX-slot4`, `dazzledraw1` x3, `SmartPort-firmware`, `Saturn64`, `Saturn128`, `6522-one-shot`, `SSI263-test` | Defeito do port em 4 arquivos (VBL do mouse x3, firmware do DIB); do Oráculo em 5 (heap dos bancos Saturn x2, SNES MAX sem E/S, ROM do SSC ausente x2) | #25 | P1 |
| Importar: 27 estados BLOCKED | vários | BLOCKED | 18 de `DiskFormat/*` e `AZTEC` (imagens ausentes), 3 SSC com TCP/serial, 2 No-Slot Clock (hora real), 2 `skm-*` (o Oráculo não tem o Shift Key Mod), `CPM2.2(56k)-3` (Z80 ativo), `contiki` (Uthernet) | Sem comparação possível | #25 | P1 |
| Pelo menos um estado importado de cada versão antiga do esquema | todos | PASS (recusa de versão desconhecida); PASS (migração, corpus real) | [savestate-schema.md](savestate-schema.md); AUD-C-012 | Versões antigas só pelo corpus | #25 | P1 |
| Salvar: gravação atômica; round-trip (salvar, carregar, executar) | todos | PASS | `apple2-savestate/tests/atomic_write.rs`, `roundtrip.rs`; casos `oracle_roundtrip.rs` (estado salvo pelo Rust carregado no Oráculo, 30.000 ciclos idênticos); AUD-C-022 | Compara também com o Oráculo, não só com o port | #26 | P0 |
| Carregador contra arquivo hostil (`Length`, bomba zlib, YAML truncado/mutado) | todos | PASS | `fuzz_loader.rs`, `audit_robustness.rs`; AUD-C-010/011 corrigidos | Nenhuma | #26 | P0 |
| Estado da carta RGB, nome da impressora, contrabarras | todos | PASS | AUD-C-020/023/024; `eec53b3` | Nenhuma | #26 | P1 |
| Save State on Exit após reinício de hardware (#564) | todos | BLOCKED | `applewin/tests/state_gh564.rs` passa; sem Oráculo de janela e encerramento | Sem medida contra o AppleWin | #26 | P0 |
| Abrir imagens citadas pelo estado em leitura e escrita (AUD-C-037) | todos | UNVERIFIED (risco) | `applewin/src/state_io.rs` | Igual ao AppleWin; um estado de terceiros pode alterar arquivos locais | #26 | P3 |

## Áudio

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Mockingboard C / Phasor: escritas 6522 e AY, timers, IRQ, IFR/IER/ACR | ][+, //e | PASS | 34 casos `audio-*` PASS: `audio-ay-tone` (225 eventos idênticos), `audio-ay-envelope` (148), `audio-ay-noise` (155), `audio-via-timers-irq` (110), `audio-phasor-two-ay` (76), `audio-ay-sockets-bad6522` (524) | Nenhuma nos eventos | #7, #27 | P1 |
| Núcleo do AY contra `AY8910.cpp` | ][+, //e | PASS | `apple2-audio/tests/ay_core_oracle.rs`: ~330 mil amostras idênticas | Nenhuma | #27 | P1 |
| PCM do AY (tom, envelope, ruído) | ][+, //e | FAIL (Limitação aceita, ADR 0006) | casos `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm` (D-2) | Frequência 8,77 %, 5,88 % e 58,33 % acima dos limiares registrados antes. O AppleWin gera blocos truncados (43 amostras onde seriam 43,2: -0,4 %); o Rust gera cada amostra no ciclo exato | #27 | P1 |
| Alto-falante: eventos | todos | PASS | `audio-speaker-clicks` (150 eventos idênticos) | Nenhuma | #7 | P1 |
| Alto-falante: PCM | todos | FAIL (Limitação aceita, ADR 0006) | `audio-speaker-clicks-pcm` (D-3): RMS 2,46 % > 2,00 % | O AppleWin amostra a cada 23 ciclos (inteiro) e tem 0,6 % mais quadros; o Rust usa clk/44100 exato | #7 | P1 |
| SSI263: fonemas encadeados por IRQ, power-down (#1412) | ][+, //e | PASS (fase 0); FAIL (fase +666; Limitação aceita, ADR 0006) | `audio-ssi263-phonemes-p0` PASS; `audio-ssi263-phonemes-p333` FAIL (D-4): 1.683 ciclos contra o limiar de 1.670 | 13 ciclos acima do limiar; o fim do fonema é visto em `PeriodicUpdate` no AppleWin | #28 | P1 |
| SSI263 x4 e SC-01 x2 (fixtures reais) | ][+, //e | PASS | `audio-ssi263-4x-fixture`, `audio-sc01-2x-fixture` | Só duração de fonema; o timbre fica para o roteiro manual | #28 | P1 |
| 6522: IRQ no último ciclo do opcode (#1176) | ][+, //e | PASS | `audio-issue-1176-last-cycle` (sem `known_divergence`); o `Reset()` do 6522 fecha o portão `m_canAssertIRQ` dos eventos de timer (`2927c25`) | Nenhuma | #7 | P1 |
| Echo+, MegaAudio, SD Music, SAM, Phasor+SSI263 | ][+, //e | IMPLEMENTED | `apple2-audio/tests/machine.rs` | Sem caso diferencial | #27, #28 | P2 |
| Audição humana, dispositivo de som, DirectSound | todos | BLOCKED | [manual-tests/audio.md](manual-tests/audio.md); 20 issues de áudio BLOCKED em [results/audio.csv](results/audio.csv) | Sem ouvinte nem dispositivo real | #7 | P1 |

## Z80 e cartas de expansão

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Z80 SoftCard: instruções, flags, MEMPTR, T-states | ][+, //e | PASS (contra vetores de terceiros) | 1.604.000 de 1.604.000 vetores SingleStepTests/z80 ([z80-singlestep.md](../../tests/conformance/reports/z80-singlestep.md)); ZEXDOC e ZEXALL 65/65 OK ([z80-zex.md](../../tests/conformance/reports/z80-zex.md)) | Não é comparação com o Z80VICE do AppleWin. F3/F5 de LDIR/INIR/OTIR repetindo seguem o hardware, não o AppleWin (candidata, veja gaps) | #8, #29 | P1 |
| Z80: IRQ/NMI/IM/HALT | ][+, //e | PASS (núcleo); a carta não liga as linhas | `apple2-cards/tests/z80_interrupts.rs`; `softcard_does_not_wire_the_interrupt_lines` | Igual ao AppleWin (também não entrega IRQ/NMI) | #29 | P1 |
| Z80 SoftCard no sistema (CP/M) contra o Oráculo | ][+, //e | BLOCKED | `CPM2.2(56k)-3` e #72: o trace do Oráculo cobre só a CPU 6502; nenhuma fixture com SoftCard e Phasor juntos | Sem comparação | #29 | P1 |
| Mouse (Mouse Interface) | ][+, //e | PASS | `A2e-internal-C4nn-with-mousecard` PASS; `dazzledraw1` x3 PASS (#381 corrigida: o primeiro evento de VBL vem 12.480 ciclos depois do carregamento e não soma de novo os ciclos do opcode) | Nenhuma | #6, #32 | P1 |
| Relógios: No-Slot Clock, Generic, ThunderClock Plus | ][+, //e | IMPLEMENTED | `apple2-cards/tests/clocks.rs`; estados `NoSlotClockTest*` BLOCKED (hora real) | ThunderClock Plus existe, sem tipo de carta escolhível na configuração | #30 | P1 |
| SSC, impressora, Uthernet I e II | ][+, //e | IMPLEMENTED; BLOCKED (rede e serial reais) | `apple2-hostio/tests/audit_entrada.rs`, `uthernet1.rs`, `uthernet2.rs`; [manual-tests/rede-serial-impressora.md](manual-tests/rede-serial-impressora.md) | Uthernet II sem SPI, PPPoE e interrupções (também no AppleWin); estados com TCP BLOCKED | #31, #33 | P1 |
| 4Play, SNES MAX, botões e paddles (#1522, #1234) | ][+, //e | PASS | `audit_wave0_peripherals.rs`, `p23_entrada.rs`; `SNES_MAX-slot4` PASS (o FAIL antigo era do Oráculo, sem tratador de E/S na carta: `audetto-snesmax.patch`) | Nenhuma | #30, #32 | P1 |
| Open Apple depois do F2 (#1396) | //e | PASS (Divergência aprovada, ADR 0006) | `crates/applewin/tests/p23_entrada.rs::gh1396_alt_f2_keeps_open_apple_pressed_approved_divergence` | O AppleWin zera o Open Apple no F2 (`JoyReset`); o port mantém e Alt+F2 funciona. Decisão do dono de 21/10/2026 | #32 | P2 |
| Breakpoint Card | ][+, //e | IMPLEMENTED | Entrega dos breakpoints pelo depurador (AUD-P-016 corrigido) | Sem Oráculo | #30 | P2 |
| Configuração de slots (conflitos) | ][+, //e | PASS (como o CardManager do AppleWin) | `374852b` e testes `aud_p_019/020` | Nenhuma | #30 | P2 |

## Depurador, interface e plataformas

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Depurador: 130 issues, 251 comandos | ][+, //e Enhanced | PASS (79); FAIL (5); BLOCKED (7); NOT_APPLICABLE (39) | [results/depurador.csv](results/depurador.csv) e [results/p23-depurador.csv](results/p23-depurador.csv); `apple2-debugger/tests/matriz_depurador*.rs`, `p23_depurador.rs`; [debugger-commands.md](audit/debugger-commands.md) (249 comandos IMPLEMENTED, 2 N/A) | Seis Divergências aprovadas (ADR 0006, 21/10/2026): #114, #245, #458, #466, #468, #1381, hoje PASS. Os 5 FAIL são comportamentos do próprio AppleWin que o Rust reproduz: #713, #1145, #1377 (paridade mantida pelo dono) e #1426, #1431 (adiadas) | #10, #34 | P1 |
| Depurador: atalhos de teclado (`DebuggerProcessKey`) | ][+, //e Enhanced | PASS | `crates/apple2-debugger/tests/hu0801.rs`, `crates/applewin/tests/debugger.rs` (AUD-P-035/036/037) | Extensão: Ctrl+F11 passo | #34 | P1 |
| Depurador: robustez (fuzz da linha de comando, Unicode, intervalos) | todos | PASS | `4cd5132`, `d2e9962`; fuzz da linha de comando | Panics com Unicode e cópia gigante corrigidos | #34 | P1 |
| Interface: tecla presa, Alt+Enter, somente leitura, soltar disco, monitor de setores, benchmark, importar configuração, tela cheia, título, status, escala | todos | PASS (testes Rust); sem Oráculo de janela | `applewin/tests/hu0702_input.rs`, `audit_persistence.rs`, `p23_ui.rs`; [results/p23-ui.csv](results/p23-ui.csv) (49 issues: 29 PASS, 20 NOT_APPLICABLE); HU-0702/0802 | Divergências aprovadas (ADR 0006: tela cheia #280/#358/#1374/#117, monitor atual #932, "Fastest" #1448): veja o gaps. #781 (cursor de espera ao carregar estado) FAIL | #32, #35 | P1 |
| Linha de comando, ROMs do usuário (`-rom`, `-f8rom`, `-videorom`) | todos | PASS | `06d3f42`, `0973c85`; proveniência em [rom-provenance.md](rom-provenance.md) | Nenhuma | #35 | P1 |
| Linux x86_64 nativo: build, 5 Modelos headless, GUI (XWayland e Xvfb), HiDPI x2, tela cheia, áudio ausente, pacote extraído | Linux | PASS | [relatorio-plataformas.md](platform/relatorio-plataformas.md); `tools/package/smoke-linux.sh`, `test-package.sh` (17 verificações) | Wayland nativo, minimizar, teclado regional, gamepad: roteiro manual | #11, #36 | P0 |
| Windows x86_64 via Wine 10.0 (build cruzado msvc) | Windows (Wine) | PASS | `applewin.exe` 16,6 MB; PNG idêntico ao do Linux em 5 Modelos e no DOS 3.3 ([windows-wine.md](platform/windows-wine.md)) | Wine não é Windows | #11, #36 | P0 |
| Windows nativo | Windows | BLOCKED | Sem execução (CI manual e sem cota; VM não autorizada) | DPI por monitor, manifesto, console, WASAPI, XInput, SmartScreen | #36 | P0 |
| macOS Intel e Apple Silicon | macOS | BLOCKED | Sem hardware nem SDK da Apple | Tudo | #36 | P0 |
| Hardware físico (gamepad, serial, pcap, escuta de áudio, captura de mouse) | todos | BLOCKED | Roteiros em [manual-tests/](manual-tests/) | Sem execução | #33, #36 | P1 |
| Distribuição: `release.yml` com pré-release por sufixo, `verify`, SHA256SUMS, SBOM CycloneDX, licenças de terceiros | todos | IMPLEMENTED | Pacote Linux reproduzido localmente; YAML válido; workflow nunca executado em CI | Pacotes não assinados nem notarizados ([assinatura.md](platform/assinatura.md)) | #37 | P1 |

## Extensões e estudos (sem equivalente no AppleWin)

| Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade |
|---|---|---|---|---|---|---|
| Depuração temporal / rewind | //e Enhanced | IMPLEMENTED | crate `apple2-rewind`; capítulo 16 do manual | Extensão; sem Oráculo | #40 | P2 |
| Adaptador DAP para VS Code | ][+, //e Enhanced | IMPLEMENTED | crate `apple2-dap`, `applewin-dap`; capítulo 15 do manual | Extensão | #41 | P2 |
| Laboratório educacional | //e Enhanced | IMPLEMENTED | crate `apple2-lab`; capítulo 14 do manual | Extensão | #42 | P3 |
| Apple I | Apple I | NOT_APPLICABLE | [ADR 0005](../adr/0005-apple-i-fora-do-escopo-ate-decisao.md); [apple1-investigation.md](apple1-investigation.md) | Fora do escopo até decisão do dono | #43 | P3 |
| Frontend web/WASM | núcleo | IMPLEMENTED (protótipo opt-in) | [wasm-estudo.md](wasm-estudo.md); crate `applewin-web`; o núcleo compila para `wasm32-unknown-unknown` sem mudanças | Protótipo | #44 | P3 |
| Desempenho determinístico | todos | IMPLEMENTED | crate `apple2-bench`, [performance.md](../../tests/conformance/reports/performance.md), `--check` do baseline | Sem hardware nem compilação fixos; não é comparação com o AppleWin | #38 | P1 |

## Issues upstream sem componente próprio

As 1.115 issues do AppleWin estão na [matriz de issues](applewin-issue-matrix.csv) com o estado de verificação de cada uma (resumo em [issue-audit.md](issue-audit.md)). Nenhuma issue fechada está em PASS sem teste. Nenhuma está UNVERIFIED: PASS 487, FAIL 6, BLOCKED 133, NOT_APPLICABLE 489.
