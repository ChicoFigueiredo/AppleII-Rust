; Phasor native mode (slot 4): chip select e endereço de latch de cada AY (GH#1192, sequências A-D
; do issue). 6522 #0 em $C410 (ORB $C410, ORA $C411, DDRB $C412, DDRA $C413). ORB bits 4:3 são
; ativos em baixo: $08 = so AY1, $10 = so AY2, $00 = os dois. Funções em 2:0: 7 latch, 6 escrita,
; 5 leitura, 4 inativo; bit 2 = /RESET.
SEL1 = $08
SEL2 = $10
    .org $0300
start:
    SEI
    CLD
    LDX #$FF
    TXS
    LDA $C0C5          ; slot 4 -> modo nativo do Phasor
    LDA #$FF
    STA $C413
    LDA #$1F
    STA $C412
    ; Sequência A
    JSR reset
    LDA #0
    LDX #SEL1
    JSR latch
    LDA #$55
    JSR write
    LDA #2
    LDX #SEL2
    JSR latch
    LDA #$66
    JSR write
    LDA #$77
    LDX #SEL1
    JSR write
    LDA #$88
    LDX #SEL2
    JSR write
    LDA #0
    LDX #SEL1
    JSR latch
    JSR read
    LDX #SEL2
    JSR read
    ; Sequência B
    JSR reset
    LDA #2
    LDX #SEL2
    JSR latch
    LDA #$11
    JSR write
    LDA #$42
    LDX #SEL1
    JSR write
    LDA #2
    LDX #SEL1
    JSR latch
    JSR read
    LDA #2
    LDX #SEL2
    JSR latch
    JSR read
    ; Sequência C
    JSR reset
    LDA #2
    LDX #SEL2
    JSR latch
    LDA #$11
    JSR write
    LDA #2
    LDX #SEL1
    JSR latch
    LDA #$42
    JSR write
    LDA #2
    LDX #SEL1
    JSR latch
    JSR read
    LDA #2
    LDX #SEL2
    JSR latch
    JSR read
    ; Sequência D
    JSR reset
    LDA #2
    LDX #SEL1
    JSR latch
    LDA #$99
    JSR write
    LDA #$69
    LDX #SEL2
    JSR write
    LDA #2
    LDX #SEL1
    JSR latch
    JSR read
    LDA #2
    LDX #SEL2
    JSR latch
    JSR read
    ; Os dois AY selecionados de uma vez: latch, escrita e leitura
    LDA #3
    LDX #0
    JSR latch
    LDA #$5A
    JSR write
    JSR read
done:
    JMP done

; reset: pulsa /RESET (ORB bit 2 em 0) nos dois AY
reset:
    LDA #$18
    STA $C410
    LDA #$1C
    STA $C410
    RTS

; latch: A = registrador, X = seleção
latch:
    STA $C411
    TXA
    ORA #$07
    STA $C410
    TXA
    ORA #$04
    STA $C410
    RTS

; write: A = valor, X = seleção
write:
    STA $C411
    TXA
    ORA #$06
    STA $C410
    TXA
    ORA #$04
    STA $C410
    RTS

; read: X = seleção; DDRA vira entrada durante a leitura
read:
    LDA #$00
    STA $C413
    TXA
    ORA #$05
    STA $C410
    LDA $C411
    TXA
    ORA #$04
    STA $C410
    LDA #$FF
    STA $C413
    RTS
