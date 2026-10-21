; Solução do exercício 3: cópia de N bytes.
        ORG $0800
        LDX #0
COPIA   LDA $0400,X     ; lê do bloco de origem
        STA $0500,X     ; grava no destino
        INX
        CPX $0300       ; já copiou N bytes?
        BNE COPIA
        BRK
