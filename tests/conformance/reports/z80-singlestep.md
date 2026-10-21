# Z80: SingleStepTests/z80 (v1)

Vetores: github.com/SingleStepTests/z80 (MIT), SHA fixo em `tools/conformance/fetch-z80-tests.sh`.
Teste: `cargo test -p apple2-cards --release --test z80_singlestep -- --ignored --nocapture`.

Compara registradores, WZ, IFF1/IFF2, IM, EI, P, Q, RAM, transação de porta e T-states.

## Resultado

- Arquivos (grupos de opcode): 1604 (0 com falha)
- Vetores que passam: 1604000
- Vetores que falham: 0

