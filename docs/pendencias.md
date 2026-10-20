# Pendências de integração

Itens que os agentes deixaram por depender de outro crate ou do núcleo. Serão resolvidos na integração.

## Núcleo (`apple2-core`)

- `CardCtx` deve expor o opcode atual para portar a falsa leitura de `STA abs,X/Y` e `STA (zp),Y` no 6522, que limpa o IFR (apple2-audio).
- `CardCtx::schedule` precisa de uma variante com `can_assert_irq` para o 6522 aplicar a regra "IRQ no último ciclo adia um opcode" ao escrever o IER (apple2-audio).

## Frontend

- Avisar na tela de configuração que a SSC em modo TCP escuta em todas as interfaces, na porta 1977, como no AppleWin (apple2-hostio).
- Mapear os estados opacos das cartas (`Card::save`) para o YAML de save-state do AppleWin.
- Termos propostos para o glossário: ACIA, Backend, DNS virtual, Barramento de áudio, Mixer, Quadro de áudio.
