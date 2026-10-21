; Solução do exercício 4: chama MAIS10 três vezes.
        ORG $0800
        LDA $0300
        JSR $0900       ; MAIS10
        JSR $0900
        JSR $0900
        STA $0301
        BRK
