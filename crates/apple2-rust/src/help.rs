//! In-app help (AppleWin opens its CHM on F1): the user manual from `docs/manual/*.md`,
//! embedded with `include_str!`, rendered with egui.
//!
//! The Markdown understood is a small subset, enough for the manual: `#` headings, paragraphs,
//! `-`/`*` and `1.` lists, fenced code blocks, pipe tables (drawn as aligned monospace text) and
//! the inline spans `**bold**`, `` `code` `` and `[text](url)` (the link target is dropped).
//! [`parse`], [`toc`] and [`search`] are pure and unit-tested; [`HelpViewer`] is the egui part.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, Context, FontId, RichText, ScrollArea, TextStyle, Ui};

/// The manual, in reading order. The first line of every file is its `#` title.
pub const MANUAL: &[&str] = &[
    include_str!("../../../docs/manual/00-indice.md"),
    include_str!("../../../docs/manual/01-introducao.md"),
    include_str!("../../../docs/manual/02-primeiros-passos.md"),
    include_str!("../../../docs/manual/03-teclado-e-atalhos.md"),
    include_str!("../../../docs/manual/04-joystick-e-mouse.md"),
    include_str!("../../../docs/manual/05-imagens-de-disco.md"),
    include_str!("../../../docs/manual/06-cartas-por-slot.md"),
    include_str!("../../../docs/manual/07-configuracao.md"),
    include_str!("../../../docs/manual/08-linha-de-comando.md"),
    include_str!("../../../docs/manual/09-save-states.md"),
    include_str!("../../../docs/manual/10-depurador.md"),
    include_str!("../../../docs/manual/11-solucao-de-problemas.md"),
    include_str!("../../../docs/manual/12-diferencas-em-relacao-ao-applewin.md"),
    include_str!("../../../docs/manual/13-creditos-e-licenca.md"),
    include_str!("../../../docs/manual/14-laboratorio.md"),
    include_str!("../../../docs/manual/15-depuracao-dap-vscode.md"),
    include_str!("../../../docs/manual/16-depuracao-temporal.md"),
    include_str!("../../../docs/manual/17-impressora-virtual.md"),
];

/// A block of the Markdown subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(String),
    List {
        ordered: bool,
        items: Vec<String>,
    },
    /// Fenced code, verbatim.
    Code(String),
    /// Table rows already aligned into monospace lines (header, rule, rows).
    Table(Vec<String>),
}

/// One parsed page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doc {
    pub title: String,
    pub blocks: Vec<Block>,
}

/// An inline run of text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub code: bool,
}

/// Parses `**bold**`, `` `code` `` and `[text](url)` (shown as `text`).
pub fn parse_inline(s: &str) -> Vec<Span> {
    let chars: Vec<char> = s.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let flush = |cur: &mut String, spans: &mut Vec<Span>| {
        if !cur.is_empty() {
            spans.push(Span {
                text: std::mem::take(cur),
                bold: false,
                code: false,
            });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '`') {
                flush(&mut cur, &mut spans);
                spans.push(Span {
                    text: chars[i + 1..end].iter().collect(),
                    bold: false,
                    code: true,
                });
                i = end + 1;
                continue;
            }
        } else if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = (i + 2..chars.len().saturating_sub(1))
                .find(|&j| chars[j] == '*' && chars[j + 1] == '*')
            {
                flush(&mut cur, &mut spans);
                spans.push(Span {
                    text: chars[i + 2..end].iter().collect(),
                    bold: true,
                    code: false,
                });
                i = end + 2;
                continue;
            }
        } else if c == '[' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(paren) = (close + 2..chars.len()).find(|&j| chars[j] == ')') {
                        cur.extend(&chars[i + 1..close]);
                        i = paren + 1;
                        continue;
                    }
                }
            }
        }
        cur.push(c);
        i += 1;
    }
    flush(&mut cur, &mut spans);
    spans
}

/// The text of an inline string without its markers.
pub fn plain(s: &str) -> String {
    parse_inline(s).into_iter().map(|sp| sp.text).collect()
}

fn heading_level(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some((hashes as u8, line[hashes..].trim()))
    } else {
        None
    }
}

/// `- item`, `* item` or `12. item`: (ordered, indent, text).
fn list_item(line: &str) -> Option<(bool, usize, &str)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some((false, indent, rest.trim()));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return Some((true, indent, t[digits + 2..].trim()));
    }
    None
}

fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| plain(c.trim())).collect()
}

fn is_rule_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn render_table(rows: &[String]) -> Vec<String> {
    let parsed: Vec<Vec<String>> = rows.iter().map(|r| split_row(r)).collect();
    let body: Vec<&Vec<String>> = parsed.iter().filter(|r| !is_rule_row(r)).collect();
    let cols = body.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for r in &body {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let line = |r: &Vec<String>| -> String {
        let mut out = String::new();
        for (i, w) in widths.iter().enumerate() {
            let cell = r.get(i).map(String::as_str).unwrap_or("");
            out.push_str(cell);
            if i + 1 < cols {
                out.extend(std::iter::repeat_n(' ', w - cell.chars().count() + 2));
            }
        }
        out.trim_end().to_string()
    };
    let mut lines = Vec::new();
    for (n, r) in body.iter().enumerate() {
        lines.push(line(r));
        if n == 0 && body.len() > 1 {
            lines.push("-".repeat(lines[0].chars().count()));
        }
    }
    lines
}

/// Parses a Markdown page. The title is the first level-1 heading (or `"?"`).
pub fn parse(md: &str) -> Doc {
    let mut blocks: Vec<Block> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut items: Vec<String> = Vec::new();
    let mut list_ordered = false;
    let mut table: Vec<String> = Vec::new();
    let mut code: Option<Vec<String>> = None;

    fn end_para(para: &mut Vec<String>, blocks: &mut Vec<Block>) {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(para.join(" ")));
            para.clear();
        }
    }
    fn end_list(items: &mut Vec<String>, ordered: bool, blocks: &mut Vec<Block>) {
        if !items.is_empty() {
            blocks.push(Block::List {
                ordered,
                items: std::mem::take(items),
            });
        }
    }
    fn end_table(table: &mut Vec<String>, blocks: &mut Vec<Block>) {
        if !table.is_empty() {
            blocks.push(Block::Table(render_table(table)));
            table.clear();
        }
    }

    for raw in md.lines() {
        let line = raw.trim_end();
        if let Some(buf) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                blocks.push(Block::Code(buf.join("\n")));
                code = None;
            } else {
                buf.push(line.to_string());
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            end_para(&mut para, &mut blocks);
            end_list(&mut items, list_ordered, &mut blocks);
            end_table(&mut table, &mut blocks);
            code = Some(Vec::new());
            continue;
        }
        if trimmed.is_empty() {
            end_para(&mut para, &mut blocks);
            end_list(&mut items, list_ordered, &mut blocks);
            end_table(&mut table, &mut blocks);
            continue;
        }
        if let Some((level, text)) = heading_level(line) {
            end_para(&mut para, &mut blocks);
            end_list(&mut items, list_ordered, &mut blocks);
            end_table(&mut table, &mut blocks);
            blocks.push(Block::Heading {
                level,
                text: plain(text),
            });
            continue;
        }
        if trimmed.starts_with('|') {
            end_para(&mut para, &mut blocks);
            end_list(&mut items, list_ordered, &mut blocks);
            table.push(line.to_string());
            continue;
        }
        if let Some((ordered, indent, text)) = list_item(line) {
            end_para(&mut para, &mut blocks);
            end_table(&mut table, &mut blocks);
            if !items.is_empty() && ordered != list_ordered && indent == 0 {
                end_list(&mut items, list_ordered, &mut blocks);
            }
            list_ordered = ordered;
            let prefix = if indent >= 2 { "  " } else { "" };
            items.push(format!("{prefix}{text}"));
            continue;
        }
        if !items.is_empty() && raw.starts_with(' ') {
            // continuation of the previous list item
            if let Some(last) = items.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
            }
            continue;
        }
        end_list(&mut items, list_ordered, &mut blocks);
        end_table(&mut table, &mut blocks);
        para.push(trimmed.to_string());
    }
    if let Some(buf) = code {
        blocks.push(Block::Code(buf.join("\n"))); // unterminated fence
    }
    end_para(&mut para, &mut blocks);
    end_list(&mut items, list_ordered, &mut blocks);
    end_table(&mut table, &mut blocks);

    let title = blocks
        .iter()
        .find_map(|b| match b {
            Block::Heading { level: 1, text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "?".to_string());
    Doc { title, blocks }
}

/// Table of contents of a page: (block index, level, text) of every heading.
pub fn toc(doc: &Doc) -> Vec<(usize, u8, &str)> {
    doc.blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match b {
            Block::Heading { level, text } => Some((i, *level, text.as_str())),
            _ => None,
        })
        .collect()
}

/// Lower-case, accent-free, one output char per input char.
pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| {
            let c = c.to_lowercase().next().unwrap_or(c);
            match c {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                other => other,
            }
        })
        .collect()
}

fn block_text(b: &Block) -> String {
    match b {
        Block::Heading { text, .. } => text.clone(),
        Block::Paragraph(t) => plain(t),
        Block::List { items, .. } => items
            .iter()
            .map(|i| plain(i))
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Code(t) => t.clone(),
        Block::Table(rows) => rows.join("\n"),
    }
}

/// A search result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub page: usize,
    pub block: usize,
    pub snippet: String,
}

/// Case- and accent-insensitive search. Every word of `query` must occur in the same block.
pub fn search(docs: &[Doc], query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    let mut hits = Vec::new();
    if words.is_empty() {
        return hits;
    }
    for (page, doc) in docs.iter().enumerate() {
        for (block, b) in doc.blocks.iter().enumerate() {
            let text = block_text(b);
            let folded = fold(&text);
            if !words.iter().all(|w| folded.contains(w.as_str())) {
                continue;
            }
            // Folding keeps the char count, so a char index in `folded` is valid in `text`.
            let byte = folded.find(words[0].as_str()).unwrap_or(0);
            let at = folded[..byte].chars().count();
            let chars: Vec<char> = text.chars().collect();
            let start = at.saturating_sub(30);
            let end = (at + 60).min(chars.len());
            let mut snippet: String = chars[start..end]
                .iter()
                .map(|&c| if c == '\n' { ' ' } else { c })
                .collect();
            if start > 0 {
                snippet.insert(0, '…');
            }
            if end < chars.len() {
                snippet.push('…');
            }
            hits.push(Hit {
                page,
                block,
                snippet,
            });
            if hits.len() >= limit {
                return hits;
            }
        }
    }
    hits
}

/// The egui help window.
pub struct HelpViewer {
    pub open: bool,
    docs: Vec<Doc>,
    page: usize,
    query: String,
    scroll_to: Option<usize>,
}

impl Default for HelpViewer {
    fn default() -> Self {
        HelpViewer::new()
    }
}

impl HelpViewer {
    pub fn new() -> Self {
        HelpViewer {
            open: false,
            docs: MANUAL.iter().map(|m| parse(m)).collect(),
            page: 0,
            query: String::new(),
            scroll_to: None,
        }
    }

    /// F1: opens the manual, or closes it when it is already open.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn docs(&self) -> &[Doc] {
        &self.docs
    }

    fn go(&mut self, page: usize, block: Option<usize>) {
        self.page = page.min(self.docs.len().saturating_sub(1));
        self.scroll_to = block;
    }

    pub fn draw(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.wants_keyboard_input() {
            self.open = false;
            return;
        }
        let mut open = true;
        egui::Window::new("Ajuda - Manual do AppleII-Rust")
            .open(&mut open)
            .default_size([860.0, 560.0])
            .show(ctx, |ui| {
                let height = ui.available_height().max(200.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(230.0);
                        self.left_column(ui, height);
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        self.content(ui, height);
                    });
                });
            });
        if !open {
            self.open = false;
        }
    }

    fn left_column(&mut self, ui: &mut Ui, height: f32) {
        ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Buscar no manual...")
                .desired_width(f32::INFINITY),
        );
        let mut jump: Option<(usize, Option<usize>)> = None;
        ScrollArea::vertical()
            .id_salt("help_toc")
            .max_height(height - 30.0)
            .auto_shrink(false)
            .show(ui, |ui| {
                if self.query.trim().is_empty() {
                    for (p, doc) in self.docs.iter().enumerate() {
                        if ui.selectable_label(p == self.page, &doc.title).clicked() {
                            jump = Some((p, Some(0)));
                        }
                        if p == self.page {
                            for (i, level, text) in toc(doc).into_iter().filter(|(_, l, _)| *l >= 2)
                            {
                                let indent = f32::from(level - 1) * 10.0;
                                ui.horizontal(|ui| {
                                    ui.add_space(indent);
                                    if ui.small_button(text).clicked() {
                                        jump = Some((p, Some(i)));
                                    }
                                });
                            }
                        }
                    }
                } else {
                    let hits = search(&self.docs, &self.query, 60);
                    ui.label(format!("{} resultado(s)", hits.len()));
                    for h in hits {
                        ui.label(RichText::new(&self.docs[h.page].title).strong());
                        if ui.link(&h.snippet).clicked() {
                            jump = Some((h.page, Some(h.block)));
                        }
                        ui.add_space(4.0);
                    }
                }
            });
        if let Some((p, b)) = jump {
            self.go(p, b);
        }
    }

    fn content(&mut self, ui: &mut Ui, height: f32) {
        let Some(doc) = self.docs.get(self.page) else {
            return;
        };
        let target = self.scroll_to.take();
        let (body_font, mono) = (
            TextStyle::Body.resolve(ui.style()),
            TextStyle::Monospace.resolve(ui.style()),
        );
        let text_color = ui.visuals().text_color();
        let strong = ui.visuals().strong_text_color();
        let code_bg = ui.visuals().code_bg_color;
        ScrollArea::both()
            .id_salt(("help_body", self.page))
            .max_height(height)
            .auto_shrink(false)
            .show(ui, |ui| {
                for (i, block) in doc.blocks.iter().enumerate() {
                    let resp = match block {
                        Block::Heading { level, text } => {
                            let size = match level {
                                1 => 26.0,
                                2 => 20.0,
                                _ => 16.0,
                            };
                            ui.add_space(if *level == 1 { 0.0 } else { 8.0 });
                            let r = ui.label(RichText::new(text).size(size).strong());
                            if *level <= 2 {
                                ui.separator();
                            }
                            r
                        }
                        Block::Paragraph(t) => {
                            ui.label(layout(t, &body_font, &mono, text_color, strong, code_bg))
                        }
                        Block::List { ordered, items } => {
                            ui.vertical(|ui| {
                                for (n, item) in items.iter().enumerate() {
                                    let marker = if *ordered {
                                        format!("{}.", n + 1)
                                    } else {
                                        "•".to_string()
                                    };
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(marker);
                                        ui.label(layout(
                                            item.trim_start(),
                                            &body_font,
                                            &mono,
                                            text_color,
                                            strong,
                                            code_bg,
                                        ));
                                    });
                                }
                            })
                            .response
                        }
                        Block::Code(t) => mono_block(ui, t, code_bg),
                        Block::Table(rows) => mono_block(ui, &rows.join("\n"), code_bg),
                    };
                    if target == Some(i) {
                        resp.scroll_to_me(Some(Align::TOP));
                    }
                    ui.add_space(4.0);
                }
            });
    }
}

fn mono_block(ui: &mut Ui, text: &str, bg: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(bg)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).monospace())
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
        })
        .response
}

fn layout(
    text: &str,
    body: &FontId,
    mono: &FontId,
    color: Color32,
    strong: Color32,
    code_bg: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    for s in parse_inline(text) {
        let format = if s.code {
            TextFormat {
                font_id: mono.clone(),
                color,
                background: code_bg,
                ..Default::default()
            }
        } else if s.bold {
            TextFormat {
                font_id: body.clone(),
                color: strong,
                ..Default::default()
            }
        } else {
            TextFormat {
                font_id: body.clone(),
                color,
                ..Default::default()
            }
        };
        job.append(&s.text, 0.0, format);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_paragraphs() {
        let d = parse("# Título\n\nprimeira linha\nsegunda linha\n\n## Seção\ntexto");
        assert_eq!(d.title, "Título");
        assert_eq!(
            d.blocks,
            vec![
                Block::Heading {
                    level: 1,
                    text: "Título".into()
                },
                Block::Paragraph("primeira linha segunda linha".into()),
                Block::Heading {
                    level: 2,
                    text: "Seção".into()
                },
                Block::Paragraph("texto".into()),
            ]
        );
    }

    #[test]
    fn hash_without_space_is_not_a_heading() {
        let d = parse("#nao\n\n####### sete");
        assert!(d.blocks.iter().all(|b| matches!(b, Block::Paragraph(_))));
    }

    #[test]
    fn lists_ordered_and_bullets() {
        let d = parse("- um\n- dois\n  continua\n\n1. a\n2. b");
        assert_eq!(
            d.blocks,
            vec![
                Block::List {
                    ordered: false,
                    items: vec!["um".into(), "dois continua".into()]
                },
                Block::List {
                    ordered: true,
                    items: vec!["a".into(), "b".into()]
                },
            ]
        );
    }

    #[test]
    fn nested_bullet_is_indented_item() {
        let d = parse("- pai\n  - filho");
        assert_eq!(
            d.blocks,
            vec![Block::List {
                ordered: false,
                items: vec!["pai".into(), "  filho".into()]
            }]
        );
    }

    #[test]
    fn code_fence_is_verbatim() {
        let d = parse("```\n# nao e titulo\n- nem lista\n```\ndepois");
        assert_eq!(
            d.blocks[0],
            Block::Code("# nao e titulo\n- nem lista".into())
        );
        assert_eq!(d.blocks[1], Block::Paragraph("depois".into()));
    }

    #[test]
    fn unterminated_fence_keeps_content() {
        let d = parse("```\nabc");
        assert_eq!(d.blocks, vec![Block::Code("abc".into())]);
    }

    #[test]
    fn tables_are_aligned_monospace() {
        let d = parse("| Tecla | Ação |\n|---|---|\n| F1 | `Ajuda` |\n| Ctrl+F2 | **Reset** |");
        assert_eq!(
            d.blocks,
            vec![Block::Table(vec![
                "Tecla    Ação".to_string(),
                "-------------".to_string(),
                "F1       Ajuda".to_string(),
                "Ctrl+F2  Reset".to_string(),
            ])]
        );
    }

    #[test]
    fn table_ends_at_blank_line() {
        let d = parse("| a |\n|---|\n| b |\n\ntexto");
        assert_eq!(d.blocks.len(), 2);
        assert!(matches!(d.blocks[0], Block::Table(_)));
        assert_eq!(d.blocks[1], Block::Paragraph("texto".into()));
    }

    #[test]
    fn inline_spans() {
        let s = parse_inline("use **F7** ou `G` no [manual](http://x)");
        assert_eq!(
            s,
            vec![
                Span {
                    text: "use ".into(),
                    bold: false,
                    code: false
                },
                Span {
                    text: "F7".into(),
                    bold: true,
                    code: false
                },
                Span {
                    text: " ou ".into(),
                    bold: false,
                    code: false
                },
                Span {
                    text: "G".into(),
                    bold: false,
                    code: true
                },
                Span {
                    text: " no manual".into(),
                    bold: false,
                    code: false
                },
            ]
        );
    }

    #[test]
    fn unbalanced_markers_stay_literal() {
        assert_eq!(plain("a ** b ` c [d"), "a ** b ` c [d");
        assert_eq!(plain("2 * 3 * 4"), "2 * 3 * 4");
    }

    #[test]
    fn toc_lists_headings_with_block_index() {
        let d = parse("# A\ntexto\n\n## B\n\n### C");
        assert_eq!(toc(&d), vec![(0, 1, "A"), (2, 2, "B"), (3, 3, "C")]);
    }

    #[test]
    fn search_ignores_case_and_accents() {
        let docs = vec![
            parse("# Doc\n\nO Depurador entra com F7."),
            parse("# Outro\n\nnada aqui"),
        ];
        let hits = search(&docs, "DÉPURADOR", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].page, hits[0].block), (0, 1));
        assert!(hits[0].snippet.contains("Depurador"));
    }

    #[test]
    fn search_requires_all_words_in_one_block() {
        let docs = vec![parse("# D\n\nalfa beta\n\nalfa\n\nbeta")];
        let hits = search(&docs, "alfa beta", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block, 1);
    }

    #[test]
    fn search_empty_query_and_limit() {
        let docs = vec![parse("# D\n\nx\n\nx\n\nx")];
        assert!(search(&docs, "   ", 10).is_empty());
        assert_eq!(search(&docs, "x", 2).len(), 2);
    }

    #[test]
    fn embedded_manual_parses_and_is_searchable() {
        let docs: Vec<Doc> = MANUAL.iter().map(|m| parse(m)).collect();
        assert_eq!(docs.len(), 18);
        for d in &docs {
            assert_ne!(d.title, "?", "every page starts with a # title");
            assert!(d.blocks.len() > 2);
        }
        let hits = search(&docs, "scroll lock", 50);
        assert!(hits
            .iter()
            .any(|h| docs[h.page].title == "Teclado e atalhos"));
        let cmd = search(&docs, "headless-frames", 5);
        assert!(!cmd.is_empty());
    }

    #[test]
    fn viewer_toggles_and_jumps() {
        let mut v = HelpViewer::new();
        assert!(!v.open);
        v.toggle();
        assert!(v.open);
        v.go(3, Some(2));
        assert_eq!((v.page, v.scroll_to), (3, Some(2)));
        v.go(999, None);
        assert_eq!(v.page, v.docs().len() - 1);
        v.toggle();
        assert!(!v.open);
    }
}
