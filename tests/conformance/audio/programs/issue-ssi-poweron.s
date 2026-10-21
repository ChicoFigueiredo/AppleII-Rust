; SSI263 power-on state (GH#1416: SSI263AP by default, power-down after RESET; GH#1302 power-on):
; a phoneme written before the chip was ever taken out of power-down must not start; then
; CTL=0 (power-up) and the same phoneme starts; the 6522 CA1 IRQ flag reports the completion.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #$0C
    STA $C48C
    LDA #$82
    STA $C48E
    ; phoneme in the power-on state (no CTL write before)
    LDA #$A0
    STA $C442
    LDA #$00
    STA $C441
    LDA #$E0
    STA $C444
    LDA #$41
    STA $C440
    LDX #30
    JSR delay
    LDA $C48D
    ; power up: CTL=0 with amplitude 8, then the phoneme again
    LDA #$08
    STA $C443
    LDA #$41
    STA $C440
    LDX #60
    JSR delay
    LDA $C48D
    LDA #$02
    STA $C48D
    ; power down again: continuous phoneme silenced (GH#1412)
    LDA #$80
    STA $C443
    LDX #30
    JSR delay
    LDA $C48D
done:
    JMP done
    .include "lib.inc"
