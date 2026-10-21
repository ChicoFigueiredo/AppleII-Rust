; AY register read-back: every register of both AYs is written with $FF and read back (the
; unimplemented bits and the I/O ports read differently on AY-3-8910, 8912, 8913 and YM2149),
; then DDRB of both 6522s is written with $FF and read ("bad" 6522s lose bit 0).
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #0
    STA $06
wr:
    LDX $06
    LDA #$FF
    JSR aywrite0
    LDX $06
    LDA #$FF
    JSR aywrite1
    INC $06
    LDA $06
    CMP #16
    BNE wr
    LDA #0
    STA $06
rd:
    LDX $06
    JSR ayread0
    LDX $06
    JSR ayread1
    INC $06
    LDA $06
    CMP #16
    BNE rd
    LDA #$FF
    STA $C402
    STA $C482
    LDA $C402
    LDA $C482
done:
    JMP done
; ayread0 / ayread1: X = register; returns the value in A.
ayread0:
    LDA #$FF
    STA $C403
    STX $C401
    LDY #7
    STY $C400
    LDY #4
    STY $C400
    LDA #0
    STA $C403
    LDY #5
    STY $C400
    LDA $C401
    LDY #4
    STY $C400
    RTS
ayread1:
    LDA #$FF
    STA $C483
    STX $C481
    LDY #7
    STY $C480
    LDY #4
    STY $C480
    LDA #0
    STA $C483
    LDY #5
    STY $C480
    LDA $C481
    LDY #4
    STY $C480
    RTS
    .include "lib.inc"
