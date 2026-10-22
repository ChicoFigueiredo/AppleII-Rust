# Roteiros manuais: Windows nativo, macOS e Linux Wayland

- Escopo: HU-0901 (#36) e HU-0902 (#37).
- Estado de todos os roteiros: **BLOCKED** até o dono executar na máquina real e preencher os campos de resultado. Este ambiente é Linux; o Windows foi exercitado só no Wine ([../platform/windows-wine.md](../platform/windows-wine.md)), que não vale como Windows nativo. Não há hardware Apple.
- Os pacotes de teste vêm de uma release (ou de um dry run autorizado do workflow `Release`). Os pacotes **não são assinados**: veja [../platform/assinatura.md](../platform/assinatura.md).

Como preencher: cada passo tem coluna de resultado. Escreva `PASS`, `FAIL` ou `N/A`. Em `FAIL`, anote a mensagem exata e, se possível, anexe captura de tela e o `stderr`. Rode sempre a partir de um terminal na primeira vez, para ver o `stderr`.

Verificação do download (todos os sistemas): na pasta do download, `sha256sum --check --ignore-missing SHA256SUMS` (Linux), `shasum -a 256 -c SHA256SUMS --ignore-missing` (macOS) ou `Get-FileHash <arquivo> -Algorithm SHA256` e comparar a olho (Windows).

## Roteiro comum (passos P1 a P14)

Os passos são os mesmos em todos os sistemas; as seções seguintes dizem como instalar e abrir, e o que é específico.

| # | Passo | Resultado esperado |
|---|---|---|
| P1 | Abra o programa (sem argumentos). | Janela "AppleII-Rust" abre em segundos, mostrando a tela do Apple //e; sem mensagem de erro. |
| P2 | Pressione F2 (liga), com disco montado (P4). Digite `PRINT "OLA"` e Enter. | O texto aparece, na posição certa do cursor. |
| P3 | Digite todas as teclas alfanuméricas e Shift+números. | Letras em maiúsculas (modelo //e: com Caps Lock). Caracteres batem com o que foi digitado. |
| P4 | F3 e escolha `disks/DOS 3.3 System Master - 680-0210-A.dsk` (ou arraste o arquivo para a janela). Pressione F2. | Boot do DOS 3.3 até o menu do System Master. |
| P5 | Repita com `disks/ProDOS_2_4_3.po` no drive 1. | Boot do ProDOS. |
| P6 | Com o DOS no ar, pressione F11 (salva estado). Digite algo diferente. Pressione F12. | A tela volta ao momento do F11. |
| P7 | Feche o programa, abra de novo, pressione F12. | O estado salvo reabre (se não, anote o caminho do arquivo de estado). |
| P8 | Áudio: com o DOS no ar, digite `PRINT CHR$(7)` ou `CALL -198`. | Ouve-se o bipe do alto-falante. Sem dispositivo de áudio (desabilite a saída do SO e reabra): o programa não cai e avisa no `stderr`. |
| P9 | Gamepad: conecte um controle; F8 > Entrada. Mova os eixos num jogo ou no teste de joystick. | O controle é detectado e responde. Sem controle: o programa roda normalmente. |
| P10 | HiDPI: arraste a janela entre dois monitores de escala diferente, ou mude a escala do SO para 150% ou 200%. | A janela mantém proporção, texto nítido, sem corte nem borrão excessivo. Anote a escala. |
| P11 | Tela cheia: F6 (e Alt+Enter). Volte com F6. | Entra e sai de tela cheia sem tela preta nem travar; proporção preservada. |
| P12 | Minimize a janela por 10 s e restaure. | Volta com a imagem; o emulador não acelera nem trava; áudio sem estalos prolongados. |
| P13 | Pressione F1. | Abre o manual no programa, legível, com índice. |
| P14 | Feche pelo botão da janela. | O programa fecha sem demora, sem mensagem de falha, sem relatório de travamento do SO. Código de saída 0 se aberto por terminal. |

Teclado regional (rodar P3 com cada layout do SO ativo):

| Layout | Teclas a testar | Esperado | Resultado |
|---|---|---|---|
| ABNT2 (Brasil) | `' " [ ] { } ~ ^ ; / ? \ \| ç` e o botão `/` do teclado numérico | Cada tecla gera o caractere ASCII correspondente no Apple II (que só tem ASCII de 7 bits); teclas mortas (til, acento) não travam o teclado | |
| AZERTY (França) | `A Z Q W M , ; : ! ù &` e dígitos (com Shift) | Letras nas posições físicas esperadas; dígitos exigem Shift como no layout | |
| US (controle) | linha numérica e pontuação | referência | |

## Windows nativo (x86_64)

Material: Windows 10 ou 11, o `apple2-rust-<versão>-windows-x64.zip` da release.

| Campo | Valor |
|---|---|
| SO e versão (winver) | |
| Arquitetura | |
| GPU e driver | |
| Escala de tela | |
| Versão do pacote e SHA-256 | |
| Data e executor | |

| # | Passo | Resultado |
|---|---|---|
| W1 | Baixe o `.zip` e verifique o SHA-256 contra o `SHA256SUMS`. | |
| W2 | Extraia para uma pasta (por exemplo `C:\AppleII-Rust`), sem espaços e depois com espaços e acentos no caminho. | |
| W3 | Abra `apple2-rust.exe` com duplo clique. Anote se o SmartScreen ou o antivírus avisou (o executável não é assinado). Use "Mais informações" > "Executar assim mesmo". | |
| W4 | Na pasta do programa, de um `cmd` (`apple2-rust`) e de um PowerShell (`.\apple2-rust`): `--help` (acha o `apple2-rust.com`): a ajuda aparece e o prompt só volta no fim. `apple2-rust --help > ajuda.txt` e `apple2-rust --help \| more` entregam a ajuda inteira. `apple2-rust.exe --help` (o GUI) mostra a ajuda no console, sem redirecionamento (#107). | |
| W5 | Passos P1 a P14. | |
| W6 | Pasta `disks\` presente com os três discos. `THIRD-PARTY-LICENSES.html` e `LICENSE` presentes. | |
| W7 | DPI por monitor (manifesto PerMonitorV2): arraste entre monitores de escalas diferentes, sem ficar borrado. | |
| W8 | Caminho longo (> 260 caracteres) num disco: monta sem erro. | |
| W9 | Configurações: ficam em `%APPDATA%\...\apple2-rust`? Anote o caminho real. | |
| W10 | Apague o programa e a pasta de configuração: não deixa lixo fora delas. | |

## macOS Intel (x86_64) e Apple Silicon (arm64)

Material: o `apple2-rust-<versão>-macos-universal.zip` (binário universal, `lipo`), assinado só ad hoc e não notarizado. Rode em **cada** máquina, preenchendo um quadro por arquitetura.

| Campo | Intel | Apple Silicon |
|---|---|---|
| macOS (versão) | | |
| Modelo da máquina | | |
| Tela (Retina? escala) | | |
| Versão do pacote e SHA-256 | | |
| Data e executor | | |

| # | Passo | Intel | Apple Silicon |
|---|---|---|---|
| M1 | Baixe o `.zip` pelo navegador (ganha quarentena), verifique o SHA-256. | | |
| M2 | Descompacte (Finder ou `ditto -x -k`) e arraste `AppleII-Rust.app` para Aplicativos. | | |
| M3 | Abra com duplo clique. O Gatekeeper deve recusar (não notarizado): anote a mensagem. Depois clique com o botão direito > Abrir > Abrir. | | |
| M4 | Alternativa: `xattr -d com.apple.quarantine /Applications/AppleII-Rust.app` e abra. | | |
| M5 | `codesign -dv --verbose=4 /Applications/AppleII-Rust.app` e `spctl --assess --type execute -vv /Applications/AppleII-Rust.app`: anote a saída (esperado: assinatura ad hoc; `spctl` rejeita). | | |
| M6 | `lipo -info AppleII-Rust.app/Contents/MacOS/applewin`: deve listar `x86_64 arm64`. | | |
| M7 | Passos P1 a P14. Em P10 use uma tela Retina: a janela deve ficar nítida (escala 2x), sem pixels borrados; teste também a janela em tela externa não Retina. | | |
| M8 | Menu da barra do macOS: nome do app, Cmd+Q fecha; Cmd+W (se existir). Anote o que não responde. | | |
| M9 | Associação de arquivo: dê duplo clique num `.dsk` do pacote. O app abre com o disco? | | |
| M10 | Permissões: o macOS pede acesso a algo (Entrada, Microfone, Pastas)? Anote os pedidos. | | |
| M11 | Cmd+Option+Esc: o app nunca aparece como "Não respondendo" em uso normal. | | |

## Linux (Wayland nativo, o que a automação não cobre)

Material: sessão Wayland (KDE ou GNOME), `apple2-rust-<versão>-linux-x64.tar.gz`.

| Campo | Valor |
|---|---|
| Distro e versão, compositor | |
| GPU e driver (NVIDIA proprietário?) | |
| Escala de tela (fracionária?) | |
| Data e executor | |

| # | Passo | Resultado |
|---|---|---|
| L1 | `tar xzf` e `./apple2-rust` dentro da pasta extraída, **sem** `WINIT_UNIX_BACKEND`, para usar Wayland nativo. | |
| L2 | Passos P1 a P14, especialmente P14: fechar a janela sob Wayland + NVIDIA **não pode** gerar SEGV. Depois: `coredumpctl list --since "5 min ago"` deve estar vazio para `applewin`. | |
| L3 | P10 com escala fracionária (125%, 150%) e inteira (200%). | |
| L4 | P12: minimizar e restaurar sob o compositor. | |
| L5 | `./install.sh` instala atalho e ícone no menu; o atalho abre o programa e o `.dsk` associa. | |
| L6 | Gamepad: `libudev` presente; conectar controle USB e usar (P9). | |

## Registro de resultados

Quando o dono preencher, copie o resumo (SO, versão, PASS/FAIL/BLOCKED por roteiro) para [../platform/relatorio-plataformas.md](../platform/relatorio-plataformas.md), trocando os BLOCKED correspondentes.
