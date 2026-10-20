# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Núcleo (`apple2-core`)

- `CardCtx` deve expor o opcode atual para portar a falsa leitura de `STA abs,X/Y` e `STA (zp),Y` no 6522, que limpa o IFR (apple2-audio).
- `CardCtx::schedule` precisa de uma variante com `can_assert_irq` para o 6522 aplicar a regra "IRQ no último ciclo adia um opcode" ao escrever o IER (apple2-audio).

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Save-state YAML do AppleWin: feito em `apple2-savestate` (`save_aws_yaml` / `load_aws_yaml`). O frontend só precisa fornecer um `LoadHost` (resolver de imagens e fábrica das cartas que dependem do hospedeiro) e a data/versão do cabeçalho.
- Termos propostos para o glossário: ACIA, Backend, DNS virtual, Barramento de áudio, Mixer, Quadro de áudio.
