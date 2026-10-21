; Phasor in native mode (slot 4): each 6522 drives two AYs selected by ORB bits 4:3 (active low).
; Tones on AY 1 and AY 2 of the left 6522, both selected at once, then reads of AY registers
; (registers 14/15 depend on the AY type), then Echo+ mode ($C0C7).
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    LDA $C0C5
    JSR mbinit
    ; AY 1 only: ORB bits 4:3 = 01 (bit4 low selects AY1); function bits in 2:0
    LDA #$FF
    STA $C403
    LDA #$1F
    STA $C402
    ; reset both AYs
    LDA #$00
    STA $C400
    LDA #$04
    STA $C400
    ; latch reg 7 on AY1 (bit 4 = 0 -> %01000 | 7 = $0F), write $3E
    LDA #$07
    STA $C401
    LDA #$0F
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$3E
    STA $C401
    LDA #$0E
    STA $C400
    LDA #$0C
    STA $C400
    ; AY1 volume A = 15, period $0120
    LDA #$08
    STA $C401
    LDA #$0F
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$0F
    STA $C401
    LDA #$0E
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$00
    STA $C401
    LDA #$0F
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$20
    STA $C401
    LDA #$0E
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$01
    STA $C401
    LDA #$0F
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$01
    STA $C401
    LDA #$0E
    STA $C400
    LDA #$0C
    STA $C400
    ; AY2 (bit 3 low, bit 4 high = %10000 | 7 = $17): mixer and a tone of its own
    LDA #$07
    STA $C401
    LDA #$17
    STA $C400
    LDA #$14
    STA $C400
    LDA #$3E
    STA $C401
    LDA #$16
    STA $C400
    LDA #$14
    STA $C400
    LDA #$08
    STA $C401
    LDA #$17
    STA $C400
    LDA #$14
    STA $C400
    LDA #$0C
    STA $C401
    LDA #$16
    STA $C400
    LDA #$14
    STA $C400
    LDA #$00
    STA $C401
    LDA #$17
    STA $C400
    LDA #$14
    STA $C400
    LDA #$90
    STA $C401
    LDA #$16
    STA $C400
    LDA #$14
    STA $C400
    LDX #20
    JSR delay
    ; read back registers 14 and 15 of AY1: DDRA = 0, function 5 (read)
    LDA #$00
    STA $C403
    LDA #$0E
    STA $C401
    LDA #$00
    STA $C403
    LDA #$FF
    STA $C403
    LDA #$0E
    STA $C401
    LDA #$0F
    STA $C400
    LDA #$0C
    STA $C400
    LDA #$00
    STA $C403
    LDA #$0D
    STA $C400
    LDA $C401
    LDA #$0C
    STA $C400
    ; Echo+ mode
    LDA $C0C7
    LDX #10
    JSR delay
done:
    JMP done
    .include "lib.inc"
