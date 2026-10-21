; Votrax SC-01 on the Mockingboard (GH#952, Ape Escape): the game sets DDRB = $BF (not $FF),
; plays a phoneme and polls IFR ($C40D) for the completion bit. Two phonemes: DDRB = $BF, then $FF.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #$B0
    STA $C40C
    LDA #$BF
    STA $C402
    LDA $C40D
    LDA #$2C
    STA $C400
    LDA $C40D
    LDX #40
    JSR delay
    LDA $C40D
    LDA #$FF
    STA $C402
    LDA #$0E
    STA $C400
    LDA $C40D
    LDX #40
    JSR delay
    LDA $C40D
done:
    JMP done
    .include "lib.inc"
