# Thread de emulação, ritmo e fronteiras do frontend

O frontend (`crates/applewin`) roda a Máquina numa thread própria (`emulation`). A thread de UI (winit, wgpu, egui) nunca toca a Máquina: ela só manda `Command`s por um canal `mpsc` e lê o último Quadro e o `EmuStatus` de um `Shared` (mutex + contador de sequência). Quando um Quadro novo sai, a thread de emulação acorda a UI por um callback (`EventLoopProxy`).

**Por que thread separada.** `Card` não é `Send`, então a Máquina nasce dentro da thread, criada por um `BackendFactory` (`Send`). Isso também impede que um diálogo de arquivo, o redimensionamento da janela ou o compositor atrasem a CPU emulada ou o áudio. O núcleo continua headless e determinístico (ADR 0001): a thread só decide *quando* rodar quadros, nunca *o que* acontece neles.

**Unidade de execução: o Quadro.** `Emulator` (síncrono, sem thread nem relógio) roda um Quadro de vídeo por vez via `FrameSource::run_frame`, porque o renderizador de `apple2-video` precisa ser alimentado depois de cada instrução (como `NTSC_VideoUpdateCycles` no AppleWin). Cada Quadro devolve também o áudio do `Mixer::produce_machine`. O modo headless (`--headless-frames`) usa o mesmo `Emulator` sem ritmo algum.

**Quem dá o ritmo** (`pacing.rs`, lógica pura):

| Modo | Condição | Relógio |
|---|---|---|
| Áudio | velocidade 1.0x e há dispositivo de som | nível do anel de áudio: roda Quadros enquanto o anel tiver menos de ~80 ms (`audio_frames_due`, rajada máxima de 4); o dispositivo de som é o relógio |
| Timer | outro multiplicador, ou sem som | `TimerPacer`: `elapsed * clk * velocidade`, com teto de 100 ms de atraso para não correr atrás de uma parada longa |
| Máximo | Scroll Lock, ou drive girando com "enhanced disk speed" | sem limite, publicando um Quadro a cada ~15 ms |
| Pausa | Pause, diálogos modais, depurador | nada roda; `Step` executa uma instrução |

Fora do modo Áudio, o áudio é descartado (AppleWin também não reamostra). Underrun do anel devolve silêncio e é contado (só no modo Áudio); overrun descarta o excedente. O volume mestre é um ganho atômico no anel.

**Fronteiras para os outros crates** (`backend.rs`): `Backend` (construir a Máquina a partir de `Settings`), `FrameSource`, `DiskSink`, `MouseSink` e `DebuggerHook`. Todos rodam na thread de emulação, com implementações nulas por padrão. O `DefaultBackend` liga `apple2-config`, `apple2-video`, `apple2-storage`, `apple2-audio` e `apple2-cards`.

**Consequências.** Latência de teclado de até um Quadro (16,7 ms) mais a fila do anel; save-state e troca de configuração passam pela thread de emulação (`Command`), então são atômicos em relação aos Quadros. O formato do save-state (F11/F12) é JSON temporário (`.aws.json`); o YAML do AppleWin entra na fase de save-state.
