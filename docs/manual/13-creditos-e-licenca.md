# Créditos e licença

## Licença

O AppleII-Rust é software livre, distribuído sob a **GNU General Public License, versão 2 ou, a seu critério, qualquer versão posterior (GPL-2.0-or-later)**. O texto completo está no arquivo `LICENSE` do repositório. O programa é distribuído na esperança de ser útil, mas sem nenhuma garantia, nem mesmo a garantia implícita de comercialização ou adequação a um fim específico.

O AppleII-Rust é uma obra derivada do **AppleWin**, que também é GPL. Todo o comportamento de emulação foi portado a partir do código-fonte do AppleWin, e cada módulo cita o arquivo de origem. As ROMs e os recursos copiados do AppleWin têm o mesmo tratamento do projeto original.

## A equipe do AppleWin

O AppleII-Rust existe graças ao trabalho da equipe do AppleWin:

- Tom Charlesworth (líder do projeto)
- Michael Pohoreski (reescrita do depurador, vídeo de meio pixel, melhorias da interface)
- Nick Westgate (Floating bus, impressora paralela)
- Linards Ticmanis (opcodes ilegais ou indefinidos)
- Glenn Jones (placa Uthernet)
- Stannev (Pravets, o clone búlgaro do Apple II)
- Ken Wessen (suporte a CP/M, a partir do emulador TK3000 //e de Fábio Belavenuto)
- Sheldon Simms (janela 2x e emulação NTSC)
- Oliver Schmidt (consultoria)

## Agradecimentos

A equipe do AppleWin agradece a estas pessoas, e nós também:

- Brian Broker: o arquivo de ajuda HTML/CHM original, em que este manual se baseia.
- Thomas Stahl: mistura vertical no modo "Color (Composite Idealized)".
- Greg Hedger: amostras de fonemas do SSI263.
- Robert Hoem: placa de disco rígido (módulo e firmware).
- Equipe do VICE: módulos TFE, Z80 e PIA MC6821. Em especial Spiro Trikaliotis (TFE), cujo código Glenn Jones adaptou para o Uthernet.
- Equipe do FUSE: módulo AY-3-8910.
- Kyle Kim: suporte à placa de mouse, a partir de código da Apple in PC.
- Fábio Belavenuto: clone brasileiro TK3000 //e (e a integração original do Z80Em).
- Bob Sander-Cederlof: símbolos do Applesoft (S-C DocuMentor).
- David Schmidt: atualizações do arquivo de ajuda.
- Mike Harvey, fundador e editor da revista Nibble.
- Andrea Odetti: portabilidade do código-fonte e placa Uthernet II.
- Iván Izaguirre: clone taiwanês Copam Base64A.
- Arnaud C: sugestões para o depurador e ajuda com a temporização do 6502, 6522 e vídeo.
- Cyril Lambin: renderização da placa e monitor RGB, melhorias do depurador.
- Alex Lukacz: placas 4Play e SNES MAX.
- Matthew D'Asaro: dongles de proteção contra cópia do conector de jogos.
- Erik Struiksma: arquivos de mapeamento do SNES MAX.
- John Leffingwell: lógica LFSR do dongle CodeWriter.
- Hugh Hood: empréstimo dos dongles Robocom.
- Klaus Dormann: suíte de testes funcionais de 6502/65C02 usada no `apple2-cpu`.
- Stanislav Georgiev: texto histórico sobre os clones Pravetz.

## Sobre os clones Pravetz

Na década de 1980 a Bulgária foi a maior fabricante de computadores da região. Os modelos IMKO 1 e IMKO 2 (1980 e 1982) foram clones do Apple II; em 1983 o IMKO 2 passou a se chamar Pravetz 82. O Pravetz 8M tinha duas CPUs (6502 e Z80 para CP/M) e 64 KB de RAM. Depois vieram clones do //e: 8E, 8A, 8C e 8S.

## Código aberto

O código-fonte, os relatórios de problemas e as contribuições ficam em:

- GitHub: https://github.com/ChicoFigueiredo/AppleII-Rust
- GitLab: https://gitlab.com/chico-figueiredo/opensourcechico/AppleII-Rust

O AppleWin original: https://github.com/AppleWin/AppleWin
