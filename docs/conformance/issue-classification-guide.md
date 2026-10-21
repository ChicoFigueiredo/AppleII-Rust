# Guia de classificação das issues upstream do AppleWin

Regras para os agentes de triagem da HU-0001. Cada agente recebe um lote
(`target/conformance-cache/upstream/batches/batch-NN.json`, com o corpo completo) e
preenche as colunas de `docs/conformance/applewin-issue-matrix.csv` das issues do seu lote.
Vocabulário de estados: ver "Conformidade" no `GLOSSARY.md`.

Regra geral: classificar pelo que a issue descreve, lendo título, rótulos e corpo. Na dúvida,
escolher o valor mais conservador (`UNVERIFIED`, prioridade mais alta) e explicar em `evidencia`.
Nunca inventar mapeamento: se não achou o módulo no código, escrever `nenhum`.

## Colunas já preenchidas pela coleta (não alterar)

`upstream_id, tipo, estado_upstream, labels, criada, fechada, titulo, commit_correcao, prs_vinculados`.
`prs_vinculados` é heurístico (PR que cita a issue com palavra de fechamento ou no título); pode
estar incompleto. `commit_correcao` vem dos eventos da issue (`closed` com `commit_id`, senão o
último `referenced`); pode estar vazio.

## aplicabilidade (um valor)

| Valor | Quando usar |
|---|---|
| `runtime` | Comportamento emulado: CPU, memória, vídeo, disco, áudio, cartas, periféricos |
| `formato` | Arquivos: imagens de disco (DSK, NIB, WOZ, HDV, 2MG), save-state, arquivos de configuração |
| `debugger` | Depurador, simbolos, assembler/disassembler, breakpoints |
| `ui` | Interface do usuário (menus, diálogos, atalhos, tela cheia) |
| `plataforma-windows` | Específico do Win32: DirectX, registro, instalador, drivers, versões do Windows |
| `build` | Compilação, Visual Studio, CI, dependências, empacotamento |
| `documentacao` | Ajuda, manuais, textos, licença |
| `feature-futura` | Pedido de recurso sem comportamento de referência a verificar |
| `pergunta-suporte` | Dúvida de uso, relato sem defeito do emulador |
| `duplicada` | Duplica outra issue (citar o número em `justificativa_na`) |
| `invalida` | Sem informação suficiente, não reproduzível por descrição, spam |

## categoria (um valor)

`cpu, memoria, video, disco, hdd, audio, mockingboard, ssi263, z80, mouse, joystick, teclado,
serial, impressora, rede, relogio, vidhd, savestate, config, debugger, ui, outra`.
Usar `outra` apenas se nenhuma servir; para `build`/`documentacao`/`plataforma-windows` sem
assunto emulado, usar `ui`, `config` ou `outra` conforme o tema.

## modelo

`todos`, ou lista separada por `;` com os modelos afetados, por exemplo
`apple2e-enhanced;apple2plus`. Usar `todos` quando o defeito não depende do modelo.
Nomes de modelo (minúsculas, correspondem ao enum `Model` de `crates/apple2-core/src/model.rs`):
`apple2, apple2plus, apple2jplus, apple2e, apple2e-enhanced, apple2c, pravets8m, pravets82,
base64a, tk3000`. Quando a issue cita um clone não listado, usar o nome citado.

## prioridade

- `P0`: perda de dados, crash, ou estado errado da emulação em recurso comum.
- `P1`: fidelidade perceptível (vídeo, som, temporização visível ao software).
- `P2`: recurso secundário ou periférico raro.
- `P3`: cosmético ou futuro.

Issues `NOT_APPLICABLE` deixam `prioridade` vazia.

## mapeamento_rust

Crate e caminho do módulo do port que cobre o assunto, por exemplo
`apple2-cpu/src/lib.rs`, ou `nenhum` quando não há cobertura. Crates existentes:
`apple2-audio, apple2-cards, apple2-config, apple2-core, apple2-cpu, apple2-debugger,
apple2-hostio, apple2-roms, apple2-savestate, apple2-storage, apple2-video, applewin`.
Conferir o arquivo no repositório antes de citar. Várias entradas separadas por `;`.

## estado_verificacao

Estados do GLOSSARY: `IMPLEMENTED, PASS, FAIL, UNVERIFIED, NOT_APPLICABLE, BLOCKED`.

- Issue **fechada** de `runtime`, `formato` ou `debugger`: fica `UNVERIFIED` até existir teste
  executado. Ler o código não basta para `PASS`; existir código sem teste é, no máximo,
  `IMPLEMENTED`. Preencher `teste` e `evidencia` só quando houver teste real.
- Issue **aberta** de `runtime`: `UNVERIFIED`, ou `FAIL` se o port herda o defeito (explicar em
  `evidencia` com o trecho de código que repete o comportamento).
- `NOT_APPLICABLE` só para `plataforma-windows`, `build`, `pergunta-suporte`, `duplicada` e
  `invalida`, sempre com `justificativa_na` preenchida com razão técnica explícita (uma frase).
  `documentacao`, `ui` e `feature-futura` ficam `UNVERIFIED` (ou `BLOCKED` com causa).
- `PASS` e `FAIL` nunca são atribuídos pelo agente de triagem: dependem de teste executado.
- `revisado_por`: identificador do agente de triagem; o auditor independente da amostra
  estratificada também registra seu nome ali (separado por `;`).

## Formato do CSV

Separador `,`, aspas duplas quando necessário, UTF-8, linhas ordenadas por `upstream_id`,
listas internas separadas por `;`. Não reordenar nem remover colunas.

## Exemplos reais

**#1360** — "NMOS 6502 illegal opcode 0x0C (aka SKW)", fechada, label `bug`, commit
`143f6679b617`. `aplicabilidade=runtime, categoria=cpu, modelo=apple2;apple2plus` (NMOS),
`mapeamento_rust=apple2-cpu/src/lib.rs`, `prioridade=P1`, `estado_verificacao=UNVERIFIED` até
existir teste do opcode `$0C` (o crate tem `tests/opcode_semantics.rs` para estender).

**#1486** — "Saturn 128K card in slot 0 does not use configured size", fechada, `bug`.
`runtime`, `memoria`, `todos`, `mapeamento_rust=apple2-core/src/language_card.rs;apple2-config/src/card.rs`,
`P0` (estado errado em recurso de memória; ligada à perda de `Saturn128K.banks`),
`UNVERIFIED` até teste com 4 e 8 bancos.

**#1414** — "RamWorks III: power-cycle should reset to bank 0", fechada, labels
`Peripheral;bug`. `runtime`, `memoria`, `apple2e-enhanced;apple2e`, `P1`, `UNVERIFIED`.

**#1443** — "PAL Color TV video mode should kill color-burst for longer", fechada, `bug`.
`runtime`, `video`, `todos`, `mapeamento_rust=apple2-video/src/renderer.rs`, `P1`, `UNVERIFIED`
(exige comparação de framebuffer com métrica definida antes do teste).

**#85** — "AppleWin does not run in Windows 95", fechada. `plataforma-windows`, categoria
`outra`, `modelo=todos`, `estado_verificacao=NOT_APPLICABLE`,
`justificativa_na="Compatibilidade com um sistema operacional host obsoleto; o port é multiplataforma e não usa Win32."`,
`prioridade` vazia.

## Passos do agente de triagem

1. Ler o lote; para cada issue, ler título, labels, corpo e `commit_correcao`.
2. Preencher as colunas conforme este guia, editando apenas as linhas do lote.
3. Em caso de issue de runtime com fix conhecido, procurar o módulo correspondente no código
   para `mapeamento_rust`; não afirmar `PASS`.
4. Registrar dúvidas em `evidencia` com o prefixo `DUVIDA:`.
