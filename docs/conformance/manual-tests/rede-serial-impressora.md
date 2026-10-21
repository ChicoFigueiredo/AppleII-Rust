# Roteiros manuais: serial, rede (pcap) e impressora

- Escopo: HU-0701 (#31) e HU-0703 (#33). Achados: AUD-P-023, 030, 031, 032 e 033 de `docs/conformance/audit/wave0-peripherals.md`.
- Estado de todos os roteiros: **BLOCKED** até o dono executar em hardware real e preencher os campos de resultado. Este ambiente não tem adaptador USB-serial, Npcap nem acesso a Windows e macOS.
- Os testes automáticos que cobrem a parte sem hardware estão em `crates/apple2-hostio` (`printer.rs`, `serial.rs`, `pcap_backend.rs`, `interfaces.rs`, `tests/uthernet2.rs`) e em `crates/apple2-rust/tests/host_io_errors.rs`.
- Os testes do AppleWin de referência (C++ `a8f0d1d`) servem de comparação: o AppleWin não mostra nenhum destes erros ao usuário, só grava no log. Aqui a mensagem aparece na barra de status e em `stderr` (prefixo `warning:`), e a emulação continua com a carta desconectada.

Como preencher: cada roteiro termina com uma tabela. Marque o resultado como `PASS`, `FAIL` ou `N/A` e anote qualquer desvio em "Observações". Para `FAIL`, guarde o texto exato da mensagem.

Preparação comum:

1. Compile com os recursos necessários: `cargo build --release -p apple2-rust --features pcap` (o recurso `serialport` já vem ligado no frontend). No Windows, o Npcap SDK precisa estar instalado para compilar com `pcap`; no Linux, `libpcap-dev`; no macOS a libpcap já vem com o sistema.
2. Rode a partir de um terminal, para ver o `stderr`.
3. Para o AppleWin de comparação, use o AppleWin 1.32.1.0 (Windows, ou Wine no Linux).

---

## R1. Serial real (adaptador USB-serial)

Material: um adaptador USB-serial (FTDI, CH340 ou PL2303) com laço de teste (TX ligado ao RX, pinos 2 e 3 do DB-9) ou ligado a um segundo adaptador/PC em modo terminal a 9600 8N1.

### R1.1 Linux

Usuário no grupo `dialout` (ou `uucp`): `groups`. Se não estiver, o passo 6 mostra o erro de permissão.

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Sem o adaptador ligado, abra Configuração > Cartas > slot 2 > Super Serial Card > Opções. | A lista de portas não mostra `ttyUSB*`/`ttyACM*`. Portas `ttyS*` aparecem só se houver UART real (`ls /sys/class/tty/ttyS0/device`). |
| 2 | Ligue o adaptador e reabra a janela de opções. | `/dev/ttyUSB0` (ou `ttyACM0`) aparece na lista. |
| 3 | Selecione a porta, aplique e reinicie. No emulador: `PR#2` e `PRINT "OLA"`. | Com o laço, o texto volta pela serial (`IN#2` / `INPUT A$`). Com terminal no outro lado, "OLA" aparece lá. |
| 4 | Pareie um dispositivo Bluetooth por RFCOMM (`sudo rfcomm bind 0 <MAC>`) e reabra a janela. | `/dev/rfcomm0` aparece na lista. (Opcional.) |
| 5 | Com a porta aberta pelo emulador, abra a mesma porta em `minicom`/`screen` num segundo terminal e inicie outro `apple2-rust` com o mesmo `-s2 ssc` e a mesma porta. | O segundo processo mostra na barra de status e no `stderr` `SSC: cannot open serial port '/dev/ttyUSB0': ...` (porta ocupada). O emulador continua rodando e a carta fica sem porta. |
| 6 | `sudo chmod 000 /dev/ttyUSB0` (ou saia do grupo `dialout`), inicie o emulador com a porta selecionada. | Mensagem `SSC: cannot open serial port '/dev/ttyUSB0': ...` com a causa (`Permission denied`). Sem travar nem fechar. Restaure com `sudo chmod 660`. |
| 7 | Retire o cabo USB com o emulador rodando e use `PR#2`. | Nenhum travamento. Se aparecer erro de escrita, ele é descrito. |

### R1.2 Windows

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Sem o adaptador: abra as opções do SSC. | Só portas COM que existem (por exemplo `COM1` da placa-mãe, se houver). Não deve listar `COM1` a `COM9` fixos. |
| 2 | Ligue o adaptador (veja o número no Gerenciador de Dispositivos). | A nova `COMn` aparece na lista, inclusive acima de `COM9`. |
| 3 | Selecione-a, reinicie, `PR#2` / `PRINT "OLA"`. | Mesmo resultado do passo 3 do Linux. |
| 4 | Abra a mesma COM no PuTTY (porta ocupada) e inicie o emulador. | `SSC: cannot open serial port 'COMn': ...` com "Access is denied" ou equivalente, sem travar. |
| 5 | Selecione uma COM de adaptador que foi desconectado (nome salvo na configuração). | `... cannot open serial port 'COMn': ...` ("file not found"). |

### R1.3 macOS

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Sem o adaptador: abra as opções do SSC. | Sem `cu.usb*`/`tty.usb*`. Portas internas como `cu.Bluetooth-Incoming-Port` podem aparecer. |
| 2 | Ligue o adaptador (driver FTDI/CH340 instalado). | `/dev/cu.usbserial-XXXX` (e `/dev/tty.usbserial-XXXX`) aparece. |
| 3 | Selecione `cu.usbserial-XXXX`, reinicie, `PR#2` / `PRINT "OLA"`. | Mesmo resultado do passo 3 do Linux. |
| 4 | Abra a porta em `screen /dev/cu.usbserial-XXXX 9600` em outro terminal e inicie o emulador com a mesma porta. | Erro descritivo de porta ocupada (`Resource busy`), emulação continua. |

### Preencher (R1)

| Sistema | Versão do SO | Versão do emulador (commit) | Adaptador | Data | Resultado | Observações |
|---|---|---|---|---|---|---|
| Linux | | | | | | |
| Windows | | | | | | |
| macOS | | | | | | |

---

## R2. Rede com pcap (Uthernet I e II)

Material: máquina com uma interface Ethernet ou Wi-Fi ativa. Para o teste de tráfego, uma segunda máquina na mesma rede (ou o roteador) e `ping`.

Configuração: slot 3 (ou outro) com Uthernet II, interface escolhida na lista; para o Uthernet I, o mesmo.

### R2.1 Linux (libpcap)

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Compile com `--features pcap`. Abra as opções do Uthernet. | A lista mostra as interfaces do libpcap (`eth0`, `wlan0`, `any`, `lo`...), cada uma com descrição. |
| 2 | Compile **sem** `--features pcap` e abra as mesmas opções. | A lista ainda aparece (via `if-addrs`), com os endereços IP como descrição. Ao aplicar uma interface, a barra de status avisa que a rede precisa do recurso `pcap`. |
| 3 | Como usuário comum, sem `CAP_NET_RAW`, selecione `eth0` e reinicie. | Mensagem `Uthernet: cannot open network interface 'eth0': ... (capturing needs root or CAP_NET_RAW; ...)`. A emulação continua; o DNS virtual e os sockets TCP/UDP do Uthernet II continuam funcionando. |
| 4 | `sudo setcap cap_net_raw,cap_net_admin=eip target/release/apple2-rust` e repita. | A interface abre. Sem mensagem de erro. |
| 5 | Rode um programa de rede no Apple (por exemplo o `ping` do pacote de utilitários do Uthernet II, ou ADT Pro com o driver Uthernet) para um host da rede. | Resposta recebida. No host: `sudo tcpdump -i eth0 ether host <MAC do Apple>` mostra os quadros. |
| 6 | Digite um nome de interface inexistente no arquivo de configuração e inicie. | `Uthernet: network interface 'xxx' does not exist`. |

### R2.2 Windows (Npcap)

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Com o Npcap instalado (modo normal ou WinPcap-compatível), compile com `pcap` e abra as opções. | As interfaces aparecem com a descrição do adaptador (`Intel(R) Ethernet...`) e o nome `\Device\NPF_{GUID}`. |
| 2 | Sem o Npcap instalado, rode o binário compilado com `pcap`. | O programa não abre por falta de `wpcap.dll`, ou mostra `cannot list network interfaces with libpcap/Npcap`. Anote o comportamento exato: se o executável nem inicia, registre como FAIL e abra um achado novo. |
| 3 | Com o Npcap, sem a opção "Support raw 802.11 traffic" e sem administrador, selecione o adaptador. | Se o Npcap exige administrador na instalação, aparece `cannot open network interface ...` com a causa. Caso abra, siga o passo 4. |
| 4 | Rode o mesmo teste de `ping` do R2.1 passo 5. | Resposta recebida; Wireshark no adaptador mostra os quadros. |
| 5 | Selecione o adaptador Wi-Fi. | O Wi-Fi não aceita MAC próprio em muitos drivers: anote se o tráfego passa. O AppleWin tem a mesma limitação. |

### R2.3 macOS (libpcap)

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Compile com `--features pcap` e abra as opções. | Interfaces `en0`, `en1`, `lo0` etc. com descrição. |
| 2 | Sem acesso a `/dev/bpf*` (usuário comum sem ChmodBPF), selecione `en0` e reinicie. | `Uthernet: cannot open network interface 'en0': ...` com `permission denied` e a dica de privilégio. Emulação continua. |
| 3 | Com permissão de BPF (`sudo chmod o+rw /dev/bpf*` ou Wireshark ChmodBPF), repita. | A interface abre; o teste de `ping` do R2.1 passa em Ethernet. |

### Preencher (R2)

| Sistema | Versão do SO | Versão libpcap/Npcap | Versão do emulador (commit) | Interface | Data | Resultado | Observações |
|---|---|---|---|---|---|---|---|
| Linux | | | | | | | |
| Windows | | | | | | | |
| macOS | | | | | | | |

---

## R3. Impressora para arquivo

Material: nenhum hardware. Em Windows, uma impressora real só se for testar "Dump to printer".

Configuração: slot 1 com Generic Printer. Para imprimir: `PR#1` e `LIST` ou `PRINT "TESTE"` (em seguida `PR#0`). O arquivo é fechado depois de 10 "segundos" de inatividade do Apple (10 x 710000 ciclos) ou no reset.

| # | Passo | Resultado esperado |
|---|---|---|
| 1 | Sem caminho configurado, imprima `TESTE`. | O arquivo `Printer.txt` aparece no diretório atual (o diretório de onde o emulador foi iniciado, ou o de `-current-dir`). Conteúdo `TESTE`. |
| 2 | Configure o nome `saida/lista.txt` (relativo; o diretório `saida` existe) e imprima. | O arquivo é criado em `<diretório atual>/saida/lista.txt`. |
| 3 | Configure `saida/../lista2.txt` e imprima. | Cai em `<diretório atual>/lista2.txt` (o `..` é resolvido; o caminho final é mostrado nas mensagens de erro, se houver). |
| 4 | Configure um caminho em diretório inexistente (`nao_existe/x.txt`) e imprima. | Barra de status e `stderr`: `Printer: cannot open printer file '.../nao_existe/x.txt': ...`. O Apple continua rodando (`PRINT` não trava). |
| 5 | Configure um diretório (por exemplo `.` ou `saida/`) como destino. | `Printer: '.../saida' is a directory, not a file`. Nenhum arquivo é criado. |
| 6 | Deixe o campo vazio. | Usa `Printer.txt` (o padrão), sem erro. |
| 7 | Em Linux/macOS: `mkdir ro && chmod 555 ro` e configure `ro/x.txt`. | `Printer: cannot open printer file '.../ro/x.txt': ... Permission denied`. Em Windows: pasta com permissão de escrita negada (Propriedades > Segurança) tem o mesmo efeito. |
| 8 | Modo "Append": marque, imprima duas vezes (com reset entre elas) e depois desmarque e imprima de novo. | Com Append as duas impressões ficam no arquivo; sem Append o arquivo é recriado. |
| 9 | Com um caminho personalizado (`C:\Users\Nome Com Espaco\print outs\job.txt` no Windows, `/tmp/meu dir/job.txt` em Linux/macOS), imprima, salve o estado (pelo menu), feche o emulador, abra e carregue o estado. Imprima de novo. | Depois do carregamento a saída continua indo para o mesmo arquivo personalizado (não para `Printer.txt`). O arquivo `.aws.yaml` traz `Print Filename:` com o caminho (barras invertidas dobradas no Windows). |
| 10 | Disco cheio (opcional): aponte o arquivo para um volume cheio (`/dev/full` no Linux) e imprima. | `printer file '/dev/full': write failed: ...` na barra de status ao fechar o trabalho. |
| 11 | Windows, opcional: marque "Dump to printer" e imprima. | O arquivo é enviado para a impressora padrão por `copy ... prn` ao fechar. |

### Preencher (R3)

| Sistema | Versão do SO | Versão do emulador (commit) | Data | Resultado | Observações |
|---|---|---|---|---|---|
| Linux | | | | | |
| Windows | | | | | |
| macOS | | | | | |
