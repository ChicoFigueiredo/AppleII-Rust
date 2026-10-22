# Parecer independente, rodada 3 (final): Impressora virtual Epson LX-310, fases A–D, v0.3.0

Verifiquei a `main` em `aab1f6c` só por leitura: nada editado ou commitado, worktree limpo. Usei um target isolado no scratchpad, apagado ao final. Ambiente: Linux e poppler 26.01. Pareceres anteriores: `parecer-impressora.md` (nota 78) e `parecer-impressora-2.md` (nota 87).

## 1. Execuções

| Comando | Resultado |
|---|---|
| `cargo test --release -p apple2-printer -p apple2-hostio -p apple2-savestate -p apple2-config` | 505 passaram, 0 falharam, 4 ignorados |
| `cargo test --release -p apple2-rust --lib --test printer_capture --test printer_center --test printer_window --test printer_export_faults --test state --test factory --test threaded` | 214 passaram, 0 falharam |
| `memory_peak -- --ignored` | ESC L de 16,5 MB: pico de 774 MiB |
| `spool_perf` | 1.041 eventos geram 3 cópias; primeiro traço P95 41 ms (do Spool ao snapshot) |
| `fmt --check`, `clippy -D warnings` (printer, hostio, rust) | ok |
| Diff de `apple2-hostio/src/printer.rs` desde `06fd69a` | Só a chamada da sonda no caminho virtual; caminho legado intacto |
| Diff de `apple2-savestate` e `apple2-core` desde `06fd69a` | Vazio |

## 2. Reverificação (reproduções próprias, `scratchpad/verif-lx310`)

| Ponto | Resultado | Veredito |
|---|---|---|
| ESC Z contínuo | 1.920 colunas `FF` por faixa (9 MB): **35.857.920 pontos**, coluna sim, coluna não (passo 18 a 240 dpi), contra 37.352 antes. ESC Y com 960 colunas úteis dá 17,9 M, a metade, coerente. Goldens `long_esc_{y,z,star2,star3}_{solid,alternating,random}` com 64 colunas | **Corrigido** |
| Leitura "comparar com a coluna impressa" | Ver a seção 3 | Plausível, não comprovada |
| Pico de memória | O mesmo fluxo ESC L de 16,5 MB do meu parecer 2 (pelo Spool, com leitor da prévia) cai de **2,1 GB para 798 MB de RSS** (meu `/usr/bin/time`). O teste do projeto dá 774 MiB, compatível com os 769 MiB documentados. Clonar o documento agora custa ~0 ms (`Arc<Page>`: só a Página em andamento é duplicada, via `make_mut`) | **Corrigido** |
| Listagem de 100 Páginas | 100 Páginas, 5,6 M pontos, nenhuma Página vazia, 0 diagnósticos, RSS 88 MB | Mantido |
| T-010 | 400 partições aleatórias, todos os 4.195 pontos de corte e 1 byte por vez, em 3 papéis: documentos idênticos (pontos, texto, diagnósticos) | Mantido |
| `close_request` por Trabalho | Lista com mutex, limitada a 64; a carta a consulta uma vez por fatia de emulação (`Card::update`), não por opcode, então o custo é desprezível. Testes passaram | **Corrigido** |
| capability-matrix | Linha 143 alinhada com a matriz de testes (25 PASS, T-025 BLOCKED) | **Corrigido** |
| Versão no PDF | 0.1.0 é a versão do workspace; consistente | Aceito (informativo) |
| Latência do byte até a tela | A sonda está desligada por padrão (uma leitura atômica no caminho quente). `desempenho.md` registra **P50 138 ms e P95 199 ms**, medidos do byte na carta ao `present`, em 3 execuções de cerca de 1.080 amostras cada. Atende os 250 ms de HU-IMP-032 com margem de ~50 ms | Aceito (medida dos autores; não reexecutei a janela) |
| T-018 nativo | Harness `APPLE2_RUST_PRINTER_T018` rodado no Linux (WSL, Wayland, Vulkan): Central fechada com 3 Páginas, reaberta com 8, manifesto `pages: 8`, `total_bytes` igual ao `raw.bin`, 0 diagnósticos. Ressalvas: é WSLg, não um desktop Linux nativo; o próprio programa relata (`eprintln`); e saiu aos 26 s com 472 de 600 linhas, um fechamento por `Shutdown` que não indica perda | Aceito como evidência Linux, com ressalva |

## 3. Avaliação da regra ESC Y/Z

- O que o [R] diz: na tabela C-177 os modos 2 e 3 têm "Adjacent dot printing: No", e C-188/C-190 falam em "pontos adjacentes não impressos". O texto **não diz** se a comparação é com a coluna pedida ou com a impressa.
- A leitura adotada (comparar com a coluna impressa: a agulha que não disparou pode disparar na coluna seguinte) é a inferência física mais plausível. A leitura anterior fazia sumir um traço inteiro, o que é implausível.
- Mesmo assim, segue sendo **inferência sem fonte inequívoca**. Pela regra do próprio `lx310.md` ("comando sem verificação não é SUPPORTED"), ESC Y, ESC Z e ESC * 2/3 deveriam estar como **PARTIAL quanto à regra de adjacência**, com a ressalva escrita.
- **B1 (baixa, só documentação)**: hoje `lx310.md:155-157` declara essas linhas SUPPORTED.

## 4. Regressões procuradas

Nenhuma encontrada.

- **`Arc<Page>`**: a serialização continua idêntica (serde `rc` no workspace), os goldens e o raster do PDF passaram, e a prévia rasteriza só a Página mostrada.
- **Sonda de latência**: não altera o contrato de captura; a fila de marcas é limitada a 4.096.
- **`close_requests`**: o mutex é tomado uma vez por fatia de emulação, sem impacto na CPU.
- **Spool**: `doc_bytes` só alimenta a sonda.
- **Printer.txt legado e `.aws.yaml`**: intactos.

## 5. Defeitos e lacunas restantes

- **B1, baixa**: ESC Y/Z/* 2/3 classificados como SUPPORTED com base em inferência (seção 3).
- **Lacuna T-020**: o conteúdo do Printer.txt depois de carregar um estado não é verificado por teste. O código da carta e do save-state é idêntico ao da baseline, então o risco é baixo.
- **T-025 BLOCKED**: foco, tela cheia, segundo monitor e DPI sem evidência humana. O roteiro está pronto em `docs/conformance/manual-tests/impressora.md`.
- **#104 BLOCKED**: aceite com AppleWorks, Print Shop e dBase II, por falta dos discos.
- **Sem spill-to-disk**: a fila fica com teto de 288 MiB e descarte contado e visível (divergência documentada).
- **Goldens próprios**: sem oráculo externo de glifo (ADR 0008: fonte própria como aproximação).

## 6. Gates (escopo Linux)

| Gate | Estado | O que falta |
|---|---|---|
| G0 Spec | PASS | Só B1 (reclassificação) |
| G1 Engine | PASS | – |
| G2 Desktop | **PARCIAL** | Janela nativa separada, auto-abrir opcional, roteamento de teclado (teste), T-018 nativo e latência P95 199 ms: OK. Falta **executar T-025 (passos A1–C3) num desktop Linux real** (de preferência fora do WSL, em X11 e Wayland), preenchendo o cabeçalho de evidência com capturas: tela cheia, segundo monitor, foco que não vaza para o Apple II, mudança de DPI, fechar e reabrir na mesma posição |
| G3 Output | PASS | – |
| G4 Release | **NÃO ATINGIDO formalmente** | (1) G2 completo, ou **aceite formal do dono** de T-025 pendente (G4 admite defeito "aceito formalmente"); (2) aceite formal de #104 como BLOCKED/PARTIAL (discos de terceiros); (3) "checks verdes" só locais: CI não executada por política do dono; (4) reclassificar B1 |

## 7. Nota Verônica (final)

| Critério | Peso | Nota | Justificativa |
|---|---|---|---|
| Especificação | 25 | 24 | Matrizes consistentes, desempenho e roteiros escritos; B1 |
| Corretude e evidência | 25 | 23 | Todos os defeitos das rodadas 1 e 2 corrigidos e reproduzidos por mim; T-025 sem evidência; T-018 nativo só em WSLg |
| Testes | 15 | 14 | Cobertura ampla (carga, memória, latência, hostis, goldens longos); lacuna T-020; goldens sem oráculo externo |
| Arquitetura | 10 | 10 | Domínio puro, captura tee, spool com contrapressão, `Arc<Page>`, um Trabalho por slot |
| Segurança e resiliência | 10 | 9 | Tetos de memória medidos e documentados, manifestos hostis, sem shell; sem spill |
| Entrega | 5 | 4 | Docs, exemplos e roteiro manual; aceite humano (T-025, #104) pendente |
| Eficiência | 10 | 9 | io_write ≈ 1,0× o legado, cópia ~0 ms, P95 199 ms (folga modesta sobre 250 ms) |
| **Total** | **100** | **93** | Zero blocker crítico. 90–95 = correção focada pela rubrica; não chega a `APPROVE_AND_STOP` (96+) sem T-025 |

## 8. Recomendação

- **Aprovo a entrega das fases A–D como entrega Linux da v0.3.0 "com T-025 pendente de evidência humana"**, desde que:
  - o dono registre o **aceite formal** da pendência (no #91/#93 ou num ADR);
  - as notas de versão digam explicitamente: "T-025 BLOCKED (sem evidência humana); Windows e macOS não validados; aceite com AppleWorks, Print Shop e dBase II BLOCKED".
- Não há defeito de código pendente que justifique outra rodada de agentes.
- Para chegar a 96+ (`APPROVE_AND_STOP`): o dono executa o roteiro T-025 num desktop Linux real e reclassifica B1. Isso é trabalho humano, não de agentes.
- Pela própria especificação, `BLOCKED` não pode virar `PASS` sem essa evidência.
