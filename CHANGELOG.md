# Changelog

## v0.2.0-rc.1 — 2026-10-21 (pré-lançamento)

**Release candidate para Linux x86_64, publicado como pré-lançamento de nível "alpha técnica" (parecer independente: 77/100, ver [docs/conformance/parecer-independente.md](docs/conformance/parecer-independente.md)); Windows e macOS sem verificação nativa.** Os pacotes de Windows e macOS são gerados pelo mesmo fluxo, mas só o Linux foi testado de ponta a ponta (o Windows também sob Wine, que não é Windows); o teste nativo dos dois fica a cargo do dono, pelo roteiro [docs/conformance/manual-tests/plataformas.md](docs/conformance/manual-tests/plataformas.md). Esta é a primeira versão depois da v0.1.0. O foco foi provar, contra o AppleWin, o que a v0.1.0 só afirmava, e corrigir o que a prova mostrou. É um pré-lançamento: leia "Divergências aprovadas" e "Limitações aceitas" abaixo e "Limitações conhecidas" no fim. Não há FAIL P0 nem P1 em issue do AppleWin. O relatório completo está em [docs/conformance/relatorio-final.md](docs/conformance/relatorio-final.md).

### Identidade própria: o programa se chama apple2-rust (ADR 0007)
- O executável passa a ser **`apple2-rust`** (`apple2-rust.exe` no Windows), assim como os crates (`apple2-rust`, `apple2-rust-dap`, `apple2-rust-web`), os pacotes (`apple2-rust-<versão>-<plataforma>.*`), a SBOM (`apple2-rust-<versão>.cdx.json`) e o diretório de configuração (`apple2-rust`). Não há alias `applewin`. O AppleWin segue como origem e oráculo, e a linha de comando, a importação de Registro/INI e o formato `.aws.yaml` continuam compatíveis.
- `cargo run` na raiz do repositório abre o emulador.
- Migração automática da configuração: na primeira execução, o `settings.toml` (e `recent_disks.txt`) do diretório antigo `applewin-rs` é copiado para o novo, com aviso na linha de status. O diretório antigo não é apagado.

### Conformidade com o AppleWin (novo)
- Uma suíte de conformidade (`conformance`) compara o programa com o AppleWin, num commit fixo, ciclo a ciclo. Resultados desta versão: trace de CPU idêntico nos Modelos ][, ][+, //e e //e Enhanced (até 5 milhões de ciclos, inclusive o boot do DOS 3.3) e mais de 10 milhões de vetores de hardware do 6502 e do 65C02; 1.573 telas idênticas pixel a pixel (todos os modos de vídeo e os 9 tipos de vídeo, NTSC e PAL, inclusive o Color Monitor PAL com floating bus, GH#241); mais de 103 milhões de instruções de disco idênticas, com a imagem final conferida; 88 de 115 save-states reais do AppleWin importados com o mesmo trace (0 divergem; 27 não puderam ser comparados); eventos de áudio idênticos, inclusive o IRQ do 6522 no último ciclo do opcode; 1.604.000 vetores e os testes ZEXDOC/ZEXALL do Z80.
- Suíte de 779 casos de conformidade (774 PASS, 5 FAIL de áudio que são Limitações aceitas) e 1.596 testes (0 falham).
- Matriz com as 1.115 issues do AppleWin e o estado de verificação de cada uma, todas com evidência ou justificativa: 489 PASS, 6 FAIL, 132 BLOCKED, 488 não aplicáveis, 0 não verificadas. Cada PASS registra o tipo de oráculo: 147 contra o AppleWin executado, 202 contra o C++ do AppleWin transcrito em teste, 128 de regressão, 7 com fixture real e 5 por especificação. Backlog importável e relatórios por área em `docs/conformance`.

### Corrigido
- **Save-states.** O primeiro evento de VBL do mouse depois de carregar um estado vinha 7 ciclos antes do AppleWin. Arquivos reais do AppleWin que travavam a emulação ao carregar (contador de ciclos do quadro acima do limite) agora carregam. Arquivos malformados ou hostis não derrubam mais o programa. A gravação do save-state é atômica: uma queda no meio não destrói mais o último save. O estado das placas RGB, o nome da impressora e as contrabarras de caminhos agora vão e voltam corretamente. A importação preserva o bit B do registrador P.
- **Saturn.** A quantidade de bancos configurada (64 K e 128 K) chega à carta e ao save-state; o banco ativo sobrevive ao power-cycle como no AppleWin.
- **Discos.** A gravação de imagens usa arquivo temporário e troca atômica (imagens `.gz` e `.zip` incluídas); uma escrita pendente não se perde mais ao mudar a configuração, carregar um estado ou receber SIGTERM/SIGINT; limites de tamanho e de descompressão protegem contra imagens malformadas; um bloco absurdo de disco rígido não pede mais 2 TB de memória.
- **Entrada.** Alt Gr segue o AppleWin: o caractere que o layout define com Alt Gr (como `/` no ABNT2 e `@` no alemão) chega ao Apple; a combinação sem caractere só chega com `-altgr-sends-wmchar` (GH#558, #583, #625). Corrigida a tecla que ficava presa ao entrar no depurador (F7) ou ao trocar o foco. Os atalhos de teclado do depurador do AppleWin (Espaço, Tab, setas, Ctrl+números, roda do mouse e outros) agora existem.
- **Depurador.** `SYMUSER LOAD` carrega arquivo sem aspas (GH#114), `SYMUSER SAVE` grava a tabela e avisa (GH#245), `SYM` aceita `#` no nome (GH#466), `CD ..` volta ao diretório pai (GH#468), `DA` prefere o símbolo exato (GH#1381) e Ctrl+Print Screen copia as 48 linhas do console (GH#458): são Divergências aprovadas, veja abaixo. Corrigidos travamentos com caracteres Unicode em comandos e intervalos invertidos que copiavam memória demais; os comandos que faltavam (`BENCHMARK`, `DISK`, `FONT`, `HCOLOR`, `NTSC`, `SOURCE`, `SYNC`) e a entrega dos breakpoints à Breakpoint card foram acrescentados.
- **CPU e memória.** O NMI deixa de chegar à CPU, como no AppleWin; o floating bus com `RAMRD` lê a RAM auxiliar, como no AppleWin; o padrão de RAM do power-on passa a ser o do AppleWin (`-memclear`, padrão 2); o Disk II usa o mesmo `rand()` do AppleWin nos bits aleatórios.
- **Vídeo.** O histórico de sinal do primeiro quadro passa a ser o do AppleWin, o que corrige 1 pixel de borda no Color Monitor PAL com floating bus (GH#241). O Super Hi-Res da VidHD desenha uma linha por linha como o AppleWin; o primeiro quadro em 80 colunas Color TV sai igual; o Color TV PAL com troca de modo no meio da linha (#1443) confere.
- **Áudio.** O Mockingboard e o Phasor têm um tipo de AY por soquete (padrão AY-3-8913) e as chaves `6522a-bad` e `6522b-bad`; o IFR do 6522 é visto no mesmo opcode que o AppleWin vê, e o reset do 6522 fecha os eventos de timer até o IER habilitar, como o AppleWin (GH#1176).
- **Disco rápido com som.** A velocidade de disco melhorada não acelera mais enquanto o alto-falante ou o Mockingboard tocam, como o AppleWin (GH#1340).
- **HDD.** O firmware do bloco de informações do SmartPort passa a 132.1, como o AppleWin `a8f0d1d`.
- **Cartas.** SNES MAX e 4Play recebem os controles do computador, com as opções `-snes-max-*`; conflitos de slot seguem o gerenciador do AppleWin; erros de rede (pcap), serial e impressora passam a aparecer na barra de status; Uthernet II recusa os modos sem suporte em vez de fingir; Z80 com IRQ/NMI/HALT no núcleo e relato dos opcodes ED indefinidos.
- **Configuração.** Discos inseridos, ejetados ou soltos na janela entram na configuração na hora (GH#98/#511/#960); `-s7-empty-on-exit` e os diretórios iniciais seguem o AppleWin; a SHIFT key mod volta ao padrão da família ao trocar de Modelo; `-no-joystick1|2` desliga o joystick emulado. A importação de `.reg` e INI do AppleWin lê o tipo de CPU; ROMs do usuário (`-rom`, `-f8rom`, `-videorom`) e `[roms]` no TOML, com fallback para a ROM embutida e aviso se o arquivo for inválido.

### Aplicativo
- F2 e o botão Run pedem confirmação antes de reiniciar (caixa única, como o AppleWin); o nome do screenshot, o título da janela com o modo de vídeo, o status dos drives em tela cheia e o ponteiro escondido em tela cheia seguem o AppleWin.
- Trocar o disco de um drive vazio ou ocupado vale na hora; "Open as Read Only", soltar imagem sobre o botão do drive, monitor de setores, importação de configuração pela interface, Alt+Enter configurável (`-alt-enter`) e "Benchmark Emulator" estão disponíveis.
- Ctrl+Print Screen copia o texto de 40 ou 80 colunas e a página 2.
- Manual: capítulos novos sobre o laboratório educacional (14), a depuração no VS Code (15) e a depuração temporal (16), todos dentro do F1.

### Divergências aprovadas pelo dono (ADR 0006, 21/10/2026)
Pontos em que o programa se comporta diferente do AppleWin (`a8f0d1d`) por decisão explícita do dono, porque o AppleWin tem ali um defeito de software que só prejudica o usuário. Nenhuma toca a emulação do hardware. Lista técnica, com o código do AppleWin e o teste de cada uma, em [docs/conformance/gaps-and-known-limitations.md](docs/conformance/gaps-and-known-limitations.md); em linguagem de usuário no [manual 12](docs/manual/12-diferencas-em-relacao-ao-applewin.md).
- **Impressora Pravets (GH#916):** a conversão de latim para cirílico sai em CP-1251 correto (as tabelas do AppleWin estão em UTF-8 e saem quebradas).
- **`DebuggerAutoRun.txt` (GH#855):** o script roda com a máquina pronta; `bsave` e `bpx` do script funcionam (no AppleWin rodam antes da memória existir).
- **Alt+F2 (GH#1396):** mantém o Open Apple apertado depois do reinício (o AppleWin o solta em `JoyReset`).
- **Tela cheia (GH#280, #358, #1374, #117, #932, #1448):** ocupa toda a altura do monitor, mantém 4:3 sem esticar em tela larga, usa o monitor onde a janela está e a velocidade "Fastest" vale em todos os Modelos; "Integer scaling" devolve a escala inteira do AppleWin.
- **Depurador (GH#114, #245, #458, #466, #468, #1381):** os seis comandos descritos em "Corrigido".
- Mantida a paridade com o AppleWin no depurador: GH#713 (aspas simples e duplas), GH#1145 (ordem de `APPLE2E.SYM`) e GH#1377 (4 das 9 tabelas na partida). Adiadas: GH#1426 e GH#1431 (mini dump `M` do Language Card por banco ou slot).

### Limitações aceitas (ADR 0006, 21/10/2026)
Continuam FAIL nos relatórios e no harness, com a causa registrada, e nunca são contadas como PASS. O dono decidiu que não bloqueiam este pré-lançamento.
- **PCM do Mockingboard/Phasor e do alto-falante, e o primeiro fonema do SSI263 (5 casos):** `audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm` e `audio-ssi263-phonemes-p333`. Os eventos de 6522, AY e SSI263 são idênticos aos do AppleWin, ciclo a ciclo; as amostras diferem porque o AppleWin gera o som em fatias de tempo do computador onde roda. Na prática o som é igual ao ouvido, mas não é bit a bit.

### Novo (extensões que o AppleWin não tem)
- **Depuração temporal (rewind)**: voltar instruções no depurador (`apple2-rewind`).
- **Adaptador DAP** (`apple2-rust-dap`) para depurar programas 6502 no VS Code.
- **Laboratório educacional** (`apple2-lab`): explica instruções, rastreia programas e corrige exercícios de Assembly sem ROM da Apple.
- **Protótipo web/WASM** (opt-in): o núcleo compila para WebAssembly sem mudanças.
- **Benchmarks determinísticos** (`apple2-bench`) com baseline de desempenho.

### Distribuição
- O workflow de release trata versões com sufixo (como `-rc.1`) como pré-lançamento, confere formatação, lints e testes nos três sistemas antes de publicar, e anexa `SHA256SUMS`, SBOM CycloneDX e as licenças de terceiros. O texto da release marca os pacotes de Windows e macOS como não verificados nativamente. Os pacotes continuam **sem assinatura** (Authenticode) e **sem notarização** (macOS). O `Info.plist` do macOS usa `CFBundleVersion` e `CFBundleShortVersionString` numéricos (`0.2.0`), como a Apple exige, e guarda a versão completa em `AppleIIRustFullVersion`.

### Limitações conhecidas
Resumo; a lista completa, com causa e prova, está em [docs/conformance/gaps-and-known-limitations.md](docs/conformance/gaps-and-known-limitations.md) e o relatório em [docs/conformance/relatorio-final.md](docs/conformance/relatorio-final.md).
- **Windows e macOS: sem verificação nativa (pendentes).** Só Linux (nativo) e Windows sob o Wine passaram no smoke; os pacotes dos dois são gerados, mas não verificados. Gamepad, serial, rede real, impressora e som audível também não foram testados em hardware.
- **Áudio (Limitação aceita):** a forma de onda (PCM) do Mockingboard/Phasor e do alto-falante e o primeiro fonema do SSI263 numa das fases diferem do AppleWin (5 casos, por causa do fatiamento de tempo do AppleWin); os eventos são idênticos.
- **Save-states reais:** 27 dos 115 arquivos de teste não puderam ser comparados (imagens ausentes do corpus, rede, hora real, Shift Key Mod, Z80); os outros 88 batem com o AppleWin.
- **Depurador:** cinco comportamentos que as issues do AppleWin pedem para mudar ficam como no AppleWin (GH#713, #1145, #1377 por decisão do dono; GH#1426 e #1431 adiadas).
- **Diferenças a favor do usuário:** são Divergências aprovadas pelo dono (seção acima), não pendências.
- **Interface:** o AppleWin mostra o cursor de espera ao carregar um save-state grande (#781); o programa ainda não.
- **Cópia do AppleWin, não do hardware:** ciclos "fantasma", RMB/SMB/BBR/BBS/WAI/STP como NOP, floating bus com `RAMRD`, NMI não tratado e Z80 sem interrupções na carta. O desvio do hardware real é o do AppleWin.
- **Não verificado ou bloqueado:** 133 issues do AppleWin estão BLOCKED (software de terceiros sem fixture, comportamento de janela sem Oráculo, hardware); 7 delas são P0 (#4, #70, #400, #422, #564, #640, #1126). Vídeo dos clones (Pravets, TK3000, Base64A, J-Plus) sem comparação de imagem; janela de configuração e atalhos F1 a F12 só têm testes próprios.
- **Windows nativo:** uma execução do dono no Windows 11 (roteiro W1 a W10) está em andamento; o resultado não faz parte desta versão.
- A nota de autoavaliação do conjunto é 84/100 (Gates: G0 PASS, G1 BLOCKED, G2 FAIL, G3 BLOCKED); a recomendação objetiva é alpha técnica e a revisão independente ainda não foi feita. **Isto é um release candidate para Linux, ainda pré-lançamento; não use como versão estável.** O que falta para o Gate G3 (escopo Linux), o parecer independente, está em `docs/conformance/release-readiness.md`.

## v0.1.0 — 2026-10-20

Primeira versão: reescrita em Rust do [AppleWin](https://github.com/AppleWin/AppleWin), com paridade de funcionalidades em Windows, Linux e macOS. As diferenças conhecidas estão em [docs/manual/12-diferencas-em-relacao-ao-applewin.md](docs/manual/12-diferencas-em-relacao-ao-applewin.md).

### Emulação
- CPU 6502 (incluindo os opcodes não documentados do AppleWin) e 65C02, com tempo por ciclo; passa nos testes de Klaus Dormann e nos testes do próprio AppleWin.
- Todos os modelos do AppleWin: II, II+, J-Plus, IIe, IIe Enhanced, Pravets 82/8M/8A, TK3000 //e e Base64A (IIc apenas como esqueleto, igual ao AppleWin).
- Memória //e completa: Language Card, Saturn 128K, 80 colunas, Extended 80 colunas e RamWorks III; floating bus e contador de vídeo NTSC/PAL.
- Vídeo: todos os modos (texto 40/80, LORES, DLORES, HIRES, DHIRES, misto), os 9 tipos de vídeo do AppleWin, cartas RGB e Super Hi-Res da VidHD.
- Disco: Disk II (DSK, DO, PO, NIB, WOZ1/2, 2MG, zip/gz) com gravação, disco rígido e SmartPort.
- Som: alto-falante, Mockingboard C, Phasor, MegaAudio, SD Music, SSI263, SC-01 e SAM.
- Cartas: mouse, Z80 SoftCard, relógios (No-Slot Clock, ThunderClock Plus), Super Serial Card, impressora, Uthernet I e II, 4Play, SNES MAX, VidHD, Breakpoint card e dongles de proteção.

### Aplicativo
- Janela com barra de ferramentas e atalhos do AppleWin (F1–F12), tela de configuração completa, menus de drive e criação de discos.
- Ícones SVG redesenhados a partir dos bitmaps do AppleWin.
- Depurador do AppleWin (F7): disassembler, assembler, símbolos, breakpoints e comandos.
- Save-states no formato `.aws.yaml` do AppleWin (F11/F12).
- Linha de comando compatível com a do AppleWin; configuração em TOML, com importação do registro/INI do AppleWin.
- Joysticks em todos os modos do AppleWin, teclado por layout do sistema, colagem de texto.
- Manual em português no próprio programa (F1).

### Distribuição
- Pacotes para Windows x64, Linux x64 e macOS universal, com os discos do DOS 3.3 e do ProDOS.
