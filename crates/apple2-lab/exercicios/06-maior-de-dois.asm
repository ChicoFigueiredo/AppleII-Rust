; Solução do exercício 6: máximo de dois bytes.
        ORG $0800
        LDA $0300
        CMP $0301       ; C = 1 se A >= segundo
        BCS GRAVA
        LDA $0301       ; o segundo era maior
GRAVA   STA $0302
        BRK
