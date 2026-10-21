# Lacunas e limitações conhecidas

Esta lista parte de duas fontes escritas pela própria equipe: [docs/manual/12-diferencas-em-relacao-ao-applewin.md](../manual/12-diferencas-em-relacao-ao-applewin.md) e [docs/pendencias.md](../pendencias.md). Cada item foi reescrito no vocabulário de Estado de verificação (veja o [README](README.md)).

Como ler:

- **FAIL** só quando o texto de origem afirma que o comportamento é diferente do AppleWin. Isso vem de leitura do código pela equipe, não de uma execução contra o Oráculo; por isso o campo "Prova" diz quando ainda falta o teste que demonstra a diferença.
- **UNVERIFIED** quando o texto de origem diz que algo existe ou é igual, mas ninguém comparou com o AppleWin. `IMPLEMENTED` nunca vale PASS.
- **NOT_APPLICABLE** não é usado aqui sem justificativa técnica; onde o texto sugere que o item pode não se aplicar, ele continua UNVERIFIED até a justificativa ser escrita.
- Os Modelos e configurações valem como o texto de origem; onde ele não diz, o item vale como "não avaliado por Modelo".
- Nada nesta página foi reexecutado em 2026-10-20: é um inventário de afirmações, não um resultado de testes. Nenhum item está em PASS.

## 1. Diferenças já declaradas (manual 12)

### Cartas

| Item | Estado | Prova pendente |
|---|---|---|
| Breakpoint card existe, mas o depurador não lhe entrega os breakpoints | FAIL | Teste que arma um breakpoint e observa que a Carta não o recebe |
| Mockingboard usa um só tipo de AY por carta; soquetes do Phasor e chaves `6522a-bad` não são modelados | FAIL | Comparação de registradores e saída com o AppleWin em Mockingboard e Phasor |
| SNES MAX nasce sem controles ligados; arquivos `-snes-max-user-joy` não são lidos | FAIL | Teste de configuração com o arquivo de usuário |
| ThunderClock Plus existe na biblioteca, mas não há como escolhê-lo na configuração | FAIL | Teste da fábrica de Cartas a partir da configuração |
| Uthernet II: PPPoE, interrupções e SPI não implementados; lado servidor com pouco teste; sockets TCP e UDP fechados ao carregar save-state | FAIL | Matriz de recursos (SPI, IRQ, TCP, UDP, PPPoE) com testes locais em loopback |
| Uthernet I e II: rede real exige o recurso `pcap` | BLOCKED | Roteiro manual com hardware de rede e permissões; não executado |
| Super Serial Card: porta COM exige o recurso `serialport` | BLOCKED | Roteiro manual com porta serial real; não executado |
| Placas RGB: estado da placa AppleColor/RGB não é guardado no save-state | FAIL | Teste de ida e volta de save-state com a placa ativa |
| VidHD: modos de texto 80x45 e cor da borda não modelados | FAIL | Captura comparada com o AppleWin |
| VidHD: tela a 640x400 e Super Hi-Res desenhado | UNVERIFIED | Comparação de Quadros com o AppleWin |
| Z80: sem interrupções; efeitos do MEMPTR em INIR/OTIR não modelados | FAIL | Vetores de instruções e Trace comparado com a referência |
| SSI263 reproduz fonemas em taxa fixa, sem inflexão nem filtros | FAIL | Comparação de eventos e envelopes com o AppleWin |
| No-Slot Clock no ][ e ][+ (soquete "F8") não funciona com o ProDOS-8 2.5.0a8, igual ao AppleWin | UNVERIFIED | Verificar que o AppleWin se comporta igual |

### Interface

| Item | Estado | Prova pendente |
|---|---|---|
| Janela de configuração reproduz as páginas do AppleWin | UNVERIFIED | Tabela página a página e opção a opção contra o AppleWin |
| Faltam "Benchmark Emulator" (desativado), volume separado do Mockingboard, lista de interfaces do Npcap/libpcap | FAIL | Lista de opções de interface do AppleWin com a equivalência |
| Lista de portas seriais vem de `/dev` ou COM1 a COM9 | UNVERIFIED | Teste em cada sistema operacional |
| Interface em inglês; manual em português | NOT_APPLICABLE como paridade | Escolha de produto, sem equivalente a comparar; o AppleWin também tem a interface em inglês |
| F1 abre o manual embutido, no lugar do CHM | NOT_APPLICABLE | O CHM é formato exclusivo do Windows |
| Menu do drive: inserir, ejetar, proteção, recentes, trocar drives, criar disco | UNVERIFIED | Teste de ponta a ponta no frontend |
| Faltam "Open as Read Only", soltar imagem sobre o botão do drive (um arquivo solto sempre vai ao drive 1 e reinicia), "Send to CiderPress" | FAIL | Lista de ações do AppleWin e a equivalência; o CiderPress é do Windows, então esse terceiro item pode ser NOT_APPLICABLE |
| Registro das extensões de disco no sistema | NOT_APPLICABLE (Windows) | Justificativa: associação de arquivos é do Windows no AppleWin |
| Alt+Enter alterna a tela cheia; `-alt-enter=open-apple-enter` aceito sem efeito | FAIL (a opção) | Teste da opção de linha de comando |
| Status de trilha e setor na linha de status, não numa dica sobre o LED | UNVERIFIED | Comparação visual com o AppleWin |
| Ctrl+V envia ^V ao Apple; colagem é Ctrl+Shift+V | UNVERIFIED | Teste de teclado |
| Print Screen grava `AppleWin_NNNN.png` no diretório atual | UNVERIFIED | Ver o item de formato de captura (BMP para PNG) em [baseline.md](baseline.md) |
| Ctrl+Print Screen copia só a página de texto de 40 colunas | FAIL | Teste em 80 colunas e página 2 |
| "SHIFT key mod" liga o Shift ao botão 2, mas a temporização dos paddles 2 e 3 é a de um joystick presente | FAIL | Teste de leitura dos paddles contra o AppleWin |
| Falta a leitura do setor ("S??") ao lado da trilha | FAIL | Comparação com a barra de status do AppleWin |
| Opções de captura de teclas do sistema (`-hook-alt-tab`, `-no-hook-system-key`) aceitas sem efeito | NOT_APPLICABLE (Windows) | Justificativa: só têm sentido no Windows |

### Depurador

| Item | Estado | Prova pendente |
|---|---|---|
| Só o console está ligado à interface | UNVERIFIED | Tabela comando, atalho e estado esperado |
| Atalhos de teclado do depurador (setas, Tab, Espaço, Shift+Espaço, Ctrl+setas) não existem | FAIL | Teste de interface que exercita cada atalho |
| Tabela de comandos e texto de ajuda iguais aos do AppleWin | UNVERIFIED | Comparação automática da tabela de comandos |

### Arquivos e configuração

| Item | Estado | Prova pendente |
|---|---|---|
| Configuração em TOML, não no Registro/INI | NOT_APPLICABLE | Escolha de produto; o formato do AppleWin é por plataforma |
| `apple2-config` importa `.reg` e INI do AppleWin; a interface não oferece o comando de importação (funciona via `-conf`) | FAIL (a interface) / UNVERIFIED (o módulo) | Teste de importação com arquivos reais do AppleWin |
| `-conf`, `-current-dir`, `-load-state`, `-screenshot-and-exit`, `-debugger-auto-run`, `-power-on` | UNVERIFIED | Teste de linha de comando comparando com as grafias do AppleWin |
| Opção `-no-joystick<1\|2>` (adicionada ao AppleWin depois da v1.32.1.0) | UNVERIFIED | Não consta do manual; conferir se foi portada (commit `6c579da`, veja [baseline.md](baseline.md)) |
| Save-state `.aws.yaml`; F11 e F12; "Save State On Exit"; arrastar um `.aws.yaml` | UNVERIFIED | Nenhum save-state real do AppleWin foi importado (veja a seção 2) |
| Nome padrão do save-state não muda ao trocar de disco durante a sessão | FAIL | Teste que troca o disco e salva |
| `DELREG.INF` não existe | NOT_APPLICABLE | Justificativa: é um script de Registro do Windows |

### Emulação

| Item | Estado | Prova pendente |
|---|---|---|
| CPU: opcodes não documentados do NMOS 6502 portados com as mesmas fórmulas | UNVERIFIED | Trace diferencial contra o AppleWin instrução a instrução |
| 65C02 sem BBR, BBS, RMB, SMB, WAI, STP; `$CB` e `$DB` são NOP de 1 ciclo, igual ao AppleWin | UNVERIFIED | Trace diferencial no 65C02 |
| Teste estendido de Klaus Dormann roda com instruções emuladas pelo próprio teste; a versão pura está `#[ignore]` | UNVERIFIED | O teste ignorado não conta como evidência; resultado conhecido, não diferencial |
| Reset segue o `CpuReset` do AppleWin (S desce 3, flags preservadas); sem leituras falsas | UNVERIFIED | Trace do power-on e do reset |
| NMI: o AppleWin não o trata na CPU; aqui existe | UNVERIFIED | Possível Divergência intencional; só entra na seção 4 com prova de hardware |
| Apple IIc: só o esqueleto, igual ao AppleWin | UNVERIFIED | Confirmar que o AppleWin se comporta igual |
| Fita cassete não é emulada, igual ao AppleWin | UNVERIFIED | Leitura de `$C020` e `$C060` contra o AppleWin |
| Extensões que o AppleWin não tem (headless, bloco de extensão no YAML, ThunderClock real, sockets UDP e servidor TCP no Uthernet II) | NOT_APPLICABLE como paridade | Não existe comportamento do AppleWin a comparar; o bloco de extensão no YAML precisa de teste de que um AppleWin real ainda lê o arquivo |

### Itens "O que já é igual"

O manual 12 afirma igualdade em Modelos, temporização por Ciclo, renderização NTSC, discos, hardware de som, linha de comando, depurador, YAML e atalhos. Nenhuma dessas afirmações tem teste diferencial; todas valem **UNVERIFIED** até ter Manifesto de execução:

| Afirmação | Estado |
|---|---|
| Todos os Modelos do AppleWin (exceto o IIc), com as mesmas ROMs e peculiaridades de memória | UNVERIFIED (a igualdade dos bytes das ROMs está provada em [rom-provenance.md](rom-provenance.md)) |
| Language Card, Saturn, cartas AUX e Floating bus | UNVERIFIED |
| Temporização por Ciclo, escalonador de eventos, IRQ no último ciclo do opcode | UNVERIFIED |
| Renderização NTSC, monitores RGB e todos os tipos de vídeo | UNVERIFIED |
| Disk II (DSK, DO, PO, NIB, WOZ, 2MG, compactados) e disco rígido (ProDOS, SmartPort) | UNVERIFIED |
| Mockingboard C, Phasor, SSI263, SC-01, SAM, MegaAudio, SD Music | UNVERIFIED |
| Linha de comando, depurador, YAML, atalhos F1 a F12, Pause, Scroll Lock, Print Screen e colagem | UNVERIFIED |

## 2. Pendências técnicas (docs/pendencias.md)

| Item | Estado | Prova pendente |
|---|---|---|
| Breakpoint card, Mockingboard, SNES MAX, ThunderClock Plus, Z80 | veja a seção 1 | (mesmos itens) |
| Disk II e HDD: trocar a imagem pela tela de configuração só vale após reiniciar; o menu do drive insere na hora | UNVERIFIED | Definir o contrato visível (HU-0302) e testar nos três sistemas |
| Atalhos de teclado do depurador, Ctrl+Print Screen, indicador de setor, importação de `.reg`/INI pela interface | veja a seção 1 | (mesmos itens) |
| `emulated_speed` em ~90 MHz e boot do DOS 3.3 em ~88 MHz (tempo de CPU) | UNVERIFIED | Medida feita pela equipe, sem hardware nem compilação fixos e sem mediana e p95; não é desempenho comparado com o AppleWin |
| Laço completo com `Renderer::update` em ~15 MHz com Color TV | UNVERIFIED | Idem |
| `tests/no_alloc.rs` garante zero alocações por passo | IMPLEMENTED | O teste existe e pode ser reexecutado; é propriedade interna, não paridade com o AppleWin |
| `CardCtx` calcula `floating_bus` de forma eager a cada chamada de Carta | UNVERIFIED | Otimizar só depois da suíte diferencial (HU-1001) |
| Nenhum save-state real do AppleWin foi testado; os fixtures foram transcritos do código C++ | FAIL | Importar o corpus do AppleWin-Test (HU-0401). Veja a nota abaixo |
| Cores NTSC conferidas por plausibilidade, sem comparação pixel a pixel | UNVERIFIED | Métricas definidas antes (veja [test-oracles.md](test-oracles.md)) |
| Gamepad, captura de mouse e áudio só testados em testes unitários e Xvfb, sem hardware real | BLOCKED | Roteiro manual com hardware físico; não executado |

Nota sobre save-states: a ausência de teste com arquivos reais é a lacuna de verificação mais importante. Fixtures escritas a partir do código C++ não são Corpus de referência e nunca contam como PASS de interoperabilidade.

## 3. Achados de auditoria anterior: HIPÓTESES a reconfirmar

Os itens abaixo vêm do §2 do plano de auditoria. São **HIPÓTESES**: não foram reproduzidas no SHA atual (`6da6048`, veja [baseline.md](baseline.md)) e não devem ser tratadas como fatos nem como corrigidas até haver reprodução independente. Todas começam como UNVERIFIED.

| Hipótese | Estado | Como reconfirmar |
|---|---|---|
| A configuração de bancos do Saturn 128K (`Saturn128K.banks`) é descartada na conversão de settings para MachineConfig e Carta (HU-0102) | UNVERIFIED (HIPÓTESE) | Teste com 4 e 8 bancos que verifica acesso e independência dos bancos |
| Atalhos do depurador ausentes | já declarado no manual 12 (seção 1); a reconfirmação é a mesma | Veja a seção 1 |
| Nenhum save-state original real executado como teste de importação | confirmado pela própria `docs/pendencias.md` (seção 2) | Veja a seção 2 |
| Verificação NTSC sem oráculo diferencial pixel a pixel | confirmado pela própria `docs/pendencias.md` (seção 2) | Veja a seção 2 |
| Testes em periféricos reais ausentes | confirmado pela própria `docs/pendencias.md` (seção 2) | Roteiros manuais |
| Z80 com interrupções e MEMPTR incompletos | já declarado (seção 1) | Veja a seção 1 |
| Uthernet II parcialmente implementado | já declarado (seção 1) | Veja a seção 1 |
| Diferenças em Mockingboard, Phasor e SSI263 | parcialmente declaradas (seção 1); o alcance é HIPÓTESE | Comparação de registradores e eventos |
| Recursos de UI e configurações não equivalentes | parcialmente declarado (seção 1); o alcance é HIPÓTESE | Tabela opção a opção |

Só o primeiro item é novo em relação à documentação interna. Os demais coincidem com o que a documentação já admite; "confirmado" nesta tabela quer dizer que a lacuna está escrita no repositório, não que foi medida.

## 4. Divergências intencionais

_Nenhuma registrada._

Uma Divergência intencional é um ponto em que o AppleII-Rust segue o hardware real em vez do AppleWin. Como o AppleWin vence (ADR 0004), um item só entra aqui quando houver, juntos:

1. **prova de hardware**: medição, documentação do fabricante ou teste em máquina real que mostre que o AppleWin difere do hardware;
2. **o comportamento do AppleWin e o do AppleII-Rust descritos**, com o SHA do AppleWin e o teste que demonstra a diferença;
3. **o motivo** de seguir o hardware, e o Modelo e a configuração a que se aplica;
4. **decisão do dono do projeto**.

Um bug do AppleWin não é, por si só, uma Divergência intencional. Uma melhoria ou uma extensão que não muda o comportamento comparável também não é: extensões (por exemplo, sockets UDP no Uthernet II) ficam na seção 1. O NMI tratado pela CPU (seção 1) é candidato a entrar aqui, mas só entrará com a prova de hardware.
