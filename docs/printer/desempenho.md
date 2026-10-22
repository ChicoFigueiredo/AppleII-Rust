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

`crates/apple2-printer/tests/load_pressure.rs` (motor `Lx310` real, Arquivo de impressões em disco, eventos de 4 KiB ou 512 bytes). Tempos nesta máquina, `--release`:

| Cenário | Entrada | Tempo | Resultado |
|---|---|---|---|
| Bitmap denso `ESC K` | 1 MiB | ~0,8 s | todo byte no `raw.bin` (SHA-256 igual), diagnóstico "limite de pontos" |
| Bitmap denso `ESC L` / `ESC *` | 1 MiB | ~0,3 s / ~0,16 s | idem |
| Bitmap esparso `ESC K` | 1 MiB | ~0,24 s | dentro dos limites, sem diagnóstico de limite |
| Texto, 120 Páginas | 187 KB | ~0,9 s | 120+ Páginas, SHA-256 igual |
| Limite de Páginas (50) com 120 Páginas | 187 KB | - | diagnóstico "limite de páginas", `raw.bin` inteiro |

Não há perda silenciosa em nenhum ponto:

- **Bytes brutos**: sempre gravados por inteiro, independentemente dos limites do motor; os limites só decidem o que vira desenho.
- **Motor** (`Lx310Config`): 5.000 Páginas, 4.000.000 pontos e 10.000 diagnósticos por Trabalho; ao atingir, **uma** nota visível por limite é gravada no manifesto e na Central.
- **Memória do Spool**: a Página guarda pontos vetoriais (12 bytes cada), não raster. O pior caso é 4.000.000 pontos = 48 MiB no motor, mais uma cópia publicada (`Arc`) para a prévia, a cada 50 ms no máximo e só se o documento mudou. Páginas de Trabalhos antigos só são renderizadas sob demanda.
- **UI bloqueada**: o snapshot é publicado sem ninguém lendo; as notas ficam limitadas a 30 e nada cresce com o tempo.
- **Consumidor lento**: a fila de captura é limitada em **bytes** (256 MiB por padrão). Ao encher, os bytes excedentes são descartados, **contados** (`CaptureReceiver::dropped_bytes`) e reportados uma vez em `diagnostics`; `Open` e `Close` nunca se perdem. Teste: `printer_budget::stalled_consumer_never_makes_the_cpu_wait_and_drops_are_counted` (2 MiB escritos contra um limite de 64 KiB sem consumidor, perda exata contada, CPU sem espera).

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
