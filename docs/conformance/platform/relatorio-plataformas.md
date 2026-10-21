# Relatório de plataforma (HU-0901 e HU-0902)

Data: 2026-10-20. Commit base: `516c123` (worktree do agente, sem push). Nenhuma execução de CI foi acionada; toda a evidência é local.

Legenda: **PASS** = executado e conferido; **FAIL** = executado e falhou; **BLOCKED** = não executável aqui, com a causa. Wine não é Windows nativo: veja [windows-wine.md](windows-wine.md).

## Ambientes

| SO | Arquitetura | Toolchain | Dependências | Situação |
|---|---|---|---|---|
| Linux nativo (Ubuntu 26.04.1, kernel 7.0.0-38, KDE Wayland + XWayland, NVIDIA RTX 4080 SUPER) | x86_64 | Rust 1.98.1, `x86_64-unknown-linux-gnu` | libasound2, libudev, libxkbcommon, libwayland, Vulkan (driver do host) | executado |
| Windows (via Wine 10.0, prefixo próprio) | x86_64 | Rust 1.98.1, `x86_64-pc-windows-msvc` por `cargo-xwin` 0.23.1 | CRT/SDK Microsoft no cache; Vulkan do host via winevulkan | executado (Wine) |
| Windows nativo | x86_64 | — | — | **BLOCKED**: precisa de CI autorizada ou da máquina do dono (roteiro pronto) |
| macOS Intel e Apple Silicon | x86_64, arm64 | — | — | **BLOCKED**: sem hardware e sem SDK da Apple aqui (roteiro pronto) |

Nota: o host tem uma VM "Windows 11" do VirtualBox visível na sessão. Ela não foi usada (fora do escopo e sem autorização); se o dono quiser, é o caminho mais curto para o Windows nativo com o roteiro manual.

## HU-0901: funcionalidade nos três sistemas

| # | Critério | Linux | Windows (Wine) | Windows nativo | macOS |
|---|---|---|---|---|---|
| 1 | Build release | PASS | PASS (cruzado, msvc) | BLOCKED | BLOCKED |
| 2 | Headless 300 quadros, 5 modelos | PASS (rc=0) | PASS, PNG idêntico ao Linux | BLOCKED | BLOCKED |
| 3 | Boot do DOS 3.3 headless | PASS | PASS, PNG idêntico ao Linux | BLOCKED | BLOCKED |
| 4 | `--help` | PASS | PASS | BLOCKED | BLOCKED |
| 5 | Abrir GUI e fechar limpo, rc=0 | PASS (XWayland e Xvfb) | PASS | BLOCKED | BLOCKED |
| 6 | Áudio sem dispositivo | PASS | não testado | BLOCKED | BLOCKED |
| 7 | HiDPI (fator de escala 2) | PASS (`WINIT_X11_SCALE_FACTOR=2`, abre e fecha); aparência: manual | não testado | BLOCKED | BLOCKED (Retina) |
| 8 | Tela cheia (`-f`) | PASS (abre e fecha, rc=0) | não testado | BLOCKED | BLOCKED |
| 9 | Janela minimizada | BLOCKED (sem automação de minimizar sob KWin Wayland); roteiro manual | não testado | BLOCKED | BLOCKED |
| 10 | Wayland nativo: fechar sem SEGV | parcial: veja abaixo | n/a | n/a | n/a |
| 11 | Teclado regional (ABNT2, AZERTY) | BLOCKED (precisa de digitação real); roteiro manual | BLOCKED | BLOCKED | BLOCKED |
| 12 | Gamepad | BLOCKED (sem controle) | esperado indisponível: gilrs não implementado no Wine | BLOCKED | BLOCKED |
| 13 | Permissões (somente leitura, diretório de config) | roteiro manual | não testado | BLOCKED | BLOCKED (Gatekeeper) |
| 14 | Pacote da release, extrair, abrir | PASS (veja HU-0902) | BLOCKED (zip não gerado; o `.exe` foi testado solto) | BLOCKED | BLOCKED |

Detalhes do Linux (`tools/package/smoke-linux.sh`, todos PASS com rc=0): headless nos 5 modelos; GUI por XWayland; HiDPI x2; tela cheia (`-f`); áudio ausente; Xvfb.

- **Áudio ausente**: com `ALSA_CONFIG_PATH=/dev/null` o programa imprime `audio disabled: The requested device is no longer available...` e segue rodando, abre e fecha com rc=0.
- **Coredump**: `coredumpctl list --since today` lista SIGSEGV de `target/debug/applewin` às 07:36, 07:40 e 11:26 (anteriores a esta sessão) e dois processos que não são o `applewin` (fuzz_parsers, audetto-oracle). **Nenhum coredump novo do `applewin` release** durante estes testes.
- **Wayland nativo**: o fechamento por protocolo (`xdg_toplevel.close`) não é automatizável aqui sem uma ferramenta de compositor. O teste por XWayland não exercita o caminho Wayland. Fica no roteiro manual e **não** é dado como PASS.
- **Aparência em HiDPI e tela cheia**: o smoke prova abrir/fechar sem queda; tamanho e nitidez exigem olho humano.

Mudança de código Rust: **nenhuma**. Nenhum bug de plataforma foi revelado pelos testes.

## HU-0902: distribuição reproduzível

| # | Critério | Resultado |
|---|---|---|
| 1 | `release.yml`: sufixo SemVer vira `prerelease: true` | feito (workflow **não** executado) |
| 2 | Job `verify` (fmt, clippy, test em Linux/Windows/macOS) antes de publicar | feito: `build` e `publish` dependem de `verify` |
| 3 | `SHA256SUMS` anexado | feito no job `publish`; gerado e conferido localmente |
| 4 | SBOM CycloneDX anexada | feito no job `publish`; local: CycloneDX 1.5, 450 componentes |
| 5 | Licenças de terceiros no pacote | feito: `THIRD-PARTY-LICENSES.html` (cargo-about, 490 entradas) |
| 6 | YAML válido | PASS (`yaml.safe_load`): jobs `prepare, verify, build, publish` |
| 7 | Pacote Linux reproduzido localmente (`tools/package/linux.sh`) | PASS |
| 8 | Pacote extraído em diretório temporário: executa, acha discos, manual | PASS (`tools/package/test-package.sh`, 17 verificações) |
| 9 | Assinatura/notarização documentada sem garantia falsa | feito: [assinatura.md](assinatura.md) |
| 10 | Dependências opcionais (pcap, serialport) por SO | feito: README, "Dependências opcionais por sistema" |

Teste do pacote (do `.tar.gz` extraído em `mktemp -d`, executando `./applewin` de dentro):

- Conteúdo presente: `applewin`, `LICENSE`, `README.md`, `THIRD-PARTY-LICENSES.html`, `install.sh`, `.desktop`, XML MIME e ícones.
- Headless a partir do pacote: PASS. Os três discos de `disks/` (BLANK, DOS 3.3 System Master, ProDOS 2.4.3) montam e dão boot headless; DOS e ProDOS mudam a tela em relação a rodar sem disco (foram lidos).
- Manual (F1): embutido no executável por `include_str!` de `docs/manual/*.md`; não depende de arquivos externos. A abertura do painel por F1 só é verificável com janela: está no roteiro manual.
- ROMs e símbolos também são embutidos.
- O programa **não** procura `disks/` sozinho; o usuário os abre pelo diálogo ou por `-d1`. É comportamento de projeto.

Limitações:

- Os passos novos do `release.yml` (instalação de `cargo-about`/`cargo-cyclonedx` via `taiki-e/install-action`, `licenses.sh` em Windows e macOS) **nunca rodaram** em CI: só YAML válido, e os scripts no Linux. O primeiro dry run (`workflow_dispatch`) pode pedir ajustes em Windows/macOS e depende de autorização do dono.
- Pacotes Windows e macOS não foram gerados aqui.
- SBOM e licenças usam `--target all`/lista de alvos, então incluem dependências específicas de Windows e macOS.
- Pacotes **não são assinados** nem notarizados.
