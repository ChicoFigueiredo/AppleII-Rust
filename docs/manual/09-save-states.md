# Save-states

Um save-state guarda o estado completo da Máquina num arquivo, a qualquer momento. Serve para continuar uma sessão depois, ou para jogos que não têm opção de salvar.

## Como usar

- **F11** salva o estado.
- **F12** carrega o estado.
- Um arquivo de save-state arrastado para a janela, ou passado com `-load-state arquivo`, também é carregado.
- A configuração tem o nome do arquivo (`savestate_filename`) e a opção de salvar o estado ao sair (`save_state_on_exit`).

Antes de salvar, as trilhas de disco modificadas são gravadas na imagem.

## O que é guardado

O estado inclui o Modelo, a memória principal e a AUX (80 colunas, Extended 80 colunas, RamWorks III), a Language Card do slot 0 ou do //e, a Saturn 128K, os soft switches, o teclado, o alto-falante, a porta de jogos, o escalonador de eventos e as Cartas dos slots (Disk II mesmo durante uma leitura ou escrita, disco rígido, Mockingboard e Phasor, SAM, mouse, Z80 SoftCard, impressora, SSC, No-Slot Clock, Uthernet e Uthernet II, 4Play, SNES MAX, VidHD e o dongle do conector de jogos).

Ainda não entram: a ROM F8 do Freeze, a ROM F8 alternativa, a ROM alternativa e a ROM de vídeo alternativa.

## Formatos

**Formato YAML do AppleWin (`.aws.yaml`).** O módulo `apple2-savestate` lê e grava o mesmo documento YAML legível do AppleWin (`File_hdr` versão 2, unidades `Apple2`, `Auxiliary Slot`, `Slots`, `Game I/O Connector` e `Miscellaneous`), com os mesmos nomes de chaves e versões de unidade. O AppleWin guarda o nome e o caminho absoluto de cada imagem de disco, o que permite mover o arquivo e redistribuí-lo.

Para o resultado ser exato, o gravador acrescenta ao documento um bloco extra, `AppleII-Rust Extension`, que o AppleWin ignora e que este programa usa para continuar do ponto exato (por exemplo a fila de eventos). Um arquivo vindo do AppleWin não tem esse bloco: é reconstruído só com o que o AppleWin guarda, tão fielmente quanto o próprio AppleWin consegue. Cartas que o AppleWin não mapeia ficam só no bloco extra, e o AppleWin carrega o arquivo sem elas, em vez de rejeitá-lo.

O formato antigo `.aws` (versão 1) não é aceito, como no AppleWin atual. Para convertê-lo, abra-o no AppleWin 1.27.13 e salve de novo.

**No programa.** F11 salva e F12 carrega (Ctrl+F12 também), sempre por um seletor de arquivos que começa no arquivo atual, como no AppleWin. Ao salvar, `.aws.yaml` é acrescentado se faltar. O arquivo padrão é `SaveState.aws.yaml` no diretório atual, ou o nome da imagem de disco rígido ou disquete inserida, no diretório dela. Também funcionam `-load-state arquivo`, soltar o arquivo na janela e a opção "Save State On Exit". As imagens de disco são procuradas no diretório do arquivo, no diretório atual e no diretório de discos configurado.

## Uthernet II

Depois de carregar um save-state, os sockets TCP e UDP do Uthernet II ficam fechados.
