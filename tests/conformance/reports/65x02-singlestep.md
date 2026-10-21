# 65x02: SingleStepTests/65x02 (v1)

Vetores: github.com/SingleStepTests/65x02 (MIT, Thomas Harte et al.), SHA fixo em `tools/conformance/fetch-65x02-tests.sh`. Sao medidas de hardware real, nao do AppleWin.
Teste: `cargo test -p apple2-cpu --release --test singlestep_65x02 -- --ignored --nocapture` (`#[ignore]`, 2,5 GB de JSON).

Compara, por vetor: PC, A, X, Y, SP, P (bits 4 e 5 mascarados: o AppleWin os mantem fixos, `EF_TO_AF`), RAM final, numero de ciclos e o acesso ao barramento de cada ciclo (endereco, valor, leitura/escrita).

Classes de resultado de cada vetor:

- **Estrito**: tudo igual ao hardware.
- **D0, ciclo fantasma**: estado, RAM e ciclos iguais; o port nao faz o acesso espurio (leitura/escrita falsa) que o hardware faz em alguns ciclos, e todo acesso que faz esta certo. O AppleWin tambem nao os faz (`_READ`/`_WRITE` em `CPU/cpu_general.inl:70-78` sao acessos unicos; cruzar pagina so soma `uExtraCycles`, `cpu_general.inl:146-155`), entao e **diferenca AppleWin x hardware**, nao corrigida.
- **D1 a D10**: demais diferencas AppleWin x hardware (tabela abaixo), tambem esperado-divergentes.
- **Inesperado**: qualquer outro desvio. O teste falha se houver algum.

Consequencia pratica de D0: leituras espurias em `$C0xx` tem efeito colateral no hardware (AppleWin GH#967, aberto); o port as omite como o AppleWin.

## Resumo por CPU

| CPU | Vetores | Estrito | D0 fantasma | D1 a D10 | Inesperado | Aprovados (estrito + esperado) |
|---|---:|---:|---:|---:|---:|---:|
| 6502 | 2560000 | 714617 | 1719564 | 125819 | 0 | 2560000 |
| rockwell65c02 | 2560000 | 935602 | 1254355 | 370043 | 0 | 2560000 |
| synertek65c02 | 2560000 | 935215 | 1254741 | 370044 | 0 | 2560000 |
| wdc65c02 | 2540000 | 935723 | 1254241 | 350036 | 0 | 2540000 |

## Diferencas AppleWin x hardware

| Id | CPUs | Opcodes | Diferenca | C++ (linha) | Vetores |
|---|---|---|---|---|---:|
| D0 | todas | todos | Sem acesso nos ciclos espurios (leitura/escrita falsa) do hardware | `cpu_general.inl:70-78,146-155` | 5482901 |
| D1 | 6502 | 02 12 22 32 42 52 62 72 92 B2 D2 F2 | HLT/KIL: o AppleWin trava com PC sobre o opcode (bJammed, 2 ciclos); o hardware para com PC no byte seguinte | `CPU/cpu_instructions.inl:379-380 (`HLT`: bJammed = 1; --regs.pc), CPU/cpu6502.h:122` | 120000 |
| D2 | 6502 | 8B | XAA/ANE ($8B): o AppleWin faz A = X & M; o hardware instavel faz A = (A | constante) & X & M | `CPU/cpu_instructions.inl:680-683 (`XAA`)` | 2379 |
| D3 | 6502 | 6B | ARR ($6B) com D=1: o ajuste BCD do AppleWin difere do hardware (a versao binaria passa) | `CPU/cpu_instructions.inl:246-264 (`ARR`, ramo `AF_DECIMAL`)` | 3158 |
| D4 | 6502 | E3 E7 EF F3 F7 FB FF | INS/ISC com o operando $FF: o AppleWin subtrai o WORD $100 (val = READ+1 sem truncar), o hardware subtrai $00 (o carry sai diferente) | `CPU/cpu_instructions.inl:387-393 (`INS`: `temp = val` com val WORD)` | 282 |
| D5 | wdc65c02, rockwell65c02, synertek65c02 | 07 17 27 37 47 57 67 77 87 97 A7 B7 C7 D7 E7 F7 | RMBn/SMBn ($x7): o AppleWin os trata como NOP de 1 byte e 1 ciclo; o hardware executa um RMB/SMB de 2 bytes e 3 a 5 ciclos (os vetores da Synertek tambem) | `CPU/cpu65C02.h:127,143,... (`case 0x07: NOP CYC(1)`; idem $17..$F7)` | 480000 |
| D6 | wdc65c02, rockwell65c02, synertek65c02 | 0F 1F 2F 3F 4F 5F 6F 7F 8F 9F AF BF CF DF EF FF | BBRn/BBSn ($xF): o AppleWin os trata como NOP de 1 byte e 1 ciclo; o hardware executa um desvio de 3 bytes | `CPU/cpu65C02.h:135,151,... (`case 0x0F: NOP CYC(1)`; idem $1F..$FF)` | 480000 |
| D7 | wdc65c02, rockwell65c02, synertek65c02 | 5C | $5C (NOP absoluto): 8 ciclos no AppleWin (tabela do WDC), 4 nos vetores | `CPU/cpu65C02.h:212 (`case 0x5C: ABS NOP CYC(8)`)` | 30000 |
| D8 | rockwell65c02, synertek65c02 | CB DB | WAI ($CB) e STP ($DB): o AppleWin os trata como NOP de 1 ciclo; o hardware espera/para (os vetores do WDC omitem os dois) | `CPU/cpu65C02.h:323,339 (`case 0xCB/0xDB: NOP CYC(1)`)` | 40000 |
| D9 | wdc65c02, rockwell65c02, synertek65c02 | DC FC | $DC/$FC (NOP absoluto de 4 ciclos): o AppleWin le o endereco do operando (`LDD`); o hardware le outro endereco no 4o ciclo. Estado e ciclos iguais | `CPU/cpu65C02.h:340,372 (`ABS LDD CYC(4)`), CPU/cpu_instructions.inl:439-440` | 60000 |
| D10 | wdc65c02, rockwell65c02, synertek65c02 | 6C | JMP (abs) com o ponteiro em $xxFF: o AppleWin acrescenta 1 ciclo (7), o hardware gasta 6 | `CPU/cpu_general.inl:200-208 (`_IABS_CMOS`: uExtraCycles=1 se (base & $FF) == $FF), CPU/cpu65C02.h:228` | 123 |

## Por opcode

Cada CPU lista os opcodes sem nenhuma diferenca, os que so tem D0 e, em tabela, os que tem D1 a D10 ou algo inesperado.

### 6502

- Opcodes 100% estritos (53): 05 09 0B 0D 24 25 29 2B 2C 2D 45 49 4B 4C 4D 65 69 6C 6D 84 85 86 87 8C 8D 8E 8F A0 A2 A4 A5 A6 A7 A9 AB AC AD AE AF C0 C4 C5 C9 CB CC CD E0 E4 E5 E9 EB EC ED
- Opcodes so com D0 (182): 00 01 03 04 06 07 08 0A 0C 0E 0F 10 11 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 23 26 27 28 2A 2E 2F 30 31 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F 40 41 43 44 46 47 48 4A 4E 4F 50 51 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F 60 61 63 64 66 67 68 6A 6E 6F 70 71 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F 80 81 82 83 88 89 8A 90 91 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F A1 A3 A8 AA B0 B1 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF C1 C2 C3 C6 C7 C8 CA CE CF D0 D1 D3 D4 D5 D6 D7 D8 D9 DA DB DC DD DE DF E1 E2 E6 E8 EA EE F0 F1 F4 F5 F6 F8 F9 FA FC FD FE

| Op | Vetores | Estrito | D0 | Divergencia (vetores) | Inesperado | Primeiro inesperado |
|---|---:|---:|---:|---|---:|---|
| 02 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 12 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 22 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 32 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 42 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 52 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 62 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 6B | 10000 | 6842 | 0 | D3 (3158) | 0 |  |
| 72 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| 8B | 10000 | 7621 | 0 | D2 (2379) | 0 |  |
| 92 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| B2 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| D2 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| E3 | 10000 | 0 | 9963 | D4 (37) | 0 |  |
| E7 | 10000 | 0 | 9949 | D4 (51) | 0 |  |
| EF | 10000 | 0 | 9963 | D4 (37) | 0 |  |
| F2 | 10000 | 0 | 0 | D1 (10000) | 0 |  |
| F3 | 10000 | 0 | 9968 | D4 (32) | 0 |  |
| F7 | 10000 | 0 | 9954 | D4 (46) | 0 |  |
| FB | 10000 | 0 | 9956 | D4 (44) | 0 |  |
| FF | 10000 | 0 | 9965 | D4 (35) | 0 |  |

### rockwell65c02

- Opcodes 100% estritos (75): 03 05 09 0B 0D 12 13 1B 23 24 25 29 2B 2C 2D 32 33 3B 43 45 49 4B 4C 4D 52 53 5B 63 64 6B 73 7B 83 84 85 86 89 8B 8C 8D 8E 92 93 9B 9C A0 A2 A3 A4 A5 A6 A9 AB AC AD AE B2 B3 BB C0 C3 C4 C5 C9 CC CD D2 D3 E0 E3 E4 EB EC F3 FB
- Opcodes so com D0 (143): 00 01 02 04 06 08 0A 0C 0E 10 11 14 15 16 18 19 1A 1C 1D 1E 20 21 22 26 28 2A 2E 30 31 34 35 36 38 39 3A 3C 3D 3E 40 41 42 44 46 48 4A 4E 50 51 54 55 56 58 59 5A 5D 5E 60 61 62 65 66 68 69 6A 6D 6E 70 71 72 74 75 76 78 79 7A 7C 7D 7E 80 81 82 88 8A 90 91 94 95 96 98 99 9A 9D 9E A1 A8 AA B0 B1 B4 B5 B6 B8 B9 BA BC BD BE C1 C2 C6 C8 CA CE D0 D1 D4 D5 D6 D8 D9 DA DD DE E1 E2 E5 E6 E8 E9 EA ED EE F0 F1 F2 F4 F5 F6 F8 F9 FA FD FE

| Op | Vetores | Estrito | D0 | Divergencia (vetores) | Inesperado | Primeiro inesperado |
|---|---:|---:|---:|---|---:|---|
| 07 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 0F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 17 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 1F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 27 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 2F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 37 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 3F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 47 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 4F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 57 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 5C | 10000 | 0 | 0 | D7 (10000) | 0 |  |
| 5F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 67 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 6C | 10000 | 0 | 9957 | D10 (43) | 0 |  |
| 6F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 77 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 7F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 87 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 8F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 97 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 9F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| A7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| AF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| B7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| BF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| C7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| CB | 10000 | 0 | 0 | D8 (10000) | 0 |  |
| CF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| D7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| DB | 10000 | 0 | 0 | D8 (10000) | 0 |  |
| DC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| DF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| E7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| EF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| F7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| FC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| FF | 10000 | 0 | 0 | D6 (10000) | 0 |  |

### synertek65c02

- Opcodes 100% estritos (75): 03 05 09 0B 0D 12 13 1B 23 24 25 29 2B 2C 2D 32 33 3B 43 45 49 4B 4C 4D 52 53 5B 63 64 6B 73 7B 83 84 85 86 89 8B 8C 8D 8E 92 93 9B 9C A0 A2 A3 A4 A5 A6 A9 AB AC AD AE B2 B3 BB C0 C3 C4 C5 C9 CC CD D2 D3 E0 E3 E4 EB EC F3 FB
- Opcodes so com D0 (143): 00 01 02 04 06 08 0A 0C 0E 10 11 14 15 16 18 19 1A 1C 1D 1E 20 21 22 26 28 2A 2E 30 31 34 35 36 38 39 3A 3C 3D 3E 40 41 42 44 46 48 4A 4E 50 51 54 55 56 58 59 5A 5D 5E 60 61 62 65 66 68 69 6A 6D 6E 70 71 72 74 75 76 78 79 7A 7C 7D 7E 80 81 82 88 8A 90 91 94 95 96 98 99 9A 9D 9E A1 A8 AA B0 B1 B4 B5 B6 B8 B9 BA BC BD BE C1 C2 C6 C8 CA CE D0 D1 D4 D5 D6 D8 D9 DA DD DE E1 E2 E5 E6 E8 E9 EA ED EE F0 F1 F2 F4 F5 F6 F8 F9 FA FD FE

| Op | Vetores | Estrito | D0 | Divergencia (vetores) | Inesperado | Primeiro inesperado |
|---|---:|---:|---:|---|---:|---|
| 07 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 0F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 17 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 1F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 27 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 2F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 37 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 3F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 47 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 4F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 57 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 5C | 10000 | 0 | 0 | D7 (10000) | 0 |  |
| 5F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 67 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 6C | 10000 | 0 | 9956 | D10 (44) | 0 |  |
| 6F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 77 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 7F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 87 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 8F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 97 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 9F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| A7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| AF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| B7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| BF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| C7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| CB | 10000 | 0 | 0 | D8 (10000) | 0 |  |
| CF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| D7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| DB | 10000 | 0 | 0 | D8 (10000) | 0 |  |
| DC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| DF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| E7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| EF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| F7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| FC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| FF | 10000 | 0 | 0 | D6 (10000) | 0 |  |

### wdc65c02

- Opcodes 100% estritos (75): 03 05 09 0B 0D 12 13 1B 23 24 25 29 2B 2C 2D 32 33 3B 43 45 49 4B 4C 4D 52 53 5B 63 64 6B 73 7B 83 84 85 86 89 8B 8C 8D 8E 92 93 9B 9C A0 A2 A3 A4 A5 A6 A9 AB AC AD AE B2 B3 BB C0 C3 C4 C5 C9 CC CD D2 D3 E0 E3 E4 EB EC F3 FB
- Opcodes so com D0 (143): 00 01 02 04 06 08 0A 0C 0E 10 11 14 15 16 18 19 1A 1C 1D 1E 20 21 22 26 28 2A 2E 30 31 34 35 36 38 39 3A 3C 3D 3E 40 41 42 44 46 48 4A 4E 50 51 54 55 56 58 59 5A 5D 5E 60 61 62 65 66 68 69 6A 6D 6E 70 71 72 74 75 76 78 79 7A 7C 7D 7E 80 81 82 88 8A 90 91 94 95 96 98 99 9A 9D 9E A1 A8 AA B0 B1 B4 B5 B6 B8 B9 BA BC BD BE C1 C2 C6 C8 CA CE D0 D1 D4 D5 D6 D8 D9 DA DD DE E1 E2 E5 E6 E8 E9 EA ED EE F0 F1 F2 F4 F5 F6 F8 F9 FA FD FE

| Op | Vetores | Estrito | D0 | Divergencia (vetores) | Inesperado | Primeiro inesperado |
|---|---:|---:|---:|---|---:|---|
| 07 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 0F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 17 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 1F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 27 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 2F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 37 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 3F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 47 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 4F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 57 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 5C | 10000 | 0 | 0 | D7 (10000) | 0 |  |
| 5F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 67 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 6C | 10000 | 0 | 9964 | D10 (36) | 0 |  |
| 6F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 77 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 7F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 87 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 8F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| 97 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| 9F | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| A7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| AF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| B7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| BF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| C7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| CF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| D7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| DC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| DF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| E7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| EF | 10000 | 0 | 0 | D6 (10000) | 0 |  |
| F7 | 10000 | 0 | 0 | D5 (10000) | 0 |  |
| FC | 10000 | 0 | 0 | D9 (10000) | 0 |  |
| FF | 10000 | 0 | 0 | D6 (10000) | 0 |  |

