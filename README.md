# AppleII-Rust

Reescrita em Rust do emulador [AppleWin](https://github.com/AppleWin/AppleWin), pensada para rodar
de forma idêntica em **Windows, Linux e macOS**. Obra derivada do AppleWin e distribuída sob a
**GPL-2.0-or-later** (veja [LICENSE](LICENSE)); créditos aos autores do AppleWin.

Repositórios: [GitHub](https://github.com/ChicoFigueiredo/AppleII-Rust) ·
[GitLab](https://gitlab.com/chico-figueiredo/opensourcechico/AppleII-Rust)

## Decisões de arquitetura

- Workspace Cargo com crates pequenos e testáveis por fase.
- Stack portátil, sem dependência de SDL: `winit` (janela) + `wgpu` (vídeo/shaders CRT) + `cpal` (áudio)
  + `gilrs` (joystick) + `egui` (UI/overlay).
- ROMs de sistema copiadas do AppleWin para [roms/](roms) (mesmo tratamento do upstream).
- Meta: paridade com o que o AppleWin faz hoje, validada por testes (CPU com os testes de Klaus Dormann).

## Estado atual

| Fase | Conteúdo | Estado |
|------|----------|--------|
| 0 | Repositório, workspace, licença, ROMs, remotes GitHub/GitLab | feito |
| 1 | `apple2-cpu`: 6502 NMOS + 65C02, tempo por ciclo, IRQ/NMI, decimal | feito (testes funcionais passam) |
| 2 | `apple2-core`: mapa de memória (II/II+/J-Plus/IIe/Enhanced e clones), soft switches, Language Card, 80col/AUX, teclado, alto-falante, porta de jogos, save-state; todos os Modelos com ROM dão boot até o prompt `]`/`*` nos testes. O IIc é só um esqueleto de porta (sem ROM), como no AppleWin | feito |
| 3 | Vídeo: texto/lo-res/hi-res/double, NTSC e monitores RGB, charsets | pendente |
| 4 | Disk II (DSK/DO/PO/NIB/WOZ/2MG), HDD/SmartPort, fita | pendente |
| 5 | Áudio: alto-falante, Mockingboard (AY8910/6522), SSI263, SAM | pendente |
| 6 | Cartas: Mouse, Videx, RamWorks/Saturn, Uthernet I/II, SSC/serial, impressora, relógios, VidHD, SNES MAX | pendente |
| 7 | Frontend `winit/wgpu/cpal/gilrs`, configuração, save-state (YAML) | pendente |
| 8 | Debugger, CLI (`CmdLine`), empacotamento nas 3 plataformas, CI | pendente |

## Limitações conhecidas da CPU (fase 1)

- Opcodes NMOS não documentados são tratados como NOPs com o tamanho correto (sem efeito real).
- 65C02 sem as instruções de bit Rockwell/WDC (BBR/BBS/RMB/SMB), igual ao AppleWin;
  o teste estendido de Klaus Dormann está marcado como `#[ignore]` por isso.

## Desenvolvimento

```sh
cargo test --workspace --release
```
