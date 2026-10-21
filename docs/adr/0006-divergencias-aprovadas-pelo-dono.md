# Divergências aprovadas pelo dono e limitações aceitas

A ADR 0004 diz que o AppleWin no SHA congelado vence, e que desviar dele exige prova de hardware (Divergência intencional). A auditoria achou casos em que seguir o AppleWin à risca significa reproduzir um defeito dele que só prejudica o usuário, sem nada a ver com fidelidade ao hardware: tabelas Pravets gravadas com o encoding errado, um script de depurador que roda antes da memória existir, comandos de depurador que não fazem nada. Também achou FAILs que vêm do hospedeiro do AppleWin, não da emulação. Em 21/10/2026 o dono decidiu como tratar essas duas classes.

## Divergência aprovada

Um ponto pode se afastar do AppleWin sem prova de hardware quando o dono aprovar explicitamente, caso a caso. Cada Divergência aprovada fica listada em `docs/conformance/gaps-and-known-limitations.md`, com a issue, o comportamento do AppleWin, o do port, a data e a decisão, e também em `docs/manual/12-diferencas-em-relacao-ao-applewin.md`. A regra não vale para a emulação do hardware (CPU, memória, vídeo, disco, áudio, cartas): ali só a Divergência intencional, com prova, permite desviar.

Aprovadas em 21/10/2026:

- **#916:** conversão Pravets latim/cirílico em CP-1251 correto (o AppleWin a8f0d1d tem as tabelas em UTF-8).
- **#855:** o `DebuggerAutoRun.txt` roda com a máquina já inicializada.
- **#1396:** Alt+F2 mantém o Open Apple apertado (o AppleWin o solta em `JoyReset`).
- **Tela cheia:** preenche a altura do monitor (#280, #358, #1374, #117, #932, #1448), com a opção de escala inteira do AppleWin disponível.
- **Depurador, defeitos baratos de ferramenta:** #114, #245, #458, #466, #468 e #1381. Os outros (#713, #1145, #1377) mantêm a paridade; #1426 e #1431 ficam adiados.

## Limitação aceita

Um FAIL conhecido pode deixar de bloquear a release quando o dono aceitar. Ele continua FAIL nos relatórios e no harness (`known_divergence`), com a causa registrada, e nunca é contado como PASS.

Aceitas em 21/10/2026: os 5 casos de PCM/SSI de áudio (`audio-ay-tone-pcm`, `audio-ay-envelope-pcm`, `audio-ay-noise-pcm`, `audio-speaker-clicks-pcm`, `audio-ssi263-phonemes-p333`). Neles os eventos de 6522, AY e SSI263 são idênticos ao AppleWin, ciclo a ciclo, mas as amostras diferem porque o AppleWin gera o som em fatias de tempo do hospedeiro.

## Consequências

- O vocabulário ganha dois termos: Divergência aprovada e Limitação aceita (GLOSSARY.md).
- Os gates do §18 contam uma Limitação aceita como FAIL aceito pelo dono, não como PASS. A recomendação de release diz isso.
- Uma nova Divergência aprovada exige decisão nova do dono. Um agente não pode aprovar sozinho.
