# Matriz de capacidades por Perfil de impressora

Um **Perfil de impressora** decide quais comandos existem e como cada byte vira pontos. Estados: **Entregue** (fases A a D, com testes), **Futuro** (issue aberta, não iniciado). Fonte da prioridade: seção 2.2 de `docs/prompts/AppleII_Rust_Prompt_Impressoras_Virtuais_LX310.md`.

| Perfil | Estado | Issue | Cabeça | Interface com o Apple II |
|---|---|---|---|---|
| **Epson LX-310** | Entregue | (épico IMP) | 9 agulhas, monocromática | Paralela (Generic Printer, slot 1) |
| Epson FX-80 | Futuro | #102 | 9 agulhas | Paralela |
| Epson MX-80 / MX-80 II | Futuro | #102 | 9 agulhas (gráficos de 8 agulhas) | Paralela |
| Apple ImageWriter II | Futuro | #103 | 9 agulhas, cor opcional (ainda por verificar) | **Serial** (Super Serial Card), não a paralela |

## Interfaces e Transporte

| Perfil | Carta | Transporte 8 bits | Transporte 7 bits |
|---|---|---|---|
| LX-310 | Generic Printer paralela | Sim (fixo) e **Automático (padrão)**: firmware da Carta com bit 7 apagado, escrita direta na porta com 8 bits; o bit 7 é a agulha de cima nos gráficos | Sim (fixo, regra do AppleWin) |
| FX-80 / MX-80 | Generic Printer paralela | Previsto (mesmo caminho, incluindo Automático) | Previsto |
| ImageWriter II | Super Serial Card | Previsto (fluxo serial 8N1) | Não se aplica |

O Printer.txt é sempre 7 bits e filtrado, qualquer que seja o Perfil. O Transporte escolhido fica gravado no manifesto de cada Trabalho.

## Matriz de recursos

| Recurso | LX-310 | FX-80 / MX-80 | ImageWriter II |
|---|---|---|---|
| Linguagem | ESC/P 9 agulhas | ESC/P histórico (subconjunto a verificar; MX-80 mais restrito) | Comandos próprios da Apple |
| Passos 10, 12, 15 cpi | Sim | A definir por Perfil | A definir |
| Condensado (17 e 20 cpi), largura dupla | Sim | A definir | A definir |
| Draft e NLQ | Sim (fonte própria, aproximação) | A definir | A definir |
| Ênfase, dupla batida, sublinhado, itálico | Sim (geometria PARTIAL em ênfase, dupla batida e itálico) | A definir | A definir |
| Margens, tabulações, comprimento de página, FF | Sim | A definir | A definir |
| Gráficos de bit-image | ESC K, L, Y, Z, `*` (m = 0 a 6), `^` | ESC K/L e correlatos (8 agulhas no MX-80) | A definir |
| Tabelas de caracteres | Variantes do ESC R, PC437, PC850, PC860, BRASCII, ABICOMP (PARTIAL: efeito depende da fonte) | A definir | A definir |
| Caracteres definidos pelo usuário (`ESC &`) | Não (diagnóstico) | A definir | A definir |
| Código de barras, ESC/P 2 | Não (diagnóstico) | Não existe | Não existe |
| Cor | Não (monocromática); a Fita pode tingir só o visual | Não | Verificar |
| Papel | Contínuo 9,5" x 11" (padrão), A4, Carta | Igual | Igual |
| Saída | Prévia, PDF, Arquivo de impressões | Igual | Igual |

"A definir" significa que o Perfil só ganha uma linha de "Sim" depois de ter protocolo verificado em fonte primária e goldens próprios, como a LX-310. Comando por comando da LX-310: [protocols/lx310.md](protocols/lx310.md). Reimprimir um Trabalho com outro Perfil será uma nova versão do mesmo Trabalho (os bytes originais não mudam); hoje só existe o Perfil LX-310.
