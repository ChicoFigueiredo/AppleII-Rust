; AY tone notes: eight notes on channel A (left AY), then a three-voice chord on the left AY with
; different volumes and a tone on the right AY. No noise, no envelope.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    ; mixer: tones A, B, C on; noise off
    LDX #7
    LDA #$38
    JSR aywrite0
    LDX #8
    LDA #$0F
    JSR aywrite0
    LDY #0
    STY $06
note:
    LDY $06
    LDX #0
    LDA periods_lo,Y
    JSR aywrite0
    LDY $06
    LDX #1
    LDA periods_hi,Y
    JSR aywrite0
    LDX #12
    JSR delay
    INC $06
    LDY $06
    CPY #8
    BNE note
    ; chord on the left AY: B and C
    LDX #2
    LDA #$80
    JSR aywrite0
    LDX #3
    LDA #$00
    JSR aywrite0
    LDX #9
    LDA #$0A
    JSR aywrite0
    LDX #4
    LDA #$55
    JSR aywrite0
    LDX #5
    LDA #$00
    JSR aywrite0
    LDX #10
    LDA #$05
    JSR aywrite0
    ; right AY: voice A
    LDX #7
    LDA #$3E
    JSR aywrite1
    LDX #0
    LDA #$C0
    JSR aywrite1
    LDX #1
    LDA #$00
    JSR aywrite1
    LDX #8
    LDA #$0C
    JSR aywrite1
    LDX #20
    JSR delay
    ; silence the left AY
    LDX #8
    LDA #0
    JSR aywrite0
    LDX #9
    LDA #0
    JSR aywrite0
    LDX #10
    LDA #0
    JSR aywrite0
done:
    JMP done
periods_lo:
    .byte $FE, $E1, $C9, $BE, $A9, $8E, $77, $6F
periods_hi:
    .byte $01, $01, $01, $01, $01, $01, $01, $00
    .include "lib.inc"
