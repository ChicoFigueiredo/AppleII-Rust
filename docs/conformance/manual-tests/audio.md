# Roteiro manual: escuta de áudio (HU-0501, HU-0502)

**Estado: BLOCKED até o dono executar o roteiro e registrar o resultado.** Este roteiro cobre o que a comparação automática com o AppleWin não decide: o timbre, a ausência de estalos e a inteligibilidade da fala. A comparação automática (eventos exatos, PCM com métricas) está em [test-oracles.md](../test-oracles.md) e [audit/audio-differential.md](../audit/audio-differential.md). Um "soa bem" aqui nunca transforma um FAIL numérico em PASS (política de áudio do test-oracles.md).

## Antes de começar

- Hardware: placa de som do computador, fones ou caixas. Volume do sistema em nível médio.
- Executável: `cargo run --release -p applewin` (Linux, Windows ou macOS; anote qual).
- Configuração: Configuration > Sound, volume do Mockingboard e do alto-falante em 0 (mais alto). Sem silenciar nada.
- Anote: sistema operacional, dispositivo de áudio, taxa de amostragem do dispositivo e o SHA do AppleII-Rust (`git rev-parse HEAD`).
- O mesmo roteiro, quando possível, no AppleWin de referência (`a8f0d1d`, no Wine ou no Windows) com as mesmas imagens, para comparar de ouvido.

Cada passo tem **o que ouvir**, a **configuração** e o **esperado**. Marque PASS, FAIL ou UNVERIFIED com uma frase; não marque PASS por dúvida.

## 1. Alto-falante (programa BASIC)

- Configuração: Apple //e Enhanced, sem cartas de som. Digite `10 FOR I=1 TO 300:X=PEEK(49200):NEXT:GOTO 10` e `RUN`.
- Ouvir: um tom grave e áspero, contínuo, sem falhas.
- Esperado: o tom é estável (sem "engasgos" a cada quadro). Ao parar com Ctrl+C o som cessa em até 1 s, sem clique forte. Repetir com `POKE 49200,0` em laço: cliques separados.

## 2. Mockingboard, notas do AY

- Configuração: slot 4 = Mockingboard C. Carregar o disco `Cybernoid.dsk` (fixture do AppleWin-Test) ou `mbtest-poll.po` do cache `target/conformance-cache/applewin-test/v2/Mockingboard/`.
- Ouvir: música com melodia, baixo e percussão; canal esquerdo e direito distintos (fones).
- Esperado: afinação correta (sem desafinar ao longo do tempo), percussão (ruído) clara, sem estalos periódicos a cada ~17 ms (período de quadro) e sem silêncios.

## 3. Mockingboard, envelope e ruído

- Configuração: igual ao passo 2. Usar um programa que use o gerador de envelope (por exemplo, a demo `Cybernoid2(Phasorv3)` com Mockingboard).
- Ouvir: notas com ataque e decaimento, "zumbido" de envelope rápido.
- Esperado: envelopes suaves, sem degraus audíveis além dos do AY real (16 níveis), ruído branco sem tom periódico.

## 4. Phasor

- Configuração: slot 4 = Phasor. Fixtures `phasor-tone.aws.yaml`, `phasor-fugue.aws.yaml` e `phasor-speech.aws.yaml` (menu Load State).
- Ouvir: tom puro (`phasor-tone`), fuga a várias vozes (`phasor-fugue`), fala (`phasor-speech`).
- Esperado: o tom não varia; a fuga tem as vozes dos dois AY de cada lado; no modo nativo o Phasor soa uma oitava acima do modo Mockingboard para o mesmo programa de período (relógio do AY dobrado).

## 5. SSI263: fala (#1372, #1412)

- Configuração: slot 4 = Mockingboard C com SSI263 (padrão: um SSI263AP no soquete 1). Carregar `SSI263-4x_phonemes_playing_continuously.aws.yaml` (Menu: Load State).
- Ouvir: fonemas repetindo continuamente (4 SSI263: dois do Mockingboard C e dois do Phasor).
- Esperado (#1372): logo depois de carregar o estado, os fonemas voltam a tocar e continuam, sem silêncio permanente. Salvar e carregar de novo várias vezes: sempre volta a tocar.
- Esperado (#1412): em `SSI263-test.aws.yaml`, ao colocar o chip em power-down (CTL=1) o som para de imediato; não fica um fonema "preso".
- Observação: o timbre dos fonemas vem das mesmas amostras do AppleWin (`SSI263Phonemes.h`); a duração é a do AppleWin dentro da tolerância declarada. Se a fala soar mais "mecânica" ou com outro ritmo que no AppleWin, registre (AUD-P-010: prosódia/inflexão não são modeladas).

## 6. SC-01 (Votrax)

- Configuração: Mockingboard C com SC-01 (padrão). Fixture `SC01-2x_phonemes_playing_continuously.aws.yaml`.
- Ouvir: fonemas do SC-01 repetindo.
- Esperado: som de voz sintética reconhecível, sem silêncio, sem estalos entre repetições.

## 7. Sem estalos nas transições

- Configuração: qualquer um dos passos acima, com a janela sendo movida/redimensionada e com o Pause do emulador.
- Ouvir: estalos ou repetições do último trecho ao pausar e retomar.
- Esperado: ao pausar o som cessa; ao retomar continua sem repetir um "loop" de áudio. Nenhum estalo ao entrar e sair da velocidade máxima (F-keys da velocidade).

## Registro

Copie a tabela abaixo para o relatório da execução (um Manifesto de execução com `result` e `reviewer`), e substitua "BLOCKED" pelo resultado.

| Passo | Sistema | Dispositivo | Resultado | Observação |
|---|---|---|---|---|
| 1 Alto-falante | | | BLOCKED | |
| 2 AY, notas | | | BLOCKED | |
| 3 AY, envelope e ruído | | | BLOCKED | |
| 4 Phasor | | | BLOCKED | |
| 5 SSI263 (#1372, #1412) | | | BLOCKED | |
| 6 SC-01 | | | BLOCKED | |
| 7 Transições | | | BLOCKED | |
