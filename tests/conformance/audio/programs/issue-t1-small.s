; Contador do TIMER1 do 6522 com valores pequenos (GH#652, tabela medida no hardware pelo issue) e a
; contagem por consulta do IFR (GH#701, AppleWin-Test 549e0ff). Mockingboard no slot 4, 6522 #0.
;  1) one-shot: T1C_L := N, T1C_H := 0, LDA T1C_L (4 ciclos, limpa IFR.T1), N = 5..1
;  2) o mesmo em modo continuo (ACR = $40)
;  3) T1 = $0100 por consulta: conta as voltas ate IFR.T1 e le o contador (X, Y:A)
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    JSR mbinit
    LDX #5
oneshot:
    STX $C404
    LDA #0
    STA $C405
    LDA $C404
    LDA $C40D
    DEX
    BNE oneshot
    LDA #$40
    STA $C40B
    LDX #5
freerun:
    STX $C404
    LDA #0
    STA $C405
    LDA $C404
    LDA $C404
    LDA $C40D
    DEX
    BNE freerun
    LDA #0
    STA $C40B
    LDA $C404
    LDA #0
    STA $C404
    LDA #1
    STA $C405
    LDX #0
poll:
    INX
    LDA $C40D
    AND #$40
    BEQ poll
    LDY $C405
    LDA $C404
    STX $10
    STY $11
    STA $12
done:
    JMP done
    .include "lib.inc"
