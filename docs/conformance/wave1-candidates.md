# Lista de trabalho da onda 1 (HU-0001)

Fonte: `docs/conformance/applewin-issue-matrix.csv` após a auditoria independente (`docs/conformance/issue-audit-review.md`). Critério: issues com prioridade **P0 ou P1** e aplicabilidade **runtime, formato ou debugger**. Total: **246** issues (P0: 30, P1: 216).

Como ler:

- `caminho Rust` = `mapeamento_rust` da matriz (arquivo existente no repositório; é o módulo provável, não prova de defeito).
- "Como reproduzir no port" descreve o cenário a ser montado no port (headless ou teste de crate), derivado do relato upstream. Quando a reprodução depende de imagem de terceiros (jogos em WOZ/DSK, demos), o teste exige registrar a fixture com proveniência e SHA-256 no corpus, e fica `BLOCKED` até lá.
- "Teste proposto" indica onde o caso deve entrar; os arquivos citados existem em `crates/*/tests` (ou, se novo, o crate indicado). Nenhum teste foi executado nesta etapa: todos continuam `UNVERIFIED`, sem PASS/FAIL.
- Dentro de cada área, a ordem é P0 antes de P1 e depois por número upstream. A ordem de risco dos P0 está em `issue-audit-review.md`, seção 6.

## Memória / Language Card / Saturn / RamWorks (22)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 164 | MMU no good when no cards in slot-4 & 5 **[P0]** | memoria | `apple2-core/src/slots.rs` | //e com slots 4 e 5 vazios: `LDA #0` em $300 e rodar o teste de MMU do a2audit (INTCXROM/SLOTC3ROM não dependem de cartões) | apple2-core/tests/paging.rs: soft switches de ROM de slot com slots 4/5 vazios |
| 392 | a2audit errors **[P0]** | memoria | `apple2-core/src/language_card.rs`, `apple2-core/src/memory.rs` | Rodar a2audit (LC e LC aux) no //e enhanced; sequência LDA $C080, LDA $C081, INC $D17B, INC $FE1F | apple2-core/tests/language_card.rs: tabela da sequência do a2audit |
| 395 | LC write-enabled state is not maintained correctly (1.26.2.0) **[P0]** | memoria | `apple2-core/src/language_card.rs` | Duas leituras de $C081 (write-enable), quatro STA $C083 e escrita no LC: LC deve continuar gravável | apple2-core/tests/language_card.rs: STA não limpa o write-enable |
| 400 | Quark Catalyst V3 & BeagleWrite crash **[P0]** | memoria | `apple2-core/src/language_card.rs`, `apple2-core/src/memory.rs`, `apple2-cards/src/mouse.rs` | Cartão de mouse no slot 4 + software que usa ROM interna: IRQ do mouse não pode corromper paginação do LC/ROM (Quark Catalyst, BeagleWrite) | apple2-core/tests/paging.rs + apple2-cards/tests/mouse.rs: IRQ de VBL durante código em ROM |
| 422 | Apple // Bejeweled crash with version 1.26.2.3 **[P0]** | memoria | `apple2-core/src/memory.rs`, `apple2-cards/src/mouse.rs` | Mouse no slot 4, IRQ de VBL com PC em ROM interna ($CCE0): ao retornar a ROM interna deve continuar selecionada (Bejeweled) | apple2-core/tests/paging.rs: INTCXROM preservado em IRQ |
| 700 | ProDOS8 2.5.0 alpha6 not working **[P0]** | memoria | `apple2-core/src/language_card.rs` | 6502 (II+/Apple LC): `INC $C08B` (duas leituras) deve habilitar escrita do LC; boot do ProDOS 2.5 alpha6 | apple2-core/tests/language_card.rs: leitura dupla por RMW |
| 1126 | emulated-memory corruption sometimes crashes AppleWin **[P0]** | memoria | `apple2-core/src/memory.rs` | Boot do Total Replay e demo do Karateka (WOZ); acesso a $C0EE (writeMode off) não pode corromper memória nem derrubar o emulador | apple2-storage/tests/disk2.rs: $C0EE com imagem WOZ sem panic |
| 1285 | CPU emulation: AppleWin crashes when doing a 16-bit read from 6502 address 0xFFFF **[P0]** | memoria | `apple2-core/src/memory.rs` | LC em HIGH RAM, `CMP $2000` em $FFFE (leitura de 16 bits em $FFFF) sem panic e Z=1 | apple2-cpu/tests/opcode_semantics.rs: operando que cruza $FFFF |
| 1486 | Saturn 128K card in slot 0 does not use configured size **[P0]** | memoria | `apple2-core/src/language_card.rs`, `apple2-config/src/card.rs` | Saturn 128K no slot 0 com 4 e 8 bancos configurados pela interface/config (não só linha de comando) | apple2-core/tests/language_card.rs: tamanho Saturn por slot 0 e config |
| 1495 | Saturn128 in AppleWin **[P0]** | memoria | `apple2-core/src/language_card.rs` | Ativar Saturn128 em qualquer slot e rodar o disco Mb3-88-88b5; sem halt/crash (DD86) | apple2-core/tests/language_card.rs: Saturn em slots 1-7 sem panic |
| 1520 | Can't start any Merlin version - v1.32.0.0 **[P0]** | memoria | `apple2-core/src/language_card.rs` | Boot de ProDOS/Merlin com LC e Saturn no estado inicial padrão: escrita habilitada em power-on/reset e ao trocar de slot | apple2-core/tests/language_card.rs: estado inicial do LC/Saturn |
| 149 | Card slot ROM issues with copy protection routines | memoria | `apple2-core/src/slots.rs` | Ler $Cn00-$CnFF de slot vazio/com cartão sem ROM e comparar com o floating bus (rotinas de proteção de cópia testam isso) | apple2-core/tests/cards.rs: leitura de ROM de slot vazio devolve floating bus |
| 158 | RAM inflation not working? | memoria | `apple2-core/src/memory.rs` | Configurar RamWorks com 127 bancos, escrever um padrão em cada banco aux via $C073 e reler | apple2-core/tests/paging.rs: 127 bancos endereçáveis sem aliasing |
| 190 | Writes to $C800-CFFF should match reads | memoria | `apple2-core/src/slots.rs` | Escrever em $C800-$CFFF e verificar o mesmo efeito de IO_STROBE que uma leitura | apple2-core/tests/cards.rs: escrita em $C800 aciona o strobe |
| 222 | Reset with MemoryInitPattern = MIP_PAGE_ADDRESS_LOW causes 'Beautiful Boot' to misread DOS3.3 disk catalog | memoria | `apple2-core/src/memory.rs` | Reset com padrão de memória inicial por página baixa e boot de disco DOS 3.3 com Beautiful Boot; catálogo deve ser lido | apple2-core/tests/boot.rs: padrões de inicialização de RAM x boot |
| 225 | AppleWin 1.25 RC2 fails to boot Castle Wolfenstein | memoria | `apple2-core/src/memory.rs` | Boot do Castle Wolfenstein com o padrão de RAM inicial padrão; não pode dar ?OVERFLOW ERROR | apple2-core/tests/boot.rs: boot com fixture castle_wolfenstein-fixed.dsk |
| 297 | RamWorks III ProDrive (ramdisk utility) corrupts 80 column display during disk access. | memoria | `apple2-core/src/memory.rs` | RamWorks III + ProDrive: em 80 colunas, CAT no RamDisk não pode piscar/corromper a tela; comparar framebuffer | apple2-video/tests/machine.rs: 80COL + troca de banco aux durante leitura de disco |
| 418 | Slot 3 ROM space does not float when Uthernet is enabled | memoria | `apple2-core/src/machine/io.rs`, `apple2-core/src/slots.rs` | Uthernet no slot 3 com firmware de 80 col desligado: leitura de $C3xx deve ser floating bus, não $00 | apple2-core/tests/cards.rs: $C3xx com SLOTC3ROM off e cartão no slot 3 |
| 423 | Support INTC8ROM soft switch for //e | memoria | `apple2-core/src/memory.rs`, `apple2-core/src/machine/io.rs` | Acessar $C3xx com SLOTC3ROM desligado deve selecionar a ROM interna em $C800-$CFFF (INTC8ROM); a2audit v1.04 | apple2-core/tests/paging.rs: INTC8ROM implícito |
| 520 | Reset not resetting the RAMworks bank causing graphic issues with DHGR games | memoria | `apple2-core/src/memory.rs` | RamWorks `-r 4`: selecionar banco 3 por $C073, Reset e boot do Airheart; DHGR deve usar banco 0 | apple2-core/tests/paging.rs: Reset/power-cycle volta ao banco 0 |
| 1414 | RamWorks III: power-cycle should reset to bank 0 | memoria | `apple2-core/src/memory.rs`, `apple2-config/src/card.rs` | RamWorks III: `C073:1` e power-cycle (F2); banco ativo deve voltar a 0 | apple2-core/tests/paging.rs: power-cycle zera o banco |
| 1428 | Enhanced //e RAM issue? | memoria | `apple2-core/src/memory.rs`, `apple2-config/src/machine.rs` | //e enhanced com configuração padrão deve ter 128K (SuperCalc exige) e permitir ajustar a RAM | apple2-config/tests (config de RAM) + apple2-core/tests/boot.rs |

## CPU e temporização (24)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 4 | Emulation Speed Control problem **[P0]** | cpu | `apple2-core/src/scheduler.rs` | Velocidade 'Fastest' e iniciar um jogo; o emulador não pode congelar | applewin/tests/headless.rs: executar N frames em velocidade máxima sem travar |
| 133 | Chessmaster 2000 crashes to Monitor on boot | cpu | `apple2-cpu/src/lib.rs` | Boot do Chessmaster 2000 (imagem virtualapple): não pode cair no Monitor (regressão desde 1.13.2) | apple2-storage/tests/boot.rs: boot com fixture Chessmaster |
| 204 | Problem running Wrath Of Denethenor   | cpu | `apple2-cpu/src/lib.rs` | Boot do Wrath of Denethenor: não pode ficar piscando/travado no título | apple2-storage/tests/boot.rs: boot com fixture |
| 206 | Pooyan Freezes After "Player 1" Prompt | cpu | `apple2-cpu/src/lib.rs` | Boot do Pooyan (Beautiful Boot): após 'Player 1' o jogo não pode congelar | apple2-storage/tests/boot.rs: boot com fixture |
| 264 | JMP (IND) is 5 cycles only - not 6 - on 6502 | cpu | `apple2-cpu/src/lib.rs` | Medir ciclos do JMP (ind) $6C em NMOS: 5 ciclos (6 só em 65C02) | apple2-cpu/tests/opcode_semantics.rs: ciclos de $6C por CPU |
| 271 | Timing problems with ASL, INC and DEC (abs,X) | cpu | `apple2-cpu/src/lib.rs` | Medir ciclos de ASL/DEC/INC abs,X ($1E,$DE,$FE) em NMOS: 7 ciclos fixos | apple2-cpu/tests/opcode_semantics.rs: tabela de ciclos NMOS |
| 278 | 6502 emulation: bSlowerOnPagecross setup too late in CPP macros | cpu | `apple2-cpu/src/lib.rs` | DEC abs,X NMOS com e sem cruzamento de página: o penalty de página deve ser avaliado depois do setup | apple2-cpu/tests/opcode_semantics.rs: ciclos de $DE |
| 288 | Timing issue with DEC (abs) | cpu | `apple2-cpu/src/lib.rs` | DEC $20FF ($CE): 6 ciclos em NMOS e em 65C02 | apple2-cpu/tests/opcode_semantics.rs: ciclos de $CE |
| 290 | Pascal Compile with AppleWin Emulation Speed at "Fastest"  | cpu | `applewin/src/pacing.rs`, `apple2-core/src/scheduler.rs` | Compilação Pascal com disco e velocidade 'Fastest': comportamento deve ser o mesmo da velocidade normal | applewin/tests/headless.rs: velocidade máxima com acesso a disco |
| 292 | More CPU errors (mostly 65C02 undocumented opcodes) | cpu | `apple2-cpu/src/lib.rs` | AND abs ($2D) 4 ciclos; opcodes indefinidos do 65C02 ($22,$42,$62,$82 ...) são NOPs com ciclos corretos | apple2-cpu/tests/opcode_semantics.rs: ciclos e bytes de opcodes indefinidos 65C02 |
| 307 | Pravets 8 emulation applies 65C02 instead of 6502. | cpu | `apple2-cpu/src/lib.rs`, `apple2-core/src/model.rs` | Modelo Pravets 8: a CPU deve ser 6502 NMOS (opcodes 65C02 viram NOP/ilegais) | apple2-core/tests/boot.rs + apple2-cpu: CPU por modelo |
| 321 | 6502 I/O access occurs early - on the cycle of the opcode | cpu | `apple2-cpu/src/lib.rs`, `apple2-core/src/machine.rs` | Instrução de 4 ciclos: acesso a I/O no último ciclo (não no 1º); detecção NTSC/PAL do Ansi Story | apple2-core/tests/cpu_events.rs: ciclo exato de leitura de $C0xx |
| 366 | Avoid double-counting extra cycles on IRQ | cpu | `apple2-cpu/src/lib.rs`, `apple2-core/src/irq.rs` | IRQ com uExtraCycles: ciclos extras não podem ser descontados duas vezes (medir ciclos até o ISR) | apple2-core/tests/cpu_events.rs: contagem de ciclos de IRQ |
| 503 | Does Bxx taken need an extra cycle penalty for this special end of page case? | cpu | `apple2-cpu/src/lib.rs` | Bxx tomado com cruzamento de página no fim da página ($2FD): conferir ciclos extras | apple2-cpu/tests/opcode_semantics.rs: ciclos de branch |
| 508 | Canyon Climber hangs when accelerated via GG | cpu | `applewin/src/pacing.rs` | Canyon Climber com aceleração: laço `LDA $C050 / BNE` (VBL do II+) não pode travar em full speed | applewin/tests/headless.rs: detecção de espera por VBL do II+ em velocidade máxima |
| 519 | Full-speed: prevent hangs when games uses II+ video sync | cpu | `applewin/src/pacing.rs` | Mesmo caso do #508: detectar espera de VBL do II+ em full-speed e evitar o travamento | applewin/tests/headless.rs |
| 651 | Interrupts are not cycle accurate | cpu | `apple2-core/src/irq.rs`, `apple2-cpu/src/lib.rs` | Interrupção cycle-accurate: test.dsk do Arnaud (IRQ no meio de uma instrução) | apple2-core/tests/cpu_events.rs: IRQ por ciclo |
| 801 | Wrong timing with LSR abs,X / ROR abs,X / ROL abs,X for the 6502 CPU | cpu | `apple2-cpu/src/lib.rs` | LSR/ROR/ROL abs,X ($5E,$3E,$7E) em NMOS: 7 ciclos | apple2-cpu/tests/opcode_semantics.rs: ciclos NMOS |
| 967 | 6502 indexed load double read when crossing page boundary | cpu | `apple2-cpu/src/lib.rs` | NMOS: leitura indexada cruzando página faz leitura dupla visível em $C0xx | apple2-cpu/tests/opcode_semantics.rs: leitura espúria no cruzamento |
| 1099 | Clearing the D flag (or not) on a BRK or Interrupt or Reset | cpu | `apple2-cpu/src/lib.rs` | BRK/IRQ/NMI/Reset: efeito no flag D em 6502 vs 65C02 | apple2-cpu/tests/opcode_semantics.rs: flag D por CPU |
| 1257 | incorrect order of stack operations | cpu | `apple2-cpu/src/lib.rs` | JSR com SP=$7D: ordem dos pushes e leitura do operando alto (resultado $0155 em vez de $1355) | apple2-cpu/tests/opcode_semantics.rs: JSR com pilha sobrescrevendo operando |
| 1303 | 6502 IRQ issue when interrupt source is not cleared | cpu | `apple2-core/src/irq.rs`, `apple2-cpu/src/lib.rs` | SEI + 6522 T1 gerando IRQ sem limpar: fim do trecho deve ter A=1 (6502/65C02) | apple2-core/tests/cpu_events.rs + apple2-audio: IRQ mascarada |
| 1356 | AppleWin 1.30.20 hangs on TR boot if any bp set | cpu | `apple2-core/src/irq.rs`, `apple2-debugger/src/bp.rs` | Debugger `bp 106`, F2 e boot do Total Replay com MB (2,5 MHz): IRQ ativa e RTI sem travar | apple2-debugger/tests/machine.rs: bp + IRQ com CPU acelerada |
| 1360 | NMOS 6502 illegal opcode 0x0C (aka SKW) | cpu | `apple2-cpu/src/lib.rs` | Opcode NMOS $0C (SKW): modo ABS, 4 ciclos (não ABSX) | apple2-cpu/tests/opcode_semantics.rs: $0C |

## Disco II / WOZ / imagens (58)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 73 | Save changes to disk image **[P0]** | disco | `apple2-storage/src/disk2.rs` | Gravar setor via RWTS e ejetar/trocar: a trilha suja deve ir para o arquivo do host | apple2-storage/tests/files.rs: flush ao ejetar |
| 119 | Catalog update not flushed **[P0]** | disco | `apple2-storage/src/store.rs` | SAVE de BASIC que aumenta o arquivo; ler a imagem do host: contagem de setores do catálogo/VTOC coerente | apple2-storage/tests/files.rs: flush após escrita de setor extra |
| 186 | ProDOS Order 2IMG crashing **[P0]** | disco | `apple2-storage/src/image.rs` | Montar 2IMG em ordem ProDOS (Microzine 7 lado A): usar ordem PO e bootar | apple2-storage/tests/images.rs: 2IMG PO |
| 640 | With Drive-1 empty, start from logo screen: inserting a disk doesn't boot **[P0]** | disco | `apple2-storage/src/disk2.rs`, `applewin/src/disk_ui.rs` | Estado de logo/drive vazio: F2 e arrastar imagem ao Drive-1; deve bootar | applewin/tests/headless.rs: inserir disco com drive vazio |
| 664 | Mounting .woz file blocks writing to .dsk images as well. **[P0]** | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/store.rs` | WOZ no drive 1 e DSK gravável no drive 2; COPYA grava sem assert/erro | apple2-storage/tests/disk2.rs: WOZ + DSK simultâneos |
| 821 | Fix the ASSERT in `Disk2InterfaceCard::ReadTrack()` **[P0]** | disco | `apple2-storage/src/disk2.rs` | DOS 3.3: `INIT HELLO,D2`, F2, `RUN HELLO,D2`; Plato Spanish sem I/O ERROR/assert | apple2-storage/tests/disk2.rs: leitura de trilha após INIT |
| 1402 | Bad WOZ crashes AppleWin **[P0]** | disco | `apple2-storage/src/woz.rs` | Montar WOZ com chunk TRKS de id [0,0,0,0]: erro tratado; formatar (Copy2Plus) sem panic | apple2-storage/tests/images.rs: WOZ inválido |
| 1444 | Write disk image to disk periodically **[P0]** | disco | `apple2-storage/src/store.rs`, `apple2-storage/src/disk2.rs` | Gravar setor sem ejetar: a imagem do host deve refletir a gravação periodicamente | apple2-storage/tests/files.rs: gravação periódica |
| 24 | Disk speed test show meaningless results | disco | `apple2-storage/src/disk2.rs` | Locksmith 6 speed test em velocidade autêntica: RPM plausível (~300 rpm) | apple2-storage/tests/disk2.rs: período de rotação |
| 29 | Fails to run Ultima I enhanced disks | disco | `apple2-storage/src/disk2.rs` | Boot de Ultima I enhanced (imagem do relator) | apple2-storage/tests/boot.rs: boot com fixture |
| 51 | Infiltrator doesn't boot | disco | `apple2-storage/src/disk2.rs` | Boot do Infiltrator (proteção de cópia): não pode travar | apple2-storage/tests/boot.rs |
| 125 | Cannot format a ProDOS discs | disco | `apple2-storage/src/disk2.rs` | Formatar disco ProDOS no drive 2 (ProDOS 1.1.1 Utility): sem 'DISK II DRIVE TOO SLOW' | apple2-storage/tests/disk2.rs: tempo de trilha na formatação |
| 138 | Platoon (Ocean) freezes on inserting disk2 | disco | `apple2-storage/src/disk2.rs` | Platoon: ao inserir o disco 2 depois do nível, não pode congelar | apple2-storage/tests/boot.rs |
| 139 | Time Zone crashes on inserting disk 5i | disco | `apple2-storage/src/disk2.rs` | Time Zone: inserir disco 5i (NIB de layout incomum): não pode travar em hlt | apple2-storage/tests/images.rs |
| 196 | Cannot format Pascal Disk | disco | `apple2-storage/src/disk2.rs` | Formatar disco no Apple Pascal 1.1/1.2: sem 'DRIVE SPEED IS TOO SLOW' | apple2-storage/tests/disk2.rs: formatação |
| 202 | Hyper-DOS crawls along at a snails pace | disco | `apple2-storage/src/disk2.rs` | Hyper-DOS: medir tempo de leitura em velocidade autêntica e comparar com o hardware | apple2-storage/tests/disk2.rs: temporização |
| 303 | reading from Q6L returns variable values even if drive is off | disco | `apple2-storage/src/disk2.rs` | Desligar o drive (`LDA $C088,X`), esperar >1 s e ler `$C08C,X`: o valor do latch deve ser constante | apple2-storage/tests/disk2.rs: latch com motor desligado |
| 338 | Ultima I Disk Drive speed error | disco | `apple2-storage/src/disk2.rs` | Formatar disco de jogador do Ultima I: sem erro de velocidade do drive | apple2-storage/tests/disk2.rs |
| 389 | AppleWin 1.26.1.1 write-protect detection misreporting | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/format_track.rs` | Gemstone Warrior: rotina em $575E lê write-protect; disco gravável não pode devolver $AA | apple2-storage/tests/disk2.rs: bit de write-protect |
| 494 | Partially formatted disk not recognised in drive | disco | `apple2-storage/src/new_disk.rs` | Criar blank.dsk novo (0 bytes), sair e reabrir: inserir não pode dar erro | apple2-storage/tests/files.rs: imagem de tamanho zero |
| 499 | "Send to CiderPress" - need to flush current track first | disco | `apple2-storage/src/store.rs` | Formatar (ProDOS) e ler o arquivo do host: a última trilha escrita deve estar no arquivo | apple2-storage/tests/files.rs: flush da trilha atual |
| 582 | Curse of Azure Bonds and Pool of Radiance crashes during character creation | disco | `apple2-storage/src/disk2.rs` | Curse of the Azure Bonds: trocar disco de save→disco 1 na criação de personagem | apple2-storage/tests/boot.rs: troca de disco com latch parcial |
| 599 | 1.27.9.0 stops early while booting Ultima V .do image with the Authentic Speed disk access setting | disco | `apple2-storage/src/disk2.rs` | Ultima V (.do) com Authentic Speed: o boot deve verificar o 2º drive e prosseguir | apple2-storage/tests/boot.rs |
| 649 | Subtle bug in dual drive support for .2mg wrapped .do/.po images | disco | `apple2-storage/src/image.rs` | Inserir .do.2mg no drive 1 e outra imagem do mesmo tipo no drive 2: contagem de trilhas/volume por imagem | apple2-storage/tests/images.rs |
| 661 | Top-level: non-working WOZ images | disco | `apple2-storage/src/woz.rs` | Tracker de WOZ que não funcionam: montar cada imagem listada (Wizardry III, Space Quest I etc.) | apple2-storage/tests/boot.rs: corpus de WOZ (ver #662, #668, #669) |
| 662 | WOZ: Wizardry III copy-protection check fails on T$22 (T34) | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/woz.rs` | Wizardry III (WOZ): verificação de proteção em T$22 não pode entrar em loop | apple2-storage/tests/boot.rs: fixture WOZ |
| 668 | WOZ: Seafox fails to boot | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Seafox (WOZ): chegar além de T$0B sem reboot | apple2-storage/tests/boot.rs: fixture WOZ |
| 669 | WOZ: Space Quest I does not boot reliably | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Space Quest I (WOZ): boot confiável até a calibração do joystick | apple2-storage/tests/boot.rs: fixture WOZ |
| 672 | WOZ: All even disk i/o read accesses will read the data latch | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/woz.rs` | WOZ: `LDA $C08E,X` (acesso par) deve devolver o latch lido (não o valor antigo) | apple2-storage/tests/disk2.rs: latch por endereço par |
| 673 | DSK/NIB: All even disk i/o read accesses will read the data latch | disco | `apple2-storage/src/disk2.rs` | DSK/NIB: leituras em endereços pares do controlador (ex. $C0E0) devolvem o latch | apple2-storage/tests/disk2.rs |
| 686 | Star Fleet I does not recognize side B woz image | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Star Fleet I (WOZ): boot do lado A, inserir lado B ao pedir; deve reconhecer | apple2-storage/tests/boot.rs: troca de lado WOZ |
| 695 | Disk ][ MC3470 noise emulation | disco | `apple2-storage/src/disk2.rs`, `apple2-audio/src/speaker.rs` | Taipan (WOZ): emular ruído MC3470 (bits aleatórios em área sem fluxo) | apple2-storage/tests/images.rs: bits de ruído no WOZ |
| 704 | Castle Wolfenstein re-release (WOZ) freezes while writing to disk | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Castle Wolfenstein re-release (WOZ): gravar arquivo ('Writing Disk File') sem travar | apple2-storage/tests/disk2.rs: escrita em WOZ |
| 705 | The Crack of Doom (WOZ) "I/O error" when swapping to side B | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | The Crack of Doom (WOZ): virar para o lado B sem 'I/O error' | apple2-storage/tests/boot.rs |
| 733 | Wasteland Booting Weirdness | disco | `apple2-storage/src/woz.rs` | Wasteland (WOZ): deve bootar (gap de 35 us entre acessos a $C0EC em T$00) | apple2-storage/tests/boot.rs |
| 745 | WOZ: Congo Bongo doesn't boot (track too big) | disco | `apple2-storage/src/woz.rs` | Congo Bongo (WOZ): trilha 2 com 0xE004 bits; usar 'Largest Track' do INFO v2 | apple2-storage/tests/images.rs: trilha grande |
| 762 | WOZ: Frog Jump doesn't boot reliably | disco | `apple2-storage/src/woz.rs` | Frog Jump (WOZ): boot confiável (sem 'SYSTEM FAILURE NUMBER 2') | apple2-storage/tests/boot.rs |
| 779 | WOZ: Fishies doesn't boot reliably | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Fishies (WOZ): deve bootar e passar da tela de introdução | apple2-storage/tests/boot.rs |
| 831 | disk2 loads wrong sector after disk switch in "authentic speed" | disco | `apple2-storage/src/disk2.rs` | Trocar o disco 2 e ler T0/S0 em velocidade autêntica: setor correto | apple2-storage/tests/disk2.rs: troca de disco em authentic speed |
| 864 | Anti-M 1.8 doesn't work | disco | `apple2-storage/src/disk2.rs` | Anti-M 1.8: alternar para o drive 2 e prosseguir | apple2-storage/tests/boot.rs |
| 921 | Gruds in Space - .WOZ Boot Error | disco | `apple2-storage/src/woz.rs` | Gruds in Space (WOZ): sem erro de boot | apple2-storage/tests/boot.rs |
| 925 | Computer Foosball - No Boot with any .woz | disco | `apple2-storage/src/woz.rs` | Computer Foosball (WOZ): boot no //e enhanced | apple2-storage/tests/boot.rs |
| 926 | Zork Zero - .woz issues with starting a game | disco | `apple2-storage/src/woz.rs` | Zork Zero (WOZ): inserir lado 3 sem 'Having problems reading side #3' | apple2-storage/tests/boot.rs |
| 928 | Hi-Res Cribbage - Crashes with .woz | disco | `apple2-storage/src/woz.rs` | Hi-Res Cribbage (WOZ): sem crash ao ir ao menu principal (//e enhanced) | apple2-storage/tests/boot.rs |
| 930 | WOZ support: add some random jitter in the bit stream | disco | `apple2-storage/src/woz.rs` | WOZ: adicionar jitter aleatório ao fluxo de bits para imagens com gap de 35 us entre acessos a $C0EC (Wasteland, Legacy of the Ancients) | apple2-storage/tests/disk2.rs: leitura de bits com jitter reprodutível (semente fixa) |
| 938 | Buzzard Bait - BOOT ERROR with .woz image | disco | `apple2-storage/src/woz.rs` | Buzzard Bait (WOZ): sem BOOT ERROR no //e, enhanced e II+ | apple2-storage/tests/boot.rs |
| 1001 | WOZ: Autoduel mostly fails to boot | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Autoduel (WOZ): bootar de forma confiável | apple2-storage/tests/boot.rs |
| 1005 | WOZ: Prisoner 2 fails to boot | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Prisoner 2 (WOZ): bootar sem o programa detectar imagem não original | apple2-storage/tests/boot.rs |
| 1020 | WOZ: What's My Logic | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | What's My Logic (WOZ): bootar | apple2-storage/tests/boot.rs |
| 1021 | WOZ: Glutton | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Glutton (WOZ): pequena variação de temporização para bootar | apple2-storage/tests/boot.rs |
| 1022 | WOZ: Balance of Power | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Balance of Power 1986 (WOZ): bootar | apple2-storage/tests/boot.rs |
| 1023 | WOZ: The Toy Shop | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | The Toy Shop (WOZ): discos de dados com proteção | apple2-storage/tests/boot.rs |
| 1078 | WOZ: Run For It fails after selecting K/J | disco | `apple2-storage/src/woz.rs`, `apple2-storage/src/disk2.rs` | Run For It (WOZ): K/J não pode levar a BRK | apple2-storage/tests/boot.rs |
| 1110 | Possible bug in .WOZ & Quarter Tracks | disco | `apple2-storage/src/disk2.rs`, `apple2-storage/src/woz.rs` | WOZ com quarter tracks: sequência de phases deve manter a cabeça entre X e Y | apple2-storage/tests/disk2.rs: movimento de cabeça por quarto de trilha |
| 1125 | WOZ: Accolade Comics - hangs after intro sequence | disco | `apple2-storage/src/woz.rs` | Accolade Comics (WOZ): sem travar após a introdução (oscila entre T8 e T33) | apple2-storage/tests/boot.rs |
| 1139 | WOZ: The Newsroom - doesn't boot | disco | `apple2-storage/src/woz.rs` | The Newsroom (WOZ): bootar (funcionava até 1.29.8.0) | apple2-storage/tests/boot.rs |
| 1278 | WOZ: Phantasie fails to boot | disco | `apple2-storage/src/woz.rs` | Phantasie (WOZ): bootar | apple2-storage/tests/boot.rs |
| 1315 | WOZ: Infocom Shogun fails to boot | disco | `apple2-storage/src/woz.rs` | Infocom Shogun (WOZ): sem erro de disco no título | apple2-storage/tests/boot.rs |

## HDD / SmartPort (4)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 319 | smartport return address wrong when crossing page | hdd | `apple2-storage/src/hdd.rs` | SmartPort: `JSR $C70D` em $08FD deve retornar a $0903 (falta ADC #0 no endereço de retorno) | apple2-storage/tests/hdd.rs: retorno de SmartPort cruzando página |
| 377 | Diskmaker 8 garbles its text screen | hdd | `apple2-storage/src/hdd.rs` | Diskmaker 8 grava imagem em .po no slot 7: a tela de texto não pode ficar embaralhada | apple2-storage/tests/hdd.rs: DMA de bloco sem corromper a tela |
| 1007 | HDD: Fail if r/w access touches $Cnnn I/O space; and fix HDD write wrapping at 64KiB boundary | hdd | `apple2-storage/src/hdd.rs` | Leitura/escrita de bloco tocando $Cnnn ou ROM deve falhar; escrita deve dar wrap em 64 KiB | apple2-storage/tests/hdd.rs: DMA em I/O/ROM e wrap |
| 1165 | Hard drive speed too fast? | hdd | `apple2-storage/src/hdd.rs` | Medir tempo de leitura de bloco no HDD frente ao AppleWin (carga mais rápida que o CFFA3000) | apple2-storage/tests/hdd.rs: temporização do HDD |

## Save-state (12)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 564 | Save State on Exit not called on restart **[P0]** | savestate | `applewin/src/emu.rs` | Salvar-ao-sair ativo, mudar config de hardware (reinicia o emulador) e sair: o estado novo deve ser salvo | applewin/tests/state.rs: ciclo restart + save on exit |
| 1262 | loading save-state in A2osX actives wrong stack page **[P0]** | savestate | `apple2-savestate/src/apple2.rs` | //e enhanced: salvar com a página de pilha preenchida (A2osX) e restaurar: pilha deve permanecer | apple2-savestate/tests/roundtrip.rs: página de pilha |
| 255 | CyclesThisFrame not saved to save-state file | savestate | `apple2-savestate/src/apple2.rs`, `apple2-core/src/machine/state.rs` | Salvar estado no meio do frame (créditos do Ansi Story) e restaurar: CyclesThisFrame deve voltar igual | apple2-savestate/tests/roundtrip.rs: ciclos do frame |
| 381 | save state inside mouse code can cause hang | savestate | `apple2-savestate/src/cards/mouse.rs`, `apple2-cards/src/mouse.rs` | Mouse: salvar com banco alternativo do firmware ativo e restaurar; o banco deve persistir | apple2-savestate/tests/cards_peripherals.rs: banco do mouse |
| 413 | Save-state: Bug when saving more than 9x RAMWorksIII 64K banks | savestate | `apple2-savestate/src/apple2.rs` | RamWorks com 0x7F bancos: gravar e reler o YAML ('Num Aux Banks: 7F' lido como 0x7F) | apple2-savestate/tests/units.rs: hex com prefixo |
| 486 | Save then restore state with IN#2 in Lisa Assembler v2.5D | savestate | `apple2-savestate/src/apple2.rs`, `apple2-savestate/src/cards/uthernet2.rs` | Lisa 2.5D com Uthernet II (TCP), `IN#2`, salvar e restaurar: não pode cair no Monitor | apple2-savestate/tests/cards_hostio.rs |
| 591 | Save-state: LDA $C000 not correct after re-loading state | savestate | `apple2-savestate/src/apple2.rs` | Salvar/restaurar com BP em `LDA $C000`: o valor lido após restaurar deve ser o original | apple2-savestate/tests/roundtrip.rs: latch do teclado |
| 677 | load state discards pending interrupt | savestate | `apple2-savestate/src/apple2.rs`, `apple2-core/src/irq.rs` | Salvar com IRQ pendente antes de PLP e restaurar: a IRQ deve disparar após o PLP | apple2-savestate/tests/roundtrip.rs: IRQ pendente |
| 838 | Loading Save State = Parser Error (invalid trailing UTF-8 octet) | savestate | `apple2-savestate/src/yaml.rs` | Carregar YAML com UTF-8 inválido/BOM: erro tratado, sem panic | apple2-savestate/tests/units.rs: parser YAML robusto |
| 1372 | SSI263 and SC01 continuous phoneme playback don't restore from save-state | savestate | `apple2-savestate/src/cards/mockingboard.rs`, `apple2-audio/src/ssi263.rs` | MB nos slots 4 e 5 com fonemas contínuos SSI263/SC01 ativos; salvar e restaurar: a fala continua | apple2-savestate/tests/cards_audio.rs |
| 1446 | Disk Drive Will Not Respond | savestate | `apple2-savestate/src/cards/disk2.rs`, `apple2-savestate/src/lib.rs` | Carregar estado com Disk II ('unexpected sub map name'): o drive deve continuar respondendo | apple2-savestate/tests/disks.rs |
| 1499 | AppleWin Savestating issues with Sylvan Idyll | savestate | `apple2-savestate/src/yaml.rs`, `apple2-savestate/src/lib.rs` | Carregar o .aws.yaml do Sylvan Idyll (anexo da issue): carrega sem erro | apple2-savestate/tests/applewin_test_corpus.rs |

## Depurador (7)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 64 | Crash from debugger for cmd: source asdf.txt **[P0]** | debugger | `apple2-debugger/src/commands.rs` | Debugger: `source asdf.txt` com arquivo existente, sem crash | apple2-debugger/tests/commands.rs: comando source |
| 109 | Debugger RUN cmd doesn't work & crashes AppleWin **[P0]** | debugger | `apple2-debugger/src/commands.rs` | Debugger: `run name.txt` com o script no diretório corrente | apple2-debugger/tests/commands.rs: comando run |
| 226 | (Debugger)[1.25] Bad arguments to SAVE crash in ConfigSave_PrepareHeader() **[P0]** | debugger | `apple2-debugger/src/cmd_mem.rs` | Debugger: `SA D000:FFFF "OVERFLOW"` sem estouro de buffer | apple2-debugger/tests/commands.rs: SAVE com argumentos inválidos |
| 349 | [Debugger] NTSC LOAD or SAVE without filename crash **[P0]** | debugger | `apple2-debugger/src/cmd_misc.rs` | Debugger: `NTSC LOAD` e `NTSC SAVE` sem nome de arquivo: erro tratado | apple2-debugger/tests/commands.rs |
| 622 | Crash when entering debugger from logo screen **[P0]** | debugger | `apple2-debugger/src/disasm.rs` | Desmontar instrução cujo operando está em $FFFF (leitura de 16 bits cruzando $FFFF/$0000) | apple2-debugger/tests/get_targets.rs |
| 1204 | Crash due to debugger command line length **[P0]** | debugger | `apple2-debugger/src/console.rs` | Digitar comando com 79+ colunas e dar backspace: sem corrupção do console | apple2-debugger/tests/commands.rs: edição de linha longa |
| 111 | Debugger RUN <script-file> still not 100% | debugger | `apple2-debugger/src/commands.rs` | Debugger: `run <script>` com várias linhas/caminho relativo ainda se comporta de forma estranha | apple2-debugger/tests/commands.rs |

## Outros (vídeo, áudio, periféricos, entrada, configuração) (119)

### video (52)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 6 | Problem with the game Karateka and new video mode | video | `apple2-video/src/renderer.rs` | Karateka: desempenho e vídeo em tela cheia versus janela | apple2-video/tests/modes.rs + perf |
| 17 | [1.12.8.0] Reset produces garbage on screen | video | `apple2-core/src/machine.rs` | Reset pelo logo da Apple não pode deixar lixo na tela | apple2-core/tests/boot.rs: framebuffer após Reset |
| 21 | FLASHing in 80-col mode | video | `apple2-core/src/text_screen.rs` | Modo 80 colunas: caracteres FLASH | apple2-video/tests/modes.rs: FLASH em 80 col |
| 37 | Letter traces on screen with 80 columns | video | `apple2-video/src/screen_text.rs` | 80 colunas: texto não deixa rastros de letras (ProDOS Utilities) | apple2-video/tests/modes.rs |
| 82 | Double-Lo-Res Graphics colours not correct | video | `apple2-video/src/renderer.rs` | Double Lo-Res: `PR#3` e POKEs de AN3/80COL; cores devem bater com a referência | apple2-video/tests/modes.rs: DLORES |
| 128 | Monochrome: half-dot-shift support | video | `apple2-video/src/renderer.rs` | Modo monocromático: suporte a half-dot-shift | apple2-video/tests/modes.rs |
| 157 | Incorporate better NTSC emulation | video | `apple2-video/src/chroma.rs` | Melhor emulação NTSC (cores/bordas) | apple2-video/tests/composite.rs |
| 165 | Incorrect Rendering of Color HGR Screen and Capture to BMP  | video | `apple2-video/src/renderer.rs` | HGR colorido e captura BMP renderizados incorretamente | apple2-video/tests/modes.rs + applewin/tests (screenshot) |
| 241 | Emulator's floating bus is ahead by 2 cycles | video | `apple2-core/src/video_scanner.rs` | Floating bus adiantado em 2 ciclos (créditos do Ansi Story) | apple2-core/tests/video_scanner.rs: posição do scanner x floating bus |
| 253 | (Graphics) GR COLORs are not accurate | video | `apple2-video/src/chroma.rs`, `apple2-video/src/renderer.rs` | GR COLOR: programa de 16 cores comparado com hardware real | apple2-video/tests/modes.rs: paleta LORES |
| 259 | Corrupt video rendering in NTSC_PreMerge | video | `apple2-video/src/renderer.rs`, `apple2-video/src/chroma.rs` | Texto/gráficos corrompidos no NTSC_PreMerge | apple2-video/tests/composite.rs |
| 341 | NTSC (color and B&W) appear misaligned relative to the other video modes | video | `apple2-video/src/renderer.rs`, `apple2-video/src/config.rs` | NTSC (cor e P&B) desalinhado em relação aos outros modos | apple2-video/tests/composite.rs |
| 344 | Color TV (NTSC) video mode not doing vertical color bleeding | video | `apple2-video/src/chroma.rs` | Color TV (NTSC): ausência de color bleeding vertical (regressão) | apple2-video/tests/composite.rs |
| 352 | Full-speed in mixed mode results in flickery video | video | `applewin/src/pacing.rs`, `apple2-video/src/source.rs` | Full-speed em modo misto: vídeo piscando durante BSAVE | applewin/tests/headless.rs + apple2-video: full-speed misto |
| 363 | no screen updating in 1.26.0.6 + debugger | video | `apple2-video/src/renderer.rs`, `applewin/src/debug_view.rs` | Depurador: `g 801` com Color Monitor: a tela não atualiza | apple2-debugger/tests/machine.rs + apple2-video |
| 371 | Flickery video when using GR from CP/M | video | `apple2-video/src/renderer.rs`, `apple2-core/src/video_scanner.rs` | GR sob CP/M (MBASIC) gera vídeo instável | apple2-cards/tests/softcard.rs + apple2-video |
| 372 | IRQ cycles are not in sync with NTSC vert/horz position | video | `apple2-core/src/irq.rs`, `apple2-core/src/video_scanner.rs` | IRQ fora de sincronia com a posição vertical/horizontal NTSC | apple2-core/tests/video_scanner.rs |
| 382 | Reset not resetting the 40/80-col text mode for mixed-mode rendering | video | `apple2-core/src/video_mode.rs`, `apple2-core/src/text_screen.rs` | Reset não restaura 40/80 col no modo misto | apple2-core/tests/boot.rs: Reset após PR#3 |
| 397 | debugger 2.9.0.2: graphics mode not maintained | video | `apple2-video/src/renderer.rs`, `applewin/src/debug_view.rs` | Depurador: voltar ao HGR página 2 mostra texto página 2 | apple2-debugger/tests/machine.rs: modo de vídeo preservado |
| 405 | Screen flicker in full-speed mode | video | `apple2-video/src/renderer.rs`, `applewin/src/emu.rs` | Full-speed: tela piscando | applewin/tests/headless.rs |
| 409 | AppleWorks 5.1 flickers during boot | video | `apple2-video/src/renderer.rs`, `apple2-storage/src/hdd.rs` | AppleWorks 5.1 (HDD) pisca no boot | apple2-video/tests/machine.rs |
| 502 | TV Emulation doesn't have accurate colors | video | `apple2-video/src/chroma.rs` | Cores do TV Emulation conferidas com o programa HGR de 8 cores | apple2-video/tests/composite.rs |
| 516 | Apple Win problem with Double Lores and Aux-Move | video | `apple2-core/src/video_mode.rs` | Double Lores + aux move: não troca página 1/2 do DLORES | apple2-core/tests/paging.rs + apple2-video |
| 555 | One pixel shifted to the right? | video | `apple2-video/src/renderer.rs` | Título do Prince of Persia (DHGR): pixel deslocado à direita | apple2-video/tests/modes.rs |
| 627 | Rainbow demo not working correctly when vertical blend is enabled | video | `apple2-video/src/renderer.rs` | Rainbow com vertical blend: renderização correta | apple2-video/tests/composite.rs |
| 631 | 560x192 B&W apps not always start in B&W mode | video | `apple2-core/src/video_mode.rs`, `apple2-video/src/renderer.rs` | 560x192 P&B: início em modo P&B | apple2-video/tests/modes.rs |
| 633 | DHGR in 1.28.x.0 | video | `apple2-video/src/rgb.rs`, `apple2-video/src/renderer.rs` | DHGR no modo RGB desde 1.28.2.0 | apple2-video/tests/rgb_card.rs |
| 636 | Support Enhanced //e soft switches | video | `apple2-core/src/machine/io.rs` | Soft switches do //e enhanced (IOUDIS/DHIRES em $C07E/$C07F) | apple2-core/tests/paging.rs + apple2-video/tests/modes.rs |
| 642 | Improved DHGR MIX mode detection for Dragon Wars | video | `apple2-core/src/video_mode.rs`, `apple2-video/src/renderer.rs` | Dragon Wars: detecção de DHGR MIX | apple2-video/tests/rgb_card.rs |
| 644 | DHGR 140-color mode: Improve rendering | video | `apple2-video/src/renderer.rs` | DHGR 140 cores: pixel com 4x a largura | apple2-video/tests/rgb_card.rs |
| 648 | Support 50Hz/PAL mode | video | `apple2-core/src/video_standard.rs` | Modo 50 Hz/PAL | apple2-core/tests/video_scanner.rs |
| 650 | One pixel shifted to the bottom in NTSC mode | video | `apple2-video/src/renderer.rs` | NTSC: linha de pixel cinza no topo/fina embaixo (deslocamento de 1 pixel) | apple2-video/tests/composite.rs |
| 654 | Sather's "Little Text Window" doesn't render correctly | video | `apple2-core/src/video_mode.rs`, `apple2-core/src/video_scanner.rs` | 'Little Text Window' do Sather: troca de modo atrasada 1 ciclo | apple2-core/tests/video_scanner.rs |
| 656 | PAGE1/2 select occurring 1 cycle early | video | `apple2-core/src/video_mode.rs` | PAGE1/2 ($C054/55) acontece 1 ciclo antes | apple2-core/tests/video_scanner.rs |
| 670 | AppleWin 1.29.0.0 page 2 display | video | `apple2-core/src/video_mode.rs` | Spy vs Spy: troca de página 2 deve persistir | apple2-core/tests/paging.rs |
| 680 | "Scroll Scroll Scroll" demo has tearing during the display of the big scroll | video | `applewin/src/pacing.rs`, `applewin/src/video.rs` | Scroll Scroll Scroll: sem tearing | applewin/tests/headless.rs: sincronia com VBL |
| 711 | "MAD EFFECT #2" demo doesn't work correctly | video | `applewin/src/pacing.rs`, `applewin/src/video.rs` | MAD EFFECT 2: tearing e outros defeitos | applewin/tests/headless.rs |
| 722 | Jerky scrolling for demos effects synchronised to the VBlank | video | `applewin/src/pacing.rs`, `applewin/src/video.rs` | Rolagem entrecortada em efeitos sincronizados ao VBL (Ansi Story) | applewin/tests/headless.rs |
| 763 | PAL Color Monitor support | video | `apple2-video/src/chroma.rs`, `apple2-core/src/video_standard.rs` | PAL Color Monitor: artefatos em HIRES+TEXT | apple2-video/tests/composite.rs |
| 828 | RGB Monitor mode: HIRES+AN3 bug | video | `apple2-video/src/rgb.rs` | RGB: bug de HIRES+AN3 (Digidream) | apple2-video/tests/rgb_card.rs |
| 850 | RGB card decoder gets into weird states | video | `apple2-video/src/rgb.rs` | Decodificador do cartão RGB (SL7) em estados estranhos (Video-7 demo) | apple2-video/tests/rgb_card.rs |
| 851 | Default AppleColor RGB card is not working correctly. | video | `apple2-video/src/rgb.rs` | Cartão AppleColor padrão (`-rgb-card-type apple`) funcionando | apple2-video/tests/rgb_card.rs |
| 1018 | Strange RGB Monitor Memory Corruption | video | `apple2-video/src/renderer.rs` | Cartão RGB: texto multicolorido (AN3 desligado/DHIRES) e AN3 corrigido | apple2-video/tests/rgb_card.rs |
| 1106 | VidHD: minor screen corruption & debugger hit-test issues | video | `apple2-video/src/renderer.rs`, `apple2-cards/src/vidhd.rs` | VidHD: meia coluna de pixel e hit-test do depurador | apple2-video/tests/vidhd.rs |
| 1119 | Video mode: Color (PAL Monitor) rendering line-0 2 pixels to the right | video | `apple2-video/src/renderer.rs`, `apple2-video/src/chroma.rs` | PAL Color Monitor: linha 0 deslocada 2 pixels | apple2-video/tests/composite.rs |
| 1131 | Video mode: Color (PAL Monitor) rendering bottom TEXT lines for Rainbow | video | `apple2-video/src/renderer.rs` | PAL Color Monitor: linhas de TEXT do Rainbow cintilando | apple2-video/tests/composite.rs |
| 1143 | Graphical glitches in A2D in RGB and Idealized Composite modes | video | `apple2-video/src/renderer.rs`, `apple2-video/src/chroma.rs` | Apple II Desktop (A2D): glitches em RGB e Composite idealizado | apple2-video/tests/composite.rs + rgb_card.rs |
| 1166 | Various bugs in NTSC.cpp leading to incorrect hues | video | `apple2-video/src/chroma.rs` | NTSC: matiz do lores color 1 muito arroxeado | apple2-video/tests/composite.rs |
| 1386 | Incorrect cycle delay when performing TXTCLR / TXTSET with lda ($02,x)" where x=0, ($02)=$c050 | video | `apple2-cpu/src/lib.rs`, `apple2-core/src/video_scanner.rs` | TXTCLR/TXTSET com `lda ($02,x)` (x=0): atraso de ciclo correto | apple2-core/tests/video_scanner.rs |
| 1435 | RGB Monitor - Default Mode Incorrect for Video7 card? | video | `apple2-video/src/rgb.rs` | Cartão Video7: modo padrão do RGB conforme hardware | apple2-video/tests/rgb_card.rs |
| 1443 | PAL Color TV video mode should kill color-burst for longer | video | `apple2-video/src/renderer.rs` | PAL: color-burst desligado por mais tempo (HGR↔TEXT por linha) | apple2-video/tests/composite.rs |
| 1480 | Issues with Vidmodes_260326.po | video | `apple2-video/src/renderer.rs`, `apple2-core/src/video_mode.rs` | Vidmodes_260326.po: 80COL 7 pixels antes e linhas de TEXT | apple2-video/tests/modes.rs |

### mockingboard (28)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 72 | Enabling CP/M card disables Phasor | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-cards/src/z80.rs` | CP/M no slot 5 + Phasor no slot 4: a detecção do Phasor não pode falhar | apple2-cards/tests/softcard.rs + apple2-audio: cartões em slots vizinhos |
| 192 | Mockingboard L/R channels wrong? | mockingboard | `apple2-audio/src/mockingboard.rs` | Mockingboard: canais L/R conforme hardware | apple2-audio/tests/machine.rs: pan por chip |
| 309 | Silent Service hangs during Mockingboard use | mockingboard | `apple2-audio/src/via6522.rs`, `apple2-audio/src/mockingboard.rs` | Silent Service: o jogo espera interrupção do Mockingboard que nunca ocorre | apple2-audio/tests/machine.rs: IRQ do 6522 |
| 320 | Mockingboard emulation should support 'inactive' function like real h/w | mockingboard | `apple2-audio/src/ay8910.rs`, `apple2-audio/src/mockingboard.rs` | Mockingboard: função 'inactive' do AY como no hardware | apple2-audio/tests/machine.rs |
| 496 | MockingBoard 6522 timer | mockingboard | `apple2-audio/src/via6522.rs` | 6522: o Timer1 deve iniciar mesmo com interrupções desabilitadas (polling de IFR) | apple2-audio/tests/machine.rs |
| 567 | Mockingboard: Reading the 6522's IER always returns 0x80 | mockingboard | `apple2-audio/src/via6522.rs` | 6522: IER deve refletir bits habilitados (não sempre $80) | apple2-audio/tests/machine.rs |
| 608 | Broadsides Does not Detect Mockingboard without Workaround | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-audio/src/via6522.rs` | Broadsides: detecção do Mockingboard sem contorno | apple2-audio/tests/machine.rs: detecção por contador do 6522 |
| 659 | Phasor noisy playback | mockingboard | `apple2-audio/src/mockingboard.rs` | Phasor Music Player (Toccata e Fuga) com ruído | apple2-audio/tests/machine.rs |
| 685 | Mockingboard: music not working when using Timer1 from both 6522's | mockingboard | `apple2-audio/src/via6522.rs`, `apple2-audio/src/mockingboard.rs` | Mockingboard: Timer1 dos dois 6522 em modo contínuo | apple2-audio/tests/machine.rs |
| 698 | Rescue Raiders hangs with Phasor enabled | mockingboard | `apple2-audio/src/mockingboard.rs` | Rescue Raiders com Phasor: não pode travar | apple2-audio/tests/machine.rs |
| 699 | "MAD EFFECT" demo doesn't work correctly | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-core/src/video_standard.rs` | MAD EFFECT: Mockingboard em PAL 50 Hz | apple2-audio/tests/machine.rs + apple2-core/tests/video_scanner.rs |
| 753 | Bejeweled issue with Phasor enabled | mockingboard | `apple2-audio/src/mockingboard.rs` | Bejeweled com Phasor: interrupções param após o preenchimento | apple2-audio/tests/machine.rs |
| 765 | Lancaster (Total Replay) hangs when starting second game | mockingboard | `apple2-audio/src/mockingboard.rs` | Lancaster (Total Replay) com MB nos slots 4 e 5: segundo jogo dispara interrupções repetidas | apple2-audio/tests/machine.rs |
| 813 | Mockingboard playback loop after entering debugger then config | mockingboard | `apple2-audio/src/mockingboard.rs` | Mockingboard: depurador→config não pode repetir reprodução | apple2-audio/tests/machine.rs |
| 929 | Mockingboard Classic Adventure - speech not working correctly | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-audio/src/ssi263.rs` | Mockingboard Classic Adventure (WOZ): fala correta | apple2-audio/tests/machine.rs |
| 952 | Fix SC-01 issue with Ape Escape | mockingboard | `apple2-audio/src/mockingboard.rs` | Ape Escape: detecção de MB e SC-01 (poll de $C40D) | apple2-audio/tests/machine.rs |
| 958 | Mockingboard speech interrupt occasionally delivered late | mockingboard | `apple2-audio/src/via6522.rs`, `apple2-audio/src/mockingboard.rs` | Fala do Mockingboard: interrupção entregue tarde com `gg` | apple2-audio/tests/machine.rs |
| 963 | mb-audit failure for Linux version | mockingboard | `apple2-audio/src/mockingboard.rs` | mb-audit: falha por execução em lotes grandes de CPU | apple2-audio/tests/machine.rs |
| 1104 | Mockingboard speech interrupt occasionally missing in debugger gg mode | mockingboard | `apple2-audio/src/ssi263.rs`, `apple2-audio/src/mockingboard.rs` | MB: SSI263 não detectado/travado em `gg` | apple2-audio/tests/machine.rs |
| 1105 | Mockingboard speech: repeat phoneme for Berzap! when selecting MB-C card | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-audio/src/ssi263.rs` | Berzap! (TR) ao escolher MB C: fonema repetido | apple2-audio/tests/machine.rs |
| 1173 | MB card's 6522 IRQ can be deasserted by another MB card | mockingboard | `apple2-audio/src/via6522.rs` | MB: IRQ de um 6522 não pode ser desligada por outro cartão MB | apple2-audio/tests/machine.rs |
| 1175 | [MB:6522] IRQ not firing after a T1 underflow when T1 latch is very small | mockingboard | `apple2-audio/src/via6522.rs` | MB: IRQ após underflow de T1 com latch pequeno | apple2-audio/tests/machine.rs |
| 1176 | [MB:6522] Can falsely set flag to indicate IRQ asserted on last cycle of opcode | mockingboard | `apple2-core/src/irq.rs`, `apple2-audio/src/via6522.rs` | MB: flag de IRQ falsamente ativado no último ciclo do opcode | apple2-audio/tests/machine.rs + apple2-core/tests/cpu_events.rs |
| 1193 | MB/Phasor: fix for reading AY regs (via 6522 PortA) after AY set to INACTIVE function | mockingboard | `apple2-audio/src/mockingboard.rs` | MB/Phasor: leitura de registradores do AY após INACTIVE | apple2-audio/tests/machine.rs |
| 1260 | Mockingboard not detected for Werewolf!! | mockingboard | `apple2-audio/src/mockingboard.rs` | Werewolf: detecção do Mockingboard | apple2-audio/tests/machine.rs |
| 1313 | AY-3-891x Noise Frequency Period Register Behavior | mockingboard | `apple2-audio/src/ay8910.rs` | AY-3-891x: período de ruído (Ultima V Hall of Doom) | apple2-audio/tests/machine.rs |
| 1330 | Mockingboard A, C & Phasor: support the AY-3-8913 | mockingboard | `apple2-audio/src/ay8910.rs` | AY-3-8913 em Mockingboard A/C e Phasor | apple2-audio/tests/machine.rs |
| 1358 | MB sound not disabled when switching between normal/full-speed modes | mockingboard | `apple2-audio/src/mockingboard.rs`, `apple2-audio/src/ay8910.rs` | MB: tom constante ao alternar velocidade normal/máxima | apple2-audio/tests/machine.rs |

### audio (11)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 19 | Beep corrupt during Merlin save | audio | `apple2-audio/src/speaker.rs` | Merlin 8 PRODOS: salvar objeto (beep); o som não pode ficar áspero | apple2-audio/tests/machine.rs: ondas do alto-falante |
| 95 | Beep reduced to wavering noise | audio | `apple2-audio/src/speaker.rs` | Beep 'tremido' por falta de CPU/ritmo de áudio | apple2-audio/tests/machine.rs: beep contínuo |
| 100 | Speaker sounds sort of echo | audio | `apple2-audio/src/speaker.rs` | Speaker com eco e leve lentidão | apple2-audio/tests/machine.rs: latência do speaker |
| 240 | Full volume blast | audio | `applewin/src/audio.rs`, `apple2-audio/src/speaker.rs` | Beep de reset com volume cheio ocasional | apple2-audio/tests/machine.rs: amplitude após Reset |
| 456 | Strange speaker clicks while changing configuration | audio | `apple2-audio/src/speaker.rs`, `applewin/src/audio.rs` | Cliques do speaker ao mudar a configuração | apple2-audio/tests/machine.rs |
| 647 | Super loud thump or extreme full volume beep. | audio | `apple2-audio/src/speaker.rs` | Thump alto ou beep em volume máximo | apple2-audio/tests/machine.rs |
| 770 | "DIGIDREAM" plays drum samples too quietly & occasional false note is played | audio | `apple2-audio/src/speaker.rs` | DIGIDREAM: amostras de bateria baixas e nota falsa | apple2-audio/tests/machine.rs: speaker em amostras |
| 833 | Mockingboard / 6522: reading TIMER1H & supporting other addressing modes | audio | `apple2-audio/src/via6522.rs` | 6522: leitura de TIMER1H e outros modos de endereçamento | apple2-audio/tests/machine.rs |
| 1088 | PWM sound pretty different from the real thing. | audio | `apple2-audio/src/speaker.rs` | A2Stream: som PWM bem diferente do hardware | apple2-audio/tests/machine.rs |
| 1159 | No sound under Win 10 | audio | `applewin/src/audio.rs` | Sem som no Windows 10 | applewin/tests (novo): seleção do dispositivo de áudio padrão no backend |
| 1333 | Bank Street Music Writer - playback in jukebox starts with lower volume | audio | `apple2-audio/src/speaker.rs` | Bank Street Music Writer: primeiro compasso mais baixo | apple2-audio/tests/machine.rs |

### teclado (7)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 103 | Keyboard input not registering in Karateka and Bilestoad | teclado | `apple2-core/src/keyboard.rs` | Karateka e Bilestoad: teclado não registra outras teclas | apple2-core/tests/input.rs: teclado com AKD/strobe |
| 116 | Any Key Down works for Apple II and II+ models | teclado | `apple2-core/src/keyboard.rs` | Apple II/II+: Any Key Down ($C010 bit 7) não deve existir | apple2-core/tests/input.rs: AKD por modelo |
| 293 | Where in the USA is Carmen Sandiego | teclado | `apple2-core/src/keyboard.rs`, `applewin/src/keymap.rs` | Where in the USA is Carmen Sandiego: menu cicla como se a tecla de cima estivesse presa | apple2-core/tests/input.rs |
| 330 | Keyboard: Any Key Down (AKD) not working | teclado | `apple2-core/src/keyboard.rs` | Any Key Down não funciona no //e | apple2-core/tests/input.rs |
| 454 | A keypress is lost after entering debugger:  Boot DOS disk, F7, set brkpt, F7, Press a key | teclado | `apple2-core/src/keyboard.rs`, `applewin/src/keymap.rs` | Depurador: tecla perdida após entrar/sair com BP (`CATALOG` digitado) | apple2-debugger/tests/machine.rs: buffer de teclado |
| 1161 | Keyboard: PoP & AKD bug | teclado | `apple2-core/src/keyboard.rs` | PoP no //e: Any Key Down | apple2-core/tests/input.rs |
| 1261 | $C010 should not read the keyboard latch on Apple II and Apple II+ | teclado | `apple2-core/src/keyboard.rs`, `apple2-core/src/machine/io.rs` | Apple II/II+: leitura de $C010 não deve ler o latch do teclado | apple2-core/tests/input.rs |

### joystick (4)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 429 | Joystick: cannot reach corners | joystick | `apple2-core/src/game_port.rs`, `applewin/src/joy.rs` | Joystick: alcançar os cantos (TopRight/TopLeft) | apple2-core/tests/input.rs: game port (cantos) |
| 778 | Joystick detection not working | joystick | `apple2-core/src/game_port.rs` | Prince of Persia com joystick desabilitado: leitura de paddle >=622 deve indicar ausência | apple2-core/tests/input.rs |
| 985 | fixed one-shot paddle timers | joystick | `apple2-core/src/game_port.rs` | Paddles: temporizadores one-shot reiniciam ao strobe | apple2-core/tests/input.rs |
| 1128 | Learning with Leeper: Joystick will not go Right or Down | joystick | `apple2-core/src/game_port.rs`, `applewin/src/joy.rs` | Learning with Leeper: joystick não vai à direita nem para baixo | apple2-core/tests/input.rs |

### ssi263 (4)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 777 | Phasor speech not working for Mockingboard titles | ssi263 | `apple2-audio/src/ssi263.rs`, `apple2-audio/src/mockingboard.rs` | Phasor SSI263: fala em títulos do Mockingboard C | apple2-audio/tests/machine.rs |
| 1172 | Speech no good when CPU freq isn't 1MHz | ssi263 | `apple2-audio/src/ssi263.rs` | CPU a 1,5/2 MHz: fala SC-01/SSI263 silenciosa/gibberish | apple2-audio/tests/machine.rs |
| 1412 | SSI263: power-down not silencing continuous phoneme | ssi263 | `apple2-audio/src/ssi263.rs` | SSI263: power-down deve silenciar fonema contínuo (`C443:80`) | apple2-audio/tests/machine.rs |
| 1416 | Mockingboard and Phasor cards now support SSI263AP (instead of older SSI263P) | ssi263 | `apple2-audio/src/ssi263.rs`, `apple2-audio/src/mockingboard.rs` | Mockingboard/Phasor usam SSI263AP por padrão; RESET entra em Power Down | apple2-audio/tests/machine.rs |

### outra (3)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 5 | MoonPatrol game bug | outra | `apple2-core/src/machine.rs` | Moon Patrol: jogar até o primeiro checkpoint (ou usar o estado do relator); não pode congelar | apple2-core/tests/boot.rs: fixture |
| 71 | Drol hangs at cut scene | outra | `apple2-core/src/machine.rs` | Drol: travamento na cena de corte | apple2-core/tests/boot.rs: fixture |
| 143 | Pravetz: Unable to start any ProDOS version | outra | `apple2-core/src/model.rs` | Pravetz 82/8M/8A: bootar qualquer ProDOS | apple2-core/tests/boot.rs: modelos Pravetz |

### relogio (3)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 115 | DDial timer running very fast | relogio | `apple2-cards/src/time.rs` | DDial: timer rodando muito rápido | apple2-cards/tests/clocks.rs |
| 127 | DDial Timer freezes in debug trace mode | relogio | `apple2-cards/src/time.rs` | DDial: o relógio congela ao abrir arquivo de trace no depurador | apple2-cards/tests/clocks.rs + apple2-debugger |
| 751 | No-Slot Clock running and power-cycle can causes lock-up on power-cycle | relogio | `apple2-cards/src/nsc.rs`, `apple2-cards/src/time.rs` | NSC + power-cycle (NoSlotClockTest): não pode travar 50% | apple2-cards/tests/clocks.rs |

### serial (2)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 70 | 1.14.2 crashes with USCD Pascal **[P0]** | serial | `apple2-hostio/src/ssc.rs`, `apple2-core/src/machine.rs` | USCD Pascal 1.2 com SSC e porta serial desativada: sem crash | apple2-hostio/tests/ssc.rs: SSC sem backend |
| 707 | SSC: Applewin + ADTpro | serial | `apple2-hostio/src/ssc.rs`, `apple2-hostio/src/serial.rs` | SSC + ADTpro via porta virtual: transferência/leitura de diretório | apple2-hostio/tests/ssc.rs |

### config (2)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 1491 | Odd behaviour of inserting disks in empty slots **[P0]** | config | `applewin/src/disk_ui.rs`, `apple2-config/src/cmdline.rs` | `-s6 empty -d1 "demo.dsk"` não pode gerar bad_cast; `-s7 empty -h1 ...` insere cartão e disco | applewin/tests/cli.rs: cmdline com slot vazio |
| 434 | Choosing Thumbstick #2 causes crash in older AppleWin's | config | `apple2-config/src/registry_map.rs`, `apple2-config/src/settings.rs` | Escolher Thumbstick #2: persistência compatível com versões antigas | apple2-config/tests: registro do Joystick1 Emu Type |

### mouse (2)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 232 | Mouse card still generating VBl after card swapped out for Mockingboard | mouse | `apple2-cards/src/mouse.rs`, `apple2-core/src/slots.rs` | Trocar mouse por Mockingboard no slot 4: o mouse não pode continuar gerando VBL | apple2-cards/tests/mouse.rs: remoção do cartão |
| 495 | Mouse crash with AppleWin1.26.3.1 | mouse | `apple2-cards/src/mouse.rs` | Mouse: sair de programa (DazzleDraw) não pode cair no Monitor em CF38 | apple2-cards/tests/mouse.rs |

### z80 (1)

| upstream # | título | categoria | caminho Rust | como reproduzir no port | teste proposto |
|---:|---|---|---|---|---|
| 498 | z80 + mouse cause screen corruption in CPM | z80 | `apple2-cards/src/z80.rs` | Z80 + mouse: tela corrompida no CP/M | apple2-cards/tests/softcard.rs |

