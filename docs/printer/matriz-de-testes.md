# Matriz de testes T-001 a T-030 da Impressora virtual

Ponto de partida: o parecer independente da v0.3.0 (fases A a D, só o Perfil Epson LX-310), atualizado com as correções desta branch. Estados: **PASS** (teste executado prova o critério), **PARTIAL** (passa com aproximação documentada ou cobertura incompleta), **FAIL**, **BLOCKED** (não executável aqui, com a causa), **N/A** (fora do escopo).

Os caminhos são relativos a `crates/`; `lx310_*`, `escp_*`, `spool_*`, `pdf_*`, `font_*`, `archive_*` ficam em `apple2-printer/tests/`.

| T | Estado | Teste que prova | Observação |
|---|---|---|---|
| T-001 | PASS | `apple2-hostio/tests/printer_virtual.rs::t001_basic_print_emits_open_bytes_close_and_legacy_is_unchanged`; `apple2-rust/tests/printer_center.rs::a_print_from_basic_reaches_the_view_model_and_opens_the_window_once` | Ponta a ponta até o view model; abertura automática única |
| T-002 | PASS | `lx310_layout.rs::t002_reset_a_crlf_b_without_literal_esc`; golden `t002_reset_lf_cr` | |
| T-003 | PASS | `lx310_layout.rs::t003_cr_lf_and_crlf_are_distinct_without_double_line` | |
| T-004 | PASS | `lx310_layout.rs::t004_form_feed_gives_second_page_in_same_job` | |
| T-005 | PASS | `lx310_layout.rs::t005_line_spacing_commands_measured_in_units`, `t005_segments_and_esc_j` | |
| T-006 | PASS | `lx310_layout.rs::t006_pitch_advance_by_physical_width`, `t006_columns_by_physical_width_80_at_10cpi_and_no_clipping` | Condensado proporcional: aproximação 6 de [protocols/lx310.md](protocols/lx310.md) |
| T-007 | PARTIAL | `lx310_layout.rs::t007_bold_double_strike_underline_affect_dots`, `t007_master_select_combines_and_resets_modes` | Atributos funcionam; deslocamentos de ênfase e dupla batida são adotados, não vêm do manual |
| T-008 | PASS | `escp_parser.rs::t008_esc_k_reads_exactly_eight_columns_with_esc_inside`, `t008_bdd_chunks_of_1_2_and_5_bytes`; `lx310_layout.rs::t008_*` | Verificação independente: 300 colunas exatas com ESC/FF na carga |
| T-009 | PASS | `escp_parser.rs::t009_esc_star_density_and_data_with_esc_and_lf`; `lx310_layout.rs::t009_*` | |
| T-010 | PASS | `escp_parser.rs::t010_every_split_point_gives_the_same_output`; `lx310_layout.rs::t010_document_hash_is_split_independent`; `lx310_font_golden.rs::real_font_output_is_split_independent_and_inside_the_page`; `escp::tests::esc_amp_split_anywhere_is_equivalent` | Verificação independente com 4.194 bytes, 3 papéis e todas as partições |
| T-011 | PASS | `escp_parser.rs::t011_*`; `lx310_layout.rs::t011_incomplete_sequences_do_not_panic_and_are_reported`; `escp::tests::esc_amp_consumes_nlq_definitions_exactly`, `esc_amp_consumes_draft_definitions_exactly`, `esc_percent_is_recognised_with_one_parameter_and_reported` | `ESC &` agora consome a carga exata (sem lixo); os caracteres do usuário não são impressos |
| T-012 | PASS | `apple2-hostio/tests/printer_virtual.rs::t012_all_256_values_arrive_identical_in_eight_bits`, `explicit_edge_values_in_eight_bits` | |
| T-013 | PASS | `printer_virtual.rs::t013_seven_bit_transport_clears_bit_7` | |
| T-014 | PASS | `printer_virtual.rs::t014_legacy_filtered_and_virtual_raw_in_the_same_job`, `initial_task_esc_at_and_ff_reach_the_virtual_printer_intact` | Printer.txt idêntico byte a byte ao anterior à fase A |
| T-015 | PARTIAL | `font_golden.rs::golden_portuguese_accents`, `golden_per_table_and_quality`; `font_rules.rs::portuguese_accents_exist_in_every_portuguese_table`; `lx310_tables.rs::*` | Mapeamentos conferidos; os glifos são aproximação própria (ADR 0008) |
| T-016 | PASS | `lx310_layout.rs::t016_one_thousand_lines_without_drift`, `t016_one_thousand_esc_j_and_ff_are_exact` | |
| T-017 | PASS | `apple2-printer/tests/load_pressure.rs::t017_*` (listagem de 100 e 300 Páginas com pontos em todas, 1 MiB de bitmap ESC K/L/*, limites de pontos e Páginas como diagnóstico, SHA-256 do raw), `spool_perf.rs` (cópias raras, primeiro traço P95 50 ms), `apple2-hostio/tests/printer_budget.rs` (fila cheia sem espera da CPU; `io_write` virtual+arquivo 1,0x o legado), `apple2-rust` `bridge_tests` (contrapressão) | Números e máquina em [desempenho.md](desempenho.md). Limite por Página (2 M pontos) e por Trabalho (64 M); pior caso de memória 768 MiB documentado |
| T-018 | PASS (sem janela; Linux nativo com janela) | `spool_flow.rs::t018_spool_alone_keeps_everything`, `threaded_service_with_ui_closed`; `apple2-rust/tests/printer_window.rs::opening_and_closing_never_needs_a_gpu` | Linux nativo (Wayland, Vulkan): Central fechada com 3 Páginas, reaberta com 8, Arquivo com 8 Páginas e 14.941 bytes, sem diagnóstico (detalhes em [manual-tests/impressora.md](../conformance/manual-tests/impressora.md)). Windows e macOS sem evidência |
| T-019 | PASS | `spool_flow.rs::t019_close_reasons_and_no_mixing`, `two_slots_interleaved_make_two_complete_jobs`, `preview_focus_follows_the_job_that_prints`; `apple2-rust/tests/printer_capture.rs::t019_state_load_and_machine_rebuild_close_the_job_with_their_reasons`, `two_cards_printing_alternately_never_mix_their_jobs`; `printer_virtual.rs::ca06_*` | Um Trabalho ativo por slot, cada um com seu motor: dois Trabalhos completos, sem Reset espúrio (Reset só se o mesmo slot abre sem fechar). A prévia segue o último aberto e só muda para outro se o seguido ficou 500 ms calado |
| T-020 | PASS | `printer_capture.rs::t019_*` (salva e carrega `.aws.yaml`, imprime de novo); `apple2-savestate/tests/cards_hostio.rs` | Lacuna: conteúdo do Printer.txt depois do load não é conferido |
| T-021 | PASS | `archive_recovery.rs::t021_crash_recovers_interrupted_truncated_to_checkpoint`, `recovery_never_invents_bytes_when_raw_is_shorter_than_checkpoint` | |
| T-022 | PASS | `pdf_export.rs::in_progress_document_exports_consistently`, `twenty_five_pages_are_valid_and_small`, `double_width_text_stays_searchable_as_whole_words`; `pdf_raster.rs::golden_*` | Texto em largura dupla agora é pesquisável ("Largura dupla", "APPLE II-RUST") |
| T-023 | PASS | `archive_faults.rs::t023_*`; `pdf_files.rs::never_overwrites_without_the_flag`, `odd_file_names_work`, `failure_leaves_nothing_behind` | |
| T-024 | N/A | cobertura parcial: `spool_flow.rs::reprint_adds_version_and_keeps_raw_sha` | Perfil FX-80 fora do escopo; o reprocessamento LX-310 para LX-310 está coberto |
| T-025 | BLOCKED | só `apple2-rust/tests/printer_window.rs::keys_of_the_secondary_window_never_reach_the_emulator` (roteamento) | Sem evidência humana; roteiro datável em [manual-tests/impressora.md](../conformance/manual-tests/impressora.md). Latência ponta a ponta medida no Linux: P95 199 ms |
| T-026 | PASS | toda a suíte de `apple2-printer` e o exemplo `imprimir`, sem GPU nem desktop | |
| T-027 | PASS | `escp_fuzz.rs::t027_ten_thousand_random_streams`, `t027_pathological_streams_stay_bounded` | |
| T-028 | PASS | `lx310_layout.rs::t028_reset_restores_format_without_erasing_the_document` | |
| T-029 | PASS | `escp_parser.rs::t029_esc_d_without_nul_applies_tabs_and_warns`; `lx310_layout.rs::t029_*` | |
| T-030 | N/A | – | Perfil MX-80 fora do escopo |

## Placar

PASS: 25. PARTIAL: 2 (T-007 e T-015; no parecer eram PASS com ressalva). FAIL: 0. BLOCKED: 1 (T-025). N/A: 2 (T-024 e T-030). Total 30.

Evidência que falta para passar de PASS a aceite: latência P95 da prévia no Windows e no macOS (no Linux, P95 199 ms até a tela), T-025 nativo, aceite com AppleWorks, Print Shop e dBase II ([aceite.md](aceite.md); BLOCKED sem discos), Windows e macOS.
