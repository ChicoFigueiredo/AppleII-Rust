# Windows: build cruzado e smoke no Wine

**Aviso: Wine não é Windows nativo.** Wine reimplementa a API do Windows sobre o Linux. Um PASS aqui prova que o executável PE gerado carrega, roda e fecha, e que o núcleo produz a mesma saída. Não prova nada sobre o Windows real: DPI por monitor, o manifesto, o `wincon.rs` (anexar ao console), WASAPI, XInput, Narrator e o comportamento do Defender/SmartScreen continuam **BLOCKED** até uma execução em Windows nativo (veja [manual-tests/plataformas.md](../manual-tests/plataformas.md)).

Data: 2026-10-20. Commit base: `516c123`.

## 1. Build Windows a partir do Linux

| Item | Valor |
|---|---|
| Alvo | `x86_64-pc-windows-msvc` |
| Ferramenta | `cargo-xwin` 0.23.1 (`cargo install cargo-xwin`, em `~/.cargo/bin`, sem sudo) |
| CRT/SDK | baixados pelo cargo-xwin da Microsoft para `target/conformance-cache/xwin` |
| Linker | `rust-lld` (vem com o rustup); `clang` do sistema para C eventual |
| Rust | 1.98.1 |
| Resultado | **PASS**: `applewin.exe`, PE32+ x86-64 GUI, 16,6 MB, compilado em 2 min 12 s (com as dependências já em cache do Linux; o primeiro download do CRT/SDK ocorreu antes) |

Comando:

```sh
XWIN_CACHE_DIR=target/conformance-cache/xwin XWIN_ACCEPT_LICENSE=1 \
CARGO_TARGET_DIR=target/conformance-cache/target-win \
cargo xwin build -p apple2-rust --release --target x86_64-pc-windows-msvc
```

Saída: `target/conformance-cache/target-win/x86_64-pc-windows-msvc/release/apple2-rust.exe`.

Observações:

- `XWIN_ACCEPT_LICENSE=1` aceita a licença do Microsoft CRT/SDK que o cargo-xwin baixa. É um ato de aceite do dono do ambiente; o cache fica fora do repositório.
- O ícone, o manifesto e a versão (`crates/apple2-rust/build.rs`, `packaging/windows/applewin.manifest`) foram embutidos sem erro no cruzado.
- A alternativa `x86_64-pc-windows-gnu` não foi necessária (não há mingw e não há sudo). O `cargo-zigbuild` também não foi preciso.
- Mac não tem equivalente: compilar macOS exige o SDK da Apple, cuja licença só permite uso em hardware Apple. Por isso macOS segue BLOCKED.

## 2. Smoke no Wine

| Item | Valor |
|---|---|
| Wine | 10.0 (Ubuntu 10.0~repack-12ubuntu1) |
| `WINEPREFIX` | `target/conformance-cache/wineprefix` (próprio, criado por `wineboot -i`) |
| Host | Ubuntu 26.04.1, kernel 7.0.0-38, x86_64, NVIDIA RTX 4080 SUPER, KDE em Wayland com XWayland (`DISPLAY=:0`) |
| Script | [`tools/package/smoke-wine.sh`](../../../tools/package/smoke-wine.sh) (e `smoke-gui.sh`, `x11-close.py`) |

Resultados (todos rc=0):

| Teste | Resultado |
|---|---|
| `--headless-frames 300 --dump-screen` em `apple2` | PASS: PNG idêntico bit a bit ao do Linux (`4606d9a0f3c3`) |
| idem `apple2p` | PASS (`f94ca8cc93b5`) |
| idem `apple2jp` | PASS (`f94ca8cc93b5`) |
| idem `apple2e` | PASS (`23c35c214b54`) |
| idem `apple2ee` | PASS (`e9a89e68e366`) |
| Boot do DOS 3.3 headless (900 quadros, `-d1 ... -power-on`) | PASS: PNG idêntico ao do Linux |
| `--help` | PASS (o `.exe` é do subsistema GUI; sob o Wine a saída chega pelo stdout redirecionado) |
| Janela real aberta por ~15 s e fechada com `WM_DELETE_WINDOW` | PASS: código de saída 0; wgpu usou Vulkan (driver NVIDIA do host, via winevulkan) |

Notas sobre o que foi observado:

- O Apple ][ liga com a RAM em padrão aleatório (como o AppleWin), então a tela de boot varia entre execuções, até no Linux. Para comparar bit a bit o script usa `-memclear 0`. Isso não é um defeito do cruzado.
- A janela do Wine leva ~7 s para aparecer; o smoke espera 15 s antes de fechar.
- `WGPU_BACKEND=dx12` no Wine falha (`graphics init failed: Parent device is lost`; causa não investigada, provavelmente o D3D12 do Wine). `WGPU_BACKEND=gl` ficou rodando sem erro por 15 s, mas não foi validado visualmente. O caminho Vulkan, que é o padrão, funciona.
- `gamepads unavailable: Não implementado` é esperado: o Wine não implementa o backend de gamepad usado pelo gilrs. Gamepad no Windows fica BLOCKED.
- Mensagens `dbus ... assertion "pending != NULL"` vêm do próprio Wine/host e são inofensivas.
- Se o processo do Wine é morto por sinal (`timeout`), aparecem `err:seh:...`: é o encerramento forçado, não o fechamento limpo. O smoke só conta o fechamento por `WM_DELETE_WINDOW`.

## 3. Como repetir

```sh
cargo install cargo-xwin
cargo build -p apple2-rust --release                        # binário Linux de referência
cargo xwin build -p apple2-rust --release --target x86_64-pc-windows-msvc   # com as variáveis acima
tools/package/smoke-wine.sh
```
