# Diferencial de traces de CPU: Rust x AppleWin (HU-0101, HU-0103)

Registro de cada divergência entre o trace do AppleII-Rust (`conformance trace`) e o do oráculo (`audetto-oracle`, fork audetto/AppleWin `a1e20c0`, upstream `3e8054b`), com a causa raiz, a correção e o teste de regressão. O formato e as convenções estão em `docs/conformance/test-oracles.md`.

## Resultado

Todos os traces batem em todos os campos (`cycle,pc,op,a,x,y,sp,p`), sem `--ignore`:

| Modelo | Ciclos | Instruções | Caso |
|---|---|---|---|
| Apple ][+ (LC no slot 0, Disk II no slot 6, sem disco) | 500.000 | 196.113 | `trace-applewin-iiplus-boot-500k` |
| Apple //e Enhanced (Disk II, sem disco) | 500.000 | 195.973 | `trace-applewin-iie-enhanced-boot-500k` |
| Apple ][+ | 2.000.000 | 776.083 | `trace-applewin-iiplus-boot-2m` |
| Apple //e Enhanced | 2.000.000 | 776.044 | `trace-applewin-iie-enhanced-boot-2m` |
| Apple ][ original | 2.000.000 | 547.232 | `trace-applewin-ii-boot-2m` |
| Apple //e (não Enhanced, 6502) | 2.000.000 | 776.050 | `trace-applewin-iie-boot-2m` |
| Apple //e Enhanced, boot do DOS 3.3 System Master | 5.000.000 | 1.821.912 | `trace-applewin-iie-enhanced-dos33-5m` |

Nenhuma divergência ficou BLOCKED. Nenhuma das seis divergências abaixo era um erro da CPU 6502/65C02 do núcleo; as de origem Rust são de configuração do harness, de convenção de representação e do Disk II.

## Divergências

A ordem é a da investigação. "Primeira linha" é o ponto em que o `diff` parou antes da correção.

### D1. P do trace: bits 4 e 5

- **Sintoma:** linha 9 (instrução 1): `p` esperado `34`, obtido `24`. Com `--ignore p` o problema desaparecia, mas escondia divergências reais de flags.
- **Causa:** o AppleWin guarda `regs.ps` com B (bit 4) e o bit reservado (bit 5) sempre em 1 (`EF_TO_AF`); o registrador do núcleo Rust deixa esses bits como estão no 6502 real (`24` após o reset).
- **Correção:** convenção do Trace v1: `p` é gravado com `|0x30` (`trace::P_FIXED_BITS`). O registrador não muda.
- **Teste:** `crates/apple2-conformance/tests/oracle_conventions.rs::p_is_recorded_with_bits_4_and_5_set`.

### D2. Estado inicial da RAM

- **Sintoma:** o núcleo zerava a RAM; o AppleWin usa `MIP_FF_FF_00_00` (`FF FF 00 00` a cada 4 bytes de `$0000-$BFFF`, mais bytes sorteados em `xx28/xx29/xx68/xx69` e `$4E/$4F`). Não gerou uma divergência visível nos primeiros 500 mil ciclos (o boot só lê RAM depois de escrever), mas qualquer leitura de RAM não inicializada, e o floating bus em modo texto, depende disso.
- **Causa:** o núcleo não implementava `g_nMemoryClearType` (`Memory.cpp::MemReset`, `Core.cpp:79`).
- **Correção:** `apple2_core::ram_init` (`RamInit`, `RandomSource::{Zero, Seed}`, `MIP_*` 0 a 7 e -1); `MachineConfig::ram_init`; `Memory::power_cycle` reaplica o padrão. `RamInit::APPLEWIN_DETERMINISTIC` = padrão do AppleWin com sorteio 0 e `$4E=$4F=$20`, idêntico ao oráculo. A opção `-memclear` já existia no `apple2-config` (clamp 0 a 7, como CmdLine.cpp:456); agora o padrão é 2 (era 0) e `Settings` ganhou `memory_seed` (o app põe a semente do relógio ao montar a máquina, o núcleo continua sem relógio). `conformance trace` ganhou `--memclear N`, `--deterministic-ram` e `--ram-seed N`. `MachineConfig::for_model` continua com RAM zerada para não mudar os testes de unidade existentes.
- **Teste:** `crates/apple2-core/src/ram_init.rs` (padrões, sorteio com semente), `crates/apple2-core/tests/ram_init.rs` (máquina inteira, power cycle, aux zerada), `crates/apple2-config/src/machine.rs::memclear_and_seed_reach_the_core_config`.

### D3. Entrada em interrupção gerava linha

- **Sintoma:** não apareceu nos traces de boot (sem IRQ/NMI), mas a convenção diferia: no oráculo o gancho roda depois do Fetch, então IRQ/NMI tomadas não geram linha; no Rust o passo que entra na interrupção gerava uma linha com o PC interrompido.
- **Causa:** `Machine::step` faz a entrada na interrupção como um passo próprio.
- **Correção:** `Machine::next_step_is_interrupt_entry()` (consulta pura, reproduz a decisão de `step` incluindo o adiamento de um opcode do IRQ e a borda do NMI) e `TraceWriter::step` não grava linha nesse passo.
- **Teste:** `oracle_conventions.rs::interrupt_entry_writes_no_row`, `::nmi_entry_writes_no_row_and_masked_irq_is_not_an_entry`.

### D4. Floating bus: relógio de vídeo parado no oráculo (origem: oráculo, não o Rust)

- **Sintoma:** com os itens acima, linha 28 (instrução 10, ciclo 34): `LDA $C056` (soft switch sem dono) devolvia `00` no oráculo e `FF` no Rust (`a` esperado `00`, obtido `FF`).
- **Causa:** `main.cpp` do oráculo chamava `CpuExecute(slice, false)`. Com `bVideoUpdate = false` o `g_nVideoClockHorz/Vert` do NTSC nunca avança (fica em 0,0, e `NTSC_VideoGetScannerAddress` devolve sempre o endereço de (vert 261, horz 64)), então todo floating bus lia o mesmo byte. O AppleWin real atualiza o relógio a cada opcode (`cpu6502.h:382`). Com a RAM zerada isso passava despercebido.
- **Correção:** no oráculo, `CpuExecute(slice, true)` (só `main.cpp`; o patch do fork e o código do AppleWin não mudaram). Os traces de referência de 500 mil ciclos foram regenerados. Detalhes em `tools/conformance/oracle/README.md`. **Não** se mexeu no Rust.
- **Teste:** os casos `trace-applewin-*` (qualquer leitura de `$C0xx` sem dono já diverge sem a correção); o Rust tem `crates/apple2-core/tests/video_scanner.rs::floating_bus_reads_the_text_byte_at_the_scanner_address`.

### D5. Disk II ausente no Rust; opcode lido em ROM de cartão

- **Sintoma (a, a medição do supervisor):** linha 49.675/49.684 (instrução ~49.660, ciclo 132.313): depois de `FAC7 B1` (`LDA (zp),Y`) o AppleWin tem o byte da ROM do slot 6 e o Rust tinha o da RAM/floating bus (`A0`/`00`). Não era a RAM nem o floating bus: o oráculo roda com o Disk II no slot 6 (`#cards=disk2`) e o `conformance trace` não instalava cartão. A varredura de slots do monitor (`$FABA`) lê `$Cn07` e compara com a assinatura do Disk II.
- **Causa:** o harness só criava a máquina do núcleo, sem cartões.
- **Correção:** `conformance trace --cards none|disk2` e `--disk IMAGEM`; cartões `"6:disk2"` e `fixture` (imagem no drive 1, aberta protegida contra escrita) nos casos TOML (`runner::build_machine`, `Disk2Setup`).
- **Sintoma (b):** linha 51.926 (ciclo 132.414): `op` esperado `A2`, obtido `00` em `C600`.
- **Causa (b):** `TraceRow::from_machine` obtém o opcode por `Machine::peek`, que não lê ROM de cartão sem efeito colateral.
- **Correção (b):** `TraceWriter::step` usa o opcode realmente buscado (`cpu.opcode`) para `pc` em `$C100-$CFFF`.
- **Teste:** `oracle_conventions.rs::opcode_in_slot_rom_comes_from_the_fetch_not_from_a_peek`; os casos `trace-applewin-*` (todos usam `6:disk2`).

### D6. Disk II: latch de drive vazio com `rand()` diferente

- **Sintoma:** linha 51.942 (ciclo 137.834): `LDA $C0EC,X` (latch de dados do Disk II com drive vazio) devolvia `67` no oráculo e `3A` no Rust.
- **Causa:** `Disk.cpp::UpdateLatchForEmptyDrive` faz `m_floppyLatch = rand() & 0xFF` (GH#748); o Rust usava um xorshift próprio no lugar do `rand()`. O oráculo roda na glibc com `srand(1)`.
- **Correção:** `Rng` do `Disk2Card` agora é o `rand()` da glibc (gerador aditivo TYPE_3) semeado com 1, e `RAND_THRESHOLD` não-Windows (`(RAND_MAX/den)*num`, `DiskImage.h:35`). O estado entra no save-state do cartão (versão 2: 31 palavras + 2 índices); a versão 1 ainda é lida e reinicia a sequência. Na Windows o `rand()` tem `RAND_MAX` 32767; o Disk II do Rust segue a glibc, igual ao oráculo.
- **Teste:** `crates/apple2-storage/src/disk2.rs` módulo `rng_tests` (`rng_is_glibc_rand_after_srand_1`: 1804289383, 846930886, ...; `chance_uses_the_non_windows_rand_threshold`; `rng_state_survives_a_save_state_and_version_1_restarts_the_sequence`).

### D7. Atraso de motor desligado do Disk II nunca expirava no trace

- **Sintoma:** com o DOS 3.3: linha 1.561.084 (ciclo 4.338.588): `LDA $C08C,X` logo após `$C08D` devolvia `AA` no oráculo (latch sem mudança) e `EB` no Rust.
- **Causa:** o Disk II desliga `m_spinning` 1 s (≈1.020.484 ciclos) depois do motor desligado, contando no `Card::Update(ciclos)` que o AppleWin chama ao fim de cada lote de 1 ms (`CpuExecute(1020)`). O `TraceWriter` chamava só `Machine::step`, nunca atualizava os cartões, então o drive "girava" para sempre. O `$BD34` do DOS 3.3 ("lê com atraso para ver se o disco está girando") revelou.
- **Correção:** `Machine::update_cards` (API pública, já usada por `run_cycles`/`run_frame`) e `TraceWriter` a chama a cada 1020 ciclos (`trace::BATCH_CYCLES`), como o oráculo.
- **Teste:** `oracle_conventions.rs::cards_are_updated_at_the_end_of_each_1ms_batch_like_the_oracle`; caso `trace-applewin-iie-enhanced-dos33-5m`.

### D8. IRQ adiada um opcode depois do CLI (GH#1176)

- **Sintoma:** com o Mockingboard no slot 4, um IRQ pendente do 6522 #0 e o T1 do 6522 #1 em modo contínuo com latch 0 (evento a cada 2 ciclos, IER nunca escrito), o AppleWin toma o IRQ logo depois do CLI (oráculo: CLI em 128, tratador a partir de 137). O Rust executava mais um opcode (`LDX #1` em 130) e só entrava em 139. Primeira divergência do Trace v1: linha 44 (`137,FA40` contra `130,033F`). Afeta a temporização de interrupção em geral, não só o áudio.
- **Causa:** a lógica da CPU e do `Scheduler` estava correta (`CPU.cpp` `IRQ()`, `SetIrqOnLastOpcodeCycle`, `SynchronousEventManager::Update`). Com o oráculo instrumentado (fprintf em `Update` e em `IRQ()`), os ciclos de todos os eventos são idênticos aos do Rust, inclusive o que expira em 130, o último ciclo do CLI. A diferença era `SyncEvent::m_canAssertIRQ` do evento de T1 do 6522 #1: o oráculo a tem **falsa**, o Rust verdadeira. `SY6522::Reset()` executa `Write(rIER, 0x7f)` (6522.cpp:50) e o ramo `case 0x0e` recalcula `m_canAssertIRQ = IER & IxR_TIMER1/2` (cpp:217-220): depois do reset, nenhum evento de timer marca "IRQ no último ciclo do opcode" até o IER habilitar o bit. Em `Via::reset` o IER era zerado direto, sem atualizar `t1_gated/t2_gated`, que ficavam no valor inicial (`false` = pode marcar). Portanto as duas hipóteses já descartadas (fase dos eventos de 2 ciclos e bandeira `irq_on_last_opcode_cycle`) realmente não eram a causa.
- **Correção:** `Via::reset` põe `t1_gated = t2_gated = true` junto com `ier = 0` (`apple2-audio/src/via6522.rs`). Nenhuma mudança em `apple2-core`.
- **Teste:** `via6522.rs::tests::reset_gates_both_timer_events_until_ier_enables_them`, `apple2-audio/tests/machine.rs::gh1176_timer_event_on_the_cli_last_cycle_does_not_defer_the_pending_irq`, `::ier_write_gates_whether_pending_timer_events_defer_an_irq` (ajustado: o evento já nasce fechado) e o caso `audio-issue-1176-last-cycle` (PASS, 20 eventos idênticos; `known_divergence` retirado).

## Hipóteses descartadas

- O estado inicial da RAM, hipótese inicial do supervisor para a divergência do ciclo 132.313: correto implementar (D2), mas a causa era o Disk II (D5).
- Commits do upstream que o fork não tem (`d9b2bf67`, `736a12b0`, `c75971ab`): nenhum afetou os traces de boot com Disk II vazio ou com o DOS 3.3; nenhuma divergência foi atribuída a eles.

## Pontos em aberto (não bloqueantes)

- A leitura do `$C061-$C063` e o Language Card com Saturn fora do slot 0 (commits do upstream que o oráculo não tem) não são exercitados por estes traces.
- O boot com disco e teclado/joystick não foi comparado; só CPU e Disk II.
- Desempenho: o caminho quente (`Machine::step`) não mudou; `next_step_is_interrupt_entry` só é chamada pelo `TraceWriter`. O teste `no_alloc` continua passando; o benchmark `emulated_speed` (`crates/apple2-core/tests/boot.rs`, `--ignored`) dá 53 a 60 MHz emulados nesta máquina.
