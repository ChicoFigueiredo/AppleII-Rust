
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
