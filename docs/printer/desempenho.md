# Impressora virtual LX-310: carga, desempenho e segurança

Provas de HU-IMP-080 (carga e desempenho, T-017) e HU-IMP-081 (segurança, R09). Vocabulário do GLOSSARY.md, seção "Impressão".

## 1. Orçamento de `io_write` da Carta

A CPU do Apple II nunca espera a Impressora virtual: o `io_write` da Carta só acrescenta o byte a um lote; o lote vai para a fila de captura a cada ~20.000 ciclos ou 4 KiB e a fila nunca bloqueia o produtor (mutex curto, sem I/O).

Medição determinística (`crates/apple2-hostio/tests/printer_budget.rs`, `#[ignore]`):

```sh
cargo test -p apple2-hostio --release --test printer_budget -- --ignored --nocapture
```

400 lotes de 4.096 bytes escritos em `$C090` de uma Máquina Apple //e Enhanced, tempo por byte de cada lote (inclui o despacho do barramento, igual nos três casos); a fila é esvaziada fora da região medida. Três execuções, ns/byte:

| Destino | P50 | P95 |
|---|---|---|
| Legado (só `Printer.txt`, `FileSink` com buffer) | 38,1 a 40,5 | 54,0 a 57,6 |
| Virtual (só fila de captura) | 36,5 a 39,0 | 55,7 a 60,2 |
| Virtual + arquivo (padrão) | 38,9 a 42,0 | 50,5 a 60,9 |
| **Razão virtual+arquivo / legado** | **1,02 a 1,10** | **0,94 a 1,06** |

Meta da especificação: <= 2x o legado. Atendida com folga (o teste falha acima de 2x). Máquina: AMD Ryzen 7 3700X (16 threads), 128 GiB, Linux 7.0 (WSL), rustc 1.98.1, perfil `--release`. Outras máquinas mudam os valores absolutos; a razão é o que importa.

## 2. Pressão sobre o Spool

`crates/apple2-printer/tests/load_pressure.rs` e `spool_perf.rs` (motor `Lx310` real, Arquivo de impressões em disco, eventos de 4 KiB ou 512 bytes). Tempos e pico de RSS do processo de teste nesta máquina, `--release`:

| Cenário | Entrada | Tempo | Pico RSS | Resultado |
|---|---|---|---|---|
| Listagem BASIC, 100 Páginas (72 colunas x 60 linhas) | 432 KB | 0,43 s (motor) | 61 MB | todas as 100 Páginas com pontos, nenhum diagnóstico de limite |
| Listagem BASIC, 300 Páginas | 1,3 MB | ~1,6 s | 226 MB | idem |
| Bitmap denso `ESC K` / `ESC L` / `ESC *` | 1 MiB | 0,3 / 0,3 / 0,1 s | 276 MB (8 M pontos) | todo byte no `raw.bin` (SHA-256 igual), sem perda de pontos |
| Bitmap esparso `ESC K` | 1 MiB | ~0,05 s | - | idem |
| Texto, 120 Páginas via Spool | 187 KB | ~0,25 s | - | 120+ Páginas, SHA-256 igual |
| Primeiro traço (evento no Spool até o snapshot trazer o ponto), 60 Trabalhos | 1 byte | P50 40,8 ms, P95 50,5 ms | - | inclui criar o Trabalho no Arquivo (fsync); abaixo dos 250 ms de HU-IMP-032 |

Não há perda silenciosa em nenhum ponto:

- **Bytes brutos**: sempre gravados por inteiro (SHA-256 conferido), independentemente dos limites do motor; os limites só decidem o que vira desenho.
- **Limites do motor** (`Lx310Config`): 5.000 Páginas, **2.000.000 pontos por Página** (uma Página física não comporta mais de ~1,8 milhão de posições; o limite só para quem sobrescreve a mesma Página sem fim e **nunca apaga as Páginas seguintes**), **64.000.000 pontos por Trabalho** (proteção de memória contra entrada adversarial: 12 bytes por ponto, ~768 MiB no motor, e **o pico total medido é 769 MiB** (teste `memory_peak`, ver "Memória de pior caso"); uma listagem real de 300 Páginas fica bem abaixo (226 MB de RSS no teste)) e 10.000 diagnósticos. Ao atingir um limite, **uma** nota visível por limite (por Página, no caso do limite de Página) vai para o manifesto e a Central. Antes, um limite de 4 milhões de pontos por Trabalho deixava em branco a partir da Página 73 uma listagem de 100 Páginas (defeito corrigido).
- **Prévia barata**: o Spool só copia o documento vivo (`Arc<PageDocument>`) para a prévia **enquanto alguém lê o snapshot** (janela de 2 s depois da última leitura): janela fechada ou UI travada = zero cópias. Lendo, o intervalo entre cópias é o maior entre 50 ms e 4x o custo da cópia anterior, de modo que copiar nunca passa de ~20% da thread (teste: 1.041 eventos, 3 a 6 cópias). A lista do Arquivo só é remontada em eventos de abrir/fechar e ações do usuário; entre eles só a linha do Trabalho ativo é atualizada. Proposta ainda aberta (mudança de contrato em `document.rs`): `Vec<Arc<Page>>` para compartilhar as Páginas já fechadas e copiar só a em construção.
- **UI bloqueada**: o snapshot é publicado sem ninguém lendo; as notas ficam limitadas a 30 e nada cresce com o tempo.
- **Consumidor lento**: duas camadas, ambas com teto e contagem.
  1. A ponte entre a fila de captura e a thread do Spool deixa no máximo 32 MiB em trânsito (`MAX_BRIDGE_BACKLOG`); acima disso a ponte espera (a CPU do Apple II, nunca), e os bytes ficam na fila de captura (teste `bridge_tests::a_stalled_spool_holds_the_bridge_and_nothing_is_lost`: nada se perde e tudo flui quando o Spool volta).
  2. A fila de captura é limitada em **bytes** (256 MiB). Ao encher, os excedentes são descartados, **contados** (`CaptureReceiver::dropped_bytes`), avisados em `diagnostics` e **mostrados na Central** ("A fila de impressão encheu: N byte(s) ..."). `Open` e `Close` nunca se perdem. Testes: `printer_budget::stalled_consumer_never_makes_the_cpu_wait_and_drops_are_counted` e `printer_export_faults::bytes_dropped_by_a_full_queue_become_a_visible_error`. Teto de memória no pior caso: 32 MiB + 256 MiB de bytes brutos. Não há spill em disco: gravar no Arquivo antes de renderizar exigiria mover a escrita para a thread da ponte; com o Spool parado só por reimpressão longa, a fila de 256 MiB (horas de impressão real) já cobre.

## 3. Falhas de disco e de caminho

- **Disco cheio / pasta sem permissão no Arquivo**: `archive_faults.rs` (T-023): erro visível, o prefixo já gravado fica íntegro, a prévia continua.
- **Salvar PDF / Salvar RAW** (`crates/apple2-rust/tests/printer_export_faults.rs`, sem emulador): pasta inexistente, destino que é pasta, pasta somente leitura, Trabalho danificado e identificador inválido dão mensagem de erro na Central ("Falha ao salvar o PDF/os bytes brutos: ..."), sem panic e sem travar o Spool (um novo Trabalho entra depois).
- **Nomes**: `#`, `%`, Unicode, espaços (inclusive no começo) funcionam; o caminho nunca é interpretado. `..` resolve como em qualquer caminho (o arquivo cai onde o caminho aponta).
- **Symlink como destino**: o link é substituído (escrita atômica por `rename`), o alvo de fora nunca é tocado.
- **Symlink dentro do Arquivo**: pastas de Trabalho, `manifest.json` e `raw.bin` que são symlinks não são seguidos (`archive_recovery.rs::symlinks_are_never_followed`).
- **Salvar RAW** copia o `raw.bin` pela memória e recusa Trabalhos acima de 512 MiB com erro visível (`printer_hub::MAX_RAW_COPY`).

## 4. Manifesto e raw hostis

`archive_hostile.rs` e `archive_recovery.rs`: o manifesto é lido com teto de 4 MiB, nunca confiado; o Trabalho vira **danificado** (listado com o motivo, `raw.bin` preservado, apagável só pelo usuário) quando o JSON é lixo, profundo demais, de schema futuro (`schema_version` acima do suportado), de tipos errados, com `id` diferente da pasta, com `total_bytes` absurdo ou com `raw.bin` menor, maior ou que não é arquivo comum. Listas e textos enormes dentro do teto são aparados (100 diagnósticos, 1.000 versões e exportações, 200 caracteres de título, 20.000 de nota). O SHA-256 errado só aparece em `verify` (a Central avisa ao reimprimir). A varredura da raiz é limitada a 200.000 entradas.

## 5. Escapes do Apple II nunca viram comando do sistema (R09)

- O caminho virtual (Carta -> fila de captura -> Spool -> `Lx310` -> PDF/Central) trata todo byte como dado. Nenhum arquivo dele inicia processo ou shell; o teste `crates/apple2-printer/tests/no_shell.rs` varre esses fontes (printer, `printer_capture.rs`, hub, janela, modelo e painéis da Central) e falha se aparecer `process::Command`, `Command::new`, `xdg-open` e afins. Outro teste imprime `; $(...) \`...\` | &` e sequências ESC estranhas e confere que nada é criado e que o `raw.bin` é idêntico.
- O único ponto do repositório que chama um processo por causa de impressão é o **legado** `dump_to_printer` ("copy file prn" do AppleWin): `crates/apple2-hostio/src/printer.rs`, `FileSink::close`, **só Windows**, copia o arquivo do caminho que o **usuário** configurou. O conteúdo impresso não entra na linha de comando e a Impressora virtual nunca o aciona (opção separada, no caminho do `Printer.txt`).

## Memória de pior caso

Fluxo ESC L denso de 16,5 MB (900 colunas cheias por linha, 4.096 bytes por evento, leitor consultando o snapshot a cada evento, fechamento pelo usuário), até o limite de 64 M pontos por Trabalho. Medido como `VmHWM` do processo de teste (`cargo test --release -p apple2-printer --test memory_peak -- --ignored --nocapture`, Linux):

| | Pico de RSS |
|---|---|
| Antes (`Vec<Page>`: motor + cópia da prévia + clone ao fechar) | 2.079 MiB |
| Depois (`Vec<Arc<Page>>`) | 769 MiB |

O documento passou a guardar `Arc<Page>` (serde `rc`, serialização idêntica): a cópia da prévia e o clone ao fechar o Trabalho só copiam ponteiros; só a Página em andamento é duplicada (no máximo 2 M pontos, ~24 MiB) quando o motor escreve nela depois de publicada (`Arc::make_mut`). O teto documentado do Trabalho é, portanto, o do motor: 64 M pontos x 12 bytes = 768 MiB, mais a fila de captura (até 256 MiB) e a ponte (32 MiB) que são contadas à parte. O teste falha se o pico passar de 850 MiB.
