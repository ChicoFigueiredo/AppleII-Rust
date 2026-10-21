# Investigação: suporte a Apple I (HU-1204)

Resumo: o Apple I é uma arquitetura diferente do Apple II (terminal por hardware com Signetics 2513, PIA em $D010–$D013, Woz Monitor de 256 bytes, sem soft switches nem framebuffer). Só o `apple2-cpu` seria reaproveitado. Se um dia entrar no escopo, ficará num crate separado `apple1-core`, sem tocar `apple2-core`. A ROM do monitor tem copyright da Apple, e a política de redistribuição é a confirmar. **Decisão proposta: não implementar até decisão explícita do dono.**

Detalhes, esforço, testes exigidos e condições de entrada: [ADR 0005](../adr/0005-apple-i-fora-do-escopo-ate-decisao.md).

Itens a confirmar: taxa exata do terminal (cerca de 60 cps), clock efetivo (o MAME usa 960 kHz), endereços da carta ACI, scan legítimo do manual de 1976.
