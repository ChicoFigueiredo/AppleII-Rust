# Proveniência das ROMs

Todas as ROMs do projeto ficam em `roms/` e são embutidas no binário pelo crate `apple2-roms` com `include_bytes!` (ADR 0002). O crate não tem arquivos de ROM próprios: `crates/apple2-roms/src/lib.rs` só aponta para `roms/`. Os 26 arquivos de `roms/` e as 26 constantes do crate correspondem um a um.

A origem de todas é o AppleWin, no commit `a8f0d1d40f551ad9e010522c29c2891e33c88275`, pasta `resource/` (veja [baseline.md](baseline.md)). Em 2026-10-20 comparamos o SHA-256 de cada arquivo com o do mesmo nome naquele commit.

**Resultado: os 26 arquivos são idênticos aos do AppleWin.** Nenhum hash diverge.

## Tabela

Reproduza com: `sha256sum roms/*` e compare com `sha256sum resource/*.rom resource/*.bin resource/*.ROM` do AppleWin em a8f0d1d.

| Arquivo em `roms/` | Tamanho (bytes) | SHA-256 | Origem no AppleWin (a8f0d1d) | Hash bate |
|---|---:|---|---|---|
| `Apple2.rom` | 12288 | `f34e573b9de203203ac4a8c6cab7ab0f974facf13c40bf6e362fbb92197199f9` | `resource/Apple2.rom` | Sim |
| `Apple2_JPlus.rom` | 12288 | `076136209353b8cec4fc7dca955998b28e0202d019b1defbd1c6b8ea2f8e030d` | `resource/Apple2_JPlus.rom` | Sim |
| `Apple2_JPlus_Video.rom` | 2048 | `c5443831794aedd9790037d274f190e2c577a0c0e65c33fe3db46e73da386052` | `resource/Apple2_JPlus_Video.rom` | Sim |
| `Apple2_Plus.rom` | 12288 | `fc3e9d41e9428534a883df5aa10eb55b73ea53d2fcbb3ee4f39bed1b07a82905` | `resource/Apple2_Plus.rom` | Sim |
| `Apple2_Video.rom` | 2048 | `08f5d22230481019844492dde0a29a018cb193712a9e4a43770a3870608f28de` | `resource/Apple2_Video.rom` | Sim |
| `Apple2e.rom` | 16384 | `1fb812584c6633fa16b77b20915986ed1178d1e6fc07a647f7ee8d4e6ab9d40b` | `resource/Apple2e.rom` | Sim |
| `Apple2e_Enhanced.rom` | 16384 | `aab38a03ca8deabbb2f868733148c2efd6f655a59cd9c5d058ef3e0b7aa86a1a` | `resource/Apple2e_Enhanced.rom` | Sim |
| `Apple2e_Enhanced_Video.rom` | 4096 | `52c3b87900ac939f6525402cab1ccfd8f8259290fc6df54da48fb4c98ae3ed0f` | `resource/Apple2e_Enhanced_Video.rom` | Sim |
| `Base64A.rom` | 49152 | `74915101a636d92a1505ba6d61ecd9e6226c03577cd61f5ba194cc9cd559016f` | `resource/Base64A.rom` | Sim |
| `Base64A_German_Video.rom` | 4096 | `c88067160e13150f5217e27e17a2a98900a33e53799c2a3afe8073cd4b8999eb` | `resource/Base64A_German_Video.rom` | Sim |
| `DISK2-13sector.rom` | 256 | `2d2599521fc5763d4e8c308c2ee7c5c4d5c93785b8fb9a4f7d0381dfd5eb60b6` | `resource/DISK2-13sector.rom` | Sim |
| `DISK2.rom` | 256 | `de1e3e035878bab43d0af8fe38f5839c527e9548647036598ee6fe7ec74d2a7d` | `resource/DISK2.rom` | Sim |
| `Freezes_Non-autostart_F8_Rom.rom` | 2048 | `20406b241dae58bb8c830d052f8a9af32acdb6a2e42b84d20999968af1b80b2d` | `resource/Freezes_Non-autostart_F8_Rom.rom` | Sim |
| `HDC-SmartPort.bin` | 256 | `f7d8f6da8f2a7a09c51b7f3d55e624a6b0272055d8b74b0eab552b174579b9c7` | `resource/HDC-SmartPort.bin` | Sim |
| `Hddrvr-v2.bin` | 256 | `0a04ea1a60c6d5400700ea5c274d230921bd7b21882806053ac27f364827d2a7` | `resource/Hddrvr-v2.bin` | Sim |
| `Hddrvr.bin` | 256 | `9e3b9004f734938b9f6ba7cee251ddbbc9514942b5f9be1b99613b7db5a3cf59` | `resource/Hddrvr.bin` | Sim |
| `Mockingboard-D.rom` | 2048 | `54eb41556dd910576c3ac7ffe56b4313685d6aa469b4b2d1d7a44a5f956efa28` | `resource/Mockingboard-D.rom` | Sim |
| `MouseInterface.rom` | 2048 | `7f8e50c0e8a409991264201f9300f109b872b68e09c43f3e6c26af35fd8b89af` | `resource/MouseInterface.rom` | Sim |
| `PRAVETS82.ROM` | 12288 | `2c2432f271869826e098c93bb271f9f7a8c37df2a34e5e8893175e881a6c126e` | `resource/PRAVETS82.ROM` | Sim |
| `PRAVETS8C.ROM` | 16384 | `54ab29eeac5845aac173c5fd424d4177d79e560f8ec0076dcae34a11bd85b501` | `resource/PRAVETS8C.ROM` | Sim |
| `PRAVETS8M.ROM` | 12288 | `ec5af0fa211e2f6c417ceab8752bd23509d067b9dca75433e2bd975b66277120` | `resource/PRAVETS8M.ROM` | Sim |
| `Parallel.rom` | 256 | `e1b91e0156cabc879663f4c3936b3866bda74182fb0506667cd2d01e19d9e471` | `resource/Parallel.rom` | Sim |
| `SSC.rom` | 2048 | `4fa1323ba31f4bd562486838397b7744257c126806766e360876720daca6e2aa` | `resource/SSC.rom` | Sim |
| `TK3000e.rom` | 16384 | `7dbf16b53fd9ce0efc42caa02f0fb4fc14031bb8e228c429269f1df7abbb3492` | `resource/TK3000e.rom` | Sim |
| `TKClock.rom` | 2304 | `0870abb7f55b0127d568383c3c735c35668f353ebe9cbc45d18df50af62a0e51` | `resource/TKClock.rom` | Sim |
| `ThunderClockPlus.rom` | 2048 | `ecca793179427fdd193373080bf23e49a95743bd7b5e0b218c747663938b6c03` | `resource/ThunderClockPlus.rom` | Sim |

Os tamanhos também são verificados pelo teste `embedded_roms_have_expected_sizes` do crate `apple2-roms`.

Esta tabela diz de onde veio cada arquivo e que ele é igual ao do AppleWin. Ela **não** diz que cada ROM é uma cópia fiel do firmware original da Apple ou do fabricante: isso o AppleWin também não garante, e aqui não foi verificado.

## Situação de licença

As ROMs de sistema (Apple ][, ][+, //e, //e Enhanced) e de cartas (Disk II, Super Serial Card, Mockingboard, Thunderclock e outras) são firmware da Apple e de terceiros. Os clones (Pravets, Base64A, TK3000 //e) e as cartas de outros fabricantes pertencem aos seus respectivos donos. O AppleWin as distribui junto com o seu código-fonte, e este projeto as mantém no repositório pelo mesmo motivo e da mesma forma, como decidido na [ADR 0004](../adr/0004-oraculo-de-conformidade.md).

O projeto não tem licença própria sobre essas ROMs: a licença GPL-2.0-or-later do repositório vale para o código, não para o firmware. Quem redistribuir o AppleII-Rust assume a mesma situação que assume ao redistribuir o AppleWin. Se um titular de direitos pedir a retirada de uma ROM, ela deve sair do repositório e do binário, e o Modelo ou a Carta correspondente passa a exigir a ROM do usuário.

## ROMs do usuário (planejado)

**Esta opção ainda não existe.** Outra frente de trabalho a implementa; a descrição abaixo é o procedimento planejado e pode mudar.

- O usuário informa o caminho de um arquivo de ROM na configuração (por Modelo ou por Carta). O frontend lê o arquivo e entrega os bytes ao núcleo, que nunca abre arquivos (ADR 0001 e ADR 0002).
- O tamanho do arquivo é conferido contra o do AppleWin para aquele Modelo (coluna "Tamanho" acima). Tamanho errado é erro explícito, não ROM truncada.
- O SHA-256 do arquivo informado é calculado e registrado no log e no Manifesto de execução. Se não coincidir com o da tabela, o programa avisa que a ROM não é a do AppleWin, mas aceita, porque há versões legítimas diferentes.
- Resultados de conformidade só valem para a ROM do AppleWin: um Manifesto com outro `rom_sha256` não conta como PASS do baseline.

## Como trocar uma ROM do repositório

1. Substitua o arquivo em `roms/`, mantendo o nome (as constantes de `apple2-roms` apontam para ele) e o tamanho esperado.
2. Calcule o SHA-256 (`sha256sum roms/ARQUIVO`) e atualize a linha desta tabela, inclusive a coluna "Origem". Se a nova ROM não vem do AppleWin, escreva de onde vem e com que licença; "Hash bate" passa a ser "Não".
3. Atualize o teste de tamanho em `crates/apple2-roms/src/lib.rs` se o tamanho mudou.
4. Reexecute os testes de boot e de conformidade dos Modelos e Cartas afetados. Os Traces de referência guardam o `rom_sha256` e deixam de valer para a ROM nova.
5. Registre a mudança no CHANGELOG e, se for uma Divergência intencional, em [gaps-and-known-limitations.md](gaps-and-known-limitations.md).
