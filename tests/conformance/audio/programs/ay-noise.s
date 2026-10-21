; AY noise: noise-only voice A with the noise period swept, then noise mixed with a tone on B,
; then noise on all three voices at different volumes (left AY); noise on the right AY.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    ; mixer: noise A on, all tones off, noise B and C off
    LDX #7
    LDA #$37
    JSR aywrite0
    LDX #8
    LDA #$0F
    JSR aywrite0
    LDA #0
    STA $06
sweep:
    LDY $06
    LDA noise_periods,Y
    LDX #6
    JSR aywrite0
    LDX #10
    JSR delay
    INC $06
    LDY $06
    CPY #6
    BNE sweep
    ; tone B + noise B
    LDX #7
    LDA #$2D
    JSR aywrite0
    LDX #2
    LDA #$90
    JSR aywrite0
    LDX #3
    LDA #$00
    JSR aywrite0
    LDX #9
    LDA #$0C
    JSR aywrite0
    LDX #8
    LDA #$00
    JSR aywrite0
    LDX #10
    JSR delay
    ; all voices noise, tones off
    LDX #7
    LDA #$07
    JSR aywrite0
    LDX #8
    LDA #$0F
    JSR aywrite0
    LDX #9
    LDA #$08
    JSR aywrite0
    LDX #10
    LDA #$04
    JSR aywrite0
    LDX #6
    LDA #$03
    JSR aywrite0
    LDX #10
    JSR delay
    ; right AY: slow noise
    LDX #7
    LDA #$37
    JSR aywrite1
    LDX #6
    LDA #$1F
    JSR aywrite1
    LDX #8
    LDA #$0E
    JSR aywrite1
    LDX #12
    JSR delay
done:
    JMP done
noise_periods:
    .byte $01, $04, $08, $10, $18, $1F
    .include "lib.inc"
