# Joystick e mouse

O Apple II lê dois joysticks (paddles analógicos) e até três botões pela porta de jogos. O programa oferece várias fontes para cada joystick. A escolha é feita na configuração (aba de entrada) ou nas configurações salvas.

## Fontes de joystick

| Fonte | Joystick 0 | Joystick 1 | Como funciona |
|---|---|---|---|
| Desativado | sim | sim | Nenhum joystick conectado |
| Controle do computador | sim (primeiro controle) | sim (segundo controle) | Gamepad detectado pelo sistema |
| Controle, segundo analógico | não | sim | Joystick 1 usa o analógico direito do primeiro controle |
| Teclado (setas) | sim | sim | As setas movem o joystick |
| Teclado (numérico) | sim | sim | Teclado numérico |
| Mouse | sim | sim | O mouse sobre a tela move o joystick |

### Controles (gamepad)

Os dois primeiros controles detectados são usados. O analógico esquerdo move o joystick (com zona morta de 10%); a cruz direcional também. Os botões "sul" e "oeste" fazem o botão 0, e "leste" e "norte" fazem o botão 1. O programa usa a biblioteca `gilrs`, que funciona em Windows, Linux e macOS.

### Teclado (numérico)

As teclas 1 a 9 do teclado numérico são as oito direções e o centro (5), como o desenho de um teclado de telefone. `0` ou `Insert` é o botão 0; `.`, `,` ou `Delete` é o botão 1. Ao soltar as teclas, o joystick volta ao centro.

### Teclado (setas)

As quatro setas movem o joystick. Os botões são as teclas Alt: Alt esquerdo (Open Apple) é o botão 0 e Alt direito (Solid Apple) é o botão 1.

### Mouse como joystick

A posição do mouse dentro da imagem do Apple vira a posição do joystick (a tela inteira corresponde a 0 a 255 nos dois eixos). Os botões esquerdo e direito são os botões 0 e 1. Quando existe uma Carta de mouse (AppleMouse), o mouse passa a controlá-la e a emulação de joystick por mouse deixa de ser usada.

## Botões e a porta de jogos

- O botão 0 é também Open Apple e o botão 1 é também Closed Apple, como nas máquinas reais.
- No Apple ][ e ][+ sem joystick configurado, os botões 0 e 1 são lidos como pressionados, igual ao AppleWin. Com o joystick 2 desativado, a "modificação da tecla Shift" do ][ permite que PB2 leia o estado da tecla Shift.
- Os paddles são lidos por temporização: cada leitura de $C070 reinicia os contadores e o programa mede quantos Ciclos levam para o bit mudar. Isso é feito com os mesmos tempos do AppleWin.
- Dongles de proteção contra cópia na porta de jogos (veja "Cartas por slot") podem fixar botões em um valor; use um dongle somente com o programa a que se destina.

## Mouse do Apple (AppleMouse)

Com a Carta de mouse inserida (normalmente no slot 4):

- Clique sobre a tela do Apple para **capturar** o mouse: o cursor do computador fica preso e escondido, e os movimentos relativos vão para a Carta.
- **Ctrl+clique** ou **Shift+clique** libera o mouse. Em programas como o GEOS, que seguram o cursor, esse é o jeito de recuperar o ponteiro do sistema.
- Os botões do mouse do computador são os botões da Carta.

As opções de mostrar uma cruz na moldura da janela e de restringir o mouse à janela existem nas configurações da Carta (`show_crosshair` e `restrict_to_window`).

## 4Play e SNES MAX

Veja "Cartas por slot". Essas Cartas leem os controles do computador pela mesma camada de gamepad descrita acima: o controle 1 e o controle 2 do computador são os controles 1 e 2 da SNES MAX e as portas 1 e 2 da 4Play, qualquer que seja o dispositivo escolhido para os joysticks 0 e 1 do Apple. Na SNES MAX os botões seguem o mapa padrão do AppleWin (Sony DS4/DualSense): X/Sul = B, Quadrado/Oeste = Y, Círculo/Leste = A, Triângulo/Norte = X, gatilhos superiores = L e R, Select e Start; as direções vêm do analógico esquerdo e da cruz. Na 4Play os gatilhos são os botões Sul e Leste, e as portas 3 e 4 são o teclado: **E/S/D/F** (cima, esquerda, baixo, direita) com **Z/X** nas portas 3, e **I/J/K/L** com **N/M** na porta 4. Essas teclas continuam digitando no Apple.
