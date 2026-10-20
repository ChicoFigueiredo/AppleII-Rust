# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Mapear os estados opacos das cartas (`Card::save`) para o YAML de save-state do AppleWin.

## Desempenho

- Depois dos merges da onda 1 (ganchos `RomTap`, snoop, `AccessObserver`, `CardCtx` com mais campos), o teste `emulated_speed` caiu de 80,5 MHz para 40 MHz no Enhanced //e. Ainda é 39× o tempo real, mas vale uma rodada de profiling no caminho `Board::read/write` (ex.: `floating_bus()` calculado mesmo quando o cartão não usa).
