# Conformidade com o AppleWin

Esta pasta guarda a evidência de que o AppleII-Rust se comporta como o AppleWin: o que foi medido, contra o quê, com que resultado, e o que ainda não foi verificado. Um CI verde ou um recurso "implementado" não provam paridade; só um teste executado contra o Oráculo prova.

A especificação de origem é o [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). A decisão sobre o Oráculo, as Fixtures e as ROMs está na [ADR 0004](../adr/0004-oraculo-de-conformidade.md). Os termos (Estado de verificação, Oráculo, Fixture, Trace de referência, Manifesto de execução, Gate) são os da seção "Conformidade" do [GLOSSARY.md](../../GLOSSARY.md).

## Estados de verificação

Cada Critério de aceite recebe um destes estados, por Modelo e por configuração. O resultado de um Modelo não vale para outro.

| Estado | Significado |
|---|---|
| `IMPLEMENTED` | O código existe. Não há prova de que está correto. **Nunca equivale a PASS.** |
| `PASS` | Um teste executado concorda com o Oráculo. Exige evidência (Manifesto de execução). |
| `FAIL` | Um teste executado discorda do Oráculo. |
| `UNVERIFIED` | Ainda não foi testado. É o estado padrão. |
| `NOT_APPLICABLE` | Não se aplica (por exemplo, recurso exclusivo do Windows), com justificativa técnica escrita. |
| `BLOCKED` | O teste não pôde ser executado, com a causa (sem rede, sem hardware físico, sem autorização para rodar CI no macOS). Não é PASS nem teste pulado. |

Um item só muda para PASS com evidência. Dúvida ou falta de teste é UNVERIFIED, nunca PASS.

## Arquivos desta pasta

| Arquivo | Conteúdo |
|---|---|
| [baseline.md](baseline.md) | SHAs congelados (AppleII-Rust, AppleWin, AppleWin-Test, fork do audetto), data de coleta, toolchain e sistema hospedeiro |
| [rom-provenance.md](rom-provenance.md) | Cada ROM do repositório, com tamanho, SHA-256 e origem no AppleWin |
| [test-oracles.md](test-oracles.md) | Pirâmide de testes, política de comparação, formato do Trace de referência e Manifesto de execução |
| [gaps-and-known-limitations.md](gaps-and-known-limitations.md) | Lacunas e limitações conhecidas no vocabulário de Estados de verificação, e a lista de Divergências intencionais |
| [release-readiness.md](release-readiness.md) | Gates G0 a G3, rubrica Verônica e estado atual de cada Gate |
| `capability-matrix.md` | Matriz de capacidades por Modelo (produzida por outra frente de trabalho) |
| `applewin-issue-matrix.csv` | Matriz das issues do AppleWin (idem) |
| `issue-audit.md` | Auditoria das issues (idem) |
| `upstream-open-issues-triage.md` | Triagem das issues abertas do upstream (idem) |

Os quatro últimos podem ainda não existir no seu checkout.

Os testes, as entradas e os relatórios gerados ficam em [tests/conformance](../../tests/conformance/README.md). Os nomes dos arquivos são em inglês (seguem o plano), e o conteúdo é em português.

## Como ler um resultado

1. Veja o Estado de verificação do item.
2. Se for PASS ou FAIL, siga o Manifesto de execução: ele diz o SHA do AppleWin, o SHA do AppleII-Rust, as Fixtures, as ROMs, o Modelo, o comando e o resultado.
3. Se for BLOCKED, leia a causa e o que falta para desbloquear.

## Regras

- Nenhum item vira PASS sem evidência reproduzível.
- Divergir do AppleWin só com prova de hardware e registro como Divergência intencional.
- Toda limitação efetiva entra em [gaps-and-known-limitations.md](gaps-and-known-limitations.md) e no README principal.
