# Pendências

Estado em 2026-10-21, na `main` `22dbf2b`, depois das ondas de conformidade. As diferenças em relação ao AppleWin que o usuário percebe estão em [manual/12-diferencas-em-relacao-ao-applewin.md](manual/12-diferencas-em-relacao-ao-applewin.md). A lista completa de FAIL, BLOCKED e UNVERIFIED, com a causa de cada um, está em [conformance/gaps-and-known-limitations.md](conformance/gaps-and-known-limitations.md); o panorama, em [conformance/relatorio-final.md](conformance/relatorio-final.md). Aqui ficam só as pendências técnicas.

## Defeitos conhecidos (FAIL contra o Oráculo)

- Áudio: o PCM do AY (tom, envelope, ruído) e o PCM do alto-falante saem dos limiares registrados; o primeiro fonema do SSI263 numa das fases passa 13 ciclos da tolerância (D-2, D-3, D-4 em [conformance/audit/audio-differential.md](conformance/audit/audio-differential.md)).
- Mockingboard: o IRQ do 6522 no último ciclo do opcode (AppleWin #1176) é adiado um opcode; os estados `6522-one-shot` e `SSI263-test` ainda divergem logo depois do carregamento.
- Vídeo: 1 pixel de borda no Color Monitor PAL na cena de floating bus do AppleWin #241.
- Save-state: nove arquivos reais do AppleWin ainda divergem (`SNES_MAX-slot4`, três `dazzledraw1`, `SmartPort-firmware`, `Saturn64`, `Saturn128`, `6522-one-shot`, `SSI263-test`). Causa raiz dos Saturn e do SmartPort ainda não investigada.
- Depurador: 10 issues do AppleWin pedem mudanças que o AppleWin também não tem; o Rust as reproduz. Só viram trabalho com decisão de Divergência intencional.

## Não executado (BLOCKED)

- Windows nativo e macOS: smoke, pacote instalado, DPI, áudio, gamepad, Gatekeeper e SmartScreen. Roteiros em [conformance/manual-tests/plataformas.md](conformance/manual-tests/plataformas.md).
- Hardware físico: gamepad, captura de mouse, som audível, serial real, pcap/Npcap, impressora, teclado regional. Roteiros em [conformance/manual-tests/](conformance/manual-tests/).
- `release.yml`: os passos novos (SBOM, licenças, SHA256SUMS) nunca rodaram em CI. A CI só roda por ordem do dono.
- 27 estados do corpus AppleWin-Test (imagens ausentes, rede, serial, hora real) e 87 issues upstream sem fixture ou sem Oráculo de frontend.

## Verificação que falta

- 524 issues do AppleWin (278 P3 e 246 P2) estão UNVERIFIED; nenhuma P0 nem P1.
- Vídeo dos clones (Pravets, TK3000, Base64A, J-Plus), VidHD em ][ e ][+, SHR em PAL, INIT/FORMAT de disco, firmware de 13 setores, Disk II fora do slot 6.
- Janela de configuração, menus e atalhos F1 a F12 só têm testes do próprio projeto.
- Z80 comparado só com vetores de terceiros; o CP/M no sistema não foi comparado com o AppleWin.
- Uma revisão independente do conjunto (G3) ainda não existe; a nota do [relatório final](conformance/relatorio-final.md) é autoavaliação.

## Cartas e emulação

- ThunderClock Plus: emulado na biblioteca, sem tipo de carta para escolher na configuração (o AppleWin também não o oferece).
- Uthernet II: PPPoE, SPI e interrupções não existem (modos sem suporte falham fechados), como no AppleWin.
- Z80: a SoftCard não liga as linhas de interrupção, como no AppleWin.

## Desempenho

- Medida na máquina de desenvolvimento, compartilhada com outros processos ([tests/conformance/reports/performance.md](../tests/conformance/reports/performance.md)): CPU pura ~25 MHz de mediana, laço completo com renderizador de 8 a 14 MHz (Color TV NTSC ~13 MHz, PAL ~14 MHz), boot do DOS 3.3 ~13 MHz. Sem hardware nem compilação fixos; não é comparação com o AppleWin. `tests/no_alloc.rs` (núcleo e vídeo) garante zero alocações por passo e por quadro.
- `CardCtx` ainda calcula `floating_bus` de forma eager a cada chamada de carta. Otimizar só com a suíte diferencial como rede de proteção.

## Distribuição

- Pacotes sem assinatura Authenticode e sem notarização ([conformance/platform/assinatura.md](conformance/platform/assinatura.md)).
