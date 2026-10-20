# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Save-state YAML do AppleWin: feito em `apple2-savestate` (`save_aws_yaml` / `load_aws_yaml`). O frontend só precisa fornecer um `LoadHost` (resolver de imagens e fábrica das cartas que dependem do hospedeiro) e a data/versão do cabeçalho.
- Termos propostos para o glossário: ACIA, Backend, DNS virtual, Barramento de áudio, Mixer, Quadro de áudio.

## Desempenho

- Resolvido: o custo estava em `update_events` (calculava `floating_bus()` a cada instrução) e nos testes de observer/rom_tap por acesso. Agora `emulated_speed` ~90 MHz e o boot do DOS 3.3 ~88 MHz (CPU time). `tests/no_alloc.rs` garante zero alocações por passo. Resta: `Renderer::update` domina o laço completo (~15 MHz com ColorTv); `CardCtx` ainda calcula `floating_bus` de forma eager por chamada de cartão.
