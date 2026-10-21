# Depuração temporal (rewind)

O crate `apple2-rewind` deixa o depurador voltar no tempo: retroceder uma instrução, voltar N instruções ou reexecutar adiante com as mesmas teclas. **É uma extensão do AppleII-Rust; o AppleWin não tem isso, e por isso os comandos `REWIND`, `STEPBACK` e `REPLAY` têm nomes nossos.**

## Como funciona

Uma `Timeline` conduz a Máquina no lugar do frontend (`step`, `run_cycles`, `run_frame`) e guarda duas coisas:

- **Checkpoints incrementais.** Um a cada N Quadros, e pelo menos um a cada 50.000 instruções quando a Máquina só anda por passos. A cada K checkpoints há um com o estado completo; os outros guardam só as páginas de 256 bytes da RAM que mudaram, mais o resto do estado (que é pequeno).
- **Um log de entradas:** teclas, colagem de texto, botões, paddles, reset, troca de disco e o fim de cada lote de execução, cada uma com a contagem de instruções e o Ciclo em que aconteceu.

Voltar para a instrução `t` restaura o checkpoint mais recente até `t` e reexecuta até lá, reaplicando as entradas do log. Como o núcleo é determinístico (ADR 0001), o estado fica idêntico ao original. Os testes percorrem 1000 passos de ida e volta, no boot e num programa que lê o teclado, e comparam o hash do estado em cada passo.

## Comandos do depurador

Eles ficam em `apple2_rewind::commands::execute`, que o frontend chama no lugar de `Debugger::execute`. Os números são hexadecimais, como em todo o depurador.

| Comando | Efeito |
| --- | --- |
| `REWIND n` | volta `n` instruções (1 se omitido) |
| `STEPBACK [n]` | passo reverso; sozinho, volta uma instrução |
| `REPLAY` | reexecuta adiante, com as entradas do log, até o ponto mais novo |
| `T [n]` | o `T` normal, mas registrado, para poder ser desfeito |

Qualquer outro comando que altere a Máquina (por exemplo `0300:EA`, ou `G`) abre uma "lacuna": dali para trás o histórico continua alcançável, mas a reexecução nunca atravessa a lacuna.

Se você voltar e então der uma entrada nova (uma tecla), o futuro antigo é descartado e um novo começa.

## Limites

`RewindConfig` define o limite de RAM (`max_ram_bytes`, com `with_ram_mb`) e o de número de checkpoints (`max_checkpoints`). Ao passar de qualquer um deles, o checkpoint **mais antigo** é descartado (o seguinte vira completo), e com ele o trecho do log que ficou para trás. Voltar além do mais antigo devolve o erro `OutOfRange`.

## Recursos externos: suspensos, não registrados

A rede do Uthernet, a porta serial real do SSC, a impressora, o mouse e o relógio do ThunderClock dependem do computador hospedeiro. Nós os **suspendemos** durante o replay, em vez de gravar o tráfego deles. Gravar pacotes e bytes seria caro, refletiria o hospedeiro daquele momento e não poderia ser reproduzido numa outra máquina. Na prática:

- o estado interno dessas cartas é restaurado como o de qualquer outra;
- o que chega de fora durante um replay não é gravado, e o frontend não deve alimentar essas cartas enquanto `Timeline::is_replaying()` for verdadeiro nem enquanto o cursor estiver atrás do ponto mais novo;
- `Timeline::suspended_cards` lista as cartas afetadas, para o frontend avisar o usuário.

## Limitações

- O conteúdo das imagens de disco não volta no tempo (só qual imagem estava montada); uma gravação do programa no disco continua no arquivo.
- Não há tecla no frontend: a integração é pelos comandos do depurador. (A tecla era opcional e foi deixada de fora para não mexer nos arquivos do frontend.)
