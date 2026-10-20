# Cartas por slot

O Apple II tem sete Slots de expansão (1 a 7), o Slot 0 do ][ e do ][+ (para a Language Card) e, no //e, o Slot AUX. Cada Slot recebe uma Carta. Na linha de comando, `-sN nome` coloca uma Carta no slot N; na configuração, a aba de slots faz o mesmo.

## Onde cada Carta pode ficar

**Slot 0** (apenas ][ e ][+): Language Card (16K) ou Saturn 128K. Opções: `-s0 languagecard` (ou `lc`), `saturn` (128K), `saturn64` e `empty`.

**Slots 1 a 7:**

| Carta | Nome em `-sN` | Instâncias |
|---|---|---|
| Disk II | `diskii`, `diskii13` | várias |
| Controlador de disco rígido | `hdc`, `hdc-sp`, `hdc-bm2`, `hdc-bm4` | várias |
| Impressora paralela | `parallel` | 1 |
| Super Serial Card | `ssc` | 1 |
| Mouse | veja a configuração | 1 |
| Saturn 128K | `saturn` | várias |
| Mockingboard C | veja a configuração | várias |
| Phasor | veja a configuração | várias |
| MegaAudio | `megaaudio` | várias |
| SD Music | `sdmusic` | várias |
| SAM | veja a configuração | várias |
| VidHD | `vidhd` | 1 |
| Breakpoint card | `breakpointcard` | várias |
| Uthernet, Uthernet II | veja a configuração | 1 |
| Z80 SoftCard | veja a configuração | 1 |
| 4Play, SNES MAX | veja a configuração | 1 |

**Slot AUX** (apenas //e): vazio, 80 colunas (1 KiB), Extended 80 colunas (64 KiB) ou RamWorks III (até 16 MiB). Na linha de comando: `-aux empty|std80|ext80|rw3`. `-r N` escolhe RamWorks III com N bancos de 64K (1 a 256).

Padrões: o ][ não tem Language Card; o ][+ tem a Language Card no slot 0; o //e tem a Extended 80 colunas.

## Disk II

Normalmente no slot 6 (às vezes um segundo no slot 5). Cada Carta tem dois drives. Opções: o arquivo de cada drive, o firmware de 13 setores (para DOS 3.2) e a opção de desconectar um drive (`-d1-disconnected`, `-d2-disconnected`). Veja "Imagens de disco".

## Controlador de disco rígido (HDC)

Normalmente no slot 7. Até 8 unidades por Carta, em arquivos `.hdv`, `.po` ou `.2mg`. Veja "Imagens de disco".

## Impressora paralela

Normalmente no slot 1. Tudo que o Apple imprime vai para um arquivo de texto (o nome é configurável); com `-use-real-printer` também é possível enviar para uma impressora real. Opções da Carta: o arquivo de saída, a conversão de codificação para clones (por exemplo MIK do Pravets 8 para ASCII), o filtro de caracteres não imprimíveis, o modo de acrescentar ao arquivo anterior e o tempo ocioso (10 segundos por padrão) após o qual o arquivo é fechado. Ao imprimir imagens numa impressora real você pode gastar dezenas de páginas; desligue o filtro e a conversão nesse caso.

## Mouse (AppleMouse)

Normalmente no slot 4. Veja "Joystick e mouse".

## Super Serial Card (SSC)

Emula uma SSC com o bloco de jumpers na posição MODEM, em qualquer slot (normalmente o 2). Ela se comunica por uma porta serial do computador ou por um servidor **TCP** (porta 1977 por padrão; mude com `-ssc-tcp-port`).

Atenção: no modo TCP o programa escuta em **todas as interfaces** de rede da máquina, como o AppleWin. Use um firewall se não quiser expor a porta.

Chaves DIP fixas: 9600 bauds, modo CIC, 1 bit de parada, 8 bits, sem paridade, sem avanço de linha e com interrupções. O 6551 emulado aceita interrupções de recepção e transmissão, RTS/CTS, DSR/DTR e a taxa não documentada de 115200 bauds.

Recursos não emulados, como no AppleWin: configurar as chaves DIP, o modo eco e a transmissão de BRK do registrador de comando, o clock externo, as taxas de 50 a 150, 1800, 3600 e 7200 bauds (mapeadas para a taxa mais próxima), e os bits de erro de overrun, framing e paridade do registrador de status. O DCD segue o DSR; a opção `-dcd` (ou `-modem`) faz o programa usar o estado real de DCD da porta serial.

No modo TCP a taxa configurada não importa: os dados passam sem limite de velocidade. Código do Apple que leia o registrador de status mais de uma vez na rotina de interrupção pode perder dados. Com uma conexão TCP ativa, os bits DCD e DSR do registrador de status ficam em zero e CTS (DIP SW2) também.

Para usar o Apple // Game Server (AGS): inicie o Apple no Applesoft, digite `IN#2` e, no computador, execute o AGS com `localhost 1977 apple2e`. Só o //e Enhanced funciona.

## Mockingboard, Phasor, MegaAudio e SD Music

Podem ficar em qualquer slot, mas muitos programas exigem o slot 4 (alguns usam placas duplas nos slots 4 e 5, como Ultima V).

- **Mockingboard C**: dois AY-3-8913 (três canais cada, com tom, ruído e envelope) controlados por dois 6522, e um SSI263 (fala). Os 6522 geram interrupções de temporizador e de fim de fonema.
- **Phasor**: equivale a duas Mockingboards, com quatro AY-3-8913. Liga em modo idêntico ao da Mockingboard C e pode mudar, por software, para o modo nativo Phasor (quatro AY) ou Echo+ (sem o chip de fala TMS5220).
- **Fala**: até dois SSI263 (SSI263P, o original com erro de reset, ou SSI263AP, a versão corrigida) e uma modificação de mercado com o Votrax SC-01 (`-sN sc01` ou `no-sc01`). A emulação do SSI263 é básica: reproduz os fonemas em taxa fixa, sem inflexão nem filtros. O SC-01 é emulado remapeando seus fonemas para os do SSI263.
- **Chips**: `-sN ay-socketK=chip` (K de 0 a 3) escolhe `empty`, `ay8910`, `ay8912`, `ay8913` ou `ym2149`; `-sN socketK=chip` (K de 0 ou 1) escolhe `empty`, `ssi263p` ou `ssi263ap`. `-sN 6522a-bad` e `6522b-bad` simulam um 6522 defeituoso.
- **MegaAudio** e **SD Music** são placas extras (veja `-mb-audit` na linha de comando).

## SAM

Um conversor digital-analógico de 8 bits (Software Automatic Mouth) que pode ficar em qualquer slot. O som sai pelos mesmos alto-falantes do computador.

## Z80 SoftCard

Emula a Carta de CP/M com um Z80 completo. Normalmente nos slots 4 ou 5.

## Relógios

- **No-Slot Clock** (Dallas DS1216 SmartWatch): fica sob um chip de ROM do Apple II. Por padrão existe, e não atrapalha quando não é usado; `-no-nsc` o remove. Precisa de um driver de software (DOS e ProDOS); o driver passa a emular uma Thunderclock. No //e, os drivers que funcionam dependem de onde o chip está: soquete "CD" ou "CF" (todos os drivers), "EF" (nenhum driver conhecido) e ROM do slot (NS.CLOCK.SYSTEM). No ][ e ][+, o soquete "F8" ainda não funciona com o ProDOS-8 2.5.0a8.
- **Relógio de slot (ThunderClock Plus e o relógio ROM do TK3000)**: um calendário uPD1990AC com 2K de firmware. No AppleWin esta Carta é só um esqueleto; aqui há uma implementação nova baseada no firmware original.
- No ProDOS, a ordem de carga dos drivers importa: o NS.CLOCK.SYSTEM encadeia o próximo driver e deve ser o primeiro; o ZIP.SYSTEM não encadeia e deve ficar por último.

## Uthernet I e II

Normalmente no slot 3. O Uthernet permite navegar na internet no Apple II com o sistema Contiki.

- **Uthernet I** emula o chip CS8900A e usa uma interface de rede do computador por meio da libpcap (Npcap no Windows), numa compilação com o recurso `pcap`.
- **Uthernet II** emula o chip W5100 com quatro sockets. Os sockets TCP e UDP são mapeados para sockets do computador (portanto funcionam sem Npcap/libpcap); sockets IP RAW e MAC RAW precisam do backend de rede bruta. Há ainda extensões do AppleII-Rust: `LISTEN` para servidores TCP e vinculação de sockets UDP à porta pedida.
- **DNS virtual**: uma extensão do AppleWin, sem equivalente em hardware real. Com ele marcado, o registro 0x28 do W5100 devolve 0x00 e programas do Apple que sabem usar o DNS virtual (como o A2Stream) resolvem nomes pelo computador. Desmarcado, o W5100 se comporta como o real (0x28). Usá-lo com programas que não o conhecem é inofensivo.
- Limitações herdadas do AppleWin: PPPoE, interrupções e SPI não estão implementados, o lado servidor tem pouco teste, e depois de carregar um save-state os sockets TCP e UDP ficam fechados.
- Se houver problemas, desative firewalls que bloqueiem a interface (o AppleWin cita o ZoneAlarm). No Windows, o Wi-Fi com WinPcap não funciona; uma máquina virtual com placa de rede em modo bridge ou NAT contorna isso.

## VidHD

Qualquer slot (no slot 3 convive com a Carta de 80 colunas do //e). Dá a todos os Modelos o modo Super Hi-Res do IIgs, usado por exemplo no Total Replay. A Carta é detectada por três bytes de ROM e espelha os soft switches do //e e os registradores de vídeo do IIgs. Não modelados: os modos de texto 80x45 e a cor da borda.

## 4Play

Normalmente nos slots 3, 4 ou 5. Na Carta real permite até quatro joysticks Atari. Os dois primeiros controles do computador fazem os joysticks 1 e 2. Usa só o espaço DEVICE SELECT do slot, então convive com a Carta de 80 colunas do //e (num //e PAL exige um riser de slot).

## SNES MAX

Normalmente nos slots 3, 4 ou 5. Permite dois controles de Super Nintendo com todos os 12 botões. Os dois primeiros controles do computador são usados, de preferência com 12 botões ou mais. Alguns controles (por exemplo 8BitDo NES30 Pro) precisam de remapeamento: use `-snes-max-alt-joy1` ou `-snes-max-alt-joy2`, ou um arquivo de mapeamento próprio com `-snes-max-user-joy1 arquivo` e `-snes-max-user-joy2 arquivo`.

## Breakpoint card

Uma Carta auxiliar de testes do depurador (como no AppleWin). O código de teste escreve numa fila de 32 entradas os acertos de breakpoint esperados; a cada acerto informado pelo depurador, uma entrada é comparada e a Carta levanta uma IRQ se houver divergência. É útil só para quem desenvolve ou testa o próprio depurador.

## Game I/O Connector: dongles

O conector de jogos interno aceita quatro dongles de proteção contra cópia: Southwestern Data Systems (SpeedStar), Dynatech/Cortechs (CodeWriter), Robocom (Robo 500/1000/1500, RoboCAD) e Hayden (Applesoft Compiler). Eles podem fixar o estado dos botões, interferir com a modificação da tecla Shift e com Cartas RGB. No ][ e ][+, um joystick selecionado continua conectado ao mesmo tempo que o dongle.

## Estado no frontend

As bibliotecas de todas as Cartas acima existem e têm testes. Quais delas o frontend já cria quando você as seleciona está descrito em "Diferenças em relação ao AppleWin".
