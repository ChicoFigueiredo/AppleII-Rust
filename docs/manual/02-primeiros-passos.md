# Primeiros passos

## Iniciando

Execute o programa `applewin` (veja o README para compilar ou instalar). Abre-se uma janela com a tela do Apple (560x384 pixels, escalável), uma barra de botões à esquerda e uma linha de status embaixo. A Máquina inicial é um Apple //e Enhanced com Disk II no slot 6, a menos que a sua configuração salva diga outra coisa.

Para começar já ligado e com um disco:

```
applewin -d1 meu-disco.dsk -power-on
```

## Início rápido

O AppleWin roda programas a partir de **imagens de disco**: arquivos que guardam o conteúdo inteiro de um disquete.

1. Clique no botão **Drive 1** da barra (ou pressione **F3**) e escolha uma imagem.
2. Clique em **Run** (ou pressione **F2**) para ligar a Máquina e dar boot.

Depois do boot você usa o Apple como o equipamento real. Sem disco no drive 1 o motor do drive fica ligado e a Máquina espera. Para cair no BASIC, simule Ctrl+Reset com **Ctrl+F2** ou **Ctrl+Pause**.

Também é possível arrastar uma imagem de disco (ou um arquivo de save-state) para a janela. Uma imagem entra no drive 1 e a Máquina reinicia; um save-state é carregado.

## A barra de botões

Os botões da esquerda correspondem às teclas F1 a F8, como no AppleWin:

| Tecla | Botão | Ação |
|---|---|---|
| F1 | Help | Abre este manual |
| F2 | Run | Liga ou reinicia (power-cycle). Ctrl+F2 faz Ctrl+Reset |
| F3 | Drive 1 | Escolhe a imagem do drive 1. Ctrl+F3 ou botão direito: menu do drive |
| F4 | Drive 2 | Escolhe a imagem do drive 2. Ctrl+F4 ou botão direito: menu do drive |
| F5 | Swap | Troca os discos dos drives 1 e 2 |
| F6 | Full | Alterna janela e tela cheia. Ctrl+F6 alterna 1x e 2x |
| F7 | Debug | Entra no depurador (ou sai dele) |
| F8 | Config | Abre a janela de configuração |

Ao lado de cada botão de drive há um LED: apagado (motor desligado), verde (lendo) ou vermelho (gravando). O menu do drive oferece "Insert disk image..." e "Eject".

A linha de status mostra o Modelo, a velocidade, o estado de pausa e o estado dos drives (trilha e atividade). O título da janela repete as informações principais. Na tela cheia a barra e a linha de status ficam ocultas.

## Salvando seu trabalho

**F11** salva o estado completo da Máquina e **F12** o restaura. Veja "Save-states". As escritas em disco vão para o arquivo da imagem; a trilha modificada é gravada ao ejetar o disco, ao salvar um estado e ao sair.

Cuidado: fechar o programa (ou remover o controlador) enquanto um disco rígido está sendo gravado pode corromper a imagem, como no equipamento real.
