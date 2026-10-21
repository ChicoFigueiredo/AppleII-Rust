; 6522 interrupt edge cases (Mockingboard or Phasor in slot 4):
;   GH#1175 free-running Timer1 with a tiny latch ($0000, $0001, $0002, $0004) must still IRQ
;   GH#685  / GH#1173 / GH#765 / GH#753 / GH#698: Timer1 of both 6522s free-running with
;           interrupts, one shared handler that acknowledges both
; The IRQ handler is reached through the ROM's $FFFE vector -> $FA40 -> ($03FE).
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
    LDA #0
    STA $06
    STA $07
    STA $0A
    ; GH#1175: tiny latches, one IRQ each (the handler disables the interrupt again)
    LDX #0
tiny:
    STX $08
    LDA #$40
    STA $C40B
    LDA #$C0
    STA $C40E
    LDX $08
    LDA latches,X
    STA $C404
    LDA #0
    STA $C405
    LDA $06
    STA $09
    CLI
twait:
    LDA $06
    CMP $09
    BEQ twait
    SEI
    LDX $08
    INX
    CPX #4
    BNE tiny
    ; stop the timer
    LDA #$40
    STA $C40E
    LDA #$00
    STA $C40B
    ; GH#685: both Timer1 free-running with interrupts; one handler acknowledges both
    LDA #$40
    STA $C40B
    STA $C48B
    LDA #$C0
    STA $C40E
    STA $C48E
    LDA #$00
    STA $C404
    LDA #$02
    STA $C405
    LDA #$80
    STA $C484
    LDA #$01
    STA $C485
    LDA #0
    STA $06
    LDA #1
    STA $0A
    CLI
w3:
    LDA $06
    CMP #12
    BCC w3
    SEI
    LDA #$40
    STA $C40E
    STA $C48E
    LDA $C40D
    LDA $C48D
done:
    JMP done
handler:
    LDA $C40D
    BPL h1
    AND #$40
    BEQ h1
    LDA $C404
h1:
    LDA $C48D
    BPL h2
    AND #$40
    BEQ h2
    LDA $C484
h2:
    ; a tiny-latch phase: switch the interrupt off after the first one
    LDA $0A
    BNE h3
    LDA #$40
    STA $C40E
h3:
    INC $06
    INC $07
    LDA $45
    RTI
latches:
    .byte $00, $01, $02, $04
    .include "lib.inc"
