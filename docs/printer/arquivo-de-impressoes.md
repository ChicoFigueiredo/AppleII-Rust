# Arquivo de impressões

O Arquivo de impressões é o registro permanente de cada Trabalho de impressão da Impressora virtual (ADR 0008). Código: `crates/apple2-printer/src/archive.rs` (disco) e `src/spool.rs` (serviço que alimenta o Arquivo). Disponível com a feature `fs` da crate.

## Onde fica

Pasta de dados da plataforma, em `apple2-rust/impressoes` (`archive::default_root()`):

| Sistema | Caminho típico |
|---|---|
| Linux | `~/.local/share/apple2-rust/impressoes` |
| macOS | `~/Library/Application Support/org.AppleII-Rust.apple2-rust/impressoes` |
| Windows | `%APPDATA%\AppleII-Rust\apple2-rust\data\impressoes` |

O frontend pode passar outra pasta (opção do usuário). O caminho pode ter `#`, espaços e Unicode.

## Estrutura de pastas

```
impressoes/
  20261021-140301-a3f9c2/      um diretório por Trabalho de impressão
    raw.bin                    bytes exatamente como a placa entregou (somente acréscimo)
    manifest.json              metadados versionados (escrita atômica)
    v2.pdf                     PDF exportado da versão 2 (opcional)
```

O nome do diretório é `AAAAMMDD-HHMMSS` (UTC) + 6 hex de um id curto. Nunca deriva de texto vindo do Apple II (nem de título ou nota). Só `[A-Za-z0-9_-]` é aceito; qualquer outra pasta dentro da raiz é listada como danificada e não é tocada.

## manifest.json (schema 2)

| Campo | Significado |
|---|---|
| `schema_version` | versão do formato (hoje 2) |
| `id` | igual ao nome da pasta (senão o trabalho é danificado) |
| `capture_job` | número do trabalho dado pela placa |
| `origin` | `machine` e `slot` |
| `transport` | `EightBit` ou `SevenBitAppleWin` |
| `profile` | `"Epson LX-310"` |
| `options` | RenderOptions congeladas na abertura (papel, fita, cor) |
| `started_at`, `completed_at` | RFC 3339 UTC |
| `close_reason` | `Idle`, `Reset`, `StateLoaded`, `CardRemoved`, `User`, `Shutdown` |
| `total_bytes` | bytes válidos em `raw.bin` |
| `confirmed_offset` | último offset confirmado por checkpoint (fsync) |
| `sha256` | SHA-256 (hex) dos `total_bytes` de `raw.bin`, preenchido ao fechar ou recuperar |
| `pages`, `diagnostics_total`, `diagnostics` | páginas e resumo (até 100) dos diagnósticos do motor |
| `title`, `title_edited` | título (padrão: primeira linha de texto não vazia + data); editável |
| `note` | nota pessoal |
| `state` | `active`, `completed` ou `interrupted` |
| `versions` | lista de versões: `id`, `created_at`, `options`, `pages`, `diagnostics_total`. A versão 1 é a original |
| `exports` | PDFs exportados: `version`, `path` (só informativo), `exported_at` |

Schema 1 (sem checkpoints, versões, nota, exportações; `size` em vez de `total_bytes`) é migrado na leitura (`migrate_manifest_value`). Um schema mais novo que o suportado vira "danificado", sem adivinhação.

## Gravação e checkpoints

`raw.bin` é aberto em modo acréscimo e nunca é reescrito. O manifesto é gravado em `manifest.json.tmp`, sincronizado e renomeado sobre `manifest.json`. A cada 32 KiB ou 2 s sem checkpoint, o Arquivo faz `fsync` do raw e grava `confirmed_offset`. Se uma escrita falhar (disco cheio), a cauda parcial é removida, o erro aparece no snapshot do spool (`storage_error` e avisos) e o diagnóstico fica no manifesto; a prévia continua funcionando.

## Recuperação

Ao abrir o Arquivo, todo trabalho `active` (o programa caiu) vira `interrupted`: `raw.bin` é truncado ao `confirmed_offset` (nunca inventa bytes; se o raw estiver menor, vale o tamanho real), o SHA-256 é recalculado e o fato aparece em `Archive::recovery()` e nos avisos do spool.

Manifesto ilegível, maior que 4 MiB, com JSON malformado, tipos errados, `id` diferente da pasta, link simbólico no lugar de pasta, `manifest.json` ou `raw.bin`, ou `raw.bin` com tamanho diferente do manifesto: o trabalho é listado como **danificado** com o motivo. Nada é lido além de 4 MiB, nada é escrito fora da raiz e nenhum link simbólico é seguido. Caminhos em `exports` são só dados exibidos.

## Retenção

O Arquivo nunca apaga nada sozinho. `total_size()` informa o espaço usado e, passando do limite configurável (`set_retention_limit`), `retention_warning()` gera um aviso (publicado no snapshot). Apagar só por chamada explícita `delete(id)`; o trabalho ativo é recusado.

## Reimpressão

`Spool::reprint(id, RenderOptions)` relê `raw.bin` (conferindo o SHA-256), gera o documento com outro papel/fita/cor/tabela e registra uma nova versão. Os bytes originais e o SHA-256 não mudam. `render_version` redesenha uma versão antiga com as opções dela.

## Backup

Feche o emulador (ou conclua o trabalho ativo) e copie a pasta `impressoes` inteira. Cada diretório é independente, então dá para copiar só alguns. Para restaurar, coloque as pastas de volta na raiz; ao abrir, o Arquivo as reconhece. O `sha256` de cada manifesto permite conferir a cópia (`Archive::verify`). Não edite `raw.bin` à mão: o trabalho passará a ser listado como danificado.
