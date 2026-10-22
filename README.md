# AppleII-Rust

Reescrita em Rust do emulador de Apple II [AppleWin](https://github.com/AppleWin/AppleWin), com o objetivo de ter o mesmo comportamento em **Windows, Linux e macOS**. O núcleo da Máquina é headless e determinístico (sem janela, áudio nem relógio do computador), e o frontend usa `winit` + `wgpu` + `egui` + `cpal` + `gilrs`, sem dependência de SDL.

É uma obra derivada do AppleWin, distribuída sob a **GPL-2.0-or-later** (veja [LICENSE](LICENSE)).

Repositórios: [GitHub](https://github.com/ChicoFigueiredo/AppleII-Rust) · [GitLab](https://gitlab.com/chico-figueiredo/opensourcechico/AppleII-Rust)

## O que é emulado

- Modelos: Apple ][, ][+, ][ J-Plus, //e, //e Enhanced e os clones Pravets 82/8M/8A, Base64A e TK3000 //e. O Apple I está fora do escopo até decisão explícita do dono (ver [ADR 0005](docs/adr/0005-apple-i-fora-do-escopo-ate-decisao.md)).
- Vídeo: texto, lo-res, hi-res, double lo-res/hi-res, 80 colunas, monitores monocromáticos, NTSC, PAL e placas RGB.
- Memória: Language Card, Saturn 128K, cartas AUX (80 colunas, Extended 80 colunas, RamWorks III).
- Discos: Disk II (DSK, DO, PO, NIB, WOZ, 2MG, gzip/zip) e disco rígido (ProDOS, SmartPort).
- Cartas: Mockingboard C, Phasor, SAM, SSC, impressora, mouse, Z80 SoftCard, relógios, Uthernet I e II, VidHD, 4Play, SNES MAX e dongles.
- Depurador, save-states (YAML do AppleWin), captura de tela e colagem de texto.

## Estado do projeto

| Fase | Conteúdo | Estado |
|------|----------|--------|
| 0 | Repositório, workspace, licença, ROMs, remotes GitHub/GitLab | feito |
| 1 | `apple2-cpu`: 6502 NMOS e 65C02 por ciclo, IRQ/NMI, decimal | feito (testes de Klaus Dormann) |
| 2 | `apple2-core`: Máquina headless, memória, slots, teclado, alto-falante, porta de jogos | feito (todos os Modelos com ROM dão boot) |
| 3 | `apple2-video`: todos os modos e monitores | feito |
| 4 | `apple2-storage`: Disk II, imagens, HDD/SmartPort | feito |
| 5 | `apple2-audio`: alto-falante, Mockingboard/Phasor, SSI263, SAM | feito |
| 6 | `apple2-cards` e `apple2-hostio`: mouse, Z80, relógios, 4Play, SNES MAX, VidHD, SSC, impressora, Uthernet | feito |
| 7 | `apple2-config` e `apple2-savestate`: configuração, linha de comando, YAML do AppleWin | feito |
| 8 | `apple2-debugger`: depurador completo sem interface | feito |
| 9 | `apple2-rust`: frontend (janela, barra, som, joystick, depurador na tela) | feito: todas as Cartas do AppleWin ligadas; diferenças em [docs/manual/12](docs/manual/12-diferencas-em-relacao-ao-applewin.md) |
| 10 | Manual em português, ajuda no programa (F1), CONTRIBUTING | feito |
| 11 | CI e empacotamento nas três plataformas | feito: CI e pacotes de release existem para Windows, Linux e macOS. Um CI verde não prova paridade com o AppleWin (veja "Conformidade e auditoria") |

A lista honesta do que falta em relação ao AppleWin está em [docs/manual/12-diferencas-em-relacao-ao-applewin.md](docs/manual/12-diferencas-em-relacao-ao-applewin.md); o histórico de decisões fica em [docs/plano-final.md](docs/plano-final.md), [docs/pendencias.md](docs/pendencias.md) e [docs/adr](docs/adr).

## Conformidade e auditoria

"Feito" na tabela acima quer dizer que o código existe, não que a paridade com o AppleWin foi demonstrada. A verificação é medida contra o AppleWin num commit fixo, e cada item tem um Estado de verificação (`PASS` só com evidência). O índice, o baseline, as limitações conhecidas e a prontidão para release estão em [docs/conformance/README.md](docs/conformance/README.md).

**Estado de conformidade (21/10/2026, release candidate para Linux `0.2.0-rc.1`, pré-lançamento).** A comparação executável é com o **fork do audetto (`a1e20c0`), cujo núcleo AppleWin é o `3e8054b`, 22 commits antes do `a8f0d1d`, com os patches `audetto-trace`, `audetto-audio`, `audetto-snesmax`, `audetto-ssc-rom` e `audetto-version`**; o `a8f0d1d` é a referência de código (testes que citam arquivo:linha). Contra esse fork: trace de CPU idêntico em quatro Modelos e mais de 10 milhões de vetores de hardware; 1.573 telas idênticas pixel a pixel, inclusive o Color Monitor PAL (#241); mais de 103 milhões de instruções de disco idênticas; 88 de 115 save-states reais importados com o mesmo fluxo de CPU em 100.000 ciclos (memória e vídeo não entram; 0 divergem; 27 não puderam ser comparados); eventos de áudio idênticos. Das 1.115 issues do AppleWin, 486 estão em PASS, 6 em FAIL (depurador e cursor de espera), 134 BLOCKED e 489 não se aplicam; não há FAIL P0 nem P1. **Os 486 PASS não valem o mesmo:** 147 comparam com o AppleWin rodando, 199 são testes que transcrevem o C++ do `a8f0d1d` com arquivo:linha, 5 são especificação (a2audit, vetores), 7 são fixture real e 128 são testes do próprio port. Por decisão do dono (ADR 0006), o PCM do Mockingboard e do alto-falante (5 casos) é **Limitação aceita** e continua FAIL nos relatórios (FAIL aceito, nunca PASS), e há **Divergências aprovadas** em relação ao AppleWin (impressora Pravets, `DebuggerAutoRun`, Alt+F2, tela cheia e seis comandos do depurador). Windows e macOS nativos e o hardware físico não foram verificados (BLOCKED; o teste nativo no Windows está em andamento). A nota de autoavaliação é 84 de 100; o [parecer independente](docs/conformance/parecer-independente.md) deu 77 e a recomendação objetiva é **alpha técnica** (o rótulo de release candidate para Linux, em pré-lançamento, é do dono); não é uma versão estável. Tudo, com provas e limitações, está no [relatório final de conformidade](docs/conformance/relatorio-final.md) e em [lacunas e limitações conhecidas](docs/conformance/gaps-and-known-limitations.md).

## Baixar e compilar

Instale o [Rust](https://rustup.rs) (versão estável recente) e o Git. Clone o repositório:

```sh
git clone https://github.com/ChicoFigueiredo/AppleII-Rust.git
cd AppleII-Rust
```

### Linux

Instale as dependências do sistema (Debian e Ubuntu):

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
```

Em outras distribuições, instale os pacotes equivalentes de desenvolvimento do ALSA, udev, xkbcommon e Wayland. É preciso também um driver Vulkan (Mesa ou o da placa de vídeo).

```sh
cargo build --release
```

### Windows

Instale o Rust com o toolchain MSVC (Visual Studio Build Tools com "Desktop development with C++"). Depois:

```sh
cargo build --release
```

### macOS

Instale as ferramentas de linha de comando (`xcode-select --install`) e o Rust. Depois:

```sh
cargo build --release
```

O executável é `target/release/apple2-rust` (`apple2-rust.exe` no Windows). No Windows, o `apple2-rust.exe` de release é um programa gráfico (não abre console); para usar no terminal com `>` ou `|`, há o `apple2-rust-console.exe`, o mesmo programa no subsistema de console, que o pacote traz como `apple2-rust.com`. As versões publicadas (releases do GitHub) trazem arquivos prontos para as três plataformas, gerados pelo CI.

### Dependências opcionais por sistema

O executável publicado não precisa de nada além do sistema: ROMs, símbolos e o manual estão embutidos. Estes recursos são opcionais:

| Recurso | Para quê | Estado no binário da release | Linux | Windows | macOS |
|---|---|---|---|---|---|
| `serialport` (feature de `apple2-rust`, ligada por padrão) | porta serial real na Super Serial Card | incluído | sem biblioteca extra para usar; o usuário precisa estar no grupo `dialout` (ou `uucp`) | portas COM; sem biblioteca extra | `/dev/cu.*`; sem biblioteca extra |
| `pcap` (feature opcional, desligada) | Ethernet bruta do Uthernet I | **não incluído** (compile com `--features pcap`) | `libpcap-dev` para compilar; `libpcap` e privilégio de captura (root ou `setcap cap_net_raw,cap_net_admin`) para usar | Npcap (instalador, modo compatível com WinPcap) e o Npcap SDK para compilar; WinPcap antigo não serve | libpcap já vem no sistema; permissão de acesso a `/dev/bpf*` |
| Uthernet II com sockets TCP e UDP | rede sem pcap | incluído | nada | nada | nada |
| Gamepad | joystick do Apple II pelo controle | incluído (gilrs) | `libudev` | XInput/DirectInput do sistema | IOKit do sistema (não testado) |
| Áudio | alto-falante, Mockingboard | incluído (cpal) | ALSA (`libasound2`); sem dispositivo o programa segue mudo | WASAPI | CoreAudio |

Sem dispositivo de áudio ou sem gamepad o programa continua funcionando e avisa em `stderr`. O Wine não implementa o gamepad. Ao compilar com `pcap` no Windows, o Npcap precisa estar instalado também na máquina de quem executa.

Os pacotes **não são assinados** (Authenticode no Windows) nem notarizados (macOS); veja [docs/conformance/platform/assinatura.md](docs/conformance/platform/assinatura.md).

### Como publicar uma versão

1. Aumente a versão em `[workspace.package]` do `Cargo.toml` da raiz.
2. Abra um pull request para `main` com `[release]` no título (sem distinguir maiúsculas), feito pelo dono do repositório.
3. Ao mesclar o PR, o workflow `Release` cria a tag `v<versão>` no commit de merge, gera os pacotes das três plataformas e publica a release (título e corpo do PR entram nas notas).

Pull requests sem `[release]` nunca publicam. Se a tag já existir, o workflow falha: aumente a versão. A execução manual (Actions > Release > Run workflow) é um dry run: só compila e guarda os artefatos.

Regras da release:

- Versão com sufixo SemVer (`0.2.0-alpha.1`, `1.0.0-rc.1`) publica como **pre-release**; só `X.Y.Z` puro é release estável.
- Antes de qualquer build, o job `verify` roda `cargo fmt --check`, `clippy -D warnings` e `cargo test --workspace --release` em Linux, Windows e macOS. Se algum falhar, nada é publicado.
- A release traz os pacotes, o `SHA256SUMS`, a SBOM CycloneDX (`apple2-rust-<versão>.cdx.json`) e, dentro de cada pacote, `THIRD-PARTY-LICENSES.html`.
- Para conferir um download: `sha256sum --check --ignore-missing SHA256SUMS`.

Reprodução local do pacote Linux (sem CI): `tools/package/linux.sh` gera o pacote, a SBOM e o `SHA256SUMS` em `dist/` e testa o pacote extraído num diretório temporário. Requer `cargo install cargo-about --features cli cargo-cyclonedx`.

## Executando

```sh
cargo run --release -p apple2-rust                      # janela vazia, Disk II no slot 6
cargo run --release -p apple2-rust -- -d1 meu.dsk -power-on
```

Ou o executável direto:

```sh
apple2-rust -d1 dos33.dsk -d2 dados.dsk -power-on
apple2-rust -model apple2p -s0 lc -50hz
apple2-rust --headless-frames 300 --dump-screen boot.png   # sem janela, para testes
```

Todas as opções de linha de comando estão em [docs/manual/08-linha-de-comando.md](docs/manual/08-linha-de-comando.md). As configurações ficam em `settings.toml` no diretório de configuração do sistema (`~/.config/apple2-rust` no Linux; quem usava o nome antigo `applewin-rs` tem o `settings.toml` copiado na primeira execução, sem apagar o original).

## Teclado

| Tecla | Ação |
|---|---|
| F1 | Abre o manual no programa |
| F2 | Liga ou reinicia; Ctrl+F2 ou Ctrl+Pause = Ctrl+Reset |
| F3, F4 | Escolhe o disco do drive 1 ou 2; Ctrl+F3 e Ctrl+F4 abrem o menu do drive |
| F5 | Troca os discos dos drives |
| F6 | Tela cheia (também Alt+Enter); Ctrl+F6 alterna janela 1x e 2x |
| F7 | Depurador; Ctrl+F11 dá um passo |
| F8 | Configuração |
| F9, Shift+F9 | Próximo e anterior modo de vídeo; Ctrl+Shift+F9 liga e desliga as linhas de varredura |
| F10 | Chave da ROM de vídeo (//e) ou Caps Lock do Pravets 8A |
| F11, F12 | Salva e carrega o estado |
| Pause | Pausa |
| Scroll Lock | Velocidade máxima |
| Print Screen | Captura a tela em PNG |
| Shift+Insert, Ctrl+Shift+V | Cola o texto da área de transferência (Ctrl+V vai para o Apple, como no AppleWin) |
| Alt esquerdo, Alt direito | Open Apple e Solid Apple (botões 0 e 1) |

O manual completo está em [docs/manual](docs/manual/00-indice.md) (em português), e também dentro do programa com F1.

## Arquitetura

Workspace Cargo com um crate por responsabilidade. O núcleo não faz E/S do computador (ADR 0001): arquivos, rede, serial e relógio entram por traits implementadas nos crates de fora.

| Crate | Conteúdo | Fonte AppleWin |
|---|---|---|
| `apple2-cpu` | 6502 e 65C02 | `CPU/` |
| `apple2-core` | Máquina headless: memória, Language Card, slots, vídeo (estado), teclado, alto-falante, porta de jogos | `Memory`, `LanguageCard`, `CardManager`, `Keyboard`, `Joystick` |
| `apple2-roms` | ROMs embutidas | `resource/`, `bin/` |
| `apple2-video` | Renderizador NTSC/RGB/mono | `NTSC`, `RGBMonitor`, `Video` |
| `apple2-storage` | Formatos de imagem, Disk II, HDD, SmartPort | `Disk*`, `Harddisk`, `ProDOS_*` |
| `apple2-audio` | Alto-falante, Mockingboard/Phasor, SSI263, SAM | `Speaker`, `Mockingboard*`, `SSI263`, `SAM` |
| `apple2-cards` | Mouse, Z80, relógios, 4Play, SNES MAX, VidHD, dongles | `MouseInterface`, `z80emu`, `FourPlay`, `SNESMAX`, `VidHD` |
| `apple2-hostio` | SSC, impressora, Uthernet I e II | `SerialComms`, `ParallelPrinter`, `Uthernet*` |
| `apple2-debugger` | Depurador sem interface | `Debugger/` |
| `apple2-config` | Configuração, linha de comando, importação do Registro | `CmdLine`, `Registry`, `Configuration/` |
| `apple2-savestate` | YAML `.aws.yaml` do AppleWin | `SaveState`, `YamlHelper` |
| `apple2-rust` | Frontend: janela, barra, som, joystick, ajuda, depurador na tela | `Windows/` |

O vocabulário do projeto está no [GLOSSARY.md](GLOSSARY.md).

## Testes

```sh
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
```

Os testes cobrem a CPU (suíte funcional de Klaus Dormann), o boot de cada Modelo até o prompt `]` ou `*`, os formatos de disco, o áudio, o depurador (com uma tela de referência) e os arquivos YAML. Os testes que medem tempo real do frontend (`crates/apple2-rust/tests/threaded.rs`) devem ser rodados em `--release`.

## Contribuindo

Veja o [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença e créditos

GPL-2.0-or-later, como o AppleWin. O AppleII-Rust deve tudo ao trabalho da equipe do AppleWin: Tom Charlesworth, Michael Pohoreski, Nick Westgate, Linards Ticmanis, Glenn Jones, Stannev, Ken Wessen, Sheldon Simms, Oliver Schmidt e todos os colaboradores listados em [docs/manual/13-creditos-e-licenca.md](docs/manual/13-creditos-e-licenca.md). Agradecimentos também às equipes do VICE e do FUSE, de cujos módulos o AppleWin se vale, e a Klaus Dormann pela suíte de testes de CPU.

**Marcas.** Apple, Apple II, Apple //e e o logotipo da maçã colorida (usado no botão Ligar, como no AppleWin) são marcas da Apple Inc. Pravets, TK3000 e Base64A pertencem a seus respectivos donos. Este projeto não tem vínculo com nenhum deles. Os ícones em `assets/ui/` redesenham os bitmaps do AppleWin e seguem a mesma licença GPL.
