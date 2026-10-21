# Versões do `.aws.yaml` que o porte lê (migração de esquema)

Fonte: AppleWin `a8f0d1d` (SaveState.cpp, `kUNIT_VERSION` de cada carta) e o corpus AppleWin-Test
(112 arquivos `.aws.yaml` + 3 gerados no Wine). Cobertura medida por `tools/conformance/savestate-compare.py`
(100.000 ciclos contra o oráculo); o relatório por arquivo está em
`tests/conformance/reports/savestate-import.md`.

## Cabeçalho do arquivo

| item | AppleWin | porte |
|---|---|---|
| `File_hdr` `Version` (`SS_FILE_VER`) | 2 | lê somente 2; qualquer outro valor é recusado com `Version mismatch` |
| formato binário antigo (`.aws`, versão 1 do arquivo, pasta `v1/` do corpus) | lido pelo AppleWin | **não lido** (só YAML); o arquivo do corpus em `v1/` não entra no índice |
| `Tag` | `AppleWin Save State` | idêntico, senão `File_hdr: Bad tag` |
| unidade desconhecida | ignorada pelo AppleWin | ignorada com aviso |
| `AppleII-Rust Extension` (dentro de `Unit: Apple2`) | ignorada (chave desconhecida, logada) | `Version: 1`, `Format: json+zlib`; outra versão ou formato volta ao caminho só-AppleWin; `Length` limitado a 64 MiB e a descompressão a 256 MiB (AUD-C-010) |

## Versões de unidade

| unidade | versão atual do AppleWin | o porte lê | notas de migração implementadas |
|---|---|---|---|
| `Apple2` | 9 | 1 a 9 | v1: `SLOTCXROM` invertido em relação a `INTCXROM`; ][/][+ guardam o LC como os 16K altos da memória principal. v2: `Key Waiting`. v3: `AnnunciatorN`. v4: `Video Refresh Rate`. v5: `Defer IRQ By 1 Opcode`. v7: `Paddle Inactive Cycle`. v9: `Last Slot to Set Main Mem LC`, `MMU LC Mode` |
| `Auxiliary Slot` | 2 | 1 a 2 | v1 tem os campos da carta direto em `State`; v2 aninha `Card`/`Version`/`State` |
| `Slots` | 1 | 1 | |
| `Game I/O Connector` | 4 | 1 a 4 | v4 acrescenta `Shift Key Mod` (o fork usado como oráculo só chega a v3: `state-compat.py` remove a unidade para ele) |
| `Miscellaneous` | 1 | 1 | só com No-Slot Clock |
| `Disk II` | 9 | 1 a 9 | |
| `Hard Disk Controller` / `Generic HDD` | 6 | 1 a 6 | v1-3 rodam o firmware `Hddrvr.bin`, v4 `Hddrvr-v2.bin`, v5+ trazem o firmware no arquivo; v<5 aplica `FixupUnitNum`; v<=2 numera `Unit0` |
| `Super Serial Card` | 2 | 1 a 2 | |
| `Mockingboard C`, `Phasor`, `MEGA Audio`, `SD Music` | 15 | 1 a 15 | v<7: latch do T1 igual a 0 vira 0xFFFF; v<=7: lê e descarta `Timer1/2/Speech IRQ` |
| `Language Card`, `Saturn 128` | 1 | 1 | |
| `Mouse Card`, `4Play`, `SNES MAX`, `VidHD`, `Breakpoint`, `SAM`, `Z80 SoftCard`, `Generic Printer`, `Uthernet`, `Uthernet II` | 1 (Uthernet II: 2) | 1 (Uthernet II: 1 a 2) | |

Qualquer versão acima da atual dá `Version N is not supported for card ...` (teste
`audit_robustness.rs::schema_version_unknown_gives_descriptive_error`); versão 0 também é recusada.

## Cobertura com o corpus

Colunas: arquivos que contêm a unidade ou a carta naquela versão, e o resultado do trace contra o
oráculo (PASS, FAIL, BLOCKED) desses arquivos. Um FAIL aqui tem causa registrada no relatório (nem
sempre é da versão da unidade: um arquivo tem várias unidades).

| unidade ou carta | versão | arquivos | PASS | FAIL | BLOCKED |
|---|---|---|---|---|---|
| Apple2 | unit v1 | 45 | 26 | 9 | 10 |
| Apple2 | unit v2 | 18 | 16 | 2 | 0 |
| Apple2 | unit v3 | 2 | 1 | 1 | 0 |
| Apple2 | unit v4 | 4 | 2 | 2 | 0 |
| Apple2 | unit v5 | 12 | 2 | 2 | 8 |
| Apple2 | unit v6 | 8 | 5 | 2 | 1 |
| Apple2 | unit v7 | 6 | 6 | 0 | 0 |
| Apple2 | unit v8 | 2 | 2 | 0 | 0 |
| Apple2 | unit v9 | 15 | 12 | 3 | 0 |
| Auxiliary Slot | unit v1 | 45 | 28 | 8 | 9 |
| Auxiliary Slot | unit v2 | 39 | 23 | 9 | 7 |
| Game I/O Connector | unit v3 | 6 | 6 | 0 | 0 |
| Game I/O Connector | unit v4 | 2 | 0 | 2 | 0 |
| Miscellaneous | unit v1 | 31 | 25 | 5 | 1 |
| card 4Play | v1 | 1 | 1 | 0 | 0 |
| card Breakpoint | v1 | 1 | 1 | 0 | 0 |
| card Disk II | v9 | 6 | 6 | 0 | 0 |
| card Disk][ | v1 | 42 | 25 | 8 | 9 |
| card Disk][ | v2 | 3 | 1 | 1 | 1 |
| card Disk][ | v3 | 19 | 16 | 3 | 0 |
| card Disk][ | v4 | 1 | 0 | 1 | 0 |
| card Disk][ | v5 | 18 | 6 | 4 | 8 |
| card Disk][ | v6 | 8 | 6 | 1 | 1 |
| card Disk][ | v7 | 1 | 1 | 0 | 0 |
| card Disk][ | v8 | 3 | 3 | 0 | 0 |
| card Disk][ | v9 | 7 | 6 | 1 | 0 |
| card Generic HDD | v1 | 1 | 1 | 0 | 0 |
| card Generic HDD | v2 | 1 | 1 | 0 | 0 |
| card Generic HDD | v3 | 1 | 1 | 0 | 0 |
| card Generic HDD | v6 | 2 | 1 | 1 | 0 |
| card Generic Printer | v1 | 102 | 64 | 19 | 19 |
| card Language Card | v1 | 13 | 10 | 1 | 2 |
| card Mockingboard C | v1 | 6 | 0 | 4 | 2 |
| card Mockingboard C | v10 | 2 | 2 | 0 | 0 |
| card Mockingboard C | v13 | 3 | 3 | 0 | 0 |
| card Mockingboard C | v2 | 2 | 2 | 0 | 0 |
| card Mockingboard C | v3 | 6 | 6 | 0 | 0 |
| card Mockingboard C | v4 | 4 | 2 | 2 | 0 |
| card Mockingboard C | v5 | 2 | 0 | 2 | 0 |
| card Mouse Card | v1 | 9 | 6 | 3 | 0 |
| card Phasor | v1 | 3 | 2 | 1 | 0 |
| card Phasor | v10 | 1 | 1 | 0 | 0 |
| card Phasor | v13 | 3 | 3 | 0 | 0 |
| card Phasor | v3 | 1 | 1 | 0 | 0 |
| card Phasor | v5 | 1 | 1 | 0 | 0 |
| card Phasor | v6 | 1 | 0 | 1 | 0 |
| card Phasor | v9 | 1 | 1 | 0 | 0 |
| card SAM | v1 | 1 | 1 | 0 | 0 |
| card SNES MAX | v1 | 1 | 0 | 1 | 0 |
| card Saturn 128 | v1 | 14 | 12 | 2 | 0 |
| card Super Serial Card | v1 | 43 | 25 | 8 | 10 |
| card Super Serial Card | v2 | 56 | 39 | 11 | 6 |
| card Uthernet | v1 | 1 | 0 | 0 | 1 |
| card VidHD | v1 | 2 | 2 | 0 | 0 |
| card Z80 | v1 | 3 | 2 | 1 | 0 |

Lacunas: o corpus não traz `Disk II`/`Hard Disk Controller` v7-8 nem unidades futuras; o formato
binário antigo (`.aws`) não é suportado.
