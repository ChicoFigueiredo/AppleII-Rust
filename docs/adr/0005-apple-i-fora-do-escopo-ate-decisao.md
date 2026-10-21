# Apple I fora do escopo até decisão explícita do dono

Status: Proposta (HU-1204, P3, não implícito)

O pedido original dizia "Apple I ou superior". O plano interpreta isso como Apple II e seus Modelos suportados, com Apple I possível no futuro mediante decisão explícita de escopo. O Apple I é uma arquitetura diferente, não uma variante do Apple II. Esta ADR compara as duas, estima o esforço, trata da questão legal da ROM e propõe **não implementar** até o dono decidir. Nada aqui promete suporte.

Fontes: [Apple-1 Operation Manual (1976), Computer History Museum](https://www.computerhistory.org/collections/catalog/102646518) (a listagem do monitor e o esquema estão nele; o catálogo não oferece link de download, então o scan a usar é a confirmar); [driver `apple1.cpp` do MAME](https://github.com/mamedev/mame/blob/master/src/mame/apple/apple1.cpp); [sbprojects, Apple-1](https://www.sbprojects.net/projects/apple1/). Onde as fontes não bastam, o texto diz "a confirmar".

## Comparação das arquiteturas

| Aspecto | Apple I | Apple II (este projeto) |
|---|---|---|
| CPU | MOS 6502. Clock nominal de 1,023 MHz; o MAME usa 960 kHz efetivos, por causa do refresh da RAM (a confirmar contra o manual) | 6502/65C02 a 1,0205 MHz, ciclo a ciclo |
| Mapa de memória | RAM em $0000 até $BFFF conforme a configuração (MAME: 4K a 44K em passos de 4K, padrão 48K); PIA em $D010–$D013 (espelhada, máscara $0FEC no MAME); RAM de BASIC em $E000–$EFFF no MAME (a confirmar a origem); ROM do monitor em $FF00–$FFFF | RAM até $BFFF, soft switches em $C000–$C0FF, ROMs de slot em $C100–$CFFF, ROM de sistema em $D000–$FFFF, com Paginação (Language Card, aux RAM no //e) |
| ROM e monitor | Woz Monitor, 256 bytes. O MAME carrega em dois PROMs de 4 bits (`apple-a1.a1`, `apple-a2.a2`) | ROMs de 12 KB (II/II+) ou 16 KB (//e), mais ROMs de carta |
| Vídeo e terminal | Terminal por hardware: Signetics 2513 (gerador de caracteres) e shift registers geram o sinal de vídeo. Tela de 24 linhas por 40 colunas (MAME e sbprojects), com cursor, rolagem e CLEAR. Não existe framebuffer endereçável pela CPU: a CPU só escreve um caractere na PIA, e o terminal o consome em ritmo próprio | Framebuffer na RAM (texto, lo-res, hi-res), varredura NTSC/PAL por Ciclo |
| I/O | PIA 6820/6821 em $D010–$D013: porta A para teclado ASCII (bit 7 sempre alto), porta B para saída de vídeo, CA1 como strobe de tecla, CB2 como "terminal ocupado" (a confirmar no manual) | Teclado em $C000/$C010, alto-falante, porta de jogos, via soft switches |
| Expansões | Barramento de expansão próprio, que não é o de slots do Apple II. Carta ACI de cassette (MAME: opção `cassette`, entrada por `C100R`) e cartões modernos como CFFA1 (MAME: opção `cffa`) | Slots 0–7, Disk II, Mockingboard, SSC etc. |
| Temporização | O terminal aceita cerca de 60 caracteres por segundo (a confirmar no manual); o programa espera o bit de ocupado. O cursor pisca por timer no MAME (0,52 s) | Tudo em Ciclos do Scheduler, vídeo preso aos Quadros de 17.030/20.280 Ciclos |

Para o Apple I, o MAME é referência de comportamento, mas modela o terminal em alto nível; a fidelidade de bit do shift register é a confirmar.

## Reuso e fronteira arquitetural

Aproveita-se:
- `apple2-cpu`: o 6502 NMOS e o trait `Bus` servem sem mudança.
- O modelo do Scheduler (eventos em Ciclos, núcleo headless e determinístico da ADR 0001), embora a implementação atual viva em `apple2-core` e precise ser extraída ou duplicada.
- Do frontend (`apple2-rust`): janela, entrada de teclado, áudio de cassette, thread de emulação e ritmo (ADR 0003), com uma tela de texto nova.
- O mecanismo de ROM como bytes na configuração (ADR 0002).

Não se aproveita: a memória com Paginação do `apple2-core`, os Soft switches, o vídeo NTSC (`apple2-video`), o Disk II e as cartas de `apple2-cards`, todos presos ao barramento e ao mapa do Apple II.

Fronteira proposta: um crate novo **`apple1-core`**, dependente de `apple2-cpu` e, se necessário, de uma extração mínima do Scheduler, **sem alterar nem depender de `apple2-core`**. O `Model` do Apple II não ganha uma variante Apple I; isso contaminaria o `Machine` com ramificações de uma arquitetura sem soft switches. O frontend escolheria a Máquina por um trait fino. O GLOSSARY.md só receberia termos do Apple I se a decisão for positiva.

## Esforço estimado

| Componente | Esforço |
|---|---|
| Crate `apple1-core`, mapa de memória e RAM configurável | P |
| Integração de `apple2-cpu` ao novo barramento | P |
| PIA 6820/6821 (teclado, saída, strobe, ocupado) | M |
| Terminal 2513 (24x40, rolagem, cursor, ritmo de 60 cps) e Quadro de texto | M |
| ROM do monitor e política de proveniência | P |
| Cassette ACI (áudio, formato de fita) | M |
| CFFA1 e armazenamento | G |
| Frontend (seleção de Máquina, tela, teclado ASCII) | M |
| Save-state, configuração e documentação | M |
| Suíte de testes e oráculos | G |

## Suíte de testes exigida

- Woz Monitor: digitar comandos (endereço, `R`, bloco `A.B`, escrita `A:XX`) e comparar o eco byte a byte, com a ROM de proveniência conhecida.
- PIA: sequência de strobe, leitura com bit 7 alto, bit de ocupado da saída.
- Temporização do terminal: caracteres consumidos a 60 cps (valor a confirmar), em Ciclos exatos, sem relógio do hospedeiro.
- Cassette: gravar e ler uma fita sintética, com ida e volta de um programa pequeno.
- Diferencial contra o driver do MAME para traces de CPU e texto na tela, com versão e hash da ROM.
- Teste de que `apple2-core` não importa `apple1-core` e que os testes atuais não mudam.

## Requisitos legais de ROM

O Woz Monitor tem copyright da Apple. Sua listagem aparece no manual de 1976, mas isso não é licença para redistribuir. A política atual embute as ROMs copiadas do AppleWin (ADR 0002) e as mantém com proveniência. Para o Apple I:
- Não embutir a ROM sem verificar o respaldo de redistribuição. A situação jurídica é a confirmar com o dono.
- Alternativa conservadora: o usuário fornece a ROM como bytes pela configuração, como já vale para a ROM customizada (ADR 0002). Os testes de CI usariam um monitor próprio, escrito do zero e de comportamento compatível, ou a ROM só localmente.
- Registrar origem, SHA e CRC em `docs/conformance` (os CRCs do MAME servem de ponto de partida: `434f8ce6` e `254bfb95`, a confirmar). Esse diretório ainda não tinha conteúdo de ROMs; a nota `apple1-investigation.md` é a primeira.

## Decisão proposta

Não implementar o Apple I até uma decisão explícita do dono. Condições de entrada para virar escopo:
1. Decisão registrada do dono, com atualização do plano e desta ADR para "Aceita".
2. Política de ROM aprovada (redistribuir, ou usuário fornece).
3. Fonte de oráculo escolhida (MAME e manual) e fixtures com licença adequada.
4. Suíte específica de testes definida, como acima.
5. Fronteira confirmada: `apple1-core` separado, sem regressão no Apple II.
6. Capacidade da equipe depois das ondas anteriores do plano.

## Consequências

- O README e o plano declaram o Apple I fora do escopo, sem promessa de suporte.
- Nenhum código muda; `apple2-core` e `apple2-cpu` ficam intactos.
- Quem quiser Apple I deve usar outro emulador (por exemplo o MAME) por enquanto.
- Se a decisão for positiva, o trabalho começa pela extração do Scheduler e por um crate novo, sem refatorar o núcleo existente.
- O risco legal da ROM fica adiado, não resolvido.
