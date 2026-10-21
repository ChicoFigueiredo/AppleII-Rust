# Oráculos e estratégia de testes

Este documento aplica ao projeto a estratégia de oráculos do §17 do [plano de auditoria](../prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md). Ele define contra o que cada teste compara, como a comparação é decidida e o que cada execução registra.

## Oráculos

| Oráculo | Quando se usa | Observação |
|---|---|---|
| AppleWin em `a8f0d1d` (padrão) | Qualquer comportamento que o AppleWin tem | Executáveis: AppleWin v1.32.1.0 no Wine 10 e o fork Linux do audetto com SHA fixo. Veja [baseline.md](baseline.md) |
| Especificação de hardware | Onde o AppleWin não cobre o caso | Só prevalece sobre o AppleWin como Divergência intencional, com prova de hardware |
| Fixture validada | Entrada com resultado conhecido, de proveniência documentada | Identificada por SHA-256 |
| Resultado conhecido | Valores publicados (por exemplo, a suíte de CPU de Klaus Dormann) | A fonte e a versão entram no Manifesto |

Quando o AppleWin e o hardware real divergem, o AppleWin vence (ADR 0004).

## Pirâmide de testes

Do mais barato ao mais caro. Cada nível tem seu lugar, e um nível baixo verde não substitui um nível alto.

1. **Unitários.** Registradores, opcodes, memória, Soft switches, relógios, analisadores de WOZ e YAML. Incluem casos de borda e propriedades invariantes (nenhum panic, integridade, ida e volta). Já existem em cada crate; seu resultado, sozinho, é IMPLEMENTED.
2. **Integração.** Boot da ROM, montagem e gravação de disco, expansão de RAM, vídeo, áudio, depurador e salvar e restaurar, sob sequências de entrada fixas. Rodam sem janela.
3. **Diferencial.** O AppleWin e o AppleII-Rust executam sob o mesmo Modelo, a mesma ROM, a mesma entrada e o mesmo Ciclo; comparam-se CPU, memória, barramento e Quadros. Na divergência, o relatório mostra a primeira diferença. Os Traces de referência gerados pelo AppleWin são versionados com a origem e o SHA (formato abaixo).
4. **Corpus real.** Fixtures geradas pelo próprio AppleWin (AppleWin-Test), em várias versões e Modelos. Nunca depende só de fixture escrita pela equipe. O corpus é baixado para `target/conformance-cache/` e não é redistribuído; sem rede o resultado é BLOCKED.
5. **Ponta a ponta entre plataformas.** Instalar o executável distribuído, abrir a janela, digitar, montar um disco, salvar e reabrir, gerar áudio e encerrar. Registra o que foi automatizado e o que foi validação humana. Linux roda local; Windows nativo e macOS ficam BLOCKED até o dono autorizar uma execução de CI.
6. **Robustez.** Entradas WOZ, NIB, DSK, YAML, configuração, serial e rede inválidas; limites de tamanho, tempo e memória. Nenhum panic por arquivo malformado do usuário e nenhuma mutação parcial de dados.
7. **Desempenho.** Benchmarks fixos. Não se sacrifica temporização correta por velocidade.

Testes que exigem hardware físico (gamepad, porta serial, captura de pacotes com pcap, escuta de áudio) têm roteiro manual e ficam BLOCKED até o dono executar o roteiro e registrar o resultado.

## Política de comparação

### Estado digital: igualdade exata

Compara-se valor a valor, sem tolerância: PC, A, X, Y, SP, flags, bancos de memória, a RAM relevante, eventos, a ordem das operações de E/S, o contador de Ciclos e o Quadro lógico antes de efeitos analógicos. Qualquer diferença é FAIL.

### NTSC, PAL e áudio: métricas definidas antes

A simulação analógica de vídeo e a síntese de áudio não têm igualdade bit a bit garantida. Nesses casos:

- Compara-se primeiro o que for digital: eventos e temporização exatos (toques do alto-falante, escritas nos registradores do AY, trocas de modo de vídeo).
- Para o que sobrar, a **métrica, o espaço de cor (ou a janela de áudio), o alinhamento temporal, as regiões críticas e o limiar são escritos no manifesto antes da primeira execução** e calibrados com a referência (por exemplo, o AppleWin contra ele mesmo em duas execuções). Candidatas de métrica: erro absoluto médio e SSIM para imagem; frequências, envelopes e eventos para áudio.
- O limiar não é ajustado depois para fazer um teste passar. Fora do limiar é FAIL, e o julgamento visual ou auditivo não o transforma em PASS. Mudar um limiar exige um novo registro com justificativa e vale só para as execuções seguintes.
- Todo diff gera um artefato (imagem de diferença, histograma, trecho de áudio) guardado em `tests/conformance/reports/`.

### Vídeo (HU-0201, HU-0202): framebuffer contra o AppleWin

Registrada em 2026-10-20, **antes da primeira execução** do diferencial. Vale para as execuções seguintes; qualquer mudança exige novo registro com justificativa.

- **Oráculo**: o fork Linux do AppleWin (`audetto-oracle --fb-out`, ver `tools/conformance/oracle/README.md`), que despeja o framebuffer da camada de vídeo do núcleo (`NTSC.cpp`, `Video.cpp`, `RGBMonitor.cpp`) com a atualização de vídeo por opcode ligada, como nos frontends reais.
- **Entrada**: cenas "scene v1" (`tests/conformance/inputs/video/*.scene`), geradas por `crates/apple2-video/examples/gen_scenes.rs`: bytes de RAM e de código 6502 escritos pela equipe, sem ROM de usuário. O oráculo e o Rust carregam os mesmos bytes logo após o power-on, apontam o PC para o ponto de entrada e rodam do ciclo 0.
- **Alinhamento temporal**: ambos executam até o fim do quadro N (primeira instrução que cruza `N x ciclos por quadro`) e comparam o framebuffer nesse instante; o contador de flash depende desse N (quadros 4 e 20 cobrem flash desligado e ligado).
- **Região**: o framebuffer inteiro de 600x420 (560x384 visíveis mais bordas de 20 e 18 pixels), em RGBA8 de cima para baixo. Cada caso declara se compara `visible` ou `full`; o diferencial amplo usa `full`.
- **Métrica**:
  - modos digitais (`MonoCustom`, `ColorIdealized`, `ColorVideocardRgb`, monocromáticos de fósforo, texto, lo-res, SHR): igualdade bit a bit, isto é, diferença máxima de canal = 0;
  - NTSC e PAL (`ColorMonitorNtsc`, `ColorTv`, `MonoTv`): diferença máxima de canal = 0, porque o `apple2-video` é uma tradução do `NTSC.cpp` e o algoritmo é o mesmo (aritmética inteira); MAE, SSIM e a caixa envolvente das diferenças entram **só como diagnóstico**.
- **Limiar de reprovação**: qualquer pixel diferente é FAIL. Cada diferença é investigada até a causa raiz no C++ e corrigida no Rust com teste de regressão, ou permanece FAIL (ou BLOCKED, se a causa estiver fora do alcance) com as hipóteses testadas; não se relaxa o limiar para fazê-la passar, e o julgamento visual não a transforma em PASS.
- **Calibração**: o oráculo contra ele mesmo em duas execuções deve dar o mesmo SHA-256 (conferido por `tools/conformance/oracle/video-differential.py --selfcheck`).
- **Saída**: `tests/conformance/cases/video-*.toml` (casos `framebuffer-hash` com `authority = "applewin"`, o comando do oráculo e o SHA-256 em `source`) e o registro de divergências em `docs/conformance/audit/video-differential.md`.

### Software de terceiros e testes instáveis

Jogos e software de terceiros só entram com licença adequada e procedimento reproduzível para obtê-los. Resultados que dependem de firmware registram o SHA-256 da ROM. Teste instável não é removido por conveniência: investiga-se a causa, isola-se o ambiente e registra-se a evidência.

## Trace de referência v1

É um arquivo CSV em UTF-8 com cabeçalho `cycle,pc,op,a,x,y,sp,p`. Há uma linha por instrução, gravada **antes** de executá-la.

| Coluna | Formato |
|---|---|
| `cycle` | Decimal. Ciclos acumulados desde o power-on |
| `pc` | 4 dígitos hexadecimais em maiúsculas |
| `op` | 2 dígitos hexadecimais em maiúsculas (o opcode a executar) |
| `a`, `x`, `y`, `sp`, `p` | 2 dígitos hexadecimais em maiúsculas |

Linhas começadas por `#` são metadados, por exemplo `# model=apple2e-enhanced` e `# rom_sha256=...`. Sugere-se também registrar `applewin_sha`, a origem do Trace (executável e opções) e a versão do formato nos metadados.

Exemplo (valores ilustrativos do formato, não de uma execução real):

```csv
# model=apple2e-enhanced
# rom_sha256=aab38a03ca8deabbb2f868733148c2efd6f655a59cd9c5d058ef3e0b7aa86a1a
cycle,pc,op,a,x,y,sp,p
0,FA62,D8,00,00,00,FF,34
2,FA63,A9,00,00,00,FF,34
```

A comparação para na **primeira linha diferente** e relata essa linha e as N anteriores (N configurável), para mostrar a sequência que levou à divergência. Os metadados (`#`) não entram na comparação linha a linha, mas diferenças entre eles (modelo, ROM) são avisadas, porque tornam o resultado inválido.

## Manifesto de execução

Cada execução de teste produz um Manifesto de execução: o registro de tudo que a reproduz. Os campos são os do §17 do plano, todos obrigatórios:

| Campo | Conteúdo |
|---|---|
| `test_id` | Identificador estável do teste |
| `user_story` | História (HU) a que o teste pertence |
| `acceptance_criterion` | Critério de aceite (CA) avaliado |
| `applewin_sha` | SHA do AppleWin usado como Oráculo |
| `rust_sha` | SHA do AppleII-Rust testado |
| `fixture_sha256` | SHA-256 das Fixtures de entrada |
| `rom_sha256` | SHA-256 das ROMs usadas |
| `machine_model` | Modelo da Máquina |
| `cpu_model` | 6502 ou 65C02 |
| `cards_slots` | Cartas e Slots configurados |
| `host_os` | Sistema operacional hospedeiro |
| `host_arch` | Arquitetura do hospedeiro |
| `toolchain` | Versões de rustc, cargo e do Wine, quando usado |
| `input_trace` | Sequência de entradas aplicada |
| `command` | Comando exato executado |
| `expected` | Resultado esperado, com a origem |
| `actual` | Resultado obtido |
| `diff_artifact` | Caminho ou hash do artefato de diferença |
| `result` | `PASS`, `FAIL`, `BLOCKED`, `UNVERIFIED` ou `NOT_APPLICABLE` |
| `timestamp_utc` | Data e hora da execução em UTC |
| `reviewer` | Quem revisou o resultado (não pode ser quem implementou a alteração) |

O formato é validado pelo esquema `tests/conformance/manifests/schema.json`, escrito por outra frente de trabalho. Este documento só o referencia: em caso de divergência entre a lista acima e o esquema, vale o esquema, e esta página deve ser corrigida. Os manifestos ficam em `tests/conformance/manifests/` e os relatórios em `tests/conformance/reports/`.

Um resultado `BLOCKED` também gera Manifesto, com a causa em `actual`.
