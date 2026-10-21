; Solução do exercício 5: soma BCD.
        ORG $0800
        SED             ; modo decimal ligado
        CLC
        LDA $0300
        ADC $0301       ; soma em BCD
        STA $0302
        LDA #0
        ROL A           ; A = carry (0 ou 1)
        STA $0303
        CLD             ; desliga o modo decimal
        BRK
