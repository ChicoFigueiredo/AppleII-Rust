# Depuração no VS Code (DAP)

O programa `applewin-dap` é um servidor do Debug Adapter Protocol. Ele deixa o VS Code (ou qualquer cliente DAP) depurar um programa 6502 numa Máquina sem janela e sem áudio. **É uma extensão do AppleII-Rust: o AppleWin não tem nada parecido.**

O adaptador cria e controla a própria Máquina. Ele não se liga à janela do emulador que está aberta. `attach` significa "ligue-se a uma Máquina headless que o adaptador mesmo cria".

## Compilar e instalar

```
cargo build --release -p apple2-dap
```

O executável fica em `target/release/applewin-dap`. A pasta `tools/vscode-applewin/` tem uma extensão mínima, que não é publicada. Para usá-la, abra a pasta no VS Code e pressione F5, ou copie a pasta para `~/.vscode/extensions`. A configuração `applewin.adapterPath` aponta para o executável.

## launch.json

Um exemplo completo está em `tools/vscode-applewin/examples/launch.json`.

| Campo | Significado |
| --- | --- |
| `model` | `apple2`, `apple2plus`, `apple2jplus`, `apple2e`, `apple2eenhanced`, `pravets82`, `pravets8m`, `pravets8a`, `base64a`, `tk3000e` (padrão: `apple2eenhanced`) |
| `rom` | arquivo de ROM de sistema que substitui a do Modelo |
| `disks` | até duas imagens para o Disk II do slot 6 |
| `load` | lista de `{ "path", "address" }`: binários copiados para a RAM antes de começar |
| `pc` | PC inicial (por exemplo `"0x0800"`); sem ele a Máquina começa pelo vetor de reset |
| `stopOnEntry` | para na primeira instrução (padrão em `launch`) |

A Máquina é ligada do zero (power cycle) antes de os binários serem carregados.

## O que o adaptador suporta

- **Execução:** `continue`, `pause`, `next` (passa por cima de um JSR), `stepIn`, `stepOut`.
- **Pontos de parada por endereço:**
  - `setInstructionBreakpoints`, com `instructionReference` como `"0x0305"`;
  - `setBreakpoints` com a fonte `memory` (o campo `line` é o endereço);
  - `setDataBreakpoints`, para leitura, escrita ou ambas, com o `dataId` que `dataBreakpointInfo` devolve para um endereço. Uma leitura de busca de opcode também conta como leitura.
  
  Pontos de parada em linhas de um arquivo-fonte não são suportados: o adaptador responde `verified: false`.
- **Pilha:** `stackTrace` mostra o PC e, abaixo dele, os endereços de retorno de JSR que encontra na pilha 6502 (heurística).
- **Registradores e flags:** escopos `Registers` (A, X, Y, SP, PC, P, Cycles) e `Flags` (N V - B D I Z C).
- **Memória:** `readMemory` e `writeMemory`. A leitura não tem efeito colateral nos soft switches. A escrita ignora ROM e a faixa de E/S (`$C000-$CFFF`), como o `poke` da Máquina.
- **Desmontagem:** `disassemble`, com o desmontador do depurador nativo.
- **Avaliação:** `evaluate` aceita expressões do depurador (como `A+1` ou `PC`, avaliadas com `CALC`) e alguns comandos de consulta: `CALC`, `U`, `MD`, `MDB`, `SYM`, `HELP`, `BPL`, `WL`, `ZPL`, `VER`. A entrada de memória `0300:A9 00` também vale. Qualquer outra coisa é tratada como expressão.

## Segurança

O adaptador **não executa código arbitrário fora do emulador**: nenhum `evaluate` inicia um processo ou toca em arquivos do computador (o depurador interno é criado sem acesso a arquivos). Os únicos arquivos que o adaptador abre são os que o `launch.json` pede (`rom`, `disks`, `load`).

## Consistência com o depurador nativo

Os testes automatizados rodam a mesma sequência de passos (`stepIn`, `next`, `stepOut`) pelo adaptador e pelo `apple2-debugger` e comparam todos os registradores a cada passo.

## Limitações

- Uma thread (a CPU 6502), sem exceções, sem condições nos pontos de parada e sem `setVariable`.
- O adaptador não toca som nem desenha a tela.
- Os cartões de rede, a porta serial real e a impressora não são ligados ao host.
