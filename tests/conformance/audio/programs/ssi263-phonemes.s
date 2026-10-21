; SSI263 on the Mockingboard (slot 4, speech chip on the right 6522 at $C440): a sequence of phonemes
; in duration mode 1; each "phoneme complete" IRQ (6522 #1 CA1) acknowledges and starts the next,
; then CTL=1 (power-down) must silence the chip (GH#1412). PHASE adds NOP cycles before the first
; write so the same program can be started at different phases of AppleWin's 1000-cycle updates.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #0
    STA $06
    LDA #<handler
    STA $03FE
    LDA #>handler
    STA $03FF
    .fill PHASE, $EA
    LDA #$0C
    STA $C48C
    LDA #$82
    STA $C48E
    LDA #$80
    STA $C443
    LDA #$A0
    STA $C442
    LDA #$00
    STA $C441
    LDA #$E0
    STA $C444
    LDA #$40
    ORA phonemes
    STA $C440
    LDA #$0F
    STA $C443
    CLI
wait:
    LDA $06
    CMP #5
    BNE wait
    SEI
    LDA #$80
    STA $C443
    LDX #60
    JSR delay
done:
    JMP done
handler:
    LDA #$02
    STA $C48D
    INC $06
    LDX $06
    LDA phonemes,X
    ORA #$40
    STA $C440
    LDA $45
    RTI
phonemes:
    .byte $0E, $0C, $2A, $30, $24, $01
    .include "lib.inc"
