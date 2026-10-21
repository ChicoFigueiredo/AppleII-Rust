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
| 9 | `applewin`: frontend (janela, barra, som, joystick, depurador na tela) | feito: todas as Cartas do AppleWin ligadas; diferenças em [docs/manual/12](docs/manual/12-diferencas-em-relacao-ao-applewin.md) |
| 10 | Manual em português, ajuda no programa (F1), CONTRIBUTING | feito |
| 11 | CI e empacotamento nas três plataformas | feito: CI e pacotes de release validados no Windows, Linux e macOS |

A lista honesta do que falta em relação ao AppleWin está em [docs/manual/12-diferencas-em-relacao-ao-applewin.md](docs/manual/12-diferencas-em-relacao-ao-applewin.md); o histórico de decisões fica em [docs/plano-final.md](docs/plano-final.md), [docs/pendencias.md](docs/pendencias.md) e [docs/adr](docs/adr).

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

O executável é `target/release/applewin` (`applewin.exe` no Windows). As versões publicadas (releases do GitHub) trazem arquivos prontos para as três plataformas, gerados pelo CI.

Recursos opcionais, para quem compila: `apple2-hostio` tem `serialport` (porta COM da Super Serial Card) e `pcap` (Ethernet bruta do Uthernet I), desligados por padrão.

### Como publicar uma versão

1. Aumente a versão em `[workspace.package]` do `Cargo.toml` da raiz.
2. Abra um pull request para `main` com `[release]` no título (sem distinguir maiúsculas), feito pelo dono do repositório.
3. Ao mesclar o PR, o workflow `Release` cria a tag `v<versão>` no commit de merge, gera os pacotes das três plataformas e publica a release (título e corpo do PR entram nas notas).

Pull requests sem `[release]` nunca publicam. Se a tag já existir, o workflow falha: aumente a versão. A execução manual (Actions > Release > Run workflow) é um dry run: só compila e guarda os artefatos.

## Executando

```sh
cargo run --release -p applewin                      # janela vazia, Disk II no slot 6
cargo run --release -p applewin -- -d1 meu.dsk -power-on
```

Ou o executável direto:

```sh
applewin -d1 dos33.dsk -d2 dados.dsk -power-on
applewin -model apple2p -s0 lc -50hz
applewin --headless-frames 300 --dump-screen boot.png   # sem janela, para testes
```

Todas as opções de linha de comando estão em [docs/manual/08-linha-de-comando.md](docs/manual/08-linha-de-comando.md). As configurações ficam em `settings.toml` no diretório de configuração do sistema (`~/.config/applewin-rs` no Linux).

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
| `applewin` | Frontend: janela, barra, som, joystick, ajuda, depurador na tela | `Windows/` |

O vocabulário do projeto está no [GLOSSARY.md](GLOSSARY.md).

## Testes

```sh
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
```

Os testes cobrem a CPU (suíte funcional de Klaus Dormann), o boot de cada Modelo até o prompt `]` ou `*`, os formatos de disco, o áudio, o depurador (com uma tela de referência) e os arquivos YAML. Os testes que medem tempo real do frontend (`crates/applewin/tests/threaded.rs`) devem ser rodados em `--release`.

## Contribuindo

Veja o [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença e créditos

GPL-2.0-or-later, como o AppleWin. O AppleII-Rust deve tudo ao trabalho da equipe do AppleWin: Tom Charlesworth, Michael Pohoreski, Nick Westgate, Linards Ticmanis, Glenn Jones, Stannev, Ken Wessen, Sheldon Simms, Oliver Schmidt e todos os colaboradores listados em [docs/manual/13-creditos-e-licenca.md](docs/manual/13-creditos-e-licenca.md). Agradecimentos também às equipes do VICE e do FUSE, de cujos módulos o AppleWin se vale, e a Klaus Dormann pela suíte de testes de CPU.

**Marcas.** Apple, Apple II, Apple //e e o logotipo da maçã colorida (usado no botão Ligar, como no AppleWin) são marcas da Apple Inc. Pravets, TK3000 e Base64A pertencem a seus respectivos donos. Este projeto não tem vínculo com nenhum deles. Os ícones em `assets/ui/` redesenham os bitmaps do AppleWin e seguem a mesma licença GPL.
