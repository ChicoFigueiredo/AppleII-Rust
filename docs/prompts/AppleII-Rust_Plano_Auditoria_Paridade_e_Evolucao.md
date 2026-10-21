# PROMPT MESTRE — AppleII-Rust: auditoria, paridade AppleWin e evolução verificável

**Versão:** 1.0 — 20/10/2026  
**Projeto-alvo:** https://github.com/ChicoFigueiredo/AppleII-Rust  
**Referência funcional:** https://github.com/AppleWin/AppleWin  
**Suíte histórica:** https://github.com/AppleWin/AppleWin-Test  
**Destinatários:** equipe de engenharia, agentes de coding, responsável por QA e auditor independente.

> **INSTRUÇÃO DIRETA À EQUIPE:** Trabalhem em Spec-Driven Development (inventário → especificação → plano → tarefas → testes → implementação → validação independente). O objetivo é demonstrar, com evidências reproduzíveis, que o AppleII-Rust emula fielmente os modelos Apple II suportados, possui comportamento consistente com o AppleWin original e funciona em Windows, macOS e Linux. Não afirmem que algo foi portado apenas porque a classe, crate, interface ou método existe. Não marquem nenhuma issue como resolvida sem prova. A intenção de “Apple I ou superior” deve ser interpretada como **Apple II e seus modelos/variantes suportados, com possibilidade futura de Apple I mediante decisão explícita de escopo**; Apple I é uma arquitetura diferente e NÃO passa a ser requisito obrigatório implicitamente.

## 1. Resultado esperado e regras inegociáveis

Produzir: (a) matriz auditável de compatibilidade; (b) backlog de épicos, histórias, bugs e tarefas técnicas; (c) suíte automatizada de conformidade; (d) relatórios de cobertura e divergência; (e) melhorias por prioridades; (f) documentação técnica e de usuário; (g) releases reproduzíveis; (h) parecer independente de prontidão.

Regras:

1. Comparar commits fixados por SHA de ambos os repositórios e registrar data de coleta; não comparar `main` com `master` móveis sem congelar os SHAs.
2. Separar `IMPLEMENTED`, `PASS`, `FAIL`, `UNVERIFIED`, `NOT_APPLICABLE` e `BLOCKED`; `IMPLEMENTED` **não equivale** a `PASS`.
3. Identificar modelo e configuração exatos por teste: Apple II, II+, IIe, IIe Enhanced, clones, CPU 6502/65C02, cartões, slots, ROMs e periféricos. Não transferir o resultado de um modelo para outro.
4. Cada afirmação importante deve apontar para arquivo/linha ou commit e evidência de teste (`comando`, `ambiente`, `fixture`, `esperado`, `obtido`, `log`, `SHA`); evidências de teste e fixture devem ser versionáveis quando legalmente permitidas.
5. Não ocultar limitações usando mocks de sistema operacional, fixtures inventadas ou screenshots plausíveis como prova de fidelidade real. Comparações com o AppleWin e fixtures de referência são obrigatórias para os pontos críticos.
6. Usar ROMs, softwares e fixtures apenas com proveniência e licença documentadas. Oferecer procedimento para o usuário fornecer ROMs próprias quando necessário; não incorporar conteúdo protegido sem autorização.
7. Preservar formatos e dados do usuário. Antes de mutações de disco/save-state, criar cenário de rollback, cópia de segurança e teste de integridade.
8. Não alterar o projeto original AppleWin. Não fechar issues do repositório upstream; apenas relacioná-las a issues do projeto Rust.
9. Atualizar README, documentação de diferenças e changelog com toda limitação efetiva. Nunca anunciar “100% portado” sem matriz integral e conformidade demonstrada.
10. Evitar grandes PRs sem evidência. Fazer PRs independentes, pequenos, com teste de regressão e revisão independente.

## 2. Linha de base conhecida, a ser revalidada

O AppleII-Rust possui workspace Rust com crates `apple2-cpu`, `apple2-core`, `apple2-video`, `apple2-storage`, `apple2-audio`, `apple2-cards`, `apple2-hostio`, `apple2-debugger`, `apple2-roms`, `apple2-config`, `apple2-savestate` e frontend `applewin`. Há suporte declarado a CPU 6502/65C02, máquinas Apple II/II+/IIe/Enhanced e alguns clones, diversas memórias, gráficos, Disk II, discos rígidos/SmartPort, Mockingboard/Phasor, SSI263, Z80, mouse/joystick, Uthernet I/II, VidHD, debugger e YAML save-states. A documentação interna relata lacunas de fidelidade e diferenças de UI. CI e release automatizados existem; `cargo fmt --all -- --check` apareceu com `continue-on-error: true`. O release v0.1.0 e CI verde não constituem prova suficiente de paridade funcional.

Documentos internos a revisar obrigatoriamente:

- `README.md`;
- `docs/manual/12-diferencas-em-relacao-ao-applewin.md`;
- `docs/pendencias.md`;
- `.github/workflows/ci.yml` e `.github/workflows/release.yml`;
- fontes e testes de todas as crates.

**Falhas/pontos já identificados em auditoria anterior (ainda exigem reprodução independente):** descarte da configuração de bancos Saturn 128K durante conversão de settings; atalhos do debugger ausentes; nenhum save-state original real executado como teste de importação; verificação NTSC sem oráculo diferencial pixel a pixel; testes em periféricos reais ausentes; Z80 com interrupções e MEMPTR incompletos; Uthernet II parcialmente implementado; diferenças Mockingboard/Phasor/SSI263; recursos de UI e configurações não equivalentes. Tratar tudo isso como **hipóteses ou achados de auditoria a confirmar no SHA atual**, não assumir que persistem.

## 3. Forma obrigatória de cada issue / história de usuário

Criar um épico por área e histórias com o modelo abaixo. Incluir BDD (`Given/When/Then`) onde fizer sentido.

```markdown
### [ID] Título conciso — P0/P1/P2/P3
**Tipo:** épico | história | bug | dívida técnica | teste
**Como** [perfil real do usuário], **quero** [capacidade], **para** [valor concreto].
**Referência:** AppleWin issue/commit/caminho/linha, documentação AppleII-Rust, teste AppleWin-Test.
**Estado atual:** IMPLEMENTED | PASS | FAIL | UNVERIFIED | N/A | BLOCKED.
**Modelos/configurações:** [...].
**Dependências:** [...].
**Critérios de aceite (CA):**
- CA-01: Dado [...], quando [...], então [...] com resultado mensurável [...].
- CA-02: Cenário de borda [...] e resultado [...].
- CA-03: Regressão associada a [...].
**Plano de teste:** unitário; integração; diferencial; fixture real; cross-platform; manual quando necessário.
**Oráculo:** especificação de hardware, fixture validado, AppleWin no SHA fixo ou resultado conhecido.
**Evidências:** comandos, logs, SHA, captura/hash/dump, SO, versão, artefatos.
**Definition of Done:** CA completos, testes verde, revisão independente, documentação e sem regressões.
**Rollback/risco:** [...].
```

## 4. ÉPICO EP-00 — Auditoria histórica de issues e rastreabilidade integral (P0)

**HU-0001 — Inventariar todas as issues relevantes do AppleWin**  
Como mantenedor, quero relacionar todo bug e feature histórico do AppleWin com o port Rust, para identificar cobertura real e não repetir falhas já resolvidas.

**CA:**
- Exportar todas as issues abertas e fechadas e PRs vinculados via API GitHub com paginação, identificador, status, labels, data, versão, descrição e referência ao commit de correção.
- Registrar contagens por data e fonte. Na coleta anterior foram observadas **1.115 issues, 757 fechadas e 358 abertas**; não tratar esses números como constantes, recalcular.
- Para cada issue: `aplicabilidade` (runtime, plataforma Windows, UI, periférico, documentação, feature futura etc.); `modelo`; `mapeamento no Rust`; `status`; `teste`; `evidência`; `justificativa N/A`; `prioridade`.
- Uma issue upstream fechada só pode virar `PASS` no Rust se existir reprodução ou teste comprovando o comportamento. `UNVERIFIED` não pode ser reclassificada por mera inspeção superficial.
- Adicionar testes para bugs críticos históricos e marcar `N/A` apenas com justificativa técnica explícita.
- Gerar `docs/conformance/applewin-issue-matrix.csv`, `docs/conformance/issue-audit.md` e um sumário automatizado com taxas por modelo e categoria, não apenas taxa geral.
- Validar amostra estratificada de issues por auditor independente; toda diferença encontrada na amostra deve promover reexame da classe inteira.

**HU-0002 — Importar e utilizar AppleWin-Test como referência**  
Como QA, quero executar fixtures reais do AppleWin-Test para comprovar a compatibilidade.

**CA:**
- Clonar/fixar SHA de https://github.com/AppleWin/AppleWin-Test em ambiente de testes separado; catalogar fixtures por tipo/modelo/periférico; conferir licença e proveniência antes de redistribuir.
- Cobrir inicialmente `v2/Memory/Saturn64.aws.yaml`, `Saturn128.aws.yaml`, `v2/Memory/LC-RAMWrite-Test.aws.yaml`, `v2/Mockingboard/SSI263-4x_phonemes_playing_continuously.aws.yaml`, `v2/HDD-32MB.aws.yaml` e `v2/AS-S1_Turn the disk over(PAL).aws.yaml`, **validando a existência e localização no SHA selecionado**.
- Fixture incompatível gera `FAIL` ou `BLOCKED` com causa; nunca é silenciosamente pulado.
- Definir golden outputs verificáveis, versões de schema e hashes SHA-256 do corpus.
- Garantir execução em CI headless para casos independentes de hardware, com relatório JUnit e artefatos de diff.

## 5. ÉPICO EP-01 — CPU, temporização, barramento e memória (P0)

**HU-0101 — Conformidade 6502/65C02, inclusive opcodes não documentados quando aplicáveis**  
Como desenvolvedor de software retro, quero que execução e temporização reproduzam a máquina real e o AppleWin.

**CA:** executar suítes de referência para instruções oficiais e opcodes ilegais suportados; comparar PC, A, X, Y, SP, P, memória afetada, ciclos totais e padrões de acesso ao barramento por instrução; abranger page crossing, decimal mode, IRQ, NMI, RESET, BRK, stack e interrupções em bordas; diferenciar 6502/65C02; falhas devem produzir primeiro opcode divergente e trace mínimo reproduzível; teste determinístico repetido dez vezes dá mesmo estado/hash; confirmar regressão upstream [#1360](https://github.com/AppleWin/AppleWin/issues/1360) ($0C) e demais correções de CPU aplicáveis.

**HU-0102 — Cartões de memória, Language Card e Saturn sem perda de configuração**  
Como usuário, quero que bancos e permissões configurados funcionem como o hardware.

**CA:** validar RAM principal, AUX, Language Card, Saturn 64K/128K, RamWorks III e cold/warm reset; **corrigir, se persistir, a perda de `Saturn128K.banks` entre settings, MachineConfig e instância de cartão**, comprovando execução com 4 e 8 bancos em cenários pertinentes; validar cartão em slot e seleção de bancos; reproduzir [#1486](https://github.com/AppleWin/AppleWin/issues/1486), [#1414](https://github.com/AppleWin/AppleWin/issues/1414), [#1520](https://github.com/AppleWin/AppleWin/issues/1520); comparar dumps de bancos e proteção de escrita antes/depois de reset; incluir fixtures Saturn64/128 e Merlin quando software/fixture licenciado estiver disponível.

**HU-0103 — Floating bus e sincronismo CPU/vídeo/I-O**  
**CA:** criar testes de leitura em endereços soft switches em diferentes posições de raster, com ciclo e byte esperados do oráculo; garantir preservação de ordem de efeitos colaterais; comparar o tempo por frame e eventos Disk II/áudio; otimização só é aceita se traces e temporização observáveis forem preservados.

## 6. ÉPICO EP-02 — Vídeo, NTSC, PAL e modos de exibição (P0)

**HU-0201 — Fidelidade gráfica verificável por imagens e eventos**  
Como usuário, quero reproduzir texto, low-res, hi-res, double hi-res, 40/80 colunas e variações de cor conforme modelo.

**CA:** montar cenas determinísticas e capturas no mesmo ciclo/frame; comparar framebuffer lógico sem pós-processamento por igualdade bit a bit onde o oráculo for determinístico; para simulação analógica NTSC/PAL, definir antes do teste espaço de cor, alinhamento temporal, métrica (ex.: MAE/SSIM), limiar baseado em conjunto de referência, histogramas e regiões críticas; evidenciar diffs visuais e não aprovar só por semelhança subjetiva; abranger flash, mixed mode, page flipping, cores artifact, bordas e [#1443](https://github.com/AppleWin/AppleWin/issues/1443); validar Color TV e performance sem alterar fidelidade.

**HU-0202 — VidHD e compatibilidade de resoluções**  
**CA:** confirmar a lista de modos exigidos pelo AppleWin; tratar explicitamente texto 80x45 e cores de borda se aplicáveis; caso não suportados, documentar `UNVERIFIED`/`FAIL` e teste que demonstra diferença; capturas e switches de vídeo devem sobreviver save/load.

## 7. ÉPICO EP-03 — Disco, mass storage e preservação de dados (P0)

**HU-0301 — Disk II/WOZ/DSK/NIB e integridade das gravações**  
Como jogador e colecionador, quero montar, usar, salvar e ejetar discos sem perda/corrupção.

**CA:** testar boot, proteção contra escrita, troca de lado, stepper/motor, trilhas, half-tracks quando aplicável, formatos suportados e imagens malformadas; comparar hash dos bytes não modificados; simular flush no desligamento do motor (referência [#1444](https://github.com/AppleWin/AppleWin/issues/1444)); reiniciar após encerramento normal e queda simulada, verificando o comportamento documentado de durabilidade; reproduzir WOZ malformado da [#1402](https://github.com/AppleWin/AppleWin/issues/1402) com fixture original quando obtido; teste de escrita nunca deve sobrescrever fixture mestre; proteger contra path traversal/arquivos arbitrariamente enormes.

**HU-0302 — Hot swap e persistência de configuração dos discos**  
**CA:** estabelecer quais alterações devem ter efeito imediato e quais exigem reset; alterar imagem nas configurações deve aplicar-se conforme contrato visível ao usuário sem estado fantasma; eject/remount/read-only devem ser testados em Linux, Windows e macOS; qualquer reset automático precisa ser explícito e prevenir perda de RAM sem aviso.

**HU-0303 — HDD, SmartPort e imagens maiores**  
**CA:** validar boot, I/O sequencial/aleatório, limites de capacidade, falhas de mídia e `v2/HDD-32MB.aws.yaml`; comparar conteúdo gravado após shutdown e restart; impedir corrupção em troca de imagem e save-state.

## 8. ÉPICO EP-04 — Save-state, interoperabilidade e serialização (P0)

**HU-0401 — Importar save-states reais do AppleWin**  
Como usuário, quero continuar meu jogo ou sessão do AppleWin no AppleII-Rust.

**CA:** importar corpus real com versões distintas documentadas do `.aws.yaml`; verificar tipos, slots, bancos, CPU, flags, memória, vídeo, cartões e mídias; comparar estado observável imediatamente após importação e depois de N ciclos com a mesma sequência de eventos; casos desconhecidos dão erro descritivo e não crash/corrupção; não confundir fixtures escritas a partir do código C++ com save-states reais gerados pelo original.

**HU-0402 — Round-trip e atomicidade da gravação**  
**CA:** `save → restart → load → step` preservar estado determinístico e sem vazamentos de recursos; teste de round-trip com valores extremos, caracteres Unicode, caminhos com espaço e barras invertidas; restaurar estado de RGB card, impressora (incluindo caminho personalizado quando aplicável) e dispositivos ativos; reproduzir [#1499](https://github.com/AppleWin/AppleWin/issues/1499), [#1510](https://github.com/AppleWin/AppleWin/issues/1510), [#1372](https://github.com/AppleWin/AppleWin/issues/1372); gravação atômica usando arquivo temporário e renomeação quando suportada; queda simulada nunca pode destruir último save válido; migração de schema explícita com fixtures de versões antigas.

## 9. ÉPICO EP-05 — Som, fala e cartões especializados (P1)

**HU-0501 — Mockingboard, Phasor e AY-3-8910**  
Como usuário, quero música, efeitos e temporização dos cartões equivalentes ao AppleWin.

**CA:** comparar leitura/escrita de registradores, IRQs VIA 6522, configuração de chips por cartão, sockets Phasor, mixagem e volumes; identificar comportamento do `6522a-bad` quando relevante; gravar PCM determinístico em condições controladas e comparar eventos/frequências/envelopes e tolerância documentada para resampling; testes unitários de registradores não substituem testes auditivos e de firmware.

**HU-0502 — SSI263/SAM, fonemas e save-states**  
**CA:** testar power-down/silêncio [#1412](https://github.com/AppleWin/AppleWin/issues/1412), retomada de fonemas em save [#1372](https://github.com/AppleWin/AppleWin/issues/1372), ritmo/prosódia e scheduling; em fixture com fala contínua, a sequência após load tem mesmos eventos e duração observável; não aprovar “áudio existe” quando reprodução está errada.

## 10. ÉPICO EP-06 — Z80 e outras placas de expansão (P1)

**HU-0601 — Z80 com interrupções e MEMPTR completos**  
Como usuário de CP/M, quero executar software Z80 sem divergências ocasionadas por interrupções e instruções repetitivas.

**CA:** cobrir Z80 com vetores de instruções, flags, ciclos, IM 0/1/2, NMI, HALT, EI/DI, interrupções e efeitos de MEMPTR em INIR/OTIR e variantes; executar pelo menos uma sessão CP/M legalmente obtida, com inicialização e operações de disco; gerar trace comparável com referência; qualquer opcode não suportado é relatado explicitamente, não mascarado.

**HU-0602 — ThunderClock, Breakpoint Card, SNES MAX e configuração por slot**  
**CA:** confrontar suporte da biblioteca com disponibilidade de configuração e conexão efetiva ao core/UI; relógio selecionável e serializável quando aplicável; Breakpoint Card integrado ao mecanismo de breakpoints ou diferença formalizada; flags SNES MAX realmente ligam controles; testar conflitos de slots/configurações inválidas e falhas seguras.

## 11. ÉPICO EP-07 — Rede, serial, impressora e dispositivos de entrada (P1)

**HU-0701 — Uthernet I/II com protocolo e ciclo de vida confiáveis**  
**CA:** definir matriz implementada de SPI, interrupções, TCP cliente/servidor, UDP, PPPoE e reconexão; realizar testes locais sem Internet, loopback, desconexão, timeout, pacotes malformados e múltiplas sessões; demonstrar comportamento explícito de sockets quando save-state é carregado; recursos sem suporte devem falhar de forma controlada e documentada.

**HU-0702 — Entrada real e mapeamento consistente**  
**CA:** testar teclado, mouse, joystick/paddles, gamepads físicos por sistema operacional, calibração e captura de mouse; reproduzir inicialização de PREAD [#1522](https://github.com/AppleWin/AppleWin/issues/1522) e botões desconectados [#1234](https://github.com/AppleWin/AppleWin/issues/1234); avaliar tempo dos paddles 2/3 e sincronismo; teclado não pode ficar preso após alternar foco, fullscreen e debug.

**HU-0703 — Serial, impressora, Npcap/libpcap e permissões**  
**CA:** detectar dispositivos disponíveis em cada SO conforme APIs reais; expor mensagens de erro ao negar permissão; preservar configuração de destino da impressora, caminhos e sanitização; permitir recursos opt-in quando requererem dependências externas; evitar crash por dispositivo inexistente ou desconectado.

## 12. ÉPICO EP-08 — Debugger e experiência AppleWin (P1)

**HU-0801 — Paridade dos comandos e atalhos de debugger**  
Como programador Assembly, quero inspecionar/seguir o fluxo e navegar como no AppleWin.

**CA:** construir tabela comando → atalho → estado esperado → equivalência; implementar ou justificar as lacunas históricas [#1460](https://github.com/AppleWin/AppleWin/issues/1460), [#1456](https://github.com/AppleWin/AppleWin/issues/1456), [#1451](https://github.com/AppleWin/AppleWin/issues/1451); testar stepping, step-over/step-out, RTS, breakpoints, watchpoints, disassembly, alterações de registradores, scrolling com teclas modificadoras; não permitir que debugger altere inadvertidamente estado do emulador apenas por renderizar a UI.

**HU-0802 — Operações de UI configuráveis e acessibilidade**  
**CA:** analisar paridade de ações, não apenas estética: montar disco por diálogo/arrastar, abrir somente leitura, Ctrl+PrintScreen com 40/80 colunas, monitor de setores, benchmark, controles de volume independentes, atalhos fullscreen/Alt+Enter, lista de interfaces de rede e portas seriais, preferência save-state e importação de config; descrever alternativas equivalentes para recursos específicos do Windows; atalhos sem ação não podem aparecer como suportados.

## 13. ÉPICO EP-09 — Windows, Linux e macOS: qualidade de plataforma (P0/P1)

**HU-0901 — Funcionalidade executável e testada nos três sistemas**  
Como usuário, quero instalar e executar sem regressões específicas da plataforma.

**CA:** CI `cargo fmt --all -- --check` deve ser bloqueante; `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, build release por matriz Windows/macOS/Linux; smoke headless + abertura GUI nos ambientes disponíveis; testar resolução/dpi, Retina/HiDPI, teclado regional, áudio padrão/sem dispositivo, janelas minimizadas, fullscreen, permissões e fechamento limpo; gerar relatórios por SO, arquitetura, dependências e SHA; nenhum build vermelho gera release `stable`.

**HU-0902 — Distribuição reproduzível e seguros de cadeia de suprimentos**  
**CA:** criar artefatos versionados, SHA-256, SBOM quando viável, changelog e licenças; testar extração e abertura a partir do pacote e não só do diretório de desenvolvimento; especificar bibliotecas opcionais pcap/serial; esclarecer assinatura/notarização macOS e assinatura Windows como requisitos de distribuição, se aplicáveis; não afirmar garantia de app assinado quando não houver evidência.

## 14. ÉPICO EP-10 — Performance preservando correção (P1)

**HU-1001 — Profiling e desempenho determinístico**  
**CA:** benchmark por cenário (CPU, texto, HGR, DHGR, NTSC/PAL, Disk II, áudio) em hardware/compilação fixos; publicar frames/s, emulated MHz, latência de input e uso de CPU/memória, com mediana e p95; perf regressivo detectado em CI com margem calibrada; otimizar `CardCtx`/floating bus, pipeline Color TV e cópias desnecessárias apenas após suite diferencial verde; evitar “otimização” que ultrapasse clock emulado, altere timing ou áudio.

## 15. ÉPICO EP-11 — Compatibilidade com problemas históricos e novas issues abertas (P1/P2)

**HU-1101 — Triar issues upstream abertas com valor alto**  
Como mantenedor, quero transformar bugs e oportunidades abertos do AppleWin em melhorias pertinentes ao Rust, sem importar cegamente soluções Windows-only.

**CA:** consultar em tempo de execução `https://github.com/AppleWin/AppleWin/issues?q=is%3Aissue+is%3Aopen` e relacionar estado/data/número/título; verificar cada descrição/etiqueta/PR; selecionar issues sobre fidelidade de hardware, formatos de disco, debugger, áudio, vídeo, save-states, acessibilidade e compatibilidade multiplataforma; para cada selecionada, abrir uma história local com valor, esforço, risco, teste de reprodução e teste de regressão; **não inventar números/títulos de issues abertas**. Se API indisponível, deixar lista como `PENDING_VERIFICATION`, sem afirmar inclusão verificada.

**Checklist inicial de issues fechadas importantes a confirmar/testar:** [#1414](https://github.com/AppleWin/AppleWin/issues/1414), [#1516](https://github.com/AppleWin/AppleWin/issues/1516), [#1460](https://github.com/AppleWin/AppleWin/issues/1460), [#1456](https://github.com/AppleWin/AppleWin/issues/1456), [#1451](https://github.com/AppleWin/AppleWin/issues/1451), [#1486](https://github.com/AppleWin/AppleWin/issues/1486), [#1444](https://github.com/AppleWin/AppleWin/issues/1444), [#1520](https://github.com/AppleWin/AppleWin/issues/1520), [#1522](https://github.com/AppleWin/AppleWin/issues/1522), [#1412](https://github.com/AppleWin/AppleWin/issues/1412), [#1372](https://github.com/AppleWin/AppleWin/issues/1372), [#1360](https://github.com/AppleWin/AppleWin/issues/1360), [#1234](https://github.com/AppleWin/AppleWin/issues/1234), [#1402](https://github.com/AppleWin/AppleWin/issues/1402), [#1443](https://github.com/AppleWin/AppleWin/issues/1443), [#1499](https://github.com/AppleWin/AppleWin/issues/1499), [#1510](https://github.com/AppleWin/AppleWin/issues/1510). Esta seleção é ponto de partida, não cobertura exaustiva.

## 16. ÉPICO EP-12 — Evoluções próprias, sem comprometer compatibilidade (P2/P3)

**HU-1201 — Depuração temporal e rewind (P2)**  
Como desenvolvedor Assembly, quero retroceder execução e comparar estados.

**CA:** checkpoints incrementais, replay determinístico de entradas, passo reverso com estado correto de CPU/memória/periféricos compatíveis; recursos externos não reprodutíveis explicitamente suspensos ou registrados; limites configuráveis de RAM/disco, teste de integridade em 1000 passos de ida/volta.

**HU-1202 — Interface DAP para VS Code (P2)**  
**CA:** sessão de debug com attach, pause, continue, step-in/out/over, breakpoints por endereço, memória e registros; cliente DAP de teste automatizado; funcionamento consistente com debugger nativo e sem execução de código arbitrário fora do escopo do emulador.

**HU-1203 — Laboratório educacional e disassembler didático (P3)**  
**CA:** apresentar instrução, mnemônico, bytes, ciclos e efeitos sobre registradores; exemplos sem ROM proprietária; exportar trace; exercício reproduzível de Assembly com resposta verificável; documentação para iniciantes.

**HU-1204 — Investigação separada: suporte a Apple I (P3, NÃO IMPLÍCITO)**  
**CA:** criar ADR comparando arquiteturas Apple I versus Apple II (mapa de memória, ROM/monitor, vídeo/terminal, CPU, I/O, periféricos), esforço e requisitos legais de ROM; não prometer suporte até decisão explícita e suíte específica de testes; preservar fronteira arquitetural.

**HU-1205 — Frontend web/WASM (P3, opt-in)**  
**CA:** estudar compatibilidade do núcleo sem APIs nativas, desempenho, licença e tratamento de ROMs; protótipo com testes headless; não introduzir regressões ao desktop.

## 17. Plano de testes detalhado e estratégia de oráculos

### Pirâmide mínima

1. **Unitários**: registradores, opcodes, memória, parser de WOZ/YAML, soft-switches, clocks e dispositivos; gerar casos de borda e propriedades invariantes (sem crash, integridade, reversibilidade apropriada).
2. **Integração**: boot ROM, montagem/gravação de disquete, expansão de RAM, vídeo, áudio, debugger e salvar/restaurar sob sequências de input fixadas.
3. **Diferencial**: executar AppleWin referência e AppleII-Rust sob mesma configuração/ROM/input/ciclo, comparar logs de CPU, memória e barramento e framebuffers; quando houver divergência, capturar primeira diferença causal. Testes dependentes de Windows podem gerar traces de referência versionados com origem e SHA.
4. **Corpus real**: fixtures autênticas AppleWin-Test, múltiplas versões e variações de máquina; nunca depender só de fixture criada pelo time.
5. **End-to-end cross-platform**: instalar executável distribuído, abrir GUI, digitar, montar disco, salvar/reabrir, gerar áudio e encerrar em Windows/macOS/Linux. Registrar automação x validação humana.
6. **Robustez/fuzzing**: entradas WOZ/NIB/DSK/YAML/config/serial/rede inválidas; limites de tamanho/tempo/memória; nenhum panic por arquivo de usuário malformado, nem mutação parcial de dados.
7. **Performance**: medir com benchmark fixo e não sacrificar timings hardware-correct.

### Política de aprovação de comparação

- Para estado digital determinístico, comparar valores exatos: PC, registradores, flags, bancos, RAM relevante, eventos, ordem de I/O, contador de ciclos e framebuffer antes de efeitos analógicos.
- Para áudio/NTSC/PAL analógicos, comparar eventos/temporização exatos quando possível; usar métricas visuais/acústicas e tolerâncias **definidas antes de executar** e calibradas com referência; diffs fora do limiar são `FAIL`, não ficam `PASS` por julgamento manual.
- Jogos e software de terceiros: somente com licença adequada e procedimento reproduzível de obtenção; resultados dependentes de firmware devem ter versão/hash de ROM.
- Testes flakey não podem ser removidos por conveniência; investigar causa, isolar ambiente e registrar evidência.

### Manifesto por execução — campos obrigatórios

`test_id`, `user_story`, `acceptance_criterion`, `applewin_sha`, `rust_sha`, `fixture_sha256`, `rom_sha256`, `machine_model`, `cpu_model`, `cards_slots`, `host_os`, `host_arch`, `toolchain`, `input_trace`, `command`, `expected`, `actual`, `diff_artifact`, `result`, `timestamp_utc`, `reviewer`.

### Exemplo BDD

```gherkin
Feature: Saturn 64K e 128K mantêm bancos configurados
  Scenario Outline: Carregar configuração respeita quantidade de bancos
    Given máquina Apple IIe com cartão Saturn configurado com <bancos> bancos
    When o emulador inicializa e troca pelos bancos válidos
    Then todos os bancos esperados estão acessíveis com conteúdo independente
    And os bancos fora da capacidade não se tornam acessíveis silenciosamente
    And o comportamento coincide com o oráculo no mesmo modelo
    Examples:
      | bancos |
      | 4      |
      | 8      |
```

## 18. Critérios de release e score independente

**Gate G0 — Inventário:** 100% das issues upstream coletadas, aplicabilidade classificada e evidência/justificativa para cada caso; divergências identificadas não são omitidas.

**Gate G1 — Alpha técnica:** CI e smoke tests dos 3 SO verdes; boot básico, CPU, memória, vídeo, disco e save-state com regressões P0 automatizadas; falhas conhecidas destacadas no release.

**Gate G2 — Beta funcional:** zero `FAIL` P0 aplicável; zero `UNVERIFIED` P0 sem bloqueio formal aceito; fixtures reais críticas aprovadas; matriz dos componentes essenciais por modelo; testes de áudio/input/GUI com evidências nos 3 SO; recuperação de estado/discos validada.

**Gate G3 — Stable candidata:** zero `FAIL` P0/P1 no escopo declarado; testes cross-platform repetidos de pacote instalado; nenhuma perda de dados nos cenários cobertos; relatório independente assinado/aprovado; problemas P2/P3 publicados e escopo explícito.

**Avaliação Verônica de 0–100, baseada em evidências:**

| Dimensão | Pontos |
|---|---:|
| Especificação e critérios de aceite | 25 |
| Corretude e evidência | 25 |
| Testes e regressão | 15 |
| Arquitetura e manutenção | 10 |
| Segurança, dados e resiliência | 10 |
| Higiene da entrega | 5 |
| Eficiência/retrabalho | 10 |

Aceite por história: **nota ≥ 96/100, todos os CA materiais `PASS`, sem bloqueador crítico e revisão por pessoa/agente que não implementou a alteração**. Nota alta não substitui gates objetivos. Nota 90–95 implica correção focada; ≤89 ou repetição de tentativas sem nova hipótese implica interromper e replanejar. Encerrar agentes paralelos quando houver aprovação comprovada, evitando loops.

## 19. Ordem de execução indicada e dependências

**Onda 0 — Congelar baseline e medir:** EP-00, selecionar AppleWin/AppleWin-Test, matriz de issues, harness de traces, manifests e owners. **Não iniciar uma refatoração ampla antes de existir um oráculo.**

**Onda 1 — Fechar risco central:** EP-01 (Saturn, CPU, memória), EP-04 (save-states reais) e EP-03 (discos) em PRs separados. São áreas que podem causar estado errado ou perda de dados.

**Onda 2 — Fidelidade perceptível:** EP-02 (NTSC/PAL), EP-05 (áudio), EP-06 (Z80), EP-07 (rede/inputs). Incluir fixtures e traces do upstream.

**Onda 3 — Produto multiplataforma:** EP-08, EP-09, EP-10. Corrigir opções que não se aplicam realmente, compatibilidade da GUI e desempenho.

**Onda 4 — Expansão:** EP-11 (novas issues abertas validadas) e EP-12 (rewind, DAP, laboratório, possível Apple I mediante ADR).

**Paralelismo:** CPU/memória, vídeo, disco e GUI podem ter agentes separados **somente se** os contratos de core e fixtures estiverem estáveis e não houver edição simultânea dos mesmos módulos. Um agente de QA independente valida cada PR. Ao segundo ciclo sem progresso comprovável, interromper, identificar hipótese testável e replanejar.

## 20. Entregáveis obrigatórios no repositório

```text
docs/conformance/
  baseline.md
  capability-matrix.md
  applewin-issue-matrix.csv
  issue-audit.md
  upstream-open-issues-triage.md
  test-oracles.md
  release-readiness.md
  gaps-and-known-limitations.md
tests/conformance/
  README.md
  manifests/
  inputs/
  reference-traces/
  reports/
.github/workflows/
  ci.yml
  conformance.yml
```

Os nomes acima são proposta de organização; respeitar convenções existentes quando tecnicamente equivalentes. Logs e corpus volumoso podem residir em armazenamento de CI com hashes e índices no Git, respeitando direitos autorais.

## 21. Formato final do relatório ao solicitante

O time deverá responder em português, com:

1. Baseline com SHAs e versões dos três repositórios.
2. Tabela objetiva `Componente | Modelo | Resultado | Evidência | Divergência | Issue Rust | Prioridade`.
3. Sumário por status das issues upstream, distinguindo abertas, fechadas, N/A e não verificadas.
4. Top 10 problemas críticos **com passos para reproduzir**, causa-raiz e estado da correção.
5. Backlog importável em GitHub Issues: épicos, HU/bugs, dependências, labels, milestones e CA.
6. Testes executados e **não executados**, ambientes e relatórios para Windows/macOS/Linux.
7. Comparação diferencial real do AppleWin × Rust e screenshots/dumps/artefatos de diferença quando aplicáveis.
8. Matriz de risco de corrupção de disco/save-state e mitigação.
9. Score 0–100 pela rubrica Verônica, com gates G0–G3 explicitamente PASS/FAIL/BLOCKED.
10. Recomendação de próximo release: `alpha`, `beta` ou `stable candidate`, **sem inflar classificação**.

### Comando final ao agente coordenador

**Comece auditando e medindo, não reescrevendo.** Preserve as capacidades já implementadas, prove o que funciona, corrija divergências confirmadas, adicione regressões para cada correção e produza um PR por grupo pequeno de mudanças. Leia a documentação real do repositório e o histórico do upstream antes de modificar código. **A entrega só é aceita quando os critérios de aceite se transformam em testes executados e evidências auditáveis.** Não considere o trabalho concluído com afirmações genéricas como “todas as fases feitas” ou “CI passou”.

---

### Fontes primárias e origem dos achados

- AppleII-Rust: https://github.com/ChicoFigueiredo/AppleII-Rust
- AppleWin: https://github.com/AppleWin/AppleWin
- AppleWin issues: https://github.com/AppleWin/AppleWin/issues
- AppleWin-Test: https://github.com/AppleWin/AppleWin-Test
- Auditorias anteriores desta conversa de 20/10/2026; **toda classificação de código/issue é sujeita a revalidação no SHA atual**.

