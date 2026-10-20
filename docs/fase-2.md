# Fase 2 — núcleo da Máquina (`apple2-core`)

Spec acordado com o usuário em 2026-10-19. Vocabulário: [GLOSSARY.md](../GLOSSARY.md). Decisões de arquitetura: [ADR 0001](adr/0001-nucleo-headless-deterministico.md) e [ADR 0002](adr/0002-roms-embutidas.md).
Referência C++: AppleWin `source/` (Memory.cpp, LanguageCard.cpp, Keyboard.cpp, Speaker.cpp, Joystick.cpp, NTSC.cpp, Video.cpp, SynchronousEventManager.cpp, Card.h, CardManager.cpp, Common.h).

## Decisões

1. **Modelos**: todos os do AppleWin, com os mesmos valores numéricos de `eApple2Type` (Common.h:177-211), incluindo as máscaras APPLE2E=0x10, APPLE2C=0x20 e CLONE=0x100:
   - II (0) e II+ (1), J-Plus (2);
   - IIe (0x10) e IIe Enhanced (0x11);
   - IIc (0x20), só como esqueleto, igual ao AppleWin: sem ROM, apenas IOUDIS, ANx e $C07E/F;
   - Pravets 8M (0x101), Pravets 82 (0x102), Base64A (0x103);
   - BAD_PRAVETS82/8M (0x110/0x111), Pravets 8A (0x112), TK3000 //e (0x113).
   - CPU: 65C02 só no Enhanced, no TK3000 e no IIc. Os demais usam NMOS.
2. **Execução**: o núcleo é headless e determinístico. `Machine` implementa `apple2_cpu::Bus`, o tempo avança por `tick` e a API é "rode N ciclos / até o fim do Quadro". Sem threads, sem relógio do host, sem I/O de arquivo.
3. **Cartas**: trait de Carta já nesta fase. Ela cobre:
   - I/O em $C080+s·16;
   - ROM de slot em $Cs00;
   - ROM de expansão em $C800–$CFFF, liberada por $CFFF;
   - INTCXROM, SLOTC3ROM e INTC8ROM.
   O Slot AUX é separado. A Language Card (slot 0) e a Saturn 128K (qualquer slot) implementam a trait.
4. **Floating bus**: entra agora, com o Contador de vídeo (65 Ciclos/linha, 262 linhas NTSC e 312 PAL). O endereço segue o NTSC.cpp/Video.cpp do AppleWin, incluindo o ajuste de 1 coluna e o HBL +$1000 no ][ em modo texto.
5. **Teclado**: o núcleo recebe Códigos de tecla Apple, o estado "tecla pressionada" (AKD) e Open/Closed Apple. A Fila de colagem também fica no núcleo, consumida como no AppleWin (um código por leitura de $C000 ou por limpeza do strobe). O mapeamento do teclado do host vai para a fase 7. As remapeações de clones (TK3000, Pravets) seguem Keyboard.cpp:131-200 e o $C060 do Pravets 8A.
6. **Alto-falante**: o núcleo registra o Ciclo de cada Toque ($C030–$C03F, leitura ou escrita). As amostras PCM vão para a fase 5.
7. **Porta de jogos**:
   - botões PB0–PB2, com Open/Closed Apple em PB0/PB1 e o ][ sem joystick lendo "pressionado";
   - paddles por temporização (`pos·2816/255` Ciclos, 255 vira 287), disparados por acesso a $C07x;
   - Anunciadores AN0–AN3.
8. **Paginação //e completa**: flags MF_* (Memory.h:8-23), ALTZP, AUXREAD/WRITE, 80STORE com PAGE2/HIRES, ALTROM0/1 do Base64A, e status $C011–$C01F com bit 7 = flag e bits 0–6 = último código de tecla (não é Floating bus). No ][, uma leitura de $C01x limpa o strobe e devolve o Floating bus.
9. **Cartas AUX**: as quatro variantes:
   - vazio (a leitura AUX devolve o Floating bus);
   - 80col 1K, que espelha o AUX em $0400–$07FF;
   - Extended 80col 64K;
   - RamWorks III, até 256 bancos de 64K, selecionados por $C071/$C073.
   Padrões: ][ sem LC; ][+ com LC no slot 0; //e com Extended 80col.
10. **Language Card**: a do //e (embutida), a 16K do slot 0 e a Saturn 128K (bancos e regra de write-enable próprias). Inclui a regra de leitura dupla (UTAIIe 5-23), o GH#700 (RMW abs em $C08x habilita escrita; a CPU precisa expor o opcode atual) e o reset (o ][/][+ só reinicia a LC no power-cycle).
11. **Vídeo (estado)**: TEXT, MIXED, PAGE2, HIRES, 80COL, ALTCHARSET, DHIRES/AN3, VBL em $C019 (bit 7 = 1 fora do VBL). A renderização vai para a fase 3.
12. **Agendador**: o Evento sincronizado é um port do SynchronousEventManager, com a regra "IRQ no último ciclo do opcode adia um opcode". A linha de IRQ/NMI é a união das Fontes de IRQ.
13. **ROMs**: o crate `apple2-roms` usa `include_bytes!` de `roms/`. A ROM de //e tem 16K (4K de $C000–$CFFF interna + 12K de $D000–$FFFF). A do Base64A tem 48K (4 páginas de 12K).
    Extras: ROM customizada (bytes na config), o "Freeze's F8" e a troca do F8 do ][ pelo do ][+ quando há LC. No-Slot-Clock e dongles ficam para a fase 6.
14. **Serialização**: o estado da Máquina e a configuração derivam `serde::{Serialize, Deserialize}`. O YAML do AppleWin vem na fase 7.
15. **Fora do escopo** (fica para outras fases): renderização, disco, fita ($C020 e $C060 leem o Floating bus com bit 7=1), Mockingboard, as demais cartas, frontend e debugger.

## Fatias (cada uma com TDD; reviso e faço commit entre elas)

1. **Esqueleto**: crates `apple2-roms` e `apple2-core`; `Model` com os valores do AppleWin; `MachineConfig` (serde); agendador e Fontes de IRQ; `Machine` com contador de Ciclos, implementando `Bus`; CPU expõe o opcode atual.
2. **Paginação**: main/AUX/ROM, os switches $C000–$C00B e $C054–$C057, o status $C011–$C01F e as quatro Cartas AUX.
3. **Language Card**: //e, slot 0 e Saturn, com leitura dupla, GH#700 e regras de reset.
4. **Cartas e slots**: a trait, o despacho de I/O e as ROMs $Cs00, $C800 e $CFFF com INTCXROM/SLOTC3ROM/INTC8ROM.
5. **Vídeo e Floating bus**: Contador de vídeo NTSC/PAL, endereço varrido, flags de vídeo e $C019.
6. **Entrada e som**: teclado (strobe, AKD, Fila de colagem, clones), Toques do alto-falante, Porta de jogos e as peculiaridades J-Plus, Pravets, TK3000 e Base64A.
7. **Boot**: testes de integração que ligam cada Modelo com a ROM real e chegam ao prompt `]`/`*` na memória de texto. ROM customizada e F8.
