# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Mapear os estados opacos das cartas (`Card::save`) para o YAML de save-state do AppleWin.

## Desempenho

- Resolvido: o custo estava em `update_events` (calculava `floating_bus()` a cada instrução) e nos testes de observer/rom_tap por acesso. Agora `emulated_speed` ~90 MHz e o boot do DOS 3.3 ~88 MHz (CPU time). `tests/no_alloc.rs` garante zero alocações por passo. Resta: `Renderer::update` domina o laço completo (~15 MHz com ColorTv); `CardCtx` ainda calcula `floating_bus` de forma eager por chamada de cartão.
