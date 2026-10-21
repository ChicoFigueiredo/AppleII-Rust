; 6522 register behaviour behind several AppleWin issues, on a Mockingboard in slot 4:
;   GH#1260 IRB read returns the input pins for DDRB=0 bits (and ORB for output bits)
;   GH#952  DDRB=$BF (not $FF) must still be usable (SC-01 detection of Ape Escape)
;   GH#567  IER reads back bit 7 plus the enabled bits ($C0 with Timer1 enabled)
;   GH#496  Timer1 started with IER.T1 = 0 must still set IFR.T1 (polling)
;   GH#833  Timer1 counter reads through other addressing modes (T1C-L / T1C-H)
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    ; GH#1260: IRB at power-on (DDRB = 0)
    LDA $C400
    LDA $C480
    ; GH#952: DDRB = $BF, then ORB and IRB
    LDA #$BF
    STA $C402
    LDA #$FF
    STA $C400
    LDA $C400
    LDA #$00
    STA $C400
    LDA $C400
    LDA #$FF
    STA $C482
    LDA #$A5
    STA $C480
    LDA $C480
    JSR mbinit
    ; GH#567: IER
    LDA $C40E
    LDA #$C0
    STA $C40E
    LDA $C40E
    LDA #$FF
    STA $C40E
    LDA $C40E
    LDA #$7F
    STA $C40E
    LDA $C40E
    LDA #$A0
    STA $C48E
    LDA $C48E
    LDA #$20
    STA $C48E
    LDA $C48E
    ; GH#496: Timer1 one-shot with the interrupt disabled; poll IFR
    LDA #$00
    STA $C40B
    LDA #$30
    STA $C404
    LDA #$00
    STA $C405
poll1:
    LDA $C40D
    AND #$40
    BEQ poll1
    LDA $C40D
    LDA $C404
    LDA $C40D
    ; free-running, interrupt disabled, polled twice
    LDA #$40
    STA $C40B
    LDA #$40
    STA $C404
    LDA #$00
    STA $C405
poll2:
    LDA $C40D
    AND #$40
    BEQ poll2
    LDA $C404
poll3:
    LDA $C40D
    AND #$40
    BEQ poll3
    LDA $C404
    ; GH#833: read the counter through other addressing modes
    LDA #$00
    STA $C40B
    LDA #$FF
    STA $C404
    LDA #$FF
    STA $C405
    LDA $C404
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C405
    LDX #$04
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C400,X
    LDX #$05
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C400,X
    LDY #$04
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C400,Y
    LDA #$00
    STA $50
    LDA #$C4
    STA $51
    LDY #$04
    LDA #$FF
    STA $C404
    STA $C405
    LDA ($50),Y
    LDY #$05
    LDA #$FF
    STA $C404
    STA $C405
    LDA ($50),Y
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C405
    LDA $C405
    LDA $C404
done:
    JMP done
    .include "lib.inc"
