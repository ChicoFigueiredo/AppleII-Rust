# Laboratório educacional

O laboratório é uma ferramenta de linha de comando, `apple2-lab`, para aprender Assembly do 6502 e do 65C02 sem precisar de nenhuma ROM da Apple. Ele faz três coisas:

- **explica** uma instrução, em português: o que ela faz, quantos bytes ocupa, quantos Ciclos gasta e o que muda nos registradores;
- **rastreia** (trace) um programa, mostrando o estado antes e depois de cada instrução, em tabela Markdown ou CSV;
- **corrige exercícios**: você escreve a solução em Assembly e o laboratório diz se ela passa nos testes.

Tudo roda numa Máquina "nua": só RAM e os vetores que o exercício define. Não há ROM da Apple, não há tela e não há teclado. Por isso os exemplos podem ser compartilhados à vontade.

## Instalação

O laboratório é compilado junto com o projeto:

```
cargo build --release -p apple2-lab
```

O executável fica em `target/release/apple2-lab`. Nos exemplos abaixo, `apple2-lab` quer dizer esse arquivo.

## Antes de começar: três ideias

- **Registradores** são as "variáveis" da CPU: `A` (acumulador), `X`, `Y` (índices), `SP` (ponteiro da pilha), `PC` (endereço da próxima instrução) e `P` (as flags).
- **Flags** são bits que dizem algo sobre o último resultado: `N` (negativo), `V` (overflow), `D` (modo decimal), `I` (interrupções desligadas), `Z` (resultado zero) e `C` (carry, o "vai um"). O laboratório mostra as flags como `NV-BDIZC`, com a letra quando ligada e um ponto quando desligada. Exemplo: `..-...Z.` quer dizer que só a flag Z está ligada.
- **Números** em Assembly de Apple II: `$1F` é hexadecimal (31 em decimal); sem o `$` é decimal. `#` antes do número quer dizer "o próprio valor" (modo imediato): `LDA #5` carrega o número 5, enquanto `LDA $05` carrega o byte que está no endereço $05.

## Explicando uma instrução (`explain`)

Passe os bytes da instrução em hexa:

```
apple2-lab explain "A9 05"
```

Saída:

```
$0800: A9 05     LDA #$05
  Mnemônico: LDA
  Modo de endereçamento: Imediato
  Tamanho: 2 byte(s); opcode $A9
  Ciclos: 2
  Flags afetadas: N, Z
  O que faz: Carrega o valor imediato $05 no acumulador (A).
  Efeito com os registradores dados:
    - A: $00 -> $05 (5 em decimal)
    - PC: $0800 -> $0802 (próxima instrução)
```

Você também pode dar o código em Assembly com `--asm` (use `\n` para separar linhas) ou ler de um arquivo binário com `--arquivo programa.bin --org 0800`.

### Mudando os registradores

As opções `--a`, `--x`, `--y`, `--sp` e `--p` dão o valor inicial dos registradores, e `--mem ENDERECO=BYTES` coloca bytes na memória (endereço sempre em hexa). Isso importa porque o custo e o efeito de uma instrução dependem deles. Veja o *page crossing*:

```
apple2-lab explain "BD F0 03" --x 0x10
```

`LDA $03F0,X` normalmente leva 4 Ciclos. Com X = $10 o endereço efetivo é $0400, que está em outra página de 256 bytes, e o 6502 gasta um Ciclo a mais para corrigir o endereço. O laboratório mostra `Ciclos: 5 (base 4, mais um extra: ... page crossing)`.

Os desvios (`BNE`, `BEQ` etc.) custam 2 Ciclos se não desviam, 3 se desviam e 4 se desviam para outra página. Mude `--p` para testar os dois caminhos.

### Várias instruções e a variante da CPU

- `-n 5` explica 5 instruções seguidas (os desvios seguem o fluxo real).
- `--cpu 65c02` troca o 6502 pelo 65C02 do Apple //e Enhanced: aparecem instruções como `BRA`, `PHX` e `STZ`.

Os Ciclos vêm de executar a instrução de verdade no núcleo do emulador, sobre uma cópia da memória. Por isso o laboratório nunca discorda do emulador.

## Rastreando um programa (`trace`)

Crie um arquivo `soma.asm`:

```
        ORG $0800
        LDA #5
        CLC
        ADC #7
        STA $0300
        BRK
```

e rode:

```
apple2-lab trace soma.asm
```

Saída (Markdown):

```
| # | Endereço | Bytes | Instrução | Ciclos | A | X | Y | SP | P (NV-BDIZC) | Memória |
|--:|---|---|---|--:|---|---|---|---|---|---|
| 1 | $0800 | A9 05 | `LDA #$05` | 2 | **00 -> 05** | 00 | 00 | FF | ..-..... |  |
| 2 | $0802 | 18 | `CLC` | 2 | 05 | 00 | 00 | FF | ..-..... |  |
| 3 | $0803 | 69 07 | `ADC #$07` | 2 | **05 -> 0C** | 00 | 00 | FF | ..-..... |  |
| 4 | $0805 | 8D 00 03 | `STA $0300` | 4 | 0C | 00 | 00 | FF | ..-..... | $0300: $00->$0C |
```

O que **mudou** aparece em negrito, no formato `antes -> depois`. A última coluna mostra as escritas na memória.

Opções:

| Opção | Efeito |
|---|---|
| `-n N` | Máximo de instruções (padrão 50). O trace para antes de um `BRK` e num laço de uma instrução. |
| `--formato csv`, `md`, `ambos` | Formato de saída (padrão `md`). |
| `--saida nome` | Grava `nome.csv` e/ou `nome.md` em vez de escrever na tela. |
| `--hex "A9 05 ..."` | Usa bytes em vez de um arquivo `.asm`. |
| `--org END`, `--cpu`, `--a`, `--x`, `--y`, `--sp`, `--p` | Como em `explain`. |

O CSV tem uma linha por instrução, com os registradores antes e depois em hexa (`pc_antes`, `a_antes`... `p_depois`), a lista do que mudou e as escritas na memória. Abra no LibreOffice, no Excel ou numa planilha online para filtrar e fazer gráficos.

## Escrevendo Assembly no laboratório

O montador é o mini-assembler do depurador, com algumas facilidades para arquivos de várias linhas:

```
        ORG $0800        ; onde o código será montado
VALOR   = $0300          ; constante
INICIO: LDA VALOR        ; rótulo com ":" ...
LOOP    DEX              ; ... ou sem, na coluna 0
        BNE LOOP         ; rótulos podem ser usados antes de definidos
        .BYTE $01,$02    ; bytes soltos
        .WORD INICIO     ; palavra de 16 bits (byte baixo primeiro)
```

- Comentários começam com `;`.
- Rótulos ficam na coluna 0. Instruções precisam de pelo menos um espaço antes.
- Números sem `$` são decimais.
- Para acumulador, escreva `ROL A` ou só `ROL`.
- Termine o programa com `BRK` ou com um laço de uma instrução (`FIM JMP FIM`). O laboratório entende os dois como "terminei".
- A RAM utilizável vai de $0000 a $BFFF. Evite $C000-$CFFF (I/O do Apple II).

## Exercícios (`check`)

Um exercício é um arquivo TOML com o enunciado e os casos de teste. Para corrigir uma solução:

```
apple2-lab check crates/apple2-lab/exercicios/01-somar.toml minha-solucao.asm
```

Saída de uma solução correta:

```
Exercício: Somar dois números
  [OK] 5 + 7 (4 instruções, 14 ciclos; parou em BRK)
  [OK] 200 + 100 passa de 255 (carry ligado) (4 instruções, 14 ciclos; parou em BRK)
  [OK] zero mais zero (4 instruções, 14 ciclos; parou em BRK)
Resultado: APROVADO (11 byte(s) de código)
```

E de uma solução errada:

```
  [FALHOU] 5 + 7 (...)
      - memória $0302: esperado $0C, obtido $FE
```

O comando termina com código 0 quando aprova e 1 quando reprova, então serve em scripts e na integração contínua.

### Exercícios incluídos

Ficam em `crates/apple2-lab/exercicios/`. Cada `.toml` tem uma solução de referência `.asm` com o mesmo nome. **Tente resolver antes de olhar a solução.**

| Arquivo | Assunto |
|---|---|
| `01-somar` | Somar dois bytes com `CLC` e `ADC`; o carry. |
| `02-laco-contador` | Laço com contador em X: soma de 1 até N. |
| `03-copia-bloco` | Copiar N bytes com modo indexado (`LDA end,X` / `STA end,X`). |
| `04-subrotina` | `JSR` e `RTS`: chamar uma rotina pronta e deixar a pilha equilibrada. |
| `05-decimal` | Modo decimal (BCD): `SED`, `ADC`, `CLD`. |
| `06-maior-de-dois` | Comparação e desvio (`CMP`, `BCS`), com limite de 40 Ciclos. |

### Escrevendo seu próprio exercício

```toml
titulo = "Dobrar um número"
enunciado = "Leia o byte em $0300, dobre-o e guarde em $0301. Termine com BRK."
dica = "ASL desloca os bits para a esquerda: multiplica por 2."
cpu = "6502"          # ou "65c02"
org = 0x0800          # onde a solução é montada; o RESET aponta para cá
max_instrucoes = 1000 # protege contra laços infinitos
# max_ciclos = 40     # opcional: desafio de desempenho
# fim = 0x08FF        # opcional: para quando o PC chega aqui (alternativa ao BRK)

# Código pronto carregado antes da solução (opcional): "assembly" ou "hex"
# [programa_inicial]
# endereco = 0x0900
# assembly = "ROTINA RTS"

[[caso]]                          # um ou mais casos de teste
nome = "21 dobra para 42"
[[caso.entrada.memoria]]          # estado antes de rodar
endereco = 0x0300
bytes = "15"
# [caso.entrada.registradores]    # a, x, y, sp, pc (os que faltam não são tocados)
# [caso.entrada.flags]            # n, v, d, i, z, c
[[caso.esperado.memoria]]         # condição de sucesso depois
endereco = 0x0301
bytes = "2A"
# [caso.esperado.registradores]   # a, x, y, sp, pc
# [caso.esperado.flags]           # c = true, z = false, ...
```

Tudo que o `esperado` não menciona é ignorado. Use vários casos para que a solução não possa "decorar" a resposta, incluindo valores de borda (zero, 255, carry).

## Limites

- Só a CPU e a RAM são simuladas: não há vídeo, teclado, disco nem som.
- Programas que leem ou escrevem em $C000-$CFFF e acima de $D000 não fazem sentido na Máquina nua.
- As instruções não documentadas do 6502 NMOS são explicadas, mas só com a frase genérica "instrução não documentada".
- Interrupções (IRQ e NMI) só acontecem por `BRK`; os vetores IRQ e NMI do exercício apontam, por padrão, para `org`.
