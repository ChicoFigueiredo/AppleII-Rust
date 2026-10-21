; AY envelope generator: every shape in turn on channel A with a tone, then a fast envelope as a
; buzzer on channel B (tone off). Left AY only.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDX #7
    LDA #$3E
    JSR aywrite0
    LDX #0
    LDA #$60
    JSR aywrite0
    LDX #1
    LDA #$00
    JSR aywrite0
    LDX #8
    LDA #$10
    JSR aywrite0
    LDX #11
    LDA #$00
    JSR aywrite0
    LDX #12
    LDA #$06
    JSR aywrite0
    LDA #0
    STA $06
shape:
    LDY $06
    LDA shapes,Y
    LDX #13
    JSR aywrite0
    LDX #24
    JSR delay
    INC $06
    LDY $06
    CPY #8
    BNE shape
    ; buzzer: envelope period 40 on channel B, shape 8 (saw down), tone B off
    LDX #7
    LDA #$3D
    JSR aywrite0
    LDX #9
    LDA #$10
    JSR aywrite0
    LDX #11
    LDA #$28
    JSR aywrite0
    LDX #12
    LDA #$00
    JSR aywrite0
    LDX #13
    LDA #$08
    JSR aywrite0
    LDX #16
    JSR delay
    LDX #13
    LDA #$0A
    JSR aywrite0
    LDX #16
    JSR delay
done:
    JMP done
shapes:
    .byte $00, $04, $08, $09, $0A, $0C, $0E, $0F
    .include "lib.inc"
