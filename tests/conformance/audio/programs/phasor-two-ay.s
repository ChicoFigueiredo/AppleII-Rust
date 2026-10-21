; Phasor in native mode (slot 4): the 6522s sit at $Cn10 (#0) and $Cn80 (#1) and each drives two AYs
; selected by ORB bits 4:3 (active low).
; Tones on AY 1 and AY 2 of the left 6522, both selected at once, then reads of AY registers
; (registers 14/15 depend on the AY type), then Echo+ mode ($C0C7).
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    LDA $C0C5
    ; AY 1 only: ORB bits 4:3 = 01 (bit4 low selects AY1); function bits in 2:0
    LDA #$FF
    STA $C413
    LDA #$1F
    STA $C412
    ; reset both AYs
    LDA #$00
    STA $C410
    LDA #$04
    STA $C410
    ; latch reg 7 on AY1 (bit 4 = 0 -> %01000 | 7 = $0F), write $3E
    LDA #$07
    STA $C411
    LDA #$0F
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$3E
    STA $C411
    LDA #$0E
    STA $C410
    LDA #$0C
    STA $C410
    ; AY1 volume A = 15, period $0120
    LDA #$08
    STA $C411
    LDA #$0F
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$0F
    STA $C411
    LDA #$0E
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$00
    STA $C411
    LDA #$0F
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$20
    STA $C411
    LDA #$0E
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$01
    STA $C411
    LDA #$0F
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$01
    STA $C411
    LDA #$0E
    STA $C410
    LDA #$0C
    STA $C410
    ; AY2 (bit 3 low, bit 4 high = %10000 | 7 = $17): mixer and a tone of its own
    LDA #$07
    STA $C411
    LDA #$17
    STA $C410
    LDA #$14
    STA $C410
    LDA #$3E
    STA $C411
    LDA #$16
    STA $C410
    LDA #$14
    STA $C410
    LDA #$08
    STA $C411
    LDA #$17
    STA $C410
    LDA #$14
    STA $C410
    LDA #$0C
    STA $C411
    LDA #$16
    STA $C410
    LDA #$14
    STA $C410
    LDA #$00
    STA $C411
    LDA #$17
    STA $C410
    LDA #$14
    STA $C410
    LDA #$90
    STA $C411
    LDA #$16
    STA $C410
    LDA #$14
    STA $C410
    LDX #20
    JSR delay
    ; read back registers 14 and 15 of AY1: DDRA = 0, function 5 (read)
    LDA #$00
    STA $C413
    LDA #$0E
    STA $C411
    LDA #$00
    STA $C413
    LDA #$FF
    STA $C413
    LDA #$0E
    STA $C411
    LDA #$0F
    STA $C410
    LDA #$0C
    STA $C410
    LDA #$00
    STA $C413
    LDA #$0D
    STA $C410
    LDA $C411
    LDA #$0C
    STA $C410
    ; the same on AY2: latch register 14 (ORB %10111), read (%10101), inactive (%10100)
    LDA #$FF
    STA $C413
    LDA #$0E
    STA $C411
    LDA #$17
    STA $C410
    LDA #$14
    STA $C410
    LDA #$00
    STA $C413
    LDA #$15
    STA $C410
    LDA $C411
    LDA #$14
    STA $C410
    ; Echo+ mode
    LDA $C0C7
    LDX #10
    JSR delay
done:
    JMP done
    .include "lib.inc"
