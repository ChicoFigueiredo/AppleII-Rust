    .org $0300
start:
    SEI
    CLD
    LDA #$FF
    STA $C405
    LDA $C404
    STA $10
    LDA #$FF
    STA $C404
    STA $C405
    LDA $C404
    LDA #$FF
    STA $C405
    LDA $C404
    LDX #0
    LDA #$FF
    STA $C405
    LDA $C404,X
done:
    JMP done
