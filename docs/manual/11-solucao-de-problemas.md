# Solução de problemas

Esta página reúne problemas conhecidos e como contorná-los. Parte dela vem da página de problemas do AppleWin e vale igualmente aqui.

## Como voltar às configurações padrão

Apague o arquivo `settings.toml` (veja "Configuração" para o local em cada sistema). Na próxima execução o programa usa os padrões do AppleWin.

## O programa não inicia

- **Linux:** confira as bibliotecas do sistema para áudio, entrada e janela (`libasound2`, `libudev`, `libxkbcommon`, `libwayland`). Veja o README.
- **Sem áudio:** se nenhum dispositivo de som puder ser aberto, o programa continua sem som e imprime `audio disabled: ...` no terminal. Verifique o servidor de som (PulseAudio, PipeWire, ALSA) ou use `--no-audio`.
- **Windows:** um antivírus pode bloquear o executável. Libere o programa no antivírus.
- **Janela preta ou erro de vídeo:** o programa usa `wgpu` (Vulkan, Metal ou DirectX 12). Atualize o driver da placa de vídeo.

## Não aparece nada, só o motor do drive girando

Não há disco no drive 1 ou ele não foi lido. Insira uma imagem (F3) e reinicie (F2), ou digite Ctrl+F2 para cair no BASIC.

## O disco `.po` não dá boot e cai no monitor

Algumas imagens `.po` na verdade têm setores na ordem DOS. Renomeie para `.do` ou `.dsk`. Também é possível usar um utilitário como o CiderPress para converter em `.po` verdadeiro.

## Teclados que não são americanos ou britânicos

Algumas teclas dependem do AltGr (Alt direito), por exemplo `@` em teclado franco-canadense (AltGr+2) ou `[` em teclado italiano (AltGr+è). O Alt direito é também a Solid Apple (botão 1 do joystick). Alguns programas usam o botão ou Ctrl+@ (Lode Runner usa Ctrl+@ para ganhar vidas), e AppleLogo // e AppleWorks 5.1 têm problemas com teclados italianos.

Como no AppleWin, o caractere que o layout define para AltGr + tecla (`@`, `[`, `/` no ABNT2...) é digitado normalmente, sem opção. Só as combinações sem caractere no layout (AltGr+A no layout americano) são descartadas por padrão; para que digitem a tecla base, use `-altgr-sends-wmchar` (ou marque "AltGr sends characters" na configuração).

Se o uso duplo do AltGr impedir de digitar um caractere, a saída é a opção `-no-hook-alt` com o Joystick 1 em "Teclado (numérico)", usando `0` e `.` como Open Apple e Solid Apple.

## Sem som do alto-falante

Verifique o volume mestre (configuração) e se `-m` ou `disable_sound` estão ligados. Em velocidade máxima o som é silenciado de propósito.

## Compatibilidade na placa RGB e na "Color (RGB Card/Monitor)"

- Dragon Wars: o bit 7 do double hi-res precisa ser invertido; use `-rgb-card-invert-bit7`.
- Renegade: muda para 160x192 na tela de título e depois para preto e branco após o jogo.
- Placa AppleColor da Apple ou RGB-SL7 da Video-7: há corrupção em títulos ou demos que mudam por acidente para o modo hi-res de primeiro e segundo plano (por exemplo a demo DIGIDREAM da French Touch).

## Não consigo mudar para a janela 2x

Se a tela não for grande o bastante, o tamanho 2x (1120x768 mais a barra) não cabe. Use a escala 1x ou a tela cheia (F6).

## A janela fica borrada em monitores 2K, 4K ou com escala de tela

Use o escalonamento inteiro (opção "Integer scaling" na configuração) ou escolha uma escala de janela que seja um número inteiro. Em sistemas com escala fracionária (por exemplo 150%), o sistema redimensiona a janela inteira.

## O mouse não sai da janela

Quando a Carta de mouse está inserida, o clique na tela captura o cursor. Pressione **Ctrl+clique** (ou Shift+clique) para liberá-lo.

## Uthernet não funciona

- Uthernet I precisa da libpcap (Linux e macOS) ou do Npcap (Windows) e de uma versão do programa compilada com o recurso `pcap`.
- Uthernet II com sockets TCP e UDP funciona sem libpcap.
- Desative ou configure o firewall (o AppleWin cita o ZoneAlarm).
- Interfaces Wi-Fi com WinPcap não funcionam; use uma máquina virtual com rede em modo bridge ou NAT.

## A SSC no modo TCP expõe uma porta

A SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin. Bloqueie a porta no firewall se não quiser que outras máquinas se conectem.

## Breakpoint zero (Ctrl+Shift+0) não funciona

No Windows, essa combinação pode ser reservada pelo seletor de teclado do sistema. Na configuração de teclado do Windows, defina "Alternar layout de teclado" como "Não atribuído". Nesta versão o depurador só tem console, portanto o problema só importa se o atalho vier a ser implementado.

## Como relatar um problema

Abra uma issue no GitHub ou no GitLab do projeto (veja o README). Informe o sistema operacional, o resultado do comando `VERSION` do depurador, a linha de comando usada e, se possível, um save-state que reproduza o problema.
