# Identidade própria: o programa se chama apple2-rust

## Contexto

O AppleII-Rust nasceu como porte do AppleWin e ainda deve tudo a ele, mas tem vida própria: roda em três plataformas, tem manual, empacotamento e depurador próprios. Mesmo assim o executável, os crates, os pacotes e o diretório de configuração se chamavam `applewin` ou `applewin-rs`, o que confunde com o projeto original (quem baixa o pacote acha que é o AppleWin) e atrapalha quem tem os dois instalados. A decisão foi tomada pelo dono em 21/10/2026, antes de qualquer release estável.

## Decisão

- O nome do nosso programa é **`apple2-rust`**: executável (`apple2-rust`, `apple2-rust.exe`), pacote e crate Cargo (`apple2-rust`, em Rust `apple2_rust`), `apple2-rust-dap`, `apple2-rust-web`, pacotes de release (`apple2-rust-<versão>-<plataforma>.*`), SBOM (`apple2-rust-<versão>.cdx.json`) e diretório de configuração (`apple2-rust`).
- `cargo run` na raiz abre o emulador (`default-members` do workspace).
- O AppleWin continua sendo a origem do código e o oráculo de conformidade (ADR 0004). Os comentários, a documentação histórica e os casos de conformidade seguem citando o AppleWin pelo nome.
- A compatibilidade com o AppleWin fica como está: opções de linha de comando (`-d1`, `-model`, `-conf applewin.ini`), importação de Registro/INI, formato `.aws.yaml` de save-state e nomes de arquivo como `AppleWin_ScreenShot_*.png`.
- **Não há alias** `applewin` nos pacotes: o nome antigo não é instalado.

## Consequências

- **Migração da configuração.** Na primeira execução, se o diretório novo não tem `settings.toml` e o antigo (`applewin-rs`) tem, o programa copia `settings.toml` e `recent_disks.txt` e avisa o usuário pela linha de status. O diretório antigo nunca é apagado; se os dois existem, vale o novo. A lógica é testada sem tocar no diretório real (`migrate_legacy_dir`).
- Scripts, atalhos e extensões que chamavam `applewin` precisam trocar para `apple2-rust`. A extensão do VS Code passou a se chamar `apple2-rust` (tipo de depuração e configuração).
- O release candidate `0.2.0-rc.1` precisa ser refeito com os nomes novos; o que foi publicado com o nome antigo não é reaproveitado.
