# Contribuindo com o AppleII-Rust

Obrigado por querer ajudar. O projeto é um porte do AppleWin para Rust, com o objetivo de ter o mesmo comportamento do original nas três plataformas. Este guia mostra como trabalhar no repositório.

## Antes de começar

1. Leia o [README](README.md) e o [manual](docs/manual/00-indice.md).
2. Leia o [GLOSSARY.md](GLOSSARY.md). **Use os termos do glossário** em código, comentários, commits e documentação (Modelo, Máquina, Ciclo, Quadro, Carta, Slot, Soft switch, Floating bus, Fila de colagem, e assim por diante). Termos a evitar estão listados em cada verbete.
3. Leia os ADRs em [docs/adr](docs/adr): núcleo headless e determinístico (0001), ROMs embutidas (0002) e thread de emulação e ritmo (0003). Eles explicam por que certas coisas não podem ser feitas de outro jeito.
4. Veja [docs/plano-final.md](docs/plano-final.md) e [docs/pendencias.md](docs/pendencias.md) para o que está em aberto.

## Ambiente

Instale o Rust estável. No Linux, instale também:

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
```

## Fluxo de trabalho

1. Abra uma issue (GitHub ou GitLab) descrevendo o que quer fazer, ou escolha uma existente.
2. Crie um branch a partir de `main`. Use uma worktree própria se estiver trabalhando em paralelo com outras pessoas (`git worktree add`).
3. Faça commits pequenos, com mensagens claras no imperativo. Prefixe o assunto com o crate ou a área (`savestate: ...`, `debugger: ...`, `docs: ...`).
4. Antes de abrir o pull request, atualize seu branch com `git merge main` e rode os testes e o clippy (abaixo).
5. Abra o pull request ou o merge request, explicando o quê e por quê, e qual arquivo do AppleWin foi portado.

Mantenha cada alteração dentro do crate a que ela pertence. `apple2-core` só recebe ganchos mínimos e genéricos; não mexa em `CardKind` (a fábrica de Cartas fica no frontend).

## Testes e qualidade

```sh
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check      # informativo por enquanto
```

- Escreva os testes primeiro sempre que possível (TDD): um teste que falha, a correção mínima, e depois a limpeza.
- O clippy precisa passar **sem avisos**. O CI roda nos três sistemas (Linux, Windows e macOS).
- Os testes de desempenho e de ritmo do frontend (`crates/applewin/tests/threaded.rs`) só são confiáveis com `--release`.
- Um teste de integração que dá boot numa Máquina e verifica a memória de texto vale mais do que muitos testes unitários isolados. Veja `crates/apple2-core/tests` e `crates/applewin/tests`.
- Mantenha o determinismo: nada de relógio do computador, de threads nem de E/S de arquivo dentro do núcleo (ADR 0001). O que precisa do computador entra por uma trait.
- O núcleo deve continuar sem alocações por passo (`tests/no_alloc.rs`).

## Publicando uma versão

Só o dono do repositório publica. O procedimento:

1. Aumente a versão em `[workspace.package]` do `Cargo.toml` da raiz.
2. Abra um pull request para `main` com `[release]` no título.
3. Ao mesclar, o workflow `Release` cria a tag `v<versão>` no commit de merge e publica a release com os pacotes de Linux, Windows e macOS.

PRs sem `[release]` (ou de outros autores) nunca publicam. Se a tag `v<versão>` já existe, o workflow falha: aumente a versão. A execução manual do workflow é um dry run (só artefatos, sem tag nem release). O GitLab CI nunca publica releases.

## ADRs e glossário

- **Decisão de arquitetura difícil de reverter, surpreendente sem contexto e resultado de uma escolha real entre alternativas?** Registre um ADR em `docs/adr/NNNN-titulo.md`, curto: o contexto, a decisão e o motivo.
- **Termo novo de domínio?** Acrescente ao `GLOSSARY.md`, com a definição em uma ou duas frases e os termos a evitar. Mantenha a ordem por seção.
- Itens que dependem de outro crate vão para `docs/pendencias.md`.

## Como portar um recurso do AppleWin

O código de referência está em https://github.com/AppleWin/AppleWin (pasta `source/`).

1. **Ache o código e o teste.** Localize o arquivo `.cpp` e o `.h` do recurso. Leia também o histórico (`bin/History.txt`) e a ajuda (`help/*.html`) para entender o comportamento esperado e os casos especiais (procure pelos números de issue `GH#...`).
2. **Decida onde o recurso mora.** Veja a tabela do README: CPU em `apple2-cpu`, memória e slots em `apple2-core`, vídeo em `apple2-video`, discos em `apple2-storage`, som em `apple2-audio`, Cartas sem E/S do computador em `apple2-cards`, Cartas com serial/rede/impressora em `apple2-hostio`, depurador em `apple2-debugger`, configuração e linha de comando em `apple2-config`, save-state em `apple2-savestate` e janela em `applewin`.
3. **Escreva primeiro o teste**, a partir do comportamento do AppleWin: valores de registradores, Ciclos, bytes lidos. Use valores obtidos do código original ou de uma execução dele.
4. **Porte a lógica, não a estrutura.** Troque o que depende do tempo real por Ciclos, o que depende de Win32 por uma trait, e variáveis globais por estado dentro de uma struct. Mantenha os nomes do original nos comentários.
5. **Cite a origem.** Cada módulo traz no comentário o arquivo e a linha do AppleWin que ele segue, como `// Memory.cpp:1234`. Isso ajuda quem for revisar e quem for atualizar o porte.
6. **Cartas.** Implemente a trait `Card` do núcleo, mantenha o estado serializável (`serde`) e acrescente o mapeamento em `apple2-savestate` com o nome e a versão da unidade do AppleWin (veja `crates/apple2-savestate/src/cards`). Se a Carta tem E/S do computador, ela fala com ele por uma trait definida no crate.
7. **Opções.** Se o recurso tem opção de linha de comando ou de configuração, acrescente ao `apple2-config` (`Settings`, `cmdline.rs`), com o nome idêntico ao do AppleWin.
8. **Documente.** Atualize o manual em `docs/manual` (em português). Se o recurso ficou incompleto, diga isso na página "Diferenças em relação ao AppleWin".
9. **Registre as diferenças.** Se você se afastou do AppleWin de propósito, explique no comentário e no manual.

## Manual e ajuda do programa

O manual fica em `docs/manual/*.md`, em português do Brasil, e é embutido no programa por `include_str!` em `crates/applewin/src/help.rs` (tecla F1). O visualizador entende apenas um subconjunto de Markdown: títulos `#`, parágrafos, listas com `-` e `1.`, blocos de código com três crases, tabelas com `|` (exibidas em fonte monoespaçada) e os trechos `**negrito**`, `` `código` `` e `[texto](link)`. Ao acrescentar uma página, inclua-a na lista `MANUAL` de `help.rs` e comece o arquivo com um título `#`.

## Licença

Ao contribuir, você concorda que sua contribuição será distribuída sob a **GPL-2.0-or-later**, a mesma licença do projeto e do AppleWin. Não copie código de projetos com licença incompatível.
