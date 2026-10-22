# Impressora virtual

O AppleII-Rust imprime como uma **Epson LX-310** de 9 agulhas. O que o Apple II imprime vira Páginas na **Central de Impressoras**, pode ser salvo em **PDF** e fica guardado no **Arquivo de impressões**. O guia completo está em `docs/printer/README.md`.

## Ligar a impressora

1. Pressione **F8** e abra a aba **Slots**.
2. No **slot 1**, escolha **Printer**.
3. Confirme com **OK**. A Máquina reinicia.

Nas opções da Carta (botão **...**), o destino padrão é **Virtual printer + file**: a impressora virtual e o arquivo `Printer.txt` do AppleWin recebem a mesma impressão.

## Imprimir

No BASIC:

```
PR#1
LIST
PR#0
```

Os programas (AppleWorks, Print Shop, dBase II no CP/M) escolhem a impressora e o slot 1 nos próprios menus. Escolha uma impressora **Epson** (FX/MX) com interface **paralela Apple** no slot 1.

## Central de Impressoras

- Abre sozinha no primeiro byte impresso, sem tirar o foco do Apple II. A opção pode ser desligada nas opções da Carta, e o botão **Printer** da barra de ferramentas abre (ou traz para a frente) a janela.
- Mostra o papel sendo preenchido, com zoom e navegação entre Páginas, o estado da impressora e os botões **Salvar PDF…**, **Salvar RAW…**, **Concluir trabalho** e **Cancelar trabalho**. A aba **Arquivo de impressões** lista os Trabalhos guardados.
- Fechar a janela não perde nada.
- Sequências que a impressora não entende aparecem como diagnósticos, nunca em silêncio.

## Transporte dos bytes

- **Automático** (padrão): o que passa pelo firmware da Carta (`PR#1`, `PRINT`, `LIST`) perde o bit 7, e o que um programa escreve direto na porta da Carta chega inteiro. Serve para BASIC e para os gráficos do Print Shop.
- **8 bits**: sempre o byte inteiro. Estraga o texto do BASIC (o BASIC liga o bit 7 de tudo).
- **7 bits (AppleWin)**: sempre sem o bit 7. Use se um driver direto mandar texto com o bit 7 ligado e sair com símbolos.

O `Printer.txt` é sempre de 7 bits.

## Papel, Fita e PDF

- **Papel**: formulário contínuo zebrado verde (padrão) ou azul, contínuo liso, A4 ou Carta. Papel, Fita, Cor e Tabela valem para o próximo Trabalho.
- **Fita**: nova (padrão) ou usada; muda só o visual.
- **Salvar PDF**: pontos vetoriais mais texto invisível para busca e cópia, com metadados; folha de rosto opcional.

## Arquivo de impressões

Cada impressão fica guardada na pasta de dados (`apple2-rust/impressoes`) com os bytes originais, título e nota. **Nada é apagado sozinho.** Reimprimir (aba Arquivo de impressões) com outro Papel, Fita ou Cor cria uma nova versão sem mexer no original.

## O que é aproximação

A fonte matricial é desenho próprio e pode diferir da Epson real; o tamanho do ponto e o visual do papel são estimados. Posições, passos, margens e os comandos marcados como suportados seguem o manual da LX-310. Só o Perfil LX-310 existe por enquanto.
