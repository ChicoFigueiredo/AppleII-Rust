; AY access sequences behind GH#320, #1193 and #929 (Mockingboard / Phasor in slot 4):
;   GH#320  register writes that skip the "inactive" function between the latch and the data
;   GH#1193 AY register reads with the 6522 PortA read before / after the AY goes INACTIVE
;   GH#929  ORB written with $FF while DDRB = $FF (function bits 7 and the /RESET bit high)
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    ; GH#320: write regs 0/1 (tone A period) and 8 (volume) without the inactive step
    LDA #$00
    STA $C401
    LDA #$07
    STA $C400
    LDA #$34
    STA $C401
    LDA #$06
    STA $C400
    LDA #$01
    STA $C401
    LDA #$07
    STA $C400
    LDA #$02
    STA $C401
    LDA #$06
    STA $C400
    LDA #$08
    STA $C401
    LDA #$07
    STA $C400
    LDA #$0F
    STA $C401
    LDA #$06
    STA $C400
    LDA #$04
    STA $C400
    ; GH#1193: read reg 0: PortA read while the function is READ, then again after INACTIVE
    LDA #$00
    STA $C401
    LDA #$07
    STA $C400
    LDA #$04
    STA $C400
    LDA #$00
    STA $C403
    LDA #$05
    STA $C400
    LDA $C401
    LDA #$04
    STA $C400
    LDA $C401
    ; the same on the right 6522
    LDA #$00
    STA $C481
    LDA #$07
    STA $C480
    LDA #$04
    STA $C480
    LDA #$00
    STA $C483
    LDA #$05
    STA $C480
    LDA $C481
    LDA #$04
    STA $C480
    LDA $C481
    LDA #$FF
    STA $C403
    STA $C483
    ; GH#929: DDRB = $FF, ORB = $FF (function 7 + /RESET high, other bits set)
    LDA #$FF
    STA $C402
    STA $C482
    LDA #$FF
    STA $C400
    STA $C400
    LDA #$04
    STA $C400
    LDA #$FF
    STA $C480
    LDA #$04
    STA $C480
    ; the AY still takes a register write afterwards
    LDA #$00
    STA $C401
    LDA #$FF
    STA $C400
    LDA #$04
    STA $C400
    LDA #$55
    STA $C401
    LDA #$FE
    STA $C400
    LDA #$04
    STA $C400
done:
    JMP done
    .include "lib.inc"
