# Pendências

Estado em 2026-10-20, depois da onda final. As diferenças em relação ao AppleWin que o usuário percebe estão em [manual/12-diferencas-em-relacao-ao-applewin.md](manual/12-diferencas-em-relacao-ao-applewin.md). Aqui ficam só as pendências técnicas.

## Cartas e emulação

- Breakpoint card: existe, mas o depurador ainda não lhe entrega os breakpoints.
- Mockingboard: um tipo de AY por carta. Os soquetes 3 e 4 do Phasor e as chaves `6522*-bad` não são modelados.
- SNES MAX: os controles do computador chegam à carta, mas `-snes-max-alt-joy*` e os arquivos `-snes-max-user-joy` não valem.
- ThunderClock Plus: emulado na biblioteca, mas sem tipo de carta para escolher na configuração.
- Z80: sem interrupções. Os efeitos colaterais do MEMPTR em INIR/OTIR não são modelados.

## Interface

- Os atalhos de teclado do depurador (setas, Tab, Espaço) não existem. Os mesmos efeitos estão nos comandos.

## Desempenho

- `emulated_speed` está em ~90 MHz e o boot do DOS 3.3 em ~88 MHz (tempo de CPU). `tests/no_alloc.rs` garante zero alocações por passo.
- O laço completo com `Renderer::update` fica em ~15 MHz com Color TV (15× o tempo real). O custo está no renderizador.
- `CardCtx` ainda calcula `floating_bus` de forma eager a cada chamada de carta.

## Verificação

- Nenhum arquivo de save-state real do AppleWin foi testado. Os fixtures foram transcritos do código C++.
- As cores NTSC foram conferidas por plausibilidade, não comparadas pixel a pixel com o AppleWin.
- Gamepad, captura de mouse e áudio só foram testados por testes unitários e em Xvfb, sem hardware real.
