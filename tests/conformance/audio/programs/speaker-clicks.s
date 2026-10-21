; Speaker: clicks at three rates (read and write accesses to $C030), a pause long enough for the
; DC filter to fade (about 0.4 s), then a last burst. No cards needed.
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    LDX #40
slow:
    LDA $C030
    LDY #60
slow1:
    DEY
    BNE slow1
    DEX
    BNE slow
    LDX #60
fast:
    LDA $C030
    LDY #12
fast1:
    DEY
    BNE fast1
    DEX
    BNE fast
    LDX #20
wr:
    STA $C030
    LDY #30
wr1:
    DEY
    BNE wr1
    DEX
    BNE wr
    LDX #255
    JSR delay
    LDX #255
    JSR delay
    LDX #50
    JSR delay
    LDX #30
last:
    LDA $C030
    LDY #25
last1:
    DEY
    BNE last1
    DEX
    BNE last
done:
    JMP done
    .include "lib.inc"
