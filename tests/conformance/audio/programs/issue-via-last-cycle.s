; GH#1176: 6522 #1 free-running with latch 0 (an event every 2 cycles, IER.T1 never written so the
; event can still raise "IRQ on the last cycle of the opcode"), 6522 #0 one-shot with an IRQ due
; right after a SEI. The CLI that follows must take the pending IRQ before the next opcode.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #<handler
    STA $03FE
    LDA #>handler
    STA $03FF
    LDA #$40
    STA $C48B
    LDA #$00
    STA $C484
    STA $C485
    LDA #$C0
    STA $C40E
    LDA #$00
    STA $C40B
    LDA #$06
    STA $C404
    LDA #$00
    STA $C405
    LDA #0
    STA $07
    CLI
    SEI
    LDA $07
    STA $0310
    CLI
    LDX #1
    JSR delay
    SEI
done:
    JMP done
handler:
    LDA $C40D
    LDA $C404
    LDA #$40
    STA $C40E
    LDA $45
    RTI
    .include "lib.inc"
