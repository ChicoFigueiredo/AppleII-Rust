; Shared helpers for the audio conformance programs. Mockingboard (or Phasor) in slot 4:
; 6522 #0 at $C400 (ORB $C400, ORA $C401, DDRB $C402, DDRA $C403), 6522 #1 at $C480.
; The AY is driven the way every Mockingboard player does: ORA = data, ORB = function
; (7 = latch address, 6 = write, 5 = read, 4 = inactive, 0 = reset).

; mbinit: DDRA = $FF, DDRB = $07 on both 6522s; pulse RESET on both AYs (ORB 0, then 4).
mbinit:
    LDA #$FF
    STA $C403
    STA $C483
    LDA #$07
    STA $C402
    STA $C482
    LDA #0
    STA $C400
    STA $C480
    LDA #4
    STA $C400
    STA $C480
    RTS

; aywrite0 / aywrite1: X = AY register, A = value; AY of 6522 #0 (left) / #1 (right).
aywrite0:
    STX $C401
    LDY #7
    STY $C400
    LDY #4
    STY $C400
    STA $C401
    LDY #6
    STY $C400
    LDY #4
    STY $C400
    RTS

aywrite1:
    STX $C481
    LDY #7
    STY $C480
    LDY #4
    STY $C480
    STA $C481
    LDY #6
    STY $C480
    LDY #4
    STY $C480
    RTS

; delay: X * about 1287 cycles.
delay:
    LDY #0
delay1:
    DEY
    BNE delay1
    DEX
    BNE delay
    RTS
