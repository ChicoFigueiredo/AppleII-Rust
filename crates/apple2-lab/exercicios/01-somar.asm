; Solução do exercício 1: soma de dois bytes.
        ORG $0800
        CLC             ; carry limpo: ADC soma também o carry
        LDA $0300       ; A = primeiro número
        ADC $0301       ; A = A + segundo número + C
        STA $0302       ; guarda o byte baixo
        BRK
