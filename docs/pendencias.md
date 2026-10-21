# Pendências

Estado em 2026-10-21, na `main` `469bf94`, depois das ondas de conformidade, da #241 corrigida e da ADR 0006. As diferenças em relação ao AppleWin que o usuário percebe estão em [manual/12-diferencas-em-relacao-ao-applewin.md](manual/12-diferencas-em-relacao-ao-applewin.md). A lista completa de FAIL, BLOCKED e UNVERIFIED, com a causa de cada um, está em [conformance/gaps-and-known-limitations.md](conformance/gaps-and-known-limitations.md); o panorama, em [conformance/relatorio-final.md](conformance/relatorio-final.md). Aqui ficam só as pendências técnicas.

## Defeitos conhecidos (FAIL)

Não há FAIL P0 nem P1 em issue. Restam 6 FAIL na matriz de issues e 5 no harness.

- Depurador, paridade com o AppleWin mantida por decisão do dono (ADR 0006): #713 (aspas simples e duplas), #1145 (ordem de `APPLE2E.SYM`), #1377 (4 das 9 tabelas de símbolos na partida). Adiadas: #1426 e #1431 (mini dump `M` do Language Card por banco ou slot; exige expor o LC por banco no núcleo).
- Interface: o AppleWin mostra o cursor de espera ao carregar um save-state grande (#781); o programa não.
- Limitação aceita pelo dono (ADR 0006), ainda FAIL no harness: o PCM do AY (tom, envelope, ruído), o PCM do alto-falante e o primeiro fonema do SSI263 numa das fases (5 casos, D-2, D-3, D-4 em [conformance/audit/audio-differential.md](conformance/audit/audio-differential.md)). Os eventos dos chips são idênticos aos do AppleWin.
- Divergências aprovadas pelo dono (não são defeitos; ver a seção 7 do gaps): #916, #855, #1396, tela cheia (#280, #358, #1374, #117, #932, #1448) e seis do depurador (#114, #245, #458, #466, #468, #1381).

## Não executado (BLOCKED)

- Windows nativo e macOS: smoke, pacote instalado, DPI, áudio, gamepad, Gatekeeper e SmartScreen. Roteiros em [conformance/manual-tests/plataformas.md](conformance/manual-tests/plataformas.md).
- Hardware físico: gamepad, captura de mouse, som audível, serial real, pcap/Npcap, impressora, teclado regional. Roteiros em [conformance/manual-tests/](conformance/manual-tests/).
- `release.yml`: os passos novos (SBOM, licenças, SHA256SUMS) nunca rodaram em CI. A CI só roda por ordem do dono.
- Windows nativo: uma sessão do dono no Windows 11 está rodando o roteiro W1 a W10; o resultado será registrado em `test/windows-nativo`.
- 27 estados do corpus AppleWin-Test (imagens ausentes, rede, serial, hora real) e 133 issues upstream BLOCKED (40 abertas e 93 fechadas; 7 são P0) sem fixture ou sem Oráculo de frontend.

## Verificação que falta

- Nenhuma das 1.115 issues do AppleWin está UNVERIFIED (487 PASS, 6 FAIL, 133 BLOCKED, 489 NOT_APPLICABLE). Os 849 resultados individuais são autoavaliação, sem amostra independente.
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
