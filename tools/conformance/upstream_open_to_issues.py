#!/usr/bin/env python3
"""HU-1101: cria uma história no GitHub local para cada issue aberta do AppleWin selecionada.

Fonte da seleção: docs/conformance/upstream-open-issues-triage.md (coluna Selecionada = sim).
Corpo das issues do AppleWin: target/conformance-cache/upstream/issues.json (somente leitura).

Uso:
  upstream_open_to_issues.py --dry-run          # mostra o que seria criado, sem escrever nada
  upstream_open_to_issues.py                    # cria as issues que ainda não existem (idempotente)
  upstream_open_to_issues.py --epic             # (re)escreve a lista de tarefas na issue épica
  upstream_open_to_issues.py --report           # escreve docs/conformance/upstream-open-selected.md

A criação é idempotente: antes de criar, busca o prefixo `[UP-NNNN]` no título com
`gh issue list --search`. Nunca faz push, PR, tag nem `gh workflow run`.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPO = "ChicoFigueiredo/AppleII-Rust"
UPSTREAM = "https://github.com/AppleWin/AppleWin/issues"
EPIC = 13
MILESTONE = "G3 — Stable candidata"
TRIAGE = ROOT / "docs/conformance/upstream-open-issues-triage.md"
REPORT = ROOT / "docs/conformance/upstream-open-selected.md"
CACHE = Path("/mnt/d/Chico/AppleRust/target/conformance-cache/upstream/issues.json")
EPIC_BEGIN = "<!-- upstream-open:begin -->"
EPIC_END = "<!-- upstream-open:end -->"

# Cada história: t=título curto, p=prioridade, w=quero, f=para, m=modelos, r=repro (CA-01),
# e=borda (CA-02), g=regressão (CA-03), pl=plano de teste, o=oráculo, who=perfil (opcional).
S = {
    241: dict(t="Floating bus adiantado em 2 ciclos", p="P1", who="desenvolvedor de demos",
        w="que a leitura do floating bus devolva o byte que o scanner de vídeo realmente apresenta naquele ciclo",
        f="que demos com troca de modo por scanline (Ansi Story) renderizem como no hardware",
        m="Apple //e e //e Enhanced, 60 Hz e 50 Hz",
        r="Dado o final de Ansi Story (DLORES/TEXT80 alternados por linha), quando o código sincroniza lendo $C070/floating bus e troca o modo em seguida, então a linha em que o modo muda coincide com a captura do hardware, e a leitura de $C051 no 4º ciclo de `LDA abs` devolve o byte do ciclo correto (erro medido hoje: 2 ciclos, segundo o próprio mantenedor).",
        e="Cenário de borda: leitura do floating bus no 1º e no último ciclo da linha e na transição de quadro (ciclo 17030) devolve os bytes de Sather, sem estado preso quando a CPU executa mais de um quadro por tick de vídeo.",
        g="Regressão: teste de ciclo/bus que fixa o deslocamento em N ciclos para cada modo de endereçamento e falha se o valor mudar.",
        pl="Unitário do gerador de endereços do scanner (tabela de Sather); integração com programa 6502 mínimo que lê $C051..$C057 em ciclos conhecidos; diferencial contra AppleWin no SHA fixo comparando bytes lidos; fixture real: Ansi Story AS-S1/S2 (frame hash).",
        o="Equações de Sather (Understanding the Apple //e) e capturas de hardware citadas na issue; AppleWin no SHA fixo apenas como segundo oráculo."),
    1166: dict(t="Matizes NTSC incorretos (lores cor 1 arroxeada)", p="P1", who="usuário que compara cores com monitor real",
        w="que o pipeline NTSC use a fase e a amplitude de crominância corretas",
        f="que as cores de lores/hires se aproximem das do hardware, em vez de arroxeadas",
        m="todos os modelos com saída composta colorida",
        r="Dado um programa Applesoft que desenha as 16 cores de lores, quando renderizado em modo 'Color (Composite)', então a cor 1 tem fase UV de 90 graus e amplitude 60/100 (RGB esperado próximo de #f00 após gama), com tolerância numérica documentada em ΔE.",
        e="Cenário de borda: mudar de lores para texto no meio da linha e o filtro de duplicação de amostras (560 para 1120) não altera a fase; HGR com bit 7 mantém o deslocamento de meio pixel.",
        g="Regressão: imagens de referência (16 blocos lores e 8x64 HGR do programa de #254) com hash/tolerância por canal versionadas.",
        pl="Unitário da matriz YIQ/RGB; integração com captura de framebuffer; diferencial contra AppleWin (documentando divergências intencionais); comparação visual manual contra fotos de hardware.",
        o="Munafo (a2info), UTAIIe e capturas de hardware real; as issues #253, #254 e #502 do AppleWin fornecem programas de teste."),
    1303: dict(t="IRQ nível-sensível quando a fonte não é limpa", p="P1", who="desenvolvedor 6502",
        w="que a CPU reentre na ISR enquanto a linha /IRQ permanecer ativa",
        f="que o comportamento coincida com o 65C02 real quando a ISR não limpa a fonte",
        m="Apple //e Enhanced (65C02) e II/II+ (6502) com Mockingboard",
        r="Dado um 6522 com T1 em estouro e IER.T1=1, e uma ISR que só incrementa uma flag sem limpar o T1, quando se executa `CLI; SEI; LDA isrFlag`, então A termina em 1 (a ISR roda exatamente uma vez antes de o SEI valer).",
        e="Cenário de borda: IRQ chega durante instrução de 7 ciclos, em CLI/SEI/PLP/RTI e com NMI simultâneo; atraso de uma instrução do CLI respeitado.",
        g="Regressão: teste de integração com o trecho acima (mb-audit contém o equivalente) que falha se a ISR for chamada em laço.",
        pl="Unitário do modelo de linha de IRQ; integração com 6522 emulado; fixture: mb-audit; diferencial contra AppleWin no SHA fixo.",
        o="Comportamento descrito pelo mantenedor como visto em hardware (Enhanced //e, 65C02), datasheets WDC/MOS e mb-audit."),
    1313: dict(t="Período do ruído do AY-3-891x com valor 0", p="P1", who="músico/porter de Mockingboard",
        w="que o gerador de ruído do AY use o LFSR e o período corretos, inclusive para o registrador 6 igual a 0",
        f="que trilhas como Ultima V (Hall of Doom) soem como no hardware",
        m="Mockingboard A/C e Phasor (AY-3-8910/8913/YM2149)",
        r="Dado o registrador de ruído (R6) escrito com 0x00 durante a trilha Hall of Doom, quando o AY é amostrado, então o período efetivo e a sequência do LFSR coincidem com a gravação do hardware (a issue aponta realimentação por bits 0 e 3 no 8910 comum, e 0 e 2 no expandido).",
        e="Cenário de borda: R6 = 0, 1 e 31; mudança de período no meio de um ciclo do contador de ruído; mixer com ruído e tom simultâneos.",
        g="Regressão: dump de amostras PCM dos primeiros N quadros de um conjunto fixo de escritas de registradores, comparado por hash/tolerância.",
        pl="Unitário do LFSR e do divisor; integração com sequência de escritas gravada da trilha; fixture real: gravações anexadas à issue; diferencial contra MAME/AppleWin.",
        o="Datasheet AY-3-8910 e implementação de referência (MAME ay8910), gravações do hardware da issue."),
    1330: dict(t="Suporte ao AY-3-8913 (Mockingboard A/C e Phasor)", p="P2", who="músico/porter de Mockingboard",
        w="um modo de emulação do AY-3-8913 além do AY-3-8910",
        f="que quem porta músicas só com emulador não receba um resultado mais limpo do que o do hardware moderno",
        m="Mockingboard A/C e Phasor com clones modernos (AY-3-8913)",
        r="Dado o conjunto de registradores de Cybernoid1 com PERIOD A/B/C iguais e deslocados, quando o chip está em modo 8913, então as formas de onda reproduzem a condição 'abafada' descrita na issue (canal em zero após escrita do byte alto do período até o contador global de 12 bits igualar o período).",
        e="Cenário de borda: dois AYs (Phasor) com períodos idênticos somam em fase; modo 8910 continua sem a condição; troca de modo em tempo de execução.",
        g="Regressão: mb-audit 'tone test' com períodos $400/$440/$480 produz o mesmo hash de áudio nos dois modos, cada um com a sua assinatura.",
        pl="Unitário do contador; integração com mb-audit; fixture: gravações de hardware (issue e artigo de Fenarinarsa); diferencial contra o protótipo do mantenedor.",
        o="Gravações de hardware real anexadas à issue e teoria de trabalho do mantenedor (ainda não confirmada por datasheet); tratar como hipótese."),
    963: dict(t="mb-audit: lotes de CPU maiores que 1 ms falham", p="P1", who="mantenedor de plataforma",
        w="que o Mockingboard seja atualizado em granularidade compatível com os testes de temporização, independentemente do tamanho do lote de CPU",
        f="que o resultado do mb-audit não dependa de quantos ciclos a CPU executa por tick do host",
        m="Mockingboard/Phasor com SSI263 e 6522",
        r="Dado o mb-audit rodando com lotes de CPU de 16,6 ms, quando o teste SSI263 41:05:00 é executado, então o resultado é o mesmo que com lotes de 1 ms (hoje falha com Expected 00, Actual 01).",
        e="Cenário de borda: lotes de 1 ms, 15 ms, 16 ms e 33 ms; teste T6522_B com intervalo de cerca de $9000 ciclos entre leituras do timer.",
        g="Regressão: mb-audit completo em CI headless com dois tamanhos de lote, ambos devem passar.",
        pl="Integração headless do mb-audit; unitário do agendador de eventos 6522/SSI263; diferencial contra AppleWin.",
        o="mb-audit (suíte de testes para emuladores) e hardware real."),
    1088: dict(t="Fidelidade do alto-falante (PWM/saída 1 bit)", p="P2", who="desenvolvedor que usa o speaker como DAC",
        w="que o modelo do alto-falante reproduza a resposta elétrica/mecânica em vez de um quadrado ideal",
        f="que programas de áudio PWM (A2Stream) soem como no hardware real",
        m="Apple //e Enhanced, //c",
        r="Dado A2Stream tocando um áudio PWM de referência, quando a saída é capturada, então o espectro fica dentro de tolerância documentada da gravação do hardware (sem o chiado de fundo e com a resposta de frequência semelhante).",
        e="Cenário de borda: taxa de amostragem de 44,1 kHz e 48 kHz; rajadas de toggles a cada 1 ciclo; silêncio prolongado sem clique.",
        g="Regressão: PCM sintético de toggles periódicos com hash/tolerância espectral versionados.",
        pl="Unitário do filtro (RC/harmônico amortecido); integração com A2Stream e tom de teste; comparação espectral automática; avaliação auditiva manual.",
        o="Gravações de hardware real da issue (IIgs e //e) e os algoritmos propostos nos comentários (KansasFest 2022, univta0001, wiz21b); escolha do modelo ainda é decisão de projeto."),
    579: dict(t="Speaker: o clique só ocorre a cada segunda referência", p="P2", who="autor de rotinas de som",
        w="que o volume dependa do espaçamento entre as referências a $C030..$C03F, como no cone físico",
        f="que o efeito de rampa de volume (Ultima IV, artigo da NIBBLE de 1984) seja audível",
        m="todos os modelos",
        r="Dado o programa TONE da NIBBLE (referências com espaçamento crescente), quando executado, então a amplitude sobe, atinge o máximo e desce, em vez de um quadrado de amplitude constante.",
        e="Cenário de borda: duas referências consecutivas no mesmo ciclo; número ímpar de referências; longos intervalos sem referência.",
        g="Regressão: PCM da rampa com envoltória verificada por teste (máximo no meio, mínimo nas pontas).",
        pl="Unitário do modelo de cone; integração com o programa TONE; fixture: código do comentário da issue.",
        o="Descrição do autor da técnica (philgoetz) e artigo NIBBLE de nov/1984; ainda sem gravação de hardware, portanto valor médio."),
    636: dict(t="Leitura de $C07E/$C07F no //e devolve floating bus", p="P2", who="desenvolvedor de software para //e",
        w="que as leituras de $C07E/$C07F no //e e //e Enhanced devolvam o floating bus, e não valores de DHIRES/IOUDIS",
        f="que detecção de hardware por software veja o mesmo que o equipamento real",
        m="Apple //e e //e Enhanced",
        r="Dado `C050 N C053 N C057 N C05E N C07F`, quando lido em //e (com e sem Enhanced), então os bits devolvidos são o floating bus (conforme repro do mantenedor e testes do arekkusu42), nunca o estado de DHIRES.",
        e="Cenário de borda: leitura logo após troca de modo de vídeo; escrita a $C07E/$C07F continua alterando IOUDIS conforme o TRM somente onde o hardware o suporta.",
        g="Regressão: teste de integração com o roteiro `Switches_251008` (MAME PR 14307) sobre os soft switches de vídeo.",
        pl="Unitário do mapa de I/O; integração com o programa Switches; diferencial contra MAME.",
        o="Hardware real (//e original e Enhanced, confirmado pelo mantenedor e por arekkusu42); o TRM de 1985 sozinho não vale como oráculo, pois não bate com o hardware."),
    520: dict(t="Reset não zera o banco RamWorks e quebra DHGR", p="P1", who="jogador de jogos DHGR",
        w="que o vídeo de 14M busque sempre do primeiro banco auxiliar de 64K e que o Reset trate o registrador de banco como no hardware",
        f="que Airheart e outros jogos DHGR não percam colunas após um Reset",
        m="Apple //e e //e Enhanced com RamWorks (por exemplo 4 bancos)",
        r="Dado RamWorks com banco 3 ativo (`300:A9 3 8D 71 C0 60`), quando se reseta e se carrega Airheart, então a tela de título DHGR mostra todas as colunas, sem cintilação.",
        e="Cenário de borda: banco ativo no Reset (manual da RamWorks III diz que o registrador não é zerado no Reset, só no power-on) e vídeo sempre lendo do banco 0.",
        g="Regressão: teste com hash do framebuffer DHGR antes e depois de Reset com banco 3.",
        pl="Unitário do mapeamento de banco/vídeo; integração com Airheart; diferencial contra AppleWin.",
        o="Manual da RamWorks III v1.41 (pg. 45) e UTAIIe; repro da issue."),
    673: dict(t="Leituras pares de $C0E0..$C0EE leem o latch (DSK/NIB)", p="P1", who="preservador de discos",
        w="que qualquer endereço par de leitura do Disk II avance e leia o latch de dados, também em DSK/NIB",
        f="que discos cujas rotinas leem $C0E0 em vez de $C0EC (cena de cracking francesa) iniciem",
        m="Disk II (16 setores) com imagens DSK/NIB",
        r="Dado um disco DSK cuja rotina usa `LDA $C0E0,X` para ler o latch (imagens anexadas à issue), quando se dá o boot, então o disco inicia, como já ocorre com WOZ.",
        e="Cenário de borda: leituras pares em modo de escrita, com motor desligado e em sequência rápida, sem avançar a posição em duplicidade.",
        g="Regressão: teste de integração com as imagens da issue e com uma rotina sintética que lê $C0E0..$C0EE.",
        pl="Unitário da máquina de estados do Disk II; integração com rotina 6502; fixture real: DSK anexadas à issue; diferencial contra AppleWin.",
        o="Comportamento do hardware (latch lido em qualquer endereço par) e imagens da issue."),
    1110: dict(t="WOZ: cabeça entre fases adjacentes e quarto de trilha", p="P1", who="preservador de discos",
        w="que o movimento da cabeça com fases adjacentes ligadas e desligadas em sequência rápida siga o modelo magnético",
        f="que discos com protocolos 3/4 de trilha e meia trilha funcionem",
        m="Disk II com imagens WOZ",
        r="Dado o trecho de `lda D2.CurrentQTrack ... Ph0Off,y ... Ph0Off,x` com o save-state anexado, quando as fases são desligadas em sequência rápida, então a cabeça permanece entre as fases X e Y (não volta para meia trilha de X).",
        e="Cenário de borda: ordem n,n+1 e n+1,n, para frente e para trás, em uma imagem WOZ pré-formatada com trilha 3/4.",
        g="Regressão: teste de trilha 3/4 para frente e para trás nas duas ordens (pendência registrada pelo mantenedor).",
        pl="Unitário do modelo de fases; integração com save-state da issue; fixture: WOZ pré-formatada; diferencial contra AppleWin 1.30.11+.",
        o="Modelo de ímãs das fases (Disk II) e save-state/YAML anexado pelo relator; sem hardware real nesta issue."),
    661: dict(t="Imagens WOZ que não funcionam (issue guarda-chuva)", p="P1", who="preservador de discos",
        w="uma suíte de regressão com as imagens WOZ conhecidas por falhar",
        f="que cada título (Frog Jump, Fishies, Autoduel, Prisoner 2, The Newsroom, Phantasie, Zork Zero) tenha um resultado conhecido",
        m="Disk II com imagens WOZ 1.0/2.0, //e e //e Enhanced",
        r="Dado cada WOZ da lista da issue #661 e das filhas ainda abertas (#762, #779, #926, #1001, #1005, #1139, #1278), quando se dá o boot em 10 execuções, então o resultado (boot, falha de proteção, erro de E/S) é registrado por título e comparado com o esperado.",
        e="Cenário de borda: imagens que exigem escrita, 13 setores, trilha grande demais (Congo Bongo) e imagens que detectam não-originalidade (Prisoner 2).",
        g="Regressão: relatório por título persistido; um título que passa não pode voltar a falhar.",
        pl="Integração headless com timeout e hash da tela de boot; fixture real: coleção Woz-a-Day (não distribuída no repositório).",
        o="Hardware real e outros emuladores citados nos comentários (MAME, Virtual II); imagens WOZ de terceiros, sem redistribuição."),
    1255: dict(t="WOZ 2.1: trilhas FLUX", p="P2", who="preservador de discos",
        w="suporte a trilhas FLUX do formato WOZ 2.1",
        f="que jogos da Sirius (Bandits, Cyclod, Fly Wars, Jellyfish, Lemmings, Minotaur) iniciem",
        m="Disk II com WOZ 2.1",
        r="Dado um WOZ 2.1 com trilhas FLUX (por exemplo Bandits), quando se dá o boot, então o jogo inicia; Cyclod, Fly Wars e Minotaur também (nem todos funcionam no MAME 0.260).",
        e="Cenário de borda: mistura de trilhas bitstream e FLUX no mesmo disco; trilha FLUX com tempos extremos; imagem truncada deve gerar erro, não pânico.",
        g="Regressão: parser do chunk FLUX com testes de propriedade e arquivos malformados (fuzz curto).",
        pl="Unitário do parser; integração com os 6 discos; fuzzing do parser; diferencial contra Accurapple/emu6502.",
        o="Especificação WOZ 2.1 (Applesauce) e emuladores que já rodam as imagens (Accurapple, emu6502, CrossRunner)."),
    1331: dict(t="Abrir imagens .po de HDD com menos de 1600 blocos", p="P2", who="desenvolvedor que usa imagens pequenas",
        w="montar imagens .po de qualquer tamanho múltiplo de 512 bytes como disco rígido",
        f="transferir imagens pequenas por ADTPro numa SSC lenta",
        m="Apple II/II+/IIe com cartão de HDD",
        r="Dado um .po de 1599 blocos criado no CiderPress II, quando montado no HDD 1, então abre sem 'Unable to open the file' (hoje exige ao menos 800 KiB para .po, enquanto .hdv aceita qualquer tamanho).",
        e="Cenário de borda: 1 bloco, 8 blocos (4 KiB), tamanho não múltiplo de 512 (deve ser rejeitado com erro claro) e 65535 blocos.",
        g="Regressão: teste de integração que monta .po e .hdv de tamanhos variados e lê o bloco 0.",
        pl="Unitário do detector de formato; integração com leitura de blocos; fixture real: imagem de CiderPress II.",
        o="Comportamento já aceito para .hdv (AppleWin-Test: Dummy-image.hdv) e confirmação do mantenedor de que o limite é um defeito."),
    1264: dict(t="HDD: devolver o tamanho em X/Y na chamada STATUS do ProDOS", p="P2", who="autor de BIOS/CP/M para o Apple II",
        w="que o driver ProDOS do disco rígido devolva o número de blocos em X (baixo) e Y (alto) na chamada de status",
        f="que programas como o CP/M Plus da Cirtech leiam o tamanho pelo driver",
        m="Apple //e e //e Enhanced com HDD/SmartPort",
        r="Dado um HDD de N blocos, quando se chama o driver ProDOS com o comando STATUS (0), então X e Y contêm N (e o carry fica limpo), conforme a documentação do driver do ProDOS 2.4.2.",
        e="Cenário de borda: imagem de 65535 blocos e de 1 bloco; tamanho do cabeçalho de $CxFC igual a 0; imagem que cresce automaticamente.",
        g="Regressão: teste de integração com um trecho 6502 que lê X/Y após o STATUS.",
        pl="Integração com firmware 6502 do HDD; fixture: BIOS do CP/M da Cirtech (disponível em fonte aberta); diferencial contra AppleWin.",
        o="Documentação do driver de disco do ProDOS 2.4.2 e a convenção de SmartPort."),
    568: dict(t="Disk II com 36 a 41 trilhas", p="P3", who="usuário de unidades de 40 trilhas",
        w="aceitar imagens de 36 a 41 trilhas",
        f="usar discos do tipo DOS 3.3 de 41 trilhas e o título Captain Goodnight",
        m="Disk II (DSK/NIB)",
        r="Dado um DSK/NIB de 41 trilhas (imagens anexadas à issue), quando montado, então a trilha 40 é legível e gravável e o catálogo é lido; imagens de 40 trilhas continuam funcionando.",
        e="Cenário de borda: imagem de 42 ou 50 trilhas deve ser rejeitada com erro explícito ou aceita por regra documentada; cabeça tentando passar da última trilha.",
        g="Regressão: imagens de 35 e 40 trilhas dão o mesmo resultado de antes.",
        pl="Unitário do tamanho de imagem; integração com as imagens da issue.",
        o="Imagens do relator (criadas à mão, não derivadas de disco físico), portanto o valor depende de software real que use a trilha 41."),
    1446: dict(t="Save-state de versão diferente deixa o emulador inutilizável", p="P1", who="jogador que usa save-state automático",
        w="que um save-state de versão anterior ou inconsistente seja rejeitado com segurança, sem corromper o estado em execução",
        f="que erros como 'unexpected sub map name state' não deixem as unidades de disco sem resposta",
        m="Apple //e Enhanced com MB-C, SSC, impressora, Disk II",
        r="Dado o save-state salvo na 1.30.21.0 (aux ext80, s1 impressora, s2 SSC, s5 MB-C, s6 Disk II) e carregado na 1.30.20.0, quando ocorre 'Card: wrong version', então o estado em execução permanece íntegro e os drives continuam respondendo.",
        e="Cenário de borda: arquivo YAML truncado, com chave desconhecida, com mapa de submódulos inesperado e com versão futura.",
        g="Regressão: teste de que o carregamento falho é atômico (nenhuma mutação parcial) e que o autosave no encerramento não sobrescreve um arquivo bom com um corrompido.",
        pl="Unitário do carregador transacional; integração com YAMLs reais; fuzzing do parser de YAML.",
        o="Save-states reais do AppleWin e o repro do mantenedor; contrato de atomicidade do plano (§17, robustez)."),
    1044: dict(t="YAML do save-state aderente à especificação", p="P3", who="autor de ferramentas que leem save-states",
        w="que o save-state use chaves únicas e tipos consistentes, aceitando o esquema `failsafe` e `core`",
        f="que parsers padrão (VS Code, JavaScript) leiam o arquivo sem ambiguidade",
        m="todos os modelos",
        r="Dado um save-state gravado pelo Rust, quando lido por um parser YAML 1.2 estrito, então não há chaves duplicadas na raiz (hoje `Unit` repete) e a imagem de memória tem tipo único (string hexadecimal).",
        e="Cenário de borda: leitura de save-states antigos do AppleWin (com `Unit` repetido) continua suportada pelo importador.",
        g="Regressão: teste que grava e relê o arquivo e valida por esquema.",
        pl="Unitário do serializador; integração com validação por esquema; round-trip com importador do AppleWin.",
        o="Especificação YAML 1.2 e save-states reais do AppleWin (para compatibilidade de leitura)."),
    226: dict(t="Argumentos longos em comandos do depurador não derrubam o emulador", p="P2", who="usuário do depurador",
        w="que `SA D000:FFFF \"OVERFLOW\"` e entradas longas produzam erro claro",
        f="que o depurador nunca caia por tamanho de argumento",
        m="todos os modelos",
        r="Dado o comando `SA D000:FFFF \"OVERFLOW\"` e variantes com cadeias de centenas de caracteres, quando executado, então o depurador mostra uma mensagem de erro e continua, sem pânico nem truncamento silencioso.",
        e="Cenário de borda: caminho com 80 caracteres ou mais (#268), nome com caracteres não ASCII, linha maior que a largura do console.",
        g="Regressão: teste de propriedade que gera argumentos aleatórios para todos os comandos e exige ausência de pânico.",
        pl="Unitário do parser de comandos; fuzzing curto; integração via script do depurador.",
        o="Repro da issue; o defeito original é um estouro de buffer do C++, aqui vira requisito de robustez (sem pânico)."),
    445: dict(t="Breakpoint de memória em acessos à pilha", p="P2", who="desenvolvedor Assembly",
        w="que `BPM` dispare em acessos implícitos à pilha (PHA/PLA/JSR/RTS/PHX/PLX/PHP/PLP)",
        f="rastrear interpretadores sem passo a passo",
        m="6502 e 65C02",
        r="Dado `BPM 180` e um programa que executa `PHX` com S apontando para $180, quando executado, então o breakpoint dispara; o mesmo para PHA, PLA, JSR, RTS, PHY, PLY, PHP e PLP.",
        e="Cenário de borda: pilha com wraparound em $1FF/$100; leitura versus escrita (BPMR/BPMW); instruções de 65C02.",
        g="Regressão: teste unitário por opcode que acessa a pilha, parametrizado em BPMR e BPMW.",
        pl="Unitário por opcode; integração via script do depurador.",
        o="Especificação do 6502/65C02 (acessos de barramento por ciclo) e relatos do solicitante e do mantenedor."),
    1381: dict(t="Disassembly do depurador deslocado em 1 para bytes-alvo", p="P2", who="usuário do depurador",
        w="que alvos de dados (`da B_2100`) sejam desmontados no endereço certo",
        f="ler código com tabelas de endereço sem erro de um byte",
        m="todos os modelos",
        r="Dado `2000:20 00 BF C7 00 21 60 DB 2003 DB 2100 DA 2101 DA 2004 2000L`, quando desmontado, então a terceira linha é `da B_2100` (hoje mostra `da A_2101-1`).",
        e="Cenário de borda: dois símbolos adjacentes de tamanhos diferentes e alvo na fronteira de página.",
        g="Regressão: o repro acima como teste de snapshot da saída do desmontador.",
        pl="Unitário do desmontador/tabela de símbolos; snapshot do texto.",
        o="Repro e saída esperada da própria issue."),
    249: dict(t="Ctrl-Espaço (trace over) em JSR às vezes para na instrução errada", p="P2", who="usuário do depurador",
        w="que 'Trace Over' em um JSR sempre pare na instrução seguinte ao JSR",
        f="pular sub-rotinas de forma confiável",
        m="Apple II+ (repro com Apple II Pascal 1.1)",
        r="Dado `BPR PC=D14E` no disco Apple2Pascal 1.1 e Ctrl-Espaço no JSR, quando repetido pelo menos 17 vezes, então para sempre na instrução seguinte (o relator vê parada em instrução diferente).",
        e="Cenário de borda: o JSR chama código que gera interrupções, desvia a pilha ou dispara outro breakpoint dentro da sub-rotina.",
        g="Regressão: teste automatizado com o mesmo roteiro e contagem de repetições.",
        pl="Integração com a imagem de disco e script do depurador; unitário do cálculo do endereço de retorno.",
        o="Roteiro de repro da issue (reproduzido por Michaelangel007) e semântica de 'step over'."),
    1426: dict(t="Ponteiro de memória do LC/Saturn no depurador", p="P2", who="usuário do depurador com Language Card/Saturn",
        w="que o painel de memória e os comandos mostrem a RAM do cartão LC/Saturn ativo",
        f="depurar programas que usam LC e Saturn de 128K",
        m="II/II+ com LC ou Saturn no slot 0 ou outro; //e com Saturn",
        r="Dado um //e com Saturn no slot 3 e o teste RAMTEST128K, quando se entra em `bp 2838` e `m1 l2/d000`, então o painel atualiza com o banco ativo do Saturn (hoje mostra o LC do slot 0).",
        e="Cenário de borda: LC e Saturn simultâneos, bancos 1 e 2, leitura com RAM desabilitada.",
        g="Regressão: teste de que o dump do depurador coincide com o conteúdo gravado no `SaveSnapshot`.",
        pl="Unitário do resolvedor de ponteiros; integração com RAMTEST128K.",
        o="Conteúdo do `SaveSnapshot` do cartão (confirmado correto pelo mantenedor) e teste RAMTEST128K da Saturn Systems."),
    375: dict(t="Echo II (síntese de fala TMS5220) para acessibilidade", p="P2", who="usuário cego do Apple II",
        w="emulação do Echo II/II+ com TMS5220 e suporte ao Textalker",
        f="usar software falante sem hardware, com leitura de tela por voz",
        m="Apple II+, //e, //e Enhanced",
        r="Dado o disco ECHOPLUS (Textalker), quando o auto-detect do BEX procura o cartão, então o Echo é detectado e o texto é falado com formas de onda coerentes com o TMS5220.",
        e="Cenário de borda: falas concorrentes, buffer de LPC vazio, taxa de quadro variável; dispositivo ausente continua não detectado.",
        g="Regressão: PCM de uma frase fixa com hash/tolerância; teste de que o cartão ausente não altera o boot.",
        pl="Unitário do decodificador LPC; integração com ECHOPLUS; avaliação auditiva por usuário real.",
        o="TMS5220 (datasheet) e tabelas do MAME. ATENÇÃO LICENÇA: o código do MAME não é GPL-compatível com o AppleWin; no Rust é preciso verificar a licença de qualquer porte (há um porte contribuído com tabelas idênticas às do MAME, em análise pelo AppleWin)."),
    1156: dict(t="Tecla Backspace do PC gera DELETE ($7F)", p="P3", who="usuário de teclado de PC",
        w="uma opção para que a tecla Backspace gere o código DELETE ($7F) do Apple II",
        f="apagar texto em programas que usam DELETE (por exemplo Merlin, ProDOS)",
        m="II+, //e e //e Enhanced",
        r="Dado o teclado de PC e a opção ativada, quando se pressiona Backspace, então o Apple II lê $7F; com a opção desativada continua lendo $08 (como a seta esquerda).",
        e="Cenário de borda: Ctrl+Backspace (hoje também $7F), Delete do PC e repetição de tecla.",
        g="Regressão: tabela de teclas com os códigos esperados para Backspace, Delete e Left com e sem a opção.",
        pl="Unitário da tabela de teclas; integração com a fila de teclado.",
        o="Comportamento do teclado do //e (Delete no canto superior direito) e o padrão de outros emuladores (Virtual II), citados na issue."),
    834: dict(t="Teclado independente do layout do sistema operacional", p="P1", who="usuário com layout não-US ou com dead keys",
        w="que as teclas físicas e os caracteres digitados sejam mapeados para o Apple II sem depender do layout do SO",
        f="digitar [ ] { } \\ | ~ @ e acentos em qualquer layout, em Windows, Linux e macOS",
        m="todos os modelos",
        r="Dado um layout espanhol, francês-canadense ou US-Internacional, quando se digita AltGr+2 (`@`), as teclas à esquerda do P (`[ ] { } \\ |`) e `'` seguido de `s`, então o Apple II recebe os caracteres correspondentes (e não perde o primeiro caractere após uma dead key), sem acionar Solid-Apple por engano.",
        e="Cenário de borda: Ctrl+@ (cheat do Lode Runner), AltGr combinado com Ctrl, e o controle remoto/automação injetando teclas direto no Apple II.",
        g="Regressão: matriz de teclas por layout (US, ES, FR-CA, UK, US-Intl) com o código esperado em cada caso.",
        pl="Unitário da camada de mapeamento; integração com fila de teclado; cross-platform (Windows, Linux, macOS); manual com layouts reais.",
        o="Tabela de teclado do Apple II e comportamento esperado descrito nas issues #834, #1268, #1418 e #974; sem equivalente Windows-only (a API de captura é a do host)."),
    1161: dict(t="Teclado: AKD e Prince of Persia", p="P1", who="jogador de Prince of Persia",
        w="que o bit AKD (any key down) reflita o momento exato de decodificação como no //e",
        f="evitar os erros de leitura de teclado que Prince of Persia cometeria no hardware",
        m="Apple //e e //e Enhanced",
        r="Dado o vídeo de demonstração de Scott Elliott (PoP no //e), quando um jogo lê AKD logo após o pressionar de tecla, então AKD e KEYSTROBE seguem a temporização do UTAIIe (p. 7-12): AKD pode ficar ativo antes de o strobe ser definido.",
        e="Cenário de borda: duas teclas pressionadas, soltar uma sem soltar a outra, e teclas Open/Solid-Apple.",
        g="Regressão: teste com sequência de eventos de tecla e leituras de $C000/$C010 em ciclos conhecidos.",
        pl="Unitário do modelo do teclado; integração com PoP; manual com o vídeo de referência.",
        o="Understanding the Apple IIe (Sather) e os detalhes publicados por Scott Elliott (applefritter)."),
    293: dict(t="Valor central do joystick ausente (Carmen Sandiego)", p="P3", who="jogador de jogos educacionais",
        w="que, sem joystick, PDL(0)/PDL(1) devolvam um valor central plausível",
        f="que 'Where in the USA is Carmen Sandiego' não fique com a seta para cima presa",
        m="Apple //e",
        r="Dado o jogo em um //e sem joystick conectado, quando se abre um menu de cursor (Ctrl-D na tela da impressora), então a seleção não percorre as opções sozinha.",
        e="Cenário de borda: valores centrais calibrados em torno de 128; trim de joystick; botão pressionado sem eixo.",
        g="Regressão: teste de que os valores de PDL sem dispositivo ficam dentro da faixa central.",
        pl="Integração com a imagem do jogo; unitário do leitor de paddle.",
        o="Comportamento do MESS e do Virtual II citados na issue (valor central 154 no MESS); hardware real sem paddles não foi medido."),
    650: dict(t="NTSC: imagem deslocada 1 pixel para baixo", p="P2", who="usuário do modo TV colorido",
        w="que a primeira e a última linha do filtro NTSC tenham a espessura correta",
        f="evitar a linha cinza no topo e a linha fina na base",
        m="todos os modelos, modos TV/monitor colorido",
        r="Dado um texto de 24 linhas, quando renderizado em modo TV colorido, então a linha superior tem a mesma espessura e cor que as intermediárias e a última não é mais fina (discussão em aberto: 561 linhas ou outra mistura entre linhas).",
        e="Cenário de borda: 50 Hz; modo com borda preta de 1 px; troca de tela cheia para janela.",
        g="Regressão: hash do framebuffer da tela de texto e da tela de teste de gráficos, com tolerância.",
        pl="Unitário do blend vertical; integração com captura; comparação com fotos do relator.",
        o="Capturas do relator e geometria do sinal NTSC (262 linhas por campo); sem hardware medido na issue."),
    1386: dict(t="Ciclo exato do acesso em modos indiretos (TXTCLR/TXTSET)", p="P1", who="desenvolvedor de demos",
        w="que o acesso de I/O por `LDA (zp,X)` e `LDA (zp),Y` ocorra no ciclo correto dentro da instrução",
        f="que mudanças de modo de vídeo caiam na posição certa da linha",
        m="Apple //e",
        r="Dado o teste de ciclo do aipc (TXTCLR/TXTSET/MIXCLR/MIXSET) com `LDA ($02,X)` apontando para $C050, quando renderizado, então a quarta linha muda no 4º ASCII 0, como no //e real (hoje no 2º).",
        e="Cenário de borda: página cruzada em `LDA (zp),Y`, modos absolutos (acesso no 4º ciclo) e indexados (5º ou 6º ciclo).",
        g="Regressão: tabela de ciclo de acesso de I/O por modo de endereçamento (duplicata de #665).",
        pl="Unitário do ciclo de barramento por opcode; integração com o teste do aipc; diferencial contra AppleWin.",
        o="Captura do Apple IIe real publicada na issue; tabela de ciclos do 6502/65C02."),
    1480: dict(t="Alinhamento de modos de vídeo (teste Vidmodes) e PAL", p="P2", who="usuário de CRT/PAL",
        w="que o desenho de texto de 40/80 colunas e o modo PAL fiquem alinhados ao hardware",
        f="evitar linhas deslocadas de 1 a 2 pixels e regressão de cor para P&B em PAL",
        m="Apple //e Enhanced, 60 Hz e 50 Hz",
        r="Dado o disco Vidmodes_260326.po, quando renderizado em 60 Hz e PAL, então o texto de 80 colunas começa na posição do hardware, as linhas intermediárias de texto não ficam em +/-1 ou 2 pixels, e o modo PAL não alterna para P&B no meio da linha.",
        e="Cenário de borda: C00D (80VID: um '<' em 40 colunas) e C00F (ALTCHAR: '[' e losango trocados) apontados por univta0001.",
        g="Regressão: captura de cada tela do Vidmodes com hash/tolerância e testes dos soft switches C00D/C00F.",
        pl="Integração com a imagem; diferencial contra a captura de CRT do arekkusu e contra o AppleWin.",
        o="Fotos de um //e Enhanced real ligado a CRT, publicadas na issue; teste Vidmodes de arekkusu."),
    1358: dict(t="Mockingboard: escritas de AY perdidas no modo velocidade máxima", p="P2", who="jogador com avanço rápido",
        w="que o estado final dos registradores do AY seja o mesmo em velocidade normal e máxima",
        f="evitar tons constantes após o 'fast-forward'",
        m="Mockingboard A/C, Phasor",
        r="Dado Willy Byte tocando e ScrollLock segurado, quando a trilha termina em velocidade máxima, então os registradores finais do AY são os mesmos da execução normal (hoje as escritas além do limite de 8000 mudanças são descartadas).",
        e="Cenário de borda: buffer de mudanças cheio, drenagem antes de aplicar escritas diretas, alternância entre normal e máximo.",
        g="Regressão: teste que injeta mais de 8000 escritas e exige o estado final correto.",
        pl="Unitário do buffer de mudanças do AY; integração com sequência gravada de Willy Byte.",
        o="Estado final dos registradores medido em velocidade normal, descrito pelo mantenedor na issue."),
    1172: dict(t="SSI263 com velocidade de CPU diferente de 1 MHz", p="P2", who="usuário de fala sintética (SC-01/SSI263)",
        w="que a SSI263 e o SC-01 funcionem com CPU a 0,5, 1,5 ou 2 MHz",
        f="que Total Replay e mb-audit não falhem quando a velocidade é alterada",
        m="Mockingboard/Phasor com SSI263",
        r="Dado `play-sc01-using-ssi263 v0.7` a 1,5 MHz, quando se toca uma frase SC-01, então há fala (hoje há silêncio, e as frases SSI263 saem em 'gibberish').",
        e="Cenário de borda: mb-audit v1.2 a 0,5 MHz e 2 MHz (teste 41:00:00 deve dar 00); troca de velocidade durante a fala.",
        g="Regressão: mb-audit a 3 velocidades em modo headless.",
        pl="Unitário da base de tempo da SSI263; integração com mb-audit e Total Replay.",
        o="mb-audit (passa a 0,5 MHz no AppleWin 1.31) e o comportamento da SSI263 em hardware descrito pelo mantenedor."),
    1353: dict(t="Slot auxiliar vazio ou com cartão de 80 colunas de 1 KiB", p="P3", who="entusiasta de hardware fiel",
        w="modelar o //e com o slot auxiliar vazio e com o cartão de 80 colunas de 1 KiB",
        f="reproduzir o floating bus e o aliasing de memória que o hardware real produz",
        m="Apple //e e //e Enhanced",
        r="Dado o slot auxiliar vazio, quando a CPU lê a memória auxiliar, então recebe o floating bus e as escritas são descartadas; com o cartão de 1 KiB, o acesso de 64 KiB é mapeado nesses 1 KiB.",
        e="Cenário de borda: soft switches RAMRD/RAMWRT continuam funcionando com slot vazio; texto de 80 colunas funciona sem aux (#1341).",
        g="Regressão: teste de integração para cada configuração de slot auxiliar.",
        pl="Unitário do mapa de memória; integração com programa de teste de 80 colunas; custo de desempenho medido (a implementação do AppleWin duplica o laço da CPU).",
        o="UTAIIe (Sather) e hardware real descrito pelo mantenedor; parte já foi entregue no AppleWin 1.30.31."),
    506: dict(t="Imagens em branco devem falhar com erro de E/S", p="P3", who="usuário que cria imagens em branco",
        w="que um .dsk/.nib recém-criado (comprimento zero) leia como disco sem formato (erro de E/S) ou seja criado com tamanho válido",
        f="evitar catálogos 'vazios' falsos e laços infinitos no DOS 3.3",
        m="Disk II (DSK/NIB)",
        r="Dado um .dsk criado como arquivo inexistente e o comando CATALOG do DOS 3.3, quando executado, então devolve I/O ERROR (hoje devolve disco vazio para .dsk; .nib já dá I/O ERROR).",
        e="Cenário de borda: arquivo de comprimento zero aceito e convertido na abertura para 140 KiB (.dsk) ou 227,5 KiB (.nib); setor de boot customizado opcional.",
        g="Regressão: teste de integração que cria .dsk e .nib vazios, faz INIT do DOS 3.3 e do ProDOS FORMAT e confirma o resultado (uso de teste headless citado pelo mantenedor).",
        pl="Unitário do carregador de imagem; integração com save-state que formata o disco.",
        o="Comportamento do hardware (disco nunca formatado devolve fluxo aleatório) e roteiro de teste do mantenedor."),
}



def parse_triage():
    rows = {}
    for ln in TRIAGE.read_text(encoding="utf-8").splitlines():
        m = re.match(r"^\| (\d+) \| (.*?) \| (\S+) \| (.*?) \| \[#\d+\]\([^)]*\) \| (\S*) \| (\S*) \| (\S*) \| (\S*) \|$", ln)
        if m:
            n = int(m.group(1))
            rows[n] = dict(n=n, titulo=m.group(2), valor=m.group(5), esforco=m.group(6), risco=m.group(7), sel=m.group(8))
    return rows


def sh(args, inp=None):
    r = subprocess.run(args, input=inp, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"{' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def title_of(n, s):
    return f"[UP-{n:04d}] {s['t']} — {s['p']}"


def body_of(n, s, row, up_title):
    who = s.get("who", "mantenedor")
    return f"""### [UP-{n:04d}] {s['t']} — {s['p']}
**Tipo:** história
**Como** {who}, **quero** {s['w']}, **para** {s['f']}.
**Referência:** [AppleWin #{n}]({UPSTREAM}/{n}) "{up_title}" (aberta no upstream, coleta fixada em `target/conformance-cache/upstream/issues.json`); triagem em `docs/conformance/upstream-open-issues-triage.md`.
**Estado atual:** UNVERIFIED (comportamento no Rust ainda não medido; a issue vem do AppleWin, onde está aberta).
**Modelos/configurações:** {s['m']}.
**Dependências:** nenhuma bloqueante; usa a infraestrutura de oráculos de `docs/conformance/test-oracles.md`.
**Critérios de aceite (CA):**
- CA-01 (teste de reprodução): {s['r']}
- CA-02 (borda): {s['e']}
- CA-03 (teste de regressão): {s['g']}
- CA-04: o resultado fica registrado como PASS, FAIL ou N/A com evidência (comando, SHA e artefato); sem evidência a história permanece UNVERIFIED.
**Plano de teste:** {s['pl']}
**Oráculo:** {s['o']}
**Valor / esforço / risco:** {row['valor']} / {row['esforco']} / {row['risco']}
**Evidências:** a preencher na execução (comandos, logs, SHA, capturas, SO e versão).
**Definition of Done:** CA completos, testes verdes, revisão independente, documentação e sem regressões.
**Rollback/risco:** soluções exclusivas do Windows não são importadas; correções devem preservar a temporização já validada (risco {row['risco']}).

---
Épico: #{EPIC}
"""


def find_existing(n):
    out = sh(["gh", "issue", "list", "-R", REPO, "--state", "all", "--limit", "20",
              "--search", f'"[UP-{n:04d}]" in:title', "--json", "number,title,state"])
    for it in json.loads(out):
        if it["title"].startswith(f"[UP-{n:04d}]"):
            return it
    return None


def selected():
    rows = parse_triage()
    sel = {n: r for n, r in rows.items() if r["sel"] == "sim"}
    missing = sorted(set(sel) - set(S))
    extra = sorted(set(S) - set(sel))
    if missing or extra:
        sys.exit(f"inconsistência: selecionadas sem história {missing}; histórias não selecionadas {extra}")
    return sel


def upstream_titles():
    d = json.loads(CACHE.read_text(encoding="utf-8"))
    return {i["numero"]: i["titulo"].strip() for i in d["itens"] if i["tipo"] == "issue"}


def create(dry):
    sel = selected()
    ups = upstream_titles()
    made = {}
    for n in sorted(sel):
        s = S[n]
        title = title_of(n, s)
        ex = find_existing(n)
        if ex:
            print(f"existe  #{ex['number']:>3}  {ex['title']}")
            made[n] = ex["number"]
            continue
        body = body_of(n, s, sel[n], ups[n])
        labels = ["tipo:historia", "epico:EP-11", f"prioridade:{s['p']}", "estado:UNVERIFIED", "onda:4"]
        if dry:
            print(f"criaria {title}  labels={labels}")
            continue
        args = ["gh", "issue", "create", "-R", REPO, "--title", title, "--body-file", "-", "--milestone", MILESTONE]
        for lb in labels:
            args += ["--label", lb]
        url = sh(args, inp=body).strip()
        num = int(url.rsplit("/", 1)[1])
        made[n] = num
        print(f"criada  #{num:>3}  {title}")
    return made


def local_numbers():
    sel = selected()
    res = {}
    for n in sorted(sel):
        ex = find_existing(n)
        if ex:
            res[n] = ex["number"]
    return res


def epic(dry):
    sel = selected()
    nums = local_numbers()
    lines = [EPIC_BEGIN, "## Histórias de issues abertas do AppleWin (HU-1101)", ""]
    for n in sorted(sel):
        if n in nums:
            lines.append(f"- [ ] #{nums[n]} {title_of(n, S[n])}")
    lines.append(EPIC_END)
    block = "\n".join(lines)
    cur = json.loads(sh(["gh", "issue", "view", str(EPIC), "-R", REPO, "--json", "body"]))["body"]
    if EPIC_BEGIN in cur:
        new = re.sub(re.escape(EPIC_BEGIN) + r".*?" + re.escape(EPIC_END), lambda _: block, cur, flags=re.S)
    else:
        new = cur.replace("\n---\nOrigem:", "\n" + block + "\n\n---\nOrigem:") if "\n---\nOrigem:" in cur else cur + "\n\n" + block + "\n"
    if dry:
        print(block)
        return
    sh(["gh", "issue", "edit", str(EPIC), "-R", REPO, "--body-file", "-"], inp=new)
    print(f"épico #{EPIC} atualizado com {len(nums)} tarefas")


def report(dry):
    sel = selected()
    nums = local_numbers()
    ups = upstream_titles()
    out = ["# Issues abertas do AppleWin selecionadas (HU-1101 / EP-11)", "",
           "Gerado por `tools/conformance/upstream_open_to_issues.py --report`. Fonte: `upstream-open-issues-triage.md` (coluna Selecionada).",
           f"Total selecionado: {len(sel)}. Histórias criadas em {REPO}, épico #{EPIC}.", "",
           "| upstream # | título | valor | esforço | risco | issue local # |", "|---:|---|---|---|---|---:|"]
    for n in sorted(sel):
        r = sel[n]
        loc = f"#{nums[n]}" if n in nums else "pendente"
        out.append(f"| [{n}]({UPSTREAM}/{n}) | {ups[n]} | {r['valor']} | {r['esforco']} | {r['risco']} | {loc} |")
    text = "\n".join(out) + "\n"
    if dry:
        print(text)
    else:
        REPORT.write_text(text, encoding="utf-8")
        print(f"escrito {REPORT}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dry-run", action="store_true", help="não escreve nada (nem no GitHub nem em arquivos)")
    ap.add_argument("--epic", action="store_true", help="atualiza a lista de tarefas da issue épica")
    ap.add_argument("--report", action="store_true", help="escreve upstream-open-selected.md")
    a = ap.parse_args()
    if a.epic:
        epic(a.dry_run)
    elif a.report:
        report(a.dry_run)
    else:
        create(a.dry_run)


if __name__ == "__main__":
    main()
