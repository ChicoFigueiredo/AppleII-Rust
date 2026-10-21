# Teclado e atalhos

## Ligando e resetando

Num Apple II real você liga o equipamento com um disco no drive. Sem disco, o motor do drive fica girando para sempre. Para entrar no Applesoft BASIC e parar o drive, pressione Ctrl+Reset. No emulador:

- **F2**: liga (power-cycle) ou reinicia.
- **Ctrl+F2** ou **Ctrl+Pause**: Ctrl+Reset.

## Teclas especiais do Apple

| Tecla Apple | Tecla do PC |
|---|---|
| Reset | Ctrl+F2 ou Ctrl+Pause (Ctrl+Break) |
| Open Apple (botão 0 do joystick) | Alt esquerdo |
| Solid Apple / Closed Apple (botão 1) | Alt direito (AltGr) |
| Teclado numérico (//e estendido) | Teclado numérico do PC |
| Caps Lock | Caps Lock |

O teclado numérico só chega ao Apple se a emulação de joystick não estiver usando o teclado (veja "Joystick e mouse").

**Caps Lock.** Ao iniciar, o Caps Lock do Apple começa ligado (desligado com `-capslock=off`), qualquer que seja o estado do PC. O programa supõe o Caps Lock do PC desligado ao iniciar: o primeiro toque deixa o do Apple ligado e o segundo o desliga, como no AppleWin. O indicador "A" (ligado) ou "a" aparece na linha de status (não existe no ][ e no ][+). Minúsculas só existem em Modelos //e, //e Enhanced e clones do //e, não no ][ nem no ][+.

## Atalhos do emulador

| Atalho | Ação |
|---|---|
| F1 | Abre este manual |
| F2 | Liga ou reinicia (power-cycle) |
| Ctrl+F2 | Ctrl+Reset |
| F3 / F4 | Escolhe a imagem do drive 1 / 2 |
| Ctrl+F3 / Ctrl+F4 | Menu do drive 1 / 2 (inserir ou ejetar) |
| F5 | Troca os discos dos drives 1 e 2 |
| F6 | Alterna janela e tela cheia |
| Alt+Enter | Alterna janela e tela cheia (com `-alt-enter=open-apple-enter`, ou desligando "Alt+Enter toggles full-screen", vira Open Apple + Return no Apple) |
| Ctrl+F6 | Alterna entre janela 1x e 2x |
| F7 | Entra e sai do depurador |
| Ctrl+F11 | No depurador: executa uma instrução (passo) |
| F8 | Abre e fecha a janela de configuração |
| F9 | Próximo modo de vídeo |
| Shift+F9 | Modo de vídeo anterior |
| Ctrl+Shift+F9 | Liga e desliga as linhas de varredura a 50% |
| F10 | //e e //e Enhanced: chave "rocker" da ROM de vídeo europeia. Pravets 8A: Caps Lock |
| F11 | Salva o estado (save-state) |
| F12 | Carrega o estado |
| Pause | Pausa e retoma a emulação |
| Scroll Lock | Velocidade máxima (veja abaixo). No TK3000 //e é também a tecla Mode |
| Ctrl+0 | Alterna entre a velocidade configurada e a máxima |
| Ctrl+1 | Velocidade de 1 MHz |
| Ctrl+3 | Velocidade máxima |
| Print Screen | Salva a tela em um arquivo PNG de 560x384 |
| Shift+Print Screen | Salva a imagem de 280x192 |
| Ctrl+Print Screen | Copia o texto da tela (40 ou 80 colunas, incluindo a memória auxiliar, e a página 2) para a área de transferência |
| Shift+Insert ou Ctrl+Shift+V | Cola o texto da área de transferência (Ctrl+V sozinho envia ^V ao Apple) |

Os atalhos F1 a F8 são os mesmos botões da barra. As teclas com auto-repetição do sistema não disparam o atalho de novo.

### Modo F9

F9 percorre os tipos de vídeo: monocromático (cor própria), Color (Composite Idealized), Color (RGB Card/Monitor), Color (Composite Monitor), TV colorida, TV preto e branco, âmbar, verde e branco. Isso permite trocar o monitor sem abrir a configuração.

### Velocidade máxima

Scroll Lock mantém a velocidade máxima enquanto está pressionado, ou liga e desliga se a opção "ScrollLock Toggle" estiver marcada (sem limite de ritmo). Nesse modo o som fica mudo e o vídeo é atualizado apenas uma vez por Quadro do relógio real; efeitos que mudam o modo de vídeo no meio do Quadro (demos com "split screen") não aparecem direito. Com a opção "Enhanced Speed" de acesso ao disco (padrão), a emulação também acelera sozinha enquanto o motor do Disk II está ligado.

### Colar texto

Shift+Insert ou Ctrl+Shift+V entregam o texto da área de transferência ao teclado do Apple, um caractere por vez (a Fila de colagem). Quebras de linha viram Return. Caracteres acima de Latin-1 são descartados.

### Captura de tela

Print Screen grava a imagem de 560x384 no diretório atual como `AppleWin_0000.png`, `AppleWin_0001.png` e assim por diante (o primeiro nome livre). A mensagem aparece na linha de status.

## Teclas do sistema (Windows)

O AppleWin intercepta Alt+Tab, Alt+Esc, Alt+Espaço e Ctrl+Esc no Windows. Este programa não faz isso: o sistema operacional continua tratando essas combinações. As opções de linha de comando `-hook-alt-tab`, `-no-hook-system-key` e semelhantes são aceitas, mas não têm efeito.

## Teclados não americanos

Caracteres que exigem AltGr (por exemplo `@` ou `[` em alguns idiomas) entram normalmente como caracteres do Apple. Como o Alt direito também é a Solid Apple, a combinação pode ser percebida pelo programa do Apple como um botão pressionado. Veja "Solução de problemas".
