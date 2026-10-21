# Parecer independente: release candidate Linux v0.2.0-rc.1

Data: 21/10/2026. Escopo: Linux x86_64. Revisor: agente que não implementou nenhuma parte deste trabalho, sem acesso a conclusões de quem implementou além dos documentos de `docs/conformance/`. SHA avaliado: `37bd9b25daed4d56524af88071182fe1c7905391` (`main`; os relatórios citam `469bf94` mais a conversão da #1396 e documentos). A branch `release/v0.2.0-rc.1` (7 arquivos: versão, CHANGELOG, `release.yml`, empacotamento) não foi construída nem testada por este parecer.

Postura: desconfiar. Cada número do relatório final foi refeito, cada amostra foi aberta na fonte (issue do AppleWin, teste, C++ do AppleWin `a8f0d1d`), e os patches do Oráculo foram comparados com o código do AppleWin. Nada foi corrigido: os achados ficam aqui.

## 1. Resumo

- **Nota Verônica: 77/100** (autoavaliação do projeto: 84).
- **Gates no escopo Linux:** G0 PASS (com ressalva), G1 BLOCKED (só a CI), G2 FAIL, G3 BLOCKED.
- **Recomendação: alpha técnica.** Publicar `v0.2.0-rc.1` apenas como pré-lançamento, com o rótulo "alpha técnica para Linux" nas notas. Não sustenta beta nem stable candidate.
- **Reprodução:** todos os números de execução do relatório final foram reproduzidos exatamente (testes, clippy, fmt, 779 casos, `apply_results --check`). O ponto fraco não está no que roda, e sim no que "PASS" significa para a maior parte das issues.
- **Concordância da amostra (80 linhas):** 92,5 % geral (74 de 80), mas **87,2 % na classe PASS** (5 discordâncias em 39), acima do limite de 10 %: a classe PASS apoiada só em teste Rust deve ser reexaminada.

## 2. Método e semente

1. `git merge main` (já em dia) e symlink `target/conformance-cache` para o cache do repositório principal, sem cópias.
2. Reprodução dos comandos da seção 3.
3. Amostra estratificada por arquivo e por estado, `random.seed(20261021)` (Python 3, `random.sample`), script de verificação mantido no scratchpad da sessão. Regra: dentro de cada grupo (arquivo, estado), `max(1, round(n * tamanho_do_grupo / total))` linhas; quando passa do alvo, descartam-se linhas ao acaso. Resultado: 60 linhas de `p23-*.csv` e 20 dos outros 9 arquivos (80 no total). O universo é `docs/conformance/results/*.csv` (849 resultados individuais). **As 266 issues N/A "sem prioridade" da triagem não estão no universo da amostra**.
4. Para cada linha: título e corpo da issue por `gh api repos/AppleWin/AppleWin/issues/N` (só leitura; comentários por `.../comments` em 5 casos); existência do teste (função ou arquivo) e `#[ignore]`; execução (todos os 46 testes Rust citados rodaram e passaram no `cargo test` completo; os 33 casos de conformidade citados estão PASS no `conformance run --all`); leitura do corpo do teste para ver se testa o que a issue descreve; nos N/A, conferência no C++ do AppleWin `a8f0d1d` (clone de referência, não o fork antigo).
5. Controle negativo do harness: um caso de trace (`trace-applewin-iiplus-boot-500k`) com uma linha adulterada na referência deve FALHAR. Falhou: `linha 30001 (instrução 29982): a esperado 07 obtido 06`. O harness compara de verdade.
6. Regeneração de uma referência no Oráculo: `run-audetto-oracle.sh --model apple2p --cycles 500000` produziu o CSV com SHA-256 `48206be7...296ae`, **idêntico bit a bit** ao versionado (`applewin-apple2p-boot-500k.csv`).

## 3. Comandos e resultados

| Comando | Resultado obtido | O relatório afirma | Diferença |
|---|---|---|---|
| `cargo test --workspace --release --no-fail-fast` | 1.584 passam, 0 falham, 15 ignorados; 185 suítes (168 executáveis, 17 doctests); exit 0 | idem | nenhuma |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, sem avisos | idem | nenhuma |
| `cargo fmt --all -- --check` | exit 0 | idem | nenhuma |
| `cargo build --release -p apple2-conformance && ./target/release/conformance run --all --report target/conformance-report` | 779 casos: 774 PASS, 5 FAIL (`audio-ay-{tone,envelope,noise}-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333`), 0 BLOCKED; exit 3 | idem | nenhuma |
| `python3 tools/conformance/apply_results.py --check` | "Matriz em dia"; 849 aplicados, 0 linhas por alterar; PASS 487, FAIL 6, BLOCKED 133, N/A 489 | idem | nenhuma |

Contagem independente na matriz: 1.115 issues, 0 UNVERIFIED, 0 PASS sem teste, FAIL = #713, #781, #1145, #1377 (P3) e #1426, #1431 (P2): **não há FAIL P0 nem P1 em issue**. P0: 23 PASS e 7 BLOCKED. P1: 130 PASS, 74 BLOCKED, 12 N/A.

Dos 779 casos, 6 não são comparação com o Oráculo (`authority` = `regression` 2, `determinism` 2, `known-result` 2) e estão contados nos 774 PASS: o resumo "774 de 779" é 768 de 773 casos contra o Oráculo (5 FAIL) mais 6 de regressão/determinismo/resultado conhecido, todos PASS.

## 4. Concordância da amostra

Veredito: **A** concorda; **R** concorda com ressalva (conta como concordância); **D** discorda (PASS fraco ou tangencial, ou N/A com justificativa falsa).

| Recorte | Linhas | D | Concordância |
|---|---:|---:|---:|
| **Geral** | 80 | 6 | **92,5 %** |
| Estado PASS | 39 | 5 | **87,2 %** (discordância 12,8 %) |
| Estado N/A | 25 | 1 | 96,0 % |
| Estado BLOCKED | 15 | 0 | 100 % (cinco com ressalva de coerência) |
| Estado FAIL | 1 | 0 | 100 % (#781) |
| `p23-*` (60) | 60 | 5 | 91,7 % |
| `p23-config-outras` | 9 | 1 | 88,9 % |
| `p23-depurador` | 6 | 0 | 100 % |
| `p23-disco` | 9 | 2 | 77,8 % |
| `p23-entrada` | 8 | 1 | 87,5 % |
| `p23-nucleo` | 9 | 1 | 88,9 % |
| `p23-perifericos` | 7 | 0 | 100 % |
| `p23-ui` | 5 | 0 | 100 % |
| `p23-video` | 7 | 0 | 100 % |
| outros 9 arquivos (20) | 20 | 1 (`disco.csv`) | 95,0 % |

Tamanho pequeno: o intervalo de confiança da classe PASS é largo (39 linhas); o resultado indica **risco**, não prova.

**Classe a reexaminar inteira (discordância acima de 10 %): PASS apoiado só em teste Rust**, sobretudo em `p23-disco`, `p23-config-outras`, `p23-nucleo` e `p23-entrada`. O padrão das discordâncias é sempre o mesmo: o teste confere uma consequência indireta (persistência de configuração, lista de cartas, leitura de opção) de uma issue que pede outro comportamento, ou confere o que o implementador leu no C++ sem Oráculo. A classe N/A e a BLOCKED ficaram abaixo do limite.

Tabela completa da amostra (a ordem é a do sorteio; a observação aparece só quando não é "A"):

| # | Arquivo | Issue | Estado | Veredito | Observação |
|---:|---|---:|---|:---:|---|
| 1 | p23-config-outras | 934 | BLOCKED | R | SAPI só do Windows; N/A seria igualmente defensável |
| 2 | p23-config-outras | 30 | NOT_APPLICABLE | A |  |
| 3 | p23-config-outras | 1411 | NOT_APPLICABLE | A |  |
| 4 | p23-config-outras | 1415 | NOT_APPLICABLE | A |  |
| 5 | p23-config-outras | 1202 | NOT_APPLICABLE | A |  |
| 6 | p23-config-outras | 511 | PASS | A |  |
| 7 | p23-config-outras | 980 | PASS | A |  |
| 8 | p23-config-outras | 853 | PASS | D | issue ampla ("melhorar a GUI"); testes só conferem a lista de cartas por slot e a persistência de disquetes |
| 9 | p23-config-outras | 1 | PASS | A |  |
| 10 | p23-depurador | 53 | BLOCKED | A |  |
| 11 | p23-depurador | 482 | NOT_APPLICABLE | A |  |
| 12 | p23-depurador | 447 | NOT_APPLICABLE | A |  |
| 13 | p23-depurador | 407 | NOT_APPLICABLE | A |  |
| 14 | p23-depurador | 467 | NOT_APPLICABLE | A |  |
| 15 | p23-depurador | 81 | PASS | A |  |
| 16 | p23-disco | 515 | BLOCKED | R | BLOCKED coerente com a política, embora haja teste Rust passando (a #511 e outras ficam PASS com o mesmo tipo de teste) |
| 17 | p23-disco | 512 | BLOCKED | R | integração só do Windows; N/A seria igualmente defensável |
| 18 | p23-disco | 281 | NOT_APPLICABLE | A |  |
| 19 | p23-disco | 323 | NOT_APPLICABLE | D | a issue foi resolvida pelo PR #370 (Open Apple pula o boot do HDD, 4ac8bb3); a justificativa "igual nos dois, sem defeito" é falsa; só o firmware idêntico (Hddrvr*.bin) sustenta a paridade, sem teste |
| 20 | p23-disco | 628 | PASS | A |  |
| 21 | p23-disco | 544 | PASS | A |  |
| 22 | p23-disco | 452 | PASS | A |  |
| 23 | p23-disco | 324 | PASS | A |  |
| 24 | p23-disco | 312 | PASS | D | teste tautológico no Linux (std::fs::write sempre funciona; o port não prova que não segura o arquivo); não exercita o bloqueio do Windows |
| 25 | p23-entrada | 1079 | BLOCKED | A |  |
| 26 | p23-entrada | 147 | NOT_APPLICABLE | A |  |
| 27 | p23-entrada | 1208 | NOT_APPLICABLE | A |  |
| 28 | p23-entrada | 92 | NOT_APPLICABLE | A |  |
| 29 | p23-entrada | 558 | PASS | D | AppleWin a8f0d1d descarta a tecla por padrão (g_bAltGrSendsWM_CHAR=false, Keyboard.cpp:55,231); o port sempre entrega: divergência não registrada, sem aprovação da ADR 0006 |
| 30 | p23-entrada | 267 | PASS | A |  |
| 31 | p23-entrada | 749 | PASS | A |  |
| 32 | p23-entrada | 23 | PASS | A |  |
| 33 | p23-nucleo | 347 | BLOCKED | R | desempenho de relógio de parede; N/A seria igualmente defensável |
| 34 | p23-nucleo | 781 | FAIL | A |  |
| 35 | p23-nucleo | 548 | NOT_APPLICABLE | A |  |
| 36 | p23-nucleo | 130 | NOT_APPLICABLE | A |  |
| 37 | p23-nucleo | 590 | PASS | A |  |
| 38 | p23-nucleo | 1066 | PASS | A |  |
| 39 | p23-nucleo | 60 | PASS | A |  |
| 40 | p23-nucleo | 74 | PASS | D | a issue pede 16/32K de RAM no ][ e ROM com minúsculas; o teste só confere a leitura de -videorom e o Saturn; o próprio resultado admite o pedido de RAM como não atendido |
| 41 | p23-nucleo | 1341 | PASS | A |  |
| 42 | p23-perifericos | 236 | BLOCKED | A |  |
| 43 | p23-perifericos | 1062 | NOT_APPLICABLE | A |  |
| 44 | p23-perifericos | 554 | NOT_APPLICABLE | A |  |
| 45 | p23-perifericos | 1197 | NOT_APPLICABLE | A |  |
| 46 | p23-perifericos | 213 | PASS | A |  |
| 47 | p23-perifericos | 969 | PASS | A |  |
| 48 | p23-perifericos | 435 | PASS | A |  |
| 49 | p23-ui | 2 | NOT_APPLICABLE | A |  |
| 50 | p23-ui | 209 | NOT_APPLICABLE | A |  |
| 51 | p23-ui | 280 | PASS | A |  |
| 52 | p23-ui | 535 | PASS | A |  |
| 53 | p23-ui | 932 | PASS | A |  |
| 54 | p23-video | 298 | BLOCKED | A |  |
| 55 | p23-video | 1168 | NOT_APPLICABLE | A |  |
| 56 | p23-video | 1297 | NOT_APPLICABLE | A |  |
| 57 | p23-video | 1323 | NOT_APPLICABLE | A |  |
| 58 | p23-video | 616 | PASS | A |  |
| 59 | p23-video | 13 | PASS | A |  |
| 60 | p23-video | 997 | PASS | A |  |
| 61 | audio | 1358 | BLOCKED | A |  |
| 62 | audio | 770 | PASS | R | a paridade do núcleo do AY vale, mas o PCM misturado é Limitação aceita (D-2); PASS é de eventos e núcleo |
| 63 | cpu | 204 | BLOCKED | A |  |
| 64 | cpu | 519 | PASS | A |  |
| 65 | depurador | 476 | PASS | A |  |
| 66 | depurador | 472 | PASS | A |  |
| 67 | depurador | 217 | PASS | A |  |
| 68 | depurador | 1460 | PASS | A |  |
| 69 | disco | 668 | BLOCKED | A |  |
| 70 | disco | 1001 | BLOCKED | A |  |
| 71 | disco | 119 | PASS | R | a escrita de setor é igual ao Oráculo, mas o cenário SAVE do DOS (a issue) não roda |
| 72 | disco | 821 | PASS | D | a issue descreve INIT/Plato Spanish; os casos (cabeça além da última trilha, imagem de zero bytes) não reproduzem o cenário; o relatório admite INIT/FORMAT sem Oráculo |
| 73 | entrada-cartas | 707 | BLOCKED | A |  |
| 74 | entrada-cartas | 5 | NOT_APPLICABLE | A |  |
| 75 | entrada-cartas | 495 | PASS | A |  |
| 76 | savestate | 564 | BLOCKED | R | idem: teste Rust passa e o estado é BLOCKED por falta de Oráculo; padrão diferente do usado nos PASS |
| 77 | video | 6 | BLOCKED | A |  |
| 78 | video | 1386 | NOT_APPLICABLE | A |  |
| 79 | video | 633 | PASS | A |  |
| 80 | video | 241 | PASS | A |  |

## 5. Fidelidade: casos e patches do Oráculo

**Traces `trace-applewin-*` (3 abertos de 7): `iie-boot-2m`, `iiplus-boot-500k`, `iie-enhanced-dos33-5m`.** O campo `source` cita o comando de regeneração; o SHA-256 dos CSV de referência bate com `trace_sha256` nos casos de 2M e 5M (`bf80669f...`, `13af08ef...`, e `c77401fa...` no `ii-boot-2m`); o caso de 500k não tem `trace_sha256` (a verificação é o arquivo versionado, que regenerei idêntico). O controle negativo falhou como devia. Ressalvas: os CSV grandes ficam em `target/conformance-cache` (fora do repositório, regeneráveis com o fork); o `source` cita `fork a1e20c0` com núcleo AppleWin `3e8054b`, e não o `a8f0d1d` que a ADR 0004 declara como Oráculo (achado 2). `trace-iiplus-boot-selfref` tem `authority = "regression"` (é auto-referência do Rust, rotulada como tal).

**Disco (3 abertos): `disk-micro-seek-beyond`, `disk-woz-gruds-in-space-boot`, `disk-micro-write-sector-*`.** Tipo `trace-checkpoints`: um ponto a cada 2.000 instruções com estado e hash em cadeia das linhas intermediárias, mais SHA-256 final da imagem; cabeçalho registra o SHA do fork, o do AppleWin, o comando e o `oracle_trace_sha256`. A comparação cobre o fluxo inteiro por hash. Não regenerei os checkpoints de disco (só o trace de 500k).

**Vídeo (3 abertos): `video-style3-hires-bars-color-idealized`, `video-style0-*`, `video-shr-*`.** `framebuffer-hash` SHA-256 do framebuffer completo 600x420; o `source` traz o comando do Oráculo com `--video-style`; o fork é compilado com `-fno-strict-aliasing` (declarado). Comparam de verdade (o Rust gera o framebuffer e o hash é comparado). Não regenerei as referências de vídeo.

**Save-state do corpus (3 abertos): `savestate-corpus-A2e-enhanced-EDM`, `-CPM2.2-56k--2-mbasic`, `-LC-*`.** Fixture com SHA-256, `trace_rows_sha256` do Oráculo, 100.000 ciclos. Compara só o fluxo de CPU nessa janela (0,1 s); memória e vídeo não entram. O relatório declara isso.

**Patches do Oráculo (`tools/conformance/oracle/*.patch`).**

| Patch | O que faz | Veredito |
|---|---|---|
| `audetto-snesmax.patch` | Restaura `IORead`/`IOWrite` e `LoadSnapshot` do SNES MAX que o fork deixou como esqueleto | **Legítimo.** Conferido linha a linha com `source/SNESMAX.cpp` do `a8f0d1d`; o latch sem joystick vira `~0` como no upstream com `controller=0` |
| `audetto-ssc-rom.patch` | Restaura a carga do firmware do SSC em `InitializeIO` (fork a deixou vazia) | **Legítimo.** Idêntico ao `SerialComms.cpp:955-975` do `a8f0d1d`, exceto os handlers do 6551, que ficam `IO_Null` (documentado; os estados `SSC-*` seguem BLOCKED) |
| `audetto-version.patch` | `1,32,0,0` para `1,32,1,0` | **Legítimo.** Alinha com a versão de referência (aparece no DIB do SmartPort) |
| `audetto-trace.patch` | Gancho de trace por instrução, `g_bOracleDeterministic` (RAM de power-on sem `rand()^timeGetTime()`), P com bits 4 e 5 | **Instrumentação e normalização, não correção.** A RAM determinística e o P normalizado são convenções do Trace v1, documentadas; reduzem o que se compara |
| `audetto-audio.patch` | Ganchos de eventos e de PCM; o alto-falante não ganha voz | **Instrumentação.** Sem mudança de lógica de emulação, exceto um `CpuCalcCycles` extra no I/O do Phasor quando o gancho está ligado |
| `run-audetto-oracle.sh` (`MALLOC_PERTURB_=255`) | Preenche toda alocação com zeros | **Adaptação do instrumento ao port; ver achado 3.** |

Nenhum patch altera lógica de emulação para casar com o port, com a ressalva do `MALLOC_PERTURB_` e da RAM determinística, que escolhem um valor para dados que o AppleWin deixa indefinidos.

## 6. Achados

Severidade: Alta (invalida uma afirmação central), Média (enfraquece uma afirmação), Baixa (ressalva ou higiene).

1. **Alta: o significado de PASS foi esticado.** O README de conformidade define PASS como "um teste executado concorda com o Oráculo". Por heurística sobre `results/*.csv`, só 127 dos 488 PASS por issue citam um caso de conformidade (Oráculo executável); o resto (cerca de 74 %, limite inferior: `cpu.csv` e `memoria.csv` podem citar traces fora do campo `teste`) cita teste Rust escrito pelo implementador, cujo "esperado" vem da leitura do C++. É evidência útil, mas não é "concorda com o Oráculo", e a taxa de PASS "77,8 % das aplicáveis" mistura os dois. Ao mesmo tempo, #515 e #564 ficam BLOCKED com teste Rust passando "por falta de Oráculo": o critério não é o mesmo nos dois sentidos. Evidência: script `oracle_share.py` (127/488), amostra #7, #23, #39, #71, #75.
2. **Média: o Oráculo executável não é o declarado.** A ADR 0004 e o relatório (item 1) chamam o `a8f0d1d` de Oráculo de código; as comparações rodam num fork cujo núcleo é o `3e8054b`, 22 commits antes. Faltam, por exemplo, `d9b2bf67` (LC com Saturn em slot diferente de 0, #1520) e as mudanças de Shift Key Mod e de firmware do HDC (#1234). Está escrito em `gaps` (L90 e L199) e no README do Oráculo, mas as frases de destaque ("idêntico ao Oráculo") não carregam a ressalva. O HDC no ][+ e os estados `skm-*` ficam sem comparação por isso.
3. **Média: `MALLOC_PERTURB_=255` adapta o Oráculo ao port.** `Saturn64` e `Saturn128` só passam porque o runner zera o heap. O `a8f0d1d` também aloca os bancos com `new BYTE[kMemBankSize]` sem zerar (`LanguageCard.cpp:345,533`), então não é "defeito do fork": é comportamento indefinido do próprio AppleWin que o runner fixa no valor que o port usa. O relatório (item 4.2 c e `gaps`) chama isso de "defeito do Oráculo". A comparação é válida só para os bytes que o estado define; a documentação deveria dizer que o resto é indefinido no AppleWin. O mesmo vale, de forma documentada, para `getRandomTime` = 0.
4. **Média: critério do G3 marcado PASS com 5 FAIL P1.** "Zero FAIL P0/P1 no escopo declarado" aparece como PASS "(com 5 Limitações aceitas)". A própria ADR 0006 diz que a Limitação aceita conta como FAIL aceito, não PASS. O correto é "FAIL aceito pelo dono" (dispensa), não PASS; o Gate depende da aceitação do dono, que existe, mas o texto deveria dizer FAIL.
5. **Média: divergência não registrada em #558 (e #583/#625).** O resultado diz que o port "sempre entrega" Alt Gr + tecla, e que o AppleWin por padrão a descarta (`g_bAltGrSendsWM_CHAR = false`, `Keyboard.cpp:55,231`, confirmado). É um desvio do `a8f0d1d` marcado PASS, sem Divergência aprovada pela ADR 0006 nem entrada em `gaps`.
6. **Média: PASS fraco ou tangencial (4 de 39 na amostra).** #312 (`mounted_files_are_not_locked` é tautológico no Linux: `std::fs::write` nunca falha, e o teste não prova a ausência de bloqueio no Windows); #821 (os casos `seek-beyond` e `zero-length` não reproduzem INIT nem o Plato Spanish da issue); #74 (a issue pede RAM de 16/32K e ROM com minúsculas; o teste lê `-videorom` e o resultado admite o pedido de RAM como não atendido); #853 (issue ampla "melhorar a GUI", teste de lista de cartas). Reexame da classe, ver seção 4.
7. **Baixa-média: N/A com justificativa falsa em #323.** A issue foi fechada pelo PR #370 (Open Apple pula o boot do HDD, commit `4ac8bb3`); o resultado diz "igual nos dois, sem defeito" e só fala de Ctrl-Reset/PR#6. A paridade existe porque o firmware (`Hddrvr.bin` e `-v2`) é idêntico byte a byte ao do AppleWin, mas nenhum teste exercita o Open Apple. O estado devia ser PASS por firmware com teste, ou UNVERIFIED.
8. **Baixa-média: cobertura declarada como PASS de componentes com janelas curtas.** Os 88 estados reais PASS comparam 100.000 ciclos de fluxo de CPU (0,1 s) e não memória nem vídeo; o relatório diz isso, mas a linha "Save-state PASS" da tabela-resumo não. 27 de 115 estados seguem BLOCKED; 74 dos 216 P1 (34 %) e 7 dos 30 P0 estão BLOCKED.
9. **Baixa: riscos de dados abertos e documentados.** HDD acima de 2 MiB gravado no lugar (sem renomeação atômica), AUD-C-037 (imagens abertas por save-state de terceiros em modo escrita), sem trava de arquivo entre instâncias. Mitigação existe para o resto; nada disso foi exercitado por este parecer.
10. **Baixa: higiene da evidência.** Os traces de referência grandes (2M e 5M ciclos) ficam no cache, fora do repositório (os hashes estão nos casos); `trace-applewin-iiplus-boot-500k` não tem `trace_sha256`; a RC (`release/v0.2.0-rc.1`) não foi retestada em suíte completa (o relatório declara a suíte rodada em `main`); 6 casos de regressão/determinismo entram na contagem de "conformidade".

Não são achados: o relatório declara com clareza Windows e macOS pendentes, as 6 Divergências e as 5 Limitações, os 7 P0 BLOCKED e a ausência de CI.

## 7. Nota pela rubrica Verônica

| Dimensão | Máx. | Nota | Autoavaliação | Justificativa |
|---|---:|---:|---:|---|
| Especificação e critérios de aceite | 25 | **20** | 23 | Plano, backlog, ADRs 0004 a 0006 e matriz completa (1.115 issues, 0 UNVERIFIED) são fortes. Descontos: PASS definido como "concorda com o Oráculo" e usado também para teste Rust sem Oráculo (achado 1); G3 com critério mal rotulado (4); o Oráculo declarado não é o executado (2); 849 resultados produzidos pelos implementadores, com amostra independente só deste parecer. |
| Corretude e evidência | 25 | **19** | 21 | Tudo o que roda reproduz exatamente; o harness reprova adulteração; a referência regenerada bate bit a bit; patches de cartas conferem com o upstream. Descontos: Oráculo 22 commits atrás, normalização por `MALLOC_PERTURB_`, ~13 % de discordância na classe PASS, divergência de #558 não registrada, 133 BLOCKED, 5 FAIL de PCM/SSI aceitos, janelas curtas nos estados reais. |
| Testes e regressão | 15 | **12** | 12 | 1.584 testes, 779 casos, fuzz, controle negativo funcional. Descontos: CI nunca executada, testes tautológicos (#312), sem repetição entre plataformas, vetores 65x02 e Z80 e fuzz longo não refeitos. |
| Arquitetura e manutenção | 10 | **8** | 8 | `clippy -D warnings` e `fmt` limpos, 18 crates, núcleo headless, patches versionados e comentados. Não auditei o desenho a fundo (ver seção 9); nota mantida, sem evidência para subir nem descer. |
| Segurança, dados e resiliência | 10 | **7** | 8 | Escrita atômica, tetos de descompressão e fuzz documentados com testes que rodam. Descontos: HDD no lugar, AUD-C-037 aberto, sem trava de arquivo, queda real de energia não testada, pacotes sem assinatura. |
| Higiene da entrega | 5 | **4** | 4 | Commits em português com `[skip ci]`, nada de tag, push ou CI acionados. Desconto: RC não retestada por completo, hashes de referência fora do repositório. |
| Eficiência/retrabalho | 10 | **7** | 8 | Vários ciclos de correção fechados; PCM aceito em vez de resolvido; a conversão de FAIL em PASS por teste do comportamento aprovado (#1396) é legítima pela ADR 0006, mas soma à impressão de nota subindo por decisão, não por evidência. |
| **Total** | **100** | **77** | **84** | |

Leitura: abaixo de 89, o §18 manda replanejar o que segura a nota (aqui, a semântica de PASS e a classe de PASS apoiada só em teste Rust), não repetir tentativas. Nota alta não substitui Gate.

## 8. Gates no escopo Linux x86_64

Windows e macOS: pendentes (o dono testa em hardware nativo); não contam como FAIL. Uma Limitação aceita conta como FAIL aceito pelo dono. Divergência aprovada (ADR 0006) conta como PASS quando o teste do comportamento aprovado passa.

| Gate | Estado | Justificativa |
|---|---|---|
| **G0 Inventário** | **PASS**, com ressalva | 1.115 de 1.115 issues com estado e justificativa; matriz em dia. Ressalva: a amostra independente achou 12,8 % de discordância na classe PASS, o que pelo próprio critério ("toda diferença provoca o reexame da classe") obriga o reexame antes de o G0 valer sem restrição. As 266 N/A da triagem não foram amostradas aqui. |
| **G1 Alpha técnica** | **BLOCKED** | `fmt`, `clippy`, `cargo test` e a conformidade reproduzem verde; falta só a CI do GitHub (nunca acionada; o dono dispara). Nenhum critério em FAIL. |
| **G2 Beta funcional** | **FAIL** | Matriz incompleta (clones, VidHD em ][/][+, SHR em PAL); 7 P0 BLOCKED sem aceite do dono; áudio, entrada e interface sem evidência de hardware; PCM/SSI em FAIL aceito. |
| **G3 Stable candidata** | **BLOCKED** | Zero FAIL P0/P1 em issue é verdadeiro e reproduzido; os 5 FAIL P1 do harness são Limitação aceita (FAIL aceito, não PASS; achado 4). Depende de G2 (FAIL), do aceite do dono a este parecer e dos testes de pacote em outros sistemas. |

## 9. O que este parecer NÃO verificou

- Windows nativo e macOS (build, pacote, janela, áudio, DPI, gamepad), Wine, CI do GitHub ou do GitLab, `release.yml`.
- A branch `release/v0.2.0-rc.1` (CHANGELOG, versão, empacotamento), o tarball, o SBOM e os hashes publicados no relatório. Não gerei o pacote.
- Os vetores 65x02 (10,22 milhões) e Z80 (1,6 milhão, ZEXALL), que exigem downloads de 2,5 GB e 1,3 GB; os 15 testes ignorados, incluindo os 5 do depurador que reproduzem os FAIL, não foram executados com `--ignored`.
- A regeneração das referências de disco (checkpoints), de vídeo, de áudio e dos save-states no Oráculo: só o trace de 500k do ][+ foi regenerado e confere. Os demais hashes foram lidos, não refeitos. A integridade do binário do Oráculo em cache não foi auditada além do que o `run-audetto-oracle.sh` mostra.
- As 769 linhas de resultado fora da amostra (849 menos 80) e as 266 N/A da triagem. A discordância medida é de amostra e não vale como taxa da população com precisão.
- O corpo completo das issues (li os primeiros 420 caracteres e os comentários finais de 5) e a validade dos números de linha do C++ citados nos resultados, exceto os que conferi.
- Desempenho, memória, fuzz de longa duração, queda de energia real, Wayland, hardware físico (gamepad, serial, áudio audível), WASM, DAP, rewind.
- Qualidade do desenho interno dos 18 crates além do que `clippy` e a estrutura de testes mostram.
- A conformidade legal das ROMs e das fixtures (a proveniência foi conferida só para `Hddrvr*.bin`).

## 10. Recomendação de release

**Alpha técnica.** O trabalho verificável é sólido: tudo o que o relatório diz ter rodado roda e reproduz; não há FAIL P0/P1 em issue; os 5 FAIL de áudio PCM/SSI são honestamente marcados. Publicar `v0.2.0-rc.1` como **pré-lançamento** é aceitável se as notas disserem "alpha técnica para Linux; Windows e macOS não verificados; Limitações aceitas e Divergências aprovadas listadas". Não chamar de beta (G2 em FAIL) nem de stable candidate (G3 não atingido).

Antes de elevar o rótulo, em ordem:

1. Reexaminar a classe "PASS apoiada só em teste Rust" nos arquivos `p23-disco`, `p23-config-outras`, `p23-nucleo`, `p23-entrada`; separar nos relatórios "PASS (Oráculo)" de "PASS (teste do port)".
2. Registrar a divergência de #558 (e #583/#625) como Divergência aprovada ou alinhar o padrão ao AppleWin; corrigir a justificativa da #323.
3. Dizer, onde "defeito do Oráculo" aparece para Saturn, que o AppleWin `a8f0d1d` também deixa o heap sem zerar; e carregar a ressalva "Oráculo no núcleo `3e8054b`" nas frases de destaque.
4. Rotular o critério de G3 como "FAIL aceito pelo dono".
5. A CI no Linux, o aceite dos 7 P0 e a matriz de clones/VidHD/SHR em PAL, para o beta.

Este parecer não aprova nem publica nada: a aprovação, o PR `[release]` e a CI continuam sendo do dono.
