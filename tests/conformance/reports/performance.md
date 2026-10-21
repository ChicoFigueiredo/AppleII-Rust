# Desempenho determinístico (HU-1001)

Gerado por `apple2-bench` (`cargo run --release -p apple2-bench -- --report tests/conformance/reports/performance.md`). Os números valem para a máquina abaixo; em outra máquina compare só tendências e use o `--check` do baseline.

## Ambiente

| Item | Valor |
|---|---|
| CPU | AMD Ryzen 7 3700X 8-Core Processor |
| Núcleos lógicos | 16 (a emulação usa uma thread) |
| Memória | 128688 MiB |
| SO | Ubuntu 26.04.1 LTS (kernel 7.0.0-38-generic, x86_64) |
| Governador de CPU | schedutil |
| SHA | `7e98a801bb0b23f80614c0774046c04d48cc85c9` (+ alterações locais) |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01) |
| Perfil | release (opt-level 3, lto thin) |
| Repetições | 10 (+1 de aquecimento descartada) |

**Condições desta medição:** máquina compartilhada com outros agentes compilando (carga média de 40 a 90 em 16 threads lógicas), sem como silenciá-la. Por isso o relógio de parede (colunas fps/MHz) mistura preempção; as colunas com base no tempo de CPU da thread, o CV e o `--check` são as que valem. O cenário `cpu` mostra CPU 46 % nesta rodada (thread esperando vaga). Refaça em máquina ociosa para números absolutos. Observação: rodadas de `--check` feitas depois, na mesma máquina ocupada, deram -30 % só nos quatro cenários que usam o renderer Color TV (tabelas de 64 KiB) e nada nos demais, compatível com um vizinho no mesmo núcleo físico disputando o cache. Por isso o baseline é desta rodada (a mais limpa) e o `--check` repete os cenários reprovados antes de falhar.

## Cenários

Cada repetição roda num processo novo (o RSS de pico é só do cenário). O trecho cronometrado exclui a criação da máquina, mas inclui, em todo quadro, a execução da CPU e dos cartões, o renderer, a cópia do quadro e a mixagem do áudio (o trabalho da thread de emulação, sem o pacing). "p95" é o pior caso: o percentil 95 do tempo, e as taxas derivadas dele (a taxa que 95 % das execuções igualaram ou superaram); para CPU e RSS é o percentil 95 direto. "MHz sobre CPU" divide os ciclos pelo tempo de CPU da thread (é a base do `--check`, imune a preempção); o CV % é o dele. Referência: 1,0205 MHz emulado = tempo real (NTSC), 60 quadros/s.

| Cenário | Quadros | fps mediana | fps p95 | MHz mediana | MHz p95 | x tempo real | MHz sobre CPU (mediana) | CPU % (med) | RSS pico MiB (p95) | CV % |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `boot_text` | 600 | 477.8 | 276.2 | 8.14 | 4.70 | 8x | 9.28 | 88 | 8.4 | 2.1 |
| `cpu` | 1174 | 1463.8 | 629.3 | 24.94 | 10.72 | 24x | 56.85 | 46 | 4.8 | 2.5 |
| `dhgr` | 300 | 641.3 | 585.9 | 10.92 | 9.98 | 11x | 11.82 | 93 | 8.3 | 3.6 |
| `disk2_dos33` | 600 | 746.0 | 618.4 | 12.70 | 10.53 | 12x | 12.71 | 100 | 8.8 | 7.1 |
| `hgr` | 300 | 712.4 | 618.8 | 12.13 | 10.54 | 12x | 12.68 | 98 | 8.3 | 3.8 |
| `mockingboard` | 400 | 715.0 | 686.8 | 12.18 | 11.70 | 12x | 12.18 | 100 | 8.9 | 2.2 |
| `ntsc_monitor` | 300 | 520.2 | 330.5 | 8.86 | 5.63 | 9x | 10.14 | 90 | 8.4 | 4.5 |
| `ntsc_tv` | 300 | 749.7 | 402.2 | 12.77 | 6.85 | 13x | 12.81 | 100 | 8.3 | 12.0 |
| `pal` | 300 | 710.5 | 701.6 | 14.41 | 14.23 | 14x | 14.42 | 100 | 8.4 | 1.5 |

| Cenário | Descrição |
|---|---|
| `boot_text` | Boot //e Enhanced até o prompt e programa BASIC imprimindo texto |
| `cpu` | CPU pura: laço de instruções (programa do Benchmark do AppleWin) |
| `dhgr` | DHGR animando (main + aux reescritas todo quadro), cores idealizadas |
| `disk2_dos33` | Disk II lendo: boot do DOS 3.3 System Master (assets/disks) |
| `hgr` | HGR animando (página inteira reescrita todo quadro), cores idealizadas |
| `mockingboard` | Áudio: Mockingboard C (2x AY-3-8910) tocando, mixer em 48 kHz |
| `ntsc_monitor` | HGR animando, renderer NTSC Color Monitor (60 Hz) |
| `ntsc_tv` | HGR animando, renderer NTSC Color TV (60 Hz) |
| `pal` | HGR animando, renderer Color TV em PAL (50 Hz) |

## Latência de entrada

| Medida | mín | mediana | p95 | máx |
|---|---:|---:|---:|---:|
| Tecla até o fim do `LDA $C000` (ciclos emulados) | 4.00 | 7.00 | 7.00 | 8.00 |
| O mesmo, em microssegundos de Apple II | 3.92 | 6.86 | 6.86 | 7.84 |
| Tempo real na thread, `Machine::key_press` (µs) | 0.06 | 0.09 | 0.11 | 2.15 |
| Tempo real na thread, via `Emulator::apply(Command::Key)` (µs) | 0.06 | 0.10 | 0.12 | 0.75 |
| `Command::Key` enviado à thread até um quadro publicado mostrar a tecla (ms) | 3.33 | 12.79 | 24.81 | 36.91 |

As quatro primeiras linhas medem o caminho do núcleo: o programa de teste espera a tecla num laço `LDA $C000 / BPL` de 7 ciclos, então a latência em ciclos é o resto do laço (fase variada de propósito) mais o `LDA`; com o firmware real o laço de `KEYIN` é parecido. A última linha é ponta a ponta no processo, sem janela nem áudio: a thread de emulação (`emu::spawn`, ritmo por timer a 1,0x, ADR 0003) só aplica o comando entre quadros e só publica o quadro depois de renderizá-lo, no máximo a cada 8 ms; por isso o valor depende da fase do quadro de 16,7 ms em que a tecla chega (de poucos ms até ~1 quadro mais a publicação). Com áudio, o anel de ~80 ms (ADR 0003) soma-se a isso.

**Não dá para medir sem GUI:**

- Entrega do evento de teclado pelo SO e pelo winit até a thread de UI (pilha de entrada do SO, compositor).
- Apresentação do quadro: fila do wgpu/swapchain, vsync e varredura do monitor físico.
- Latência do dispositivo de áudio (buffer do cpal e do driver): só existe com placa de som real.
- Tempo de resposta do próprio teclado/USB (debounce, polling de 1 ms a 8 ms).

## Decomposição do custo por quadro

Tempo de CPU da thread por quadro (mínimo de 3 execuções), acrescentando uma etapa por vez (veja `breakdown.rs`). 16,7 ms por quadro é o orçamento de tempo real a 60 Hz.

| Cenário | Total ms/quadro | Máquina (CPU, scanner, cartões) | Renderer a cada instrução | Conversão para RGBA | Mixagem de áudio |
|---|---:|---:|---:|---:|---:|
| `dhgr` | 1.046 | 0.183 (18 %) | 0.857 (82 %) | 0.000 (0 %) | 0.006 (1 %) |
| `disk2_dos33` | 1.284 | 0.248 (19 %) | 0.939 (73 %) | 0.112 (9 %) | 0.000 (0 %) |
| `hgr` | 1.022 | 0.183 (18 %) | 0.673 (66 %) | 0.112 (11 %) | 0.053 (5 %) |
| `mockingboard` | 1.996 | 0.359 (18 %) | 1.424 (71 %) | 0.165 (8 %) | 0.049 (2 %) |
| `ntsc_monitor` | 1.163 | 0.173 (15 %) | 0.910 (78 %) | 0.050 (4 %) | 0.030 (3 %) |
| `ntsc_tv` | 1.279 | 0.173 (14 %) | 1.017 (79 %) | 0.086 (7 %) | 0.003 (0 %) |
| `pal` | 1.347 | 0.207 (15 %) | 1.057 (78 %) | 0.097 (7 %) | 0.000 (0 %) |

## Gargalos e otimizações

**Como foi perfilado.** O `perf` está bloqueado neste kernel (`perf_event_paranoid=4`, sem root). Usei duas fontes: a decomposição acima (determinística, mede cada etapa do quadro) e amostragem por `gdb` (SIGALRM a cada ~10 ms, 137 amostras do cenário `ntsc_tv` antes da otimização, empilhamento de 5 níveis).

**Onde o tempo vai (depois da otimização 1).** O núcleo é barato: CPU, memória, scanner de vídeo e cartões (incluindo `CardCtx`/floating bus) custam 0,17 a 0,36 ms por quadro (o Mockingboard é o mais caro), ou seja, 14 a 19 % do quadro, ~100 MHz emulados. O que domina é o renderer alimentado a cada instrução (`Renderer::update` → `video_update_cycles` → `update_pixels` → `dot`): 66 a 82 % do quadro, uns 0,7 a 1,4 ms. Na amostragem, `Renderer::dot` sozinho foi 39 % das amostras (3 escritas e 1 leitura no framebuffer por ponto, 14 pontos por ciclo de CPU, com checagem de limites em `put`/`get` e a tabela `chroma()` consultada a cada ponto); `run`/`sync`/`eol`/`update` somam ~20 % (o `sync` relê as chaves de vídeo a cada instrução). Em tempo real a 1,0x o emulador usa ~8 % de um núcleo com vídeo NTSC TV.

**Otimização feita (commit próprio, 7e98a80, mais um ajuste de clippy logo depois): `Renderer::rgba8_into`.** A conversão do framebuffer para RGBA alocava um `Vec` por quadro e fazia um `extend_from_slice` por pixel (215 mil por quadro): 25 % das amostras. Agora é uma passada por linha sobre um buffer reaproveitado.

| Cenário (ms/quadro, tempo de CPU, mín. de 3) | antes | depois | ganho |
|---|---:|---:|---:|
| `hgr` | 1,56 a 1,58 | 1,24 a 1,36 | 14 a 21 % |
| `ntsc_monitor` | 1,87 a 1,91 | 1,58 | 17 % |
| `ntsc_tv` | 2,01 a 2,14 | 1,67 a 1,83 | 14 a 22 % |
| `pal` | 2,13 | 1,88 a 2,01 | 6 a 12 % |
| `disk2_dos33` | 2,05 a 2,11 | 1,84 a 1,93 | 9 a 13 % |

(Duas rodadas A/B alternadas, binário anterior contra o novo, com a máquina com carga 70 a 90; a etapa de conversão caiu de ~0,35 ms para ~0,08 ms.) Garantias: o sha256 do framebuffer da conformidade ficou idêntico (`f627c6de…`), `conformance run --all` deu 12 PASS antes e depois (traces contra o AppleWin iguais), `cargo test --workspace --release` verde (1160 testes) e o teste novo `rgba8_into_matches_pixel_by_pixel_and_reuses_the_buffer` compara a saída pixel a pixel. Não toca em CPU, timing nem áudio: o fingerprint (ciclos, hash do quadro, hash do áudio) de cada cenário só depende do que o emulador faz, não de como o quadro é exportado.

**Descartadas (sem ganho acima de 10 % sem risco).**

- `CardCtx`/floating bus: faz parte da etapa "Máquina", que inteira é 14 a 19 % do quadro; mesmo zerando o custo do floating bus o ganho no quadro ficaria bem abaixo de 10 %.
- Pipeline Color TV: `ntsc_tv` custa só ~0,1 ms a mais por quadro que `ntsc_monitor` (a mistura de linhas em `dot`), menos de 10 %.
- Cópias do quadro entre threads (`Shared::publish` e `copy_frame`, ~0,9 MB cada, no máximo a cada 8 ms): ~2 a 3 % de um núcleo, menos de 10 %.
- Reescrever `dot`/`put`/`get` (sem checagem de limites, `chroma()` fora do laço de 14 pontos) e fazer o `sync` só quando uma chave de vídeo mudar: é onde está o ganho grande restante (o renderer é 66 a 82 % do quadro), mas mexe no caminho que o oráculo do AppleWin valida ponto a ponto. Fica como próxima otimização, a fazer com a suíte diferencial e este benchmark rodando antes e depois.
