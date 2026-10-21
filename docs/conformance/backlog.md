# Backlog de conformidade — AppleII-Rust

Gerado por `tools/conformance/backlog.py` a partir de `docs/prompts/AppleII-Rust_Plano_Auditoria_Paridade_e_Evolucao.md`. Não editar à mão; rodar o script de novo.

| ID | Título | Prioridade | Onda | Gate | Issue GitHub (#N) |
|---|---|---|---|---|---|
| EP-00 | Auditoria histórica de issues e rastreabilidade integral | P0 | 0 | G0 — Inventário | #2 |
| EP-01 | CPU, temporização, barramento e memória | P0 | 1 | G2 — Beta funcional | #3 |
| EP-02 | Vídeo, NTSC, PAL e modos de exibição | P0 | 2 | G2 — Beta funcional | #4 |
| EP-03 | Disco, mass storage e preservação de dados | P0 | 1 | G2 — Beta funcional | #5 |
| EP-04 | Save-state, interoperabilidade e serialização | P0 | 1 | G2 — Beta funcional | #6 |
| EP-05 | Som, fala e cartões especializados | P1 | 2 | G3 — Stable candidata | #7 |
| EP-06 | Z80 e outras placas de expansão | P1 | 2 | G3 — Stable candidata | #8 |
| EP-07 | Rede, serial, impressora e dispositivos de entrada | P1 | 2 | G3 — Stable candidata | #9 |
| EP-08 | Debugger e experiência AppleWin | P1 | 3 | G3 — Stable candidata | #10 |
| EP-09 | Windows, Linux e macOS: qualidade de plataforma | P0 | 3 | G2 — Beta funcional | #11 |
| EP-10 | Performance preservando correção | P1 | 3 | G3 — Stable candidata | #12 |
| EP-11 | Compatibilidade com problemas históricos e novas issues abertas | P1 | 4 | G3 — Stable candidata | #13 |
| EP-12 | Evoluções próprias, sem comprometer compatibilidade | P2 | 4 | G3 — Stable candidata | #14 |
| HU-0001 | Inventariar todas as issues relevantes do AppleWin | P0 | 0 | G0 — Inventário | #15 |
| HU-0002 | Importar e utilizar AppleWin-Test como referência | P0 | 0 | G0 — Inventário | #16 |
| HU-0101 | Conformidade 6502/65C02, inclusive opcodes não documentados quando aplicáveis | P0 | 1 | G2 — Beta funcional | #17 |
| HU-0102 | Cartões de memória, Language Card e Saturn sem perda de configuração | P0 | 1 | G2 — Beta funcional | #18 |
| HU-0103 | Floating bus e sincronismo CPU/vídeo/I-O | P0 | 1 | G2 — Beta funcional | #19 |
| HU-0201 | Fidelidade gráfica verificável por imagens e eventos | P0 | 2 | G2 — Beta funcional | #20 |
| HU-0202 | VidHD e compatibilidade de resoluções | P0 | 2 | G2 — Beta funcional | #21 |
| HU-0301 | Disk II/WOZ/DSK/NIB e integridade das gravações | P0 | 1 | G2 — Beta funcional | #22 |
| HU-0302 | Hot swap e persistência de configuração dos discos | P0 | 1 | G2 — Beta funcional | #23 |
| HU-0303 | HDD, SmartPort e imagens maiores | P0 | 1 | G2 — Beta funcional | #24 |
| HU-0401 | Importar save-states reais do AppleWin | P0 | 1 | G2 — Beta funcional | #25 |
| HU-0402 | Round-trip e atomicidade da gravação | P0 | 1 | G2 — Beta funcional | #26 |
| HU-0501 | Mockingboard, Phasor e AY-3-8910 | P1 | 2 | G3 — Stable candidata | #27 |
| HU-0502 | SSI263/SAM, fonemas e save-states | P1 | 2 | G3 — Stable candidata | #28 |
| HU-0601 | Z80 com interrupções e MEMPTR completos | P1 | 2 | G3 — Stable candidata | #29 |
| HU-0602 | ThunderClock, Breakpoint Card, SNES MAX e configuração por slot | P1 | 2 | G3 — Stable candidata | #30 |
| HU-0701 | Uthernet I/II com protocolo e ciclo de vida confiáveis | P1 | 2 | G3 — Stable candidata | #31 |
| HU-0702 | Entrada real e mapeamento consistente | P1 | 2 | G3 — Stable candidata | #32 |
| HU-0703 | Serial, impressora, Npcap/libpcap e permissões | P1 | 2 | G3 — Stable candidata | #33 |
| HU-0801 | Paridade dos comandos e atalhos de debugger | P1 | 3 | G3 — Stable candidata | #34 |
| HU-0802 | Operações de UI configuráveis e acessibilidade | P1 | 3 | G3 — Stable candidata | #35 |
| HU-0901 | Funcionalidade executável e testada nos três sistemas | P0 | 3 | G2 — Beta funcional | #36 |
| HU-0902 | Distribuição reproduzível e seguros de cadeia de suprimentos | P1 | 3 | G3 — Stable candidata | #37 |
| HU-1001 | Profiling e desempenho determinístico | P1 | 3 | G3 — Stable candidata | #38 |
| HU-1101 | Triar issues upstream abertas com valor alto | P1 | 4 | G3 — Stable candidata | #39 |
| HU-1201 | Depuração temporal e rewind | P2 | 4 | G3 — Stable candidata | #40 |
| HU-1202 | Interface DAP para VS Code | P2 | 4 | G3 — Stable candidata | #41 |
| HU-1203 | Laboratório educacional e disassembler didático | P3 | 4 | G3 — Stable candidata | #42 |
| HU-1204 | Investigação separada: suporte a Apple I | P3 | 4 | G3 — Stable candidata | #43 |
| HU-1205 | Frontend web/WASM | P3 | 4 | G3 — Stable candidata | #44 |
