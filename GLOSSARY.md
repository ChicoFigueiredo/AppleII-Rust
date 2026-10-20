# Apple II Emulation

Vocabulário do emulador AppleII-Rust, reescrita do AppleWin. Os termos seguem o hardware Apple II e os nomes usados no AppleWin.

## Máquina

**Modelo**:
A variante de computador emulada (II, II+, J-Plus, IIe, IIe Enhanced ou um clone, como Pravets, TK3000 ou Base64A). Ela determina as ROMs, a CPU e as peculiaridades de memória.
_Avoid_: tipo, plataforma, machine type

**Máquina**:
Uma instância emulada de um Modelo: CPU, memória, cartas e dispositivos, avançando ciclo a ciclo.
_Avoid_: emulador, sistema, core

**Ciclo**:
Um período do clock de 1,0205 MHz do Apple II, a unidade de tempo de toda a Máquina.
_Avoid_: tick, clock

**Quadro**:
Uma varredura completa da tela: 17.030 Ciclos em NTSC e 20.280 em PAL.
_Avoid_: frame, VBL

**Padrão de vídeo**:
NTSC ou PAL. Ele define o clock da CPU e o número de linhas por Quadro.
_Avoid_: região

**Contador de vídeo**:
A posição (linha, coluna em Ciclos) que o circuito de vídeo está varrendo, derivada dos Ciclos do Quadro.
_Avoid_: scanner, raster, beam

**Evento sincronizado**:
Uma ação de dispositivo agendada para daqui a N Ciclos. Ela pode levantar uma IRQ.
_Avoid_: timer, callback

**Fonte de IRQ**:
Um dispositivo que pode manter a linha de IRQ (ou NMI) da CPU ativa. A linha fica ativa enquanto houver alguma fonte ativa.
_Avoid_: interrupção pendente

## Memória

**Soft switch**:
Um endereço em $C000–$C0FF cuja leitura ou escrita muda o estado da Máquina ou devolve um estado dela.
_Avoid_: registrador de I/O, porta

**Paginação**:
O mapeamento atual de cada página de 256 bytes da CPU para a memória principal, a AUX ou a ROM, definido pelos Soft switches.
_Avoid_: banking, mapeamento

**Memória AUX**:
O segundo banco de 64K do IIe, fornecido pela carta no Slot AUX.
_Avoid_: memória estendida, banco 1

**Language Card**:
A RAM de 16K que se sobrepõe à ROM em $D000–$FFFF, com dois bancos em $D000. É nativa no IIe e fica no Slot 0 do II/II+.
_Avoid_: LC (só em código), RAM card

**Saturn 128K**:
Uma Carta de qualquer Slot com 8 bancos de 16K que se comportam como Language Card. As regras de habilitação de escrita diferem um pouco das da Language Card.
_Avoid_: Saturn card, RAM card

**Carta AUX**:
O que ocupa o Slot AUX: vazio, 80 colunas (1K), Extended 80 colunas (64K) ou RamWorks III (bancos de 64K até 16MB).
_Avoid_: carta 80 colunas (quando se refere a qualquer uma)

**ROM de sistema**:
A ROM de $D000–$FFFF do Modelo (12K). No IIe ela vem com mais 4K de ROM interna em $C100–$CFFF.
_Avoid_: firmware, BIOS

**Floating bus**:
O valor devolvido pela leitura de um endereço sem dono: o último byte que o circuito de vídeo leu da memória.
_Avoid_: barramento flutuante, lixo de leitura

## Cartas

**Carta**:
Um periférico conectado a um Slot. Ela responde a Soft switches do próprio slot e pode expor ROM.
_Avoid_: periférico, dispositivo, placa

**Slot**:
Uma das posições 0–7 de expansão. O Slot AUX do IIe é uma posição à parte.
_Avoid_: porta de expansão

**ROM de slot**:
Os 256 bytes de firmware de uma Carta em $Cn00–$CnFF, onde n é o número do Slot.
_Avoid_: firmware, PROM

**ROM de expansão**:
Os 2K de firmware em $C800–$CFFF que a Carta ativa assume; uma leitura de $CFFF libera essa faixa.
_Avoid_: C8 ROM, ROM estendida

## Entrada e som

**Código de tecla Apple**:
O valor ASCII de 7 bits que o teclado Apple coloca em $C000, independente do teclado do computador hospedeiro.
_Avoid_: scancode, keycode

**Strobe do teclado**:
O bit 7 de $C000: indica que há uma tecla ainda não lida. É limpo por um acesso a $C010.
_Avoid_: key ready, flag de tecla

**Fila de colagem**:
Os Códigos de tecla Apple de um texto colado, entregues um a um no ritmo em que o programa lê o teclado.
_Avoid_: clipboard, buffer de teclado

**Toque do alto-falante**:
Um acesso a $C030 que inverte o cone do alto-falante. É registrado com o Ciclo em que ocorreu.
_Avoid_: click, beep

**Porta de jogos**:
Os botões (PB0–PB2), os paddles (PDL0–PDL3, lidos por temporização) e os anunciadores (AN0–AN3) em $C058–$C07F.
_Avoid_: joystick (quando se refere à porta inteira)

**Anunciador**:
Uma das quatro saídas digitais AN0–AN3 da Porta de jogos, ligadas e desligadas por Soft switches.
_Avoid_: annunciator, saída TTL

**Open Apple / Closed Apple**:
As teclas modificadoras do IIe, lidas como os botões 0 e 1 do joystick.
_Avoid_: Command, Option
