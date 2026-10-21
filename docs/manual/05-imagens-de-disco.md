# Imagens de disco

## Introdução

Um computador comum não consegue ler um disquete de Apple II. Os disquetes são analógicos e o Apple II os codifica por software em GCR (Group Code Recording), enquanto os PCs usam MFM. Por isso o emulador não lê disquetes de verdade: ele usa **imagens de disco**, arquivos que guardam o conteúdo completo de um disquete e que a Máquina trata exatamente como um disco real.

## Formatos aceitos

| Extensão | Formato | Observações |
|---|---|---|
| `.dsk`, `.do` | Sectors em ordem DOS 3.3 | 143.360 bytes (35 trilhas de 16 setores de 256 bytes) |
| `.po` | Setores em ordem ProDOS | Mesmo tamanho; a extensão força a ordem ProDOS |
| `.nib` | Nibbles | 232.960 bytes (35 x 6656); guarda cabeçalhos e sincronismos. Serve para alguns discos protegidos |
| `.2mg`, `.2img` | Envoltório 2IMG | Embrulha DOS, ProDOS ou Nibble com metadados (volume DOS, proteção contra gravação) |
| `.woz` | WOZ (Applesauce) | Armazena os bits da superfície do disco; aceita as proteções contra cópia mais difíceis |
| `.hdv` | Disco rígido | Imagens de HDD; também `.po` e `.2mg` |
| `.gz`, `.zip` | Compactados | Qualquer formato acima pode vir compactado; num `.zip` só o primeiro arquivo é usado |

Detalhes sobre os formatos de 143.360 bytes:

- Quando a extensão é `.dsk`, o programa tenta descobrir se a ordem é DOS ou ProDOS examinando o conteúdo do disco. Se o disco não tiver um sistema operacional padrão, assume a ordem DOS, que é a mais comum. Renomear para `.po` força a ordem ProDOS.
- Imagens com cabeçalho ou rodapé extra costumam ser detectadas e tratadas automaticamente.
- Duas variantes de tamanho são aceitas por causa de conversões mal feitas: 143.403 bytes (Castle Wolfenstein) e 143.488 bytes (Rescue Raiders).
- Discos de até 40 trilhas (163.840 bytes) são aceitos desde que o tamanho seja múltiplo de 4.096 bytes por trilha.
- Imagens de 3,5" (800 KB) e do tipo UniDisk/IIgs não cabem num Disk II; elas pertencem ao controlador de disco rígido.

Os discos protegidos contra cópia que usam trilhas especiais só funcionam como `.nib` ou, os mais difíceis, como `.woz`.

## Tamanhos

- Disquete padrão de 5,25": 35 trilhas, 140 KB.
- Disquete estendido: até 40 trilhas, 160 KB.
- Disquete de 3,5": 800 KB (precisa de ProDOS; o DOS 3.3 não suporta).
- Disco rígido: máximo de 32 MB por unidade no ProDOS.

## Carregando imagens

- **F3** (drive 1) e **F4** (drive 2) abrem o seletor de arquivos. Enquanto o seletor está aberto a emulação fica em pausa, para o som não gaguejar.
- O menu do drive (botão direito sobre o botão, ou **Ctrl+F3** e **Ctrl+F4**) permite inserir uma imagem, inserir como **somente leitura** ("Insert disk image (read only)...": o disco entra com a proteção de gravação ligada, como a caixa "Open as Read Only" do AppleWin), ejetar, proteger contra gravação e trocar os drives.
- **Arrastar e soltar** um arquivo de disco sobre a janela, como no AppleWin: solto sobre o botão do drive 2, vai para o drive 2; em qualquer outro lugar, para o drive 1. A Máquina reinicia (como se o botão Run fosse apertado) se o disco foi aceito, **exceto** quando o arquivo foi solto sobre o botão do drive 1. Um arquivo que não é disco só mostra o erro. Arquivos `.aws.yaml` carregam o save-state.
- Na linha de comando: `-d1 arquivo.dsk -d2 outro.dsk` ou, para outros slots, `-s5d1 arquivo.dsk`. Um nome sem opção (como `apple2-rust jogo.dsk`) também insere o disco no drive 1.
- **F5** troca os discos dos drives 1 e 2. Não faça isso durante um acesso ao disco: o programa do Apple pode travar e a imagem pode ser corrompida.

## Trocar imagens com a Máquina ligada

O que vale na hora e o que pede reinício segue o AppleWin (`ApplyConfigAfterClose` e `IsOkToRestart`):

| Mudança | Quando vale |
|---|---|
| Imagem de um Disk II (F3/F4, menu do drive, arrastar, página Slots) | Na hora, a Máquina continua rodando |
| Ejetar, proteção contra gravação, trocar drives | Na hora |
| Imagem de uma unidade de disco rígido de um controlador já no slot (menu do drive, seção "Hard disks") | Na hora (as unidades são ligadas e desligadas a quente) |
| Vídeo, entrada, som, velocidade, preferências, opções de mouse e impressora | Na hora |
| Modelo, CPU, taxa de atualização, uma Carta num slot, opções que montam a Carta (soquetes do Mockingboard, porta do SSC, Uthernet, tamanho da RamWorks, firmware) | Reinício |

O reinício nunca é silencioso: a janela de configuração mostra a lista do que mudou e avisa "Restarting the emulator will reset the state of the emulated machine, causing you to lose any unsaved work". **Cancel** deixa a Máquina como estava. Só F2 e Ctrl+F2 reiniciam por ação direta sua.

A linha de status mostra, por drive, a trilha e o setor (`Drive 1: T 5 S11 jogo.dsk`). Como no AppleWin, o setor não vem do hardware: é o que a RWTS do DOS 3.3 ou do ProDOS diz estar lendo para aquele slot, e aparece como `S??` quando o sistema não é um deles (ou fala de outro slot).

Os LEDs ao lado dos botões de drive indicam o estado: apagado (motor desligado), verde (lendo) ou vermelho (gravando). Uma imagem aberta como somente leitura se comporta como um disquete com a proteção de gravação ligada.

Um disco WOZ que se identifica como de 13 setores (DOS 3.2) faz a Carta Disk II usar o firmware antigo de 13 setores no próximo reset. A opção `-s6 diskii13` força o firmware de 13 setores para qualquer imagem.

## Criando imagens

A biblioteca de armazenamento sabe criar os mesmos tipos de imagem do AppleWin: um disquete padrão de 140 KB, 35 trilhas e 16 setores por trilha, vazio e sem boot (extensões `.dsk`, `.do`, `.nib` e `.woz`; sem extensão vale `.dsk`), e discos já formatados com DOS 3.3 ou ProDOS. Um disco ProDOS novo recebe `PRODOS`, `BASIC.SYSTEM`, `QUIT.SYSTEM` e `BITSY.BOOT`.

Nesta versão o seletor de arquivos da interface só abre imagens que já existem, e os itens "New ProDOS" e "New DOS 3.3" do menu de drive do AppleWin ainda não estão na interface. Veja "Diferenças em relação ao AppleWin". Se você passar na linha de comando o nome de uma imagem vazia ou inexistente, o comportamento é o da biblioteca: uma imagem em branco do tipo que a extensão indica.

**Formatar de dentro do Apple.** O jeito clássico continua valendo: dê boot num disco mestre do DOS 3.3, troque para um disco vazio, escreva um programa de boot, por exemplo `10 PRINT CHR$(4);"CATALOG"`, e digite `INIT HELLO`. Isso grava o DOS 3.3 nas três primeiras trilhas e torna o disco bootável.

Os discos de exemplo do AppleWin (DOS 3.3 System Master, ProDOS 2.4.3) não fazem parte deste repositório.

## Cópia e discos protegidos

Muitos programas do Apple II eram protegidos contra cópia gravando setores com tamanhos ou identificações especiais. Copiadores que trabalham por setores não os copiam, mas copiadores de nibbles (bit a bit por trilha) sim. Para rodar um desses programas no emulador, basta ter uma imagem `.nib`. Para as proteções que nem os copiadores de nibbles copiam, só uma imagem `.woz`, gerada com o hardware e software Applesauce, funciona.

## Disco rígido

O controlador de disco rígido (HDC) vai tipicamente no slot 7, e cada Carta aceita até 8 unidades (`-s7h1` a `-s7h8`, ou `-h1` e `-h2`). As imagens são `.hdv`, `.po` ou `.2mg` de até 32 MB.

- O Apple tenta dar boot pelo slot 7 primeiro. Sem imagens ligadas ao HDC, tenta o slot 6. Para dar boot num disquete com o HDC ativo, segure Open Apple (//e) ou Shift (][ e ][+) durante o reset, ou digite `PR#6`.
- Firmware padrão: ][, ][+ e //e original usam o modo bloco do ProDOS (até 4 unidades); //e Enhanced usa SmartPort (até 8 unidades). As opções `-s7 hdc-sp`, `hdc-bm2` e `hdc-bm4` forçam SmartPort e modo bloco com 2 ou 4 unidades; `-hdc-firmware-v1` e `-hdc-firmware-v2` escolhem a versão do firmware.
- `-harddisknumblocks N` define o tamanho (em blocos de 512 bytes) das imagens criadas pelo HDC.
- Não remova a Carta nem o disco enquanto o Apple grava.
- O Copy ][+ 7.1 trava ao calcular blocos livres; isso é um erro do próprio programa, corrigido nas versões seguintes.

## Opções avançadas de formatação

A formatação em nível de trilha (nibblização, setores 13 e 16, volume, trilhas extras) é feita pela biblioteca `apple2-storage`. Para a maior parte dos usos, a criação de um disco novo e a formatação pelo DOS ou ProDOS bastam.
