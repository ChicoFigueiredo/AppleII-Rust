# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Save-state YAML do AppleWin: feito em `apple2-savestate` (`save_aws_yaml` / `load_aws_yaml`). O frontend só precisa fornecer um `LoadHost` (resolver de imagens e fábrica das cartas que dependem do hospedeiro) e a data/versão do cabeçalho.
- Termos propostos para o glossário: ACIA, Backend, DNS virtual, Barramento de áudio, Mixer, Quadro de áudio.

## Desempenho

- Depois dos merges da onda 1 (ganchos `RomTap`, snoop, `AccessObserver`, `CardCtx` com mais campos), o teste `emulated_speed` caiu de 80,5 MHz para 40 MHz no Enhanced //e. Ainda é 39× o tempo real, mas vale uma rodada de profiling no caminho `Board::read/write` (ex.: `floating_bus()` calculado mesmo quando o cartão não usa).
