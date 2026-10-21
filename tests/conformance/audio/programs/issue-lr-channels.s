; GH#192: which stereo channel plays which 6522/AY. A tone on the AY of the 6522 at $C400 only
; (about 0.1 s), a pause, then a tone on the AY of the 6522 at $C480 only.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDX #7
    LDA #$38
    JSR aywrite0
    LDX #7
    LDA #$38
    JSR aywrite1
    LDX #0
    LDA #$A0
    JSR aywrite0
    LDX #1
    LDA #$00
    JSR aywrite0
    LDX #8
    LDA #$0F
    JSR aywrite0
    LDX #80
    JSR delay
    LDX #8
    LDA #$00
    JSR aywrite0
    LDX #40
    JSR delay
    LDX #0
    LDA #$60
    JSR aywrite1
    LDX #1
    LDA #$00
    JSR aywrite1
    LDX #8
    LDA #$0F
    JSR aywrite1
    LDX #80
    JSR delay
    LDX #8
    LDA #$00
    JSR aywrite1
done:
    JMP done
    .include "lib.inc"
