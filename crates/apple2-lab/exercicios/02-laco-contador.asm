; Solução do exercício 2: soma de 1 até N com um laço.
        ORG $0800
        LDA #0          ; acumulador começa em zero
        LDX $0300       ; X = N (o contador)
        BEQ FIM         ; N = 0: nada a somar
LOOP    CLC
        STX $FB         ; temporário na página zero
        ADC $FB         ; A = A + X
        DEX
        BNE LOOP        ; repete até X chegar a zero
FIM     STA $0301
        BRK
