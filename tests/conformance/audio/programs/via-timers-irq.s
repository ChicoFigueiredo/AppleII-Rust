; 6522 timers and IRQ: T1 free-running on 6522 #0 with IRQs (six of them, handler acknowledges by
; reading T1C-L), then T1 one-shot with an IRQ, then T2 one-shot on 6522 #1 polled through IFR,
; with IFR / IER / ACR read back. The ROM's IRQ entry ($FFFE -> $FA40) jumps through ($03FE).
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDA #0
    STA $06
    STA $07
    LDA #<handler
    STA $03FE
    LDA #>handler
    STA $03FF
    ; phase 1: T1 free-running, period $0400 + 2
    LDA #$40
    STA $C40B
    LDA #$C0
    STA $C40E
    LDA #$00
    STA $C404
    LDA #$04
    STA $C405
    CLI
wait1:
    LDA $06
    CMP #6
    BNE wait1
    SEI
    LDA $C40D
    LDA $C40E
    LDA $C40B
    ; stop the free-run interrupts
    LDA #$40
    STA $C40E
    ; phase 2: T1 one-shot, $0300 cycles, IRQ once
    LDA #$00
    STA $C40B
    LDA #$C0
    STA $C40E
    LDA #$00
    STA $C404
    LDA #$03
    STA $C405
    CLI
wait2:
    LDA $06
    CMP #7
    BNE wait2
    SEI
    LDA #$40
    STA $C40E
    ; phase 3: T2 one-shot on 6522 #1, polled
    LDA #$00
    STA $C488
    LDA #$02
    STA $C489
poll:
    LDA $C48D
    AND #$20
    BEQ poll
    LDA $C48D
    LDA $C488
    LDA $C48D
    ; phase 4: re-arm T2 with a short count, read the counter back while it runs
    LDA #$20
    STA $C488
    LDA #$00
    STA $C489
    LDA $C488
    LDA $C489
    LDA $C48D
    LDX #8
    JSR delay
    LDA $C48D
done:
    JMP done
handler:
    LDA $C404
    INC $06
    LDA $45
    RTI
    .include "lib.inc"
