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

- Abre sozinha no primeiro byte impresso, sem tirar o foco do Apple II. A opção pode ser desligada nas opções da Carta, e o botão **Printer** da barra de ferramentas abre e fecha a janela.
- Mostra o papel sendo preenchido, com zoom e navegação entre Páginas.
- Fechar a janela não perde nada.
- Sequências que a impressora não entende aparecem como diagnósticos, nunca em silêncio.

## 8 bits ou 7 bits

- **8 bits** (padrão) entrega o byte inteiro e é necessário para os gráficos (a agulha de cima de cada coluna é o bit 7).
- **7 bits (AppleWin)** apaga o bit 7, como o AppleWin.

Para **listagens e programas BASIC**, escolha **7 bits**: o BASIC liga o bit 7 de todo caractere, e em 8 bits o texto sai como símbolos. Para **gráficos** (Print Shop), use 8 bits. O `Printer.txt` é sempre de 7 bits.

## Papel, Fita e PDF

- **Papel**: formulário contínuo zebrado (padrão), A4 ou Carta.
- **Fita**: nova (padrão) ou usada; muda só o visual.
- **Salvar PDF**: pontos vetoriais mais texto invisível para busca e cópia, com metadados; folha de rosto opcional.

## Arquivo de impressões

Cada impressão fica guardada na pasta de dados (`apple2-rust/impressoes`) com os bytes originais, título e nota. **Nada é apagado sozinho.** Reimprimir com outro Papel ou Fita cria uma nova versão sem mexer no original.

## O que é aproximação

A fonte matricial é desenho próprio e pode diferir da Epson real; o tamanho do ponto e o visual do papel são estimados. Posições, passos, margens e os comandos marcados como suportados seguem o manual da LX-310. Só o Perfil LX-310 existe por enquanto.
