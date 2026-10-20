# Introdução

O AppleII-Rust emula a família Apple II com o objetivo de ter o mesmo comportamento do AppleWin, e o mesmo comportamento nos três sistemas operacionais. O núcleo da Máquina é determinístico: não depende de janela, de áudio nem do relógio do computador, e avança Ciclo a Ciclo. A janela, o som, o joystick e os arquivos ficam em uma camada separada (o frontend). Por isso um disco que dá boot em uma plataforma dá boot nas outras, no mesmo número de Ciclos.

## O que é emulado

**Modelos**

- Apple ][, Apple ][+ e Apple ][ J-Plus.
- Apple //e (original) e Apple //e Enhanced (com 65C02).
- Clones: Pravets 82, Pravets 8M, Pravets 8A, Base64A e TK3000 //e.

O Apple IIc existe apenas como esqueleto de porta, sem ROM, igual ao AppleWin.

**Vídeo.** Texto de 40 e 80 colunas, lo-res, hi-res, double lo-res e double hi-res, em vários tipos de monitor: monocromático (verde, âmbar, branco ou cor própria), "Color (Composite Idealized)", "Color (RGB Card/Monitor)", "Color (Composite Monitor)", TV colorida e TV preto e branco. Há ainda linhas de varredura a 50%, mistura vertical e o padrão PAL de 50 Hz.

**Memória.** Language Card de 16K (slot 0), Saturn 128K, as quatro cartas AUX do //e (vazia, 80 colunas 1K, Extended 80 colunas 64K e RamWorks III) e o Floating bus.

**Cartas e periféricos.** Disk II, controlador de disco rígido, Super Serial Card, impressora paralela, mouse, Mockingboard C, Phasor, SAM, Z80 SoftCard (CP/M), relógios, Uthernet I e II, VidHD, 4Play, SNES MAX, Breakpoint card e dongles de proteção contra cópia. Veja "Cartas por slot".

**Ferramentas.** Depurador, save-states, captura de tela e colagem de texto da área de transferência.

## Como o manual está organizado

Comece por "Primeiros passos". Quem já conhece o AppleWin pode ir direto a "Teclado e atalhos" (as teclas são as mesmas) e a "Diferenças em relação ao AppleWin".

## Licença

O AppleII-Rust é software livre, distribuído sob a **GPL-2.0-or-later**. É uma obra derivada do AppleWin e depende do trabalho de seus autores; veja "Créditos e licença".
