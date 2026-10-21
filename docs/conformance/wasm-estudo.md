# Estudo: frontend web/WASM (HU-1205)

Estado: **protótipo opt-in**, medido em 2026-10-20. O núcleo compila para `wasm32-unknown-unknown` **sem nenhuma mudança** e o boot dá o mesmo framebuffer que no desktop. Nada aqui entra no binário desktop.

## 1. O núcleo compila para WASM?

Alvo instalado com `rustup target add wasm32-unknown-unknown` (sem sudo). Resultado de `cargo build -p CRATE --target wasm32-unknown-unknown`:

| Crate | Compila | Observação |
|---|---|---|
| `apple2-cpu` | sim | Só `core`; sem dependências. |
| `apple2-roms` | sim | `include_bytes!`: as ROMs vão para dentro do .wasm. |
| `apple2-core` | sim | Depende de `serde`. Sem relógio, sem threads, sem arquivos. |
| `apple2-video` | sim | Só aritmética e tabelas. |
| `apple2-audio` | sim | Gera amostras a partir de Ciclos; não abre dispositivo de som. |
| `apple2-storage` | sim | Ver `std::fs` abaixo. |

### O que poderia impedir, e o que realmente acontece

Busca por APIs problemáticas nesses seis crates (`std::fs`, `std::thread`, `Instant`, `SystemTime`, `std::env`, `std::net`, `std::process`):

- **`std::fs`**: aparece só em `apple2-storage`, em `DiskImage::open_path` (`image.rs`) e no `FileStore` de gravação em disco (`store.rs`). Em `wasm32-unknown-unknown` o `std::fs` **compila**, mas toda chamada falha em tempo de execução com `Unsupported`. Não precisa de `cfg`: o protótipo nunca chama essas funções, porque `DiskImage::from_bytes` (já existente) recebe os bytes. É o mesmo desenho da ADR 0002 ("o núcleo nunca abre arquivos").
- **Threads**: nenhuma. O núcleo é determinístico e de uma thread só (ADR 0001 e 0003).
- **Relógio**: nenhum uso de `Instant` ou `SystemTime` nesses crates. O tempo é contado em Ciclos e Quadros; quem decide o ritmo é o frontend (`requestAnimationFrame`).
- **Dependências**: `serde`, `flate2` e `zip` (com `miniz_oxide`, Rust puro, sem C). Todas compilam para WASM; não há `-sys` nem código nativo.
- **Aleatoriedade**: o núcleo não chama o sistema; o padrão de RAM do power-on vem de `RamInit` com semente explícita.

Fora do escopo desta HU, e que **não** foi testado: `apple2-cards` (relógio da Thunderclock), `apple2-hostio`, `apple2-config` e `apple2-savestate`.

### Mudanças feitas no núcleo

**Nenhuma.** Nem `cfg` nem feature. O "sem mudar o comportamento no desktop" está garantido por construção: o diff do núcleo é vazio.

## 2. Protótipo `crates/apple2-rust-web`

- Membro do workspace, `publish = false`, `crate-type = ["cdylib", "rlib"]`.
- `wasm-bindgen` (`=0.2.129`, a versão já travada no `Cargo.lock` por outras dependências do projeto) e `wasm-bindgen-test` ficam em `[target.'cfg(target_arch = "wasm32")'.dependencies]`. **No build desktop o crate não puxa nenhuma dependência nova**: `cargo build --workspace` só compila Rust puro a mais. Por isso não foi preciso `default-members`.
- `Emulator` (Rust puro, em `src/lib.rs`) é a API; `wasm::Apple2` (só em `wasm32`) a expõe ao JavaScript.

### API

| JavaScript (`Apple2`) | Rust (`Emulator`) | O que faz |
|---|---|---|
| `new Apple2(modelo)` | `Emulator::new` | Cria a Máquina com as ROMs embutidas e um Disk II no slot 6. Modelos: `apple2`, `apple2plus`, `apple2e`, `apple2e-enhanced`. |
| `Apple2.with_rom(modelo, bytes)` | `Emulator::with_rom` | Como acima, com a ROM de sistema fornecida pelo usuário (16K do //e ou 12K × 1-4 das linhas ][). |
| `run_frame()` | `run_frame` | Roda um Quadro (17.030 Ciclos NTSC) renderizando a cada instrução. |
| `width()`, `height()` | `size` | 560 × 384. |
| `framebuffer()` | `framebuffer_rgba` | RGBA8, pronto para `ImageData`. |
| `key(ascii)` | `key` | Digita um caractere. |
| `insert_disk(drive, nome, bytes)` | `insert_disk` | Insere imagem de disco a partir de bytes (`.dsk`, `.do`, `.po`, `.nib`, `.woz`, `.gz`, `.zip`); o `nome` só define o formato. |
| `reset()`, `frames()`, `framebuffer_hash()` | idem | Reset a quente, contador de Quadros, hash FNV-1a 64 do framebuffer. |

A página `crates/apple2-rust-web/www/index.html` desenha o framebuffer num `<canvas>`, trata teclado, aceita arrastar um disco e permite escolher uma ROM própria.

### Como construir e abrir

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129      # mesma versão do Cargo.lock
cargo build -p apple2-rust-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir crates/apple2-rust-web/www/pkg \
  target/wasm32-unknown-unknown/release/apple2_rust_web.wasm
cd crates/apple2-rust-web/www && python3 -m http.server 8000     # abra http://localhost:8000
```

`www/pkg/` está no `.gitignore`. A página foi escrita para o navegador, mas **não foi aberta num navegador real nesta sessão**: só a API foi exercitada em Node.

## 3. Decisão sobre as ROMs

**Decisão:** o protótipo embute as ROMs no .wasm (porque `apple2-core` já depende de `apple2-roms`), e **também** aceita a ROM de sistema do usuário por `with_rom`.

- **Embutir** tem exatamente as mesmas implicações da distribuição desktop (ADR 0002 e [rom-provenance.md](rom-provenance.md)): as ROMs são firmware da Apple e de terceiros, redistribuídas como o AppleWin faz, sem licença própria do projeto. Hospedar o .wasm numa página pública é **redistribuir** essas ROMs a qualquer visitante, o que é mais exposto do que entregar um instalador. Quem publicar uma instância assume o mesmo risco que assume ao redistribuir o AppleWin e deve estar pronto para retirar a ROM a pedido de um titular.
- **O usuário fornece a ROM** evita redistribuir firmware: a página lê o arquivo escolhido no navegador (nada sai do computador do visitante) e chama `with_rom`. O tamanho é validado pelo núcleo (`ConfigError::BadRomSize`, erro explícito).
- **Limite atual:** hoje as ROMs ainda estão dentro do .wasm mesmo quando o usuário passa a dele, porque `apple2-roms` é dependência incondicional do núcleo. Também sobram embutidas as ROMs de cartas (Disk II etc.) e o gerador de caracteres do vídeo, que `with_rom` não substitui. Uma versão que **não** redistribua firmware exigiria uma feature `embedded-roms` em `apple2-roms`/`apple2-core` (desligável) e um carregamento das ROMs de cartas e de caracteres por bytes. Fica como próximo passo, e é mudança de núcleo, de outro dono.
- Resultados de conformidade só valem para a ROM do AppleWin (mesma regra do desktop).

## 4. Testes headless

Dois níveis, o mesmo golden:

| Arquivo | Onde roda | O que prova |
|---|---|---|
| `crates/apple2-rust-web/tests/boot.rs` | nativo (`cargo test`) | Hash do framebuffer da API web após 60 Quadros de boot a frio do //e Enhanced == hash pelo caminho direto do desktop (`Machine` + `Renderer`, mesmo Disk II no slot 6); hash dourado `0x4f1dc409f736b225`; tamanho 560×384; ~17.030 Ciclos por Quadro; ROM do usuário; disco a partir de bytes; modelo inválido. |
| `crates/apple2-rust-web/tests/wasm.rs` | WASM, Node, `wasm-bindgen-test` | O **mesmo** hash dourado calculado dentro do .wasm; a API JavaScript (`Apple2`) roda 60 Quadros e devolve RGBA do tamanho certo; disco por bytes. |

**Resultado:** o hash em WASM é idêntico ao do desktop (`0x4f1dc409f736b225`). A emulação em WASM é bit a bit a do desktop para esse boot.

Como rodar os testes WASM (Node 20 já instalado):

```
cargo install wasm-bindgen-cli --version 0.2.129
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test -p apple2-rust-web --target wasm32-unknown-unknown --release --test wasm
```

No desktop `tests/wasm.rs` compila vazio (`#![cfg(target_arch = "wasm32")]`), então `cargo test --workspace` não depende de Node nem do target WASM.

## 5. Relatório

### Tamanho do .wasm (release, `opt-level = 3`, `lto = "thin"`, sem `wasm-opt`)

| Artefato | Bytes |
|---|---:|
| `apple2_rust_web.wasm` (saída do cargo) | 845.189 |
| `apple2_rust_web_bg.wasm` (depois do `wasm-bindgen`) | 799.763 |
| Mesmo arquivo com gzip | ≈ 281.000 |

Quase todo o tamanho é código; as ROMs embutidas somam algumas centenas de KB e comprimem bem. `wasm-opt -Oz` e `lto = "fat"` com `opt-level = "s"` devem reduzir; não foram medidos (`wasm-opt` não está instalado).

### Desempenho (Node v20.11, WSL2, o mesmo código de `Apple2`)

- `run_frame()` com vídeo: **≈ 258 Quadros/s** (600 Quadros em 2,33 s), cerca de **4,3× o tempo real** (59,92 Quadros/s no NTSC). Há folga para o navegador desenhar e para a página fazer outras coisas.
- `framebuffer()` (cópia de 860 KB para o JavaScript): ≈ 590 cópias/s. Em tempo real isso custa ~10% de um Quadro; dá para evitar expondo a memória do WASM (`Uint8ClampedArray` sobre a memória linear) em vez de copiar.
- Medido uma vez, numa máquina; use como ordem de grandeza. O Node não mede o custo de `putImageData` num canvas.

### Limitações do protótipo

- **Áudio:** não há. `apple2-audio` compila para WASM, mas falta a ponte com `AudioWorklet`/Web Audio e a política de autoplay do navegador. O alto-falante do núcleo (`drain_speaker_toggles`) já fornece os dados.
- **Threads:** nenhuma. O núcleo é de uma thread, o que combina com o navegador. Rodar em Web Worker (para não travar a página) é possível, mas não foi feito; `SharedArrayBuffer` não é necessário.
- **Sistema de arquivos:** inexistente. Imagens de disco entram por bytes; **gravações no disco ficam só na memória** e se perdem ao recarregar a página (o `FileStore` do `apple2-storage` usa `std::fs`). Persistir exigiria um `Store` sobre IndexedDB/OPFS (a trait `Store` já existe).
- **Teclado:** só ASCII; sem Open/Closed Apple, sem teclas de navegação do //e, sem colar texto, sem joystick/mouse.
- **Cartas:** só o Disk II no slot 6. Sem Mockingboard, SSC, discos rígidos, VidHD etc.
- **Ritmo:** a página roda um Quadro por `requestAnimationFrame`; em monitores de 120/144 Hz a emulação ficaria mais rápida que o real. Falta um relógio por Quadros acumulados.
- **Salvar estado, depurador e configuração** não estão ligados.
- **Navegador real:** não testado (só Node).

### Dependências adicionadas

Só em `wasm32` (não entram no build desktop); `wasm-bindgen` já estava no `Cargo.lock`.

| Crate | Versão | Licença | Uso |
|---|---|---|---|
| `wasm-bindgen` (e `-macro`, `-shared`, ...) | 0.2.129 | MIT OR Apache-2.0 | API JS (já no lock) |
| `wasm-bindgen-test` (+ `-macro`, `-shared`) | 0.3.79 / 0.2.129 | MIT OR Apache-2.0 | só testes |
| `cast` | 0.3.0 | MIT OR Apache-2.0 | transitiva de `wasm-bindgen-test` |
| `libm`, `nu-ansi-term`, `oorandom` | 0.2.16 / 0.50.3 / 11.1.5 | MIT | transitivas de `wasm-bindgen-test` |
| `minicov` | 0.3.8 | Apache-2.0 WITH LLVM-exception (segundo o crates.io; o fonte não foi baixado) | transitiva de `wasm-bindgen-test`, só cobertura |

Todas compatíveis com GPL-2.0-or-later do projeto (as de Apache-2.0 só vêm de ferramentas de teste e não entram em binário distribuído).

### Próximos passos

1. Feature `embedded-roms` desligável + ROMs de cartas e de caracteres por bytes, para uma versão sem redistribuição de firmware.
2. Áudio: `AudioWorklet` alimentado pelo alto-falante do núcleo e pelo Mockingboard.
3. `Store` sobre IndexedDB/OPFS para gravar discos; download da imagem modificada.
4. Teclado completo (modificadores, Open/Closed Apple, colar), joystick pela Gamepad API, mouse.
5. Ritmo por relógio (acumulador de Quadros) e Web Worker com `OffscreenCanvas`.
6. Expor o framebuffer sem cópia (visão sobre a memória linear).
7. `wasm-opt -Oz`, avaliar `opt-level = "s"`, e publicar o tamanho como métrica de CI (quando a cota de CI permitir).
8. Testar `apple2-cards`, `apple2-savestate` e `apple2-config` em WASM e ligar salvar/restaurar estado.
9. Testar a página em Chrome, Firefox e Safari.
