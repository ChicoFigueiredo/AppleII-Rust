//! The Central de Impressoras' controls (HU-IMP-031): status, setup, action buttons, job list,
//! Arquivo de impressões tab and the confirmation / PDF dialogs. Pure egui over a
//! [`PrinterViewModel`]; every click becomes a [`CenterEvent`] for the integration layer.

use crate::printer_model::{
    CenterEvent, JobEntry, JobState, PageRange, PdfRequest, PrintSetup, PrinterViewModel, Status,
};
use apple2_printer::font::CharTable;
use apple2_printer::paper::{InkColor, Paper, PaperKind, RenderOptions, Ribbon};
use egui::{Color32, Ui};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Preview,
    Archive,
}

/// The "Salvar PDF" form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfForm {
    pub job: String,
    pub version: u32,
    pub pages: usize,
    pub all: bool,
    pub first: usize,
    pub last: usize,
    pub cover: bool,
    pub title: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    Cancel,
    Delete { job: String, title: String },
}

/// Title / note being edited for one archived job.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Edit {
    job: String,
    title: String,
    note: String,
}

/// State of the Central that is not part of the view model (survives closing the window).
#[derive(Clone, Debug, Default)]
pub struct CenterUi {
    pub tab: Tab,
    pub pdf: Option<PdfForm>,
    pub confirm: Option<Confirm>,
    edit: Option<Edit>,
    /// Options offered by "Reimprimir".
    reprint: Option<RenderOptions>,
}

pub fn paper_label(k: PaperKind) -> &'static str {
    match k {
        PaperKind::GreenBar => "Formulário zebrado verde",
        PaperKind::BlueBar => "Formulário zebrado azul",
        PaperKind::PlainContinuous => "Contínuo liso",
        PaperKind::A4 => "A4",
        PaperKind::Letter => "Carta",
    }
}

pub fn ribbon_label(r: Ribbon) -> &'static str {
    match r {
        Ribbon::New => "Nova",
        Ribbon::Worn => "Usada",
    }
}

pub fn ink_label(i: InkColor) -> &'static str {
    match i {
        InkColor::Black => "Preto",
        InkColor::Blue => "Azul",
        InkColor::Purple => "Roxo",
    }
}

pub const ALL_TABLES: [CharTable; 6] = [
    CharTable::Pc437,
    CharTable::Pc850,
    CharTable::Pc860,
    CharTable::Brascii,
    CharTable::Abicomp,
    CharTable::Italic,
];

pub fn table_label(t: CharTable) -> &'static str {
    match t {
        CharTable::Pc437 => "PC437",
        CharTable::Pc850 => "PC850",
        CharTable::Pc860 => "PC860",
        CharTable::Brascii => "BRASCII",
        CharTable::Abicomp => "ABICOMP",
        CharTable::Italic => "Itálico",
    }
}

pub const COUNTRIES: [&str; 13] = [
    "EUA",
    "França",
    "Alemanha",
    "Reino Unido",
    "Dinamarca I",
    "Suécia",
    "Itália",
    "Espanha I",
    "Japão",
    "Noruega",
    "Dinamarca II",
    "Espanha II",
    "América Latina",
];

pub const ALL_PAPERS: [PaperKind; 5] = [
    PaperKind::GreenBar,
    PaperKind::BlueBar,
    PaperKind::PlainContinuous,
    PaperKind::A4,
    PaperKind::Letter,
];
const ALL_INKS: [InkColor; 3] = [InkColor::Black, InkColor::Blue, InkColor::Purple];
const ALL_RIBBONS: [Ribbon; 2] = [Ribbon::New, Ribbon::Worn];

fn status_color(s: Status) -> Color32 {
    match s {
        Status::Offline => Color32::GRAY,
        Status::Online => Color32::from_rgb(60, 160, 80),
        Status::Receiving => Color32::from_rgb(230, 150, 20),
        Status::Waiting => Color32::from_rgb(200, 180, 40),
        Status::Finished => Color32::from_rgb(60, 130, 210),
        Status::Error => Color32::from_rgb(220, 60, 60),
    }
}

/// Status line: state, Apple II, slot and profile.
pub fn status_bar(ui: &mut Ui, vm: &PrinterViewModel) {
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(r.center(), 5.0, status_color(vm.status));
        ui.strong(format!("Impressora {}", vm.status.label()));
        if !vm.detail.is_empty() {
            ui.label(format!("— {}", vm.detail));
        }
        ui.separator();
        match vm.slot {
            Some(slot) => {
                ui.label(format!("Apple II: {}, slot {slot}", vm.machine));
            }
            None => {
                ui.label("Sem placa Generic Printer enviando à impressora virtual");
            }
        }
        ui.separator();
        ui.label(format!("Perfil: {}", vm.profile_or_default()));
        if let Some(j) = vm.selected().filter(|j| !j.transport.is_empty()) {
            ui.separator();
            ui.label(format!("Transporte do trabalho: {}", j.transport));
        }
    });
}

impl PrinterViewModel {
    fn profile_or_default(&self) -> &str {
        if self.profile.is_empty() {
            "Epson LX-310"
        } else {
            &self.profile
        }
    }
}

/// Paper, ribbon, colour and table for the next job. Returns the new setup when it changed.
pub fn setup_bar(ui: &mut Ui, vm: &PrinterViewModel) -> Option<PrintSetup> {
    let mut s = vm.setup;
    let enabled = vm.slot.is_some();
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Papel:");
            egui::ComboBox::from_id_salt("printer-paper")
                .selected_text(paper_label(s.options.paper.kind))
                .show_ui(ui, |ui| {
                    for k in ALL_PAPERS {
                        if ui
                            .selectable_label(s.options.paper.kind == k, paper_label(k))
                            .clicked()
                        {
                            s.options.paper = Paper::preset(k);
                        }
                    }
                });
            ui.label("Fita:");
            egui::ComboBox::from_id_salt("printer-ribbon")
                .selected_text(ribbon_label(s.options.ribbon))
                .show_ui(ui, |ui| {
                    for r in ALL_RIBBONS {
                        ui.selectable_value(&mut s.options.ribbon, r, ribbon_label(r));
                    }
                });
            ui.label("Cor:");
            egui::ComboBox::from_id_salt("printer-ink")
                .selected_text(ink_label(s.options.ink))
                .show_ui(ui, |ui| {
                    for i in ALL_INKS {
                        ui.selectable_value(&mut s.options.ink, i, ink_label(i));
                    }
                });
            ui.label("Tabela:");
            egui::ComboBox::from_id_salt("printer-table")
                .selected_text(table_label(s.options.table))
                .show_ui(ui, |ui| {
                    for t in ALL_TABLES {
                        ui.selectable_value(&mut s.options.table, t, table_label(t));
                    }
                });
            ui.label("País:");
            egui::ComboBox::from_id_salt("printer-country")
                .selected_text(COUNTRIES[usize::from(s.options.country.min(12))])
                .show_ui(ui, |ui| {
                    for (i, name) in COUNTRIES.iter().enumerate() {
                        ui.selectable_value(&mut s.options.country, i as u8, *name);
                    }
                });
        });
    })
    .response
    .on_disabled_hover_text("Insira uma placa Generic Printer (impressora virtual) num slot.");
    ui.weak(
        "Papel, fita, cor e tabela valem para o próximo trabalho; o trabalho em andamento \
         continua com os que tinha ao começar.",
    );
    (s != vm.setup).then_some(s)
}

fn button(ui: &mut Ui, label: &str, enabled: bool, why_not: &str) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label))
        .on_disabled_hover_text(why_not)
        .clicked()
}

/// Save PDF / RAW, finish and cancel. Opens forms and confirmations in `ui_state`.
pub fn action_bar(
    ui: &mut Ui,
    vm: &PrinterViewModel,
    ui_state: &mut CenterUi,
    events: &mut Vec<CenterEvent>,
) {
    let shown = vm.shown.job.is_some();
    let pages = vm.document().pages.len();
    let active = vm.active_job.and_then(|i| vm.jobs.get(i));
    ui.horizontal_wrapped(|ui| {
        let why_pdf = if !shown {
            "Não há trabalho para salvar."
        } else if vm.shown.loading {
            "Aguarde: o trabalho está sendo lido do Arquivo."
        } else {
            "O trabalho ainda não tem páginas."
        };
        if button(
            ui,
            "Salvar PDF…",
            shown && pages > 0 && !vm.shown.loading,
            why_pdf,
        ) {
            if let (Some(job), Some(entry)) = (vm.shown.job.clone(), vm.selected()) {
                ui_state.pdf = Some(PdfForm {
                    job,
                    version: vm.shown.version,
                    pages,
                    all: true,
                    first: 1,
                    last: pages,
                    cover: false,
                    title: entry.title.clone(),
                    note: entry.note.clone(),
                });
            }
        }
        if button(ui, "Salvar RAW…", shown, "Não há trabalho para salvar.") {
            if let Some(job) = vm.shown.job.clone() {
                events.push(CenterEvent::SaveRaw { job });
            }
        }
        ui.separator();
        let why = "Não há trabalho em andamento.";
        if button(ui, "Concluir trabalho", active.is_some(), why) {
            events.push(CenterEvent::FinishJob);
        }
        if button(ui, "Cancelar trabalho", active.is_some(), why) {
            ui_state.confirm = Some(Confirm::Cancel);
        }
    });
}

/// Job list of the preview tab.
pub fn job_list(ui: &mut Ui, vm: &PrinterViewModel, events: &mut Vec<CenterEvent>) {
    ui.heading("Trabalhos");
    if vm.jobs.is_empty() {
        ui.weak("Nenhum trabalho de impressão.");
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, j) in vm.jobs.iter().enumerate() {
            let sel = vm.selected_job == Some(i);
            let text = format!("{} — {}", j.title, j.state.label());
            if ui.selectable_label(sel, text).clicked() && !sel {
                events.push(CenterEvent::SelectJob(j.id.clone()));
            }
        }
    });
}

fn options_text(o: &RenderOptions) -> String {
    format!(
        "{}, fita {}, {}",
        paper_label(o.paper.kind),
        ribbon_label(o.ribbon).to_lowercase(),
        ink_label(o.ink).to_lowercase()
    )
}

/// The "Arquivo de impressões" tab. Returns true when the user asked to look at a job (switch back
/// to the preview).
pub fn archive_tab(
    ui: &mut Ui,
    vm: &PrinterViewModel,
    ui_state: &mut CenterUi,
    events: &mut Vec<CenterEvent>,
) -> bool {
    let mut show_preview = false;
    ui.horizontal(|ui| {
        ui.heading("Arquivo de impressões");
        ui.weak(format!(
            "{} trabalho(s); nada é apagado sem o seu pedido",
            vm.jobs.len()
        ));
    });
    ui.separator();
    egui::TopBottomPanel::bottom("archive-detail")
        .resizable(true)
        .default_height(260.0)
        .show_inside(ui, |ui| {
            show_preview = archive_detail(ui, vm, ui_state, events);
        });
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("archive-grid")
                .striped(true)
                .num_columns(7)
                .show(ui, |ui| {
                    for h in [
                        "Título",
                        "Data",
                        "Máquina",
                        "Transporte",
                        "Páginas",
                        "Estado",
                        "Versões",
                    ] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for (i, j) in vm.jobs.iter().enumerate() {
                        let sel = vm.selected_job == Some(i);
                        if ui.selectable_label(sel, &j.title).clicked() && !sel {
                            events.push(CenterEvent::SelectJob(j.id.clone()));
                        }
                        ui.label(&j.started);
                        ui.label(if j.slot > 0 || !j.machine.is_empty() {
                            format!("{} (slot {})", j.machine, j.slot)
                        } else {
                            "—".into()
                        });
                        ui.label(&j.transport);
                        ui.label(j.pages.to_string());
                        ui.label(match &j.damaged {
                            Some(_) => "danificado".to_string(),
                            None => j.state.label().to_string(),
                        });
                        ui.label(j.versions.len().to_string());
                        ui.end_row();
                    }
                });
        });
    show_preview
}

fn archive_detail(
    ui: &mut Ui,
    vm: &PrinterViewModel,
    ui_state: &mut CenterUi,
    events: &mut Vec<CenterEvent>,
) -> bool {
    let mut show_preview = false;
    let Some(job) = vm.selected() else {
        ui.weak("Selecione um trabalho na lista.");
        return false;
    };
    if let Some(why) = &job.damaged {
        ui.colored_label(
            Color32::from_rgb(220, 60, 60),
            format!("Trabalho danificado: {why}"),
        );
        delete_button(ui, job, ui_state);
        return false;
    }
    // Edit buffers follow the selected job.
    if ui_state.edit.as_ref().is_none_or(|e| e.job != job.id) {
        ui_state.reprint = None;
        ui_state.edit = Some(Edit {
            job: job.id.clone(),
            title: job.title.clone(),
            note: job.note.clone(),
        });
    }
    let edit = ui_state.edit.as_mut().expect("edit buffer just created");
    ui.horizontal(|ui| {
        ui.label("Título:");
        ui.add(egui::TextEdit::singleline(&mut edit.title).desired_width(320.0));
        let changed = edit.title.trim() != job.title && !edit.title.trim().is_empty();
        if button(ui, "Salvar título", changed, "O título não foi alterado.") {
            events.push(CenterEvent::SetTitle {
                job: job.id.clone(),
                title: edit.title.trim().to_string(),
            });
        }
    });
    ui.horizontal(|ui| {
        ui.label("Nota:");
        ui.add(
            egui::TextEdit::multiline(&mut edit.note)
                .desired_rows(2)
                .desired_width(320.0),
        );
        if button(
            ui,
            "Salvar nota",
            edit.note != job.note,
            "A nota não foi alterada.",
        ) {
            events.push(CenterEvent::SetNote {
                job: job.id.clone(),
                note: edit.note.clone(),
            });
        }
    });
    ui.separator();
    ui.strong("Versões");
    if job.versions.is_empty() {
        ui.weak("Sem versões registradas ainda.");
    }
    for v in &job.versions {
        ui.horizontal_wrapped(|ui| {
            let cur = vm.shown.job.as_deref() == Some(job.id.as_str()) && vm.shown.version == v.id;
            ui.label(format!(
                "v{} — {} — {} página(s) — {}{}",
                v.id,
                options_text(&v.options),
                v.pages,
                v.created,
                if cur { " (na prévia)" } else { "" }
            ));
            if ui.small_button("Ver").clicked() {
                events.push(CenterEvent::SelectVersion {
                    job: job.id.clone(),
                    version: v.id,
                });
                show_preview = true;
            }
            if button(
                ui,
                "Exportar PDF…",
                !job.active && v.pages > 0,
                if job.active {
                    "O trabalho em andamento ainda não tem versão fechada."
                } else {
                    "Esta versão não tem páginas."
                },
            ) {
                ui_state.pdf = Some(PdfForm {
                    job: job.id.clone(),
                    version: v.id,
                    pages: v.pages as usize,
                    all: true,
                    first: 1,
                    last: (v.pages as usize).max(1),
                    cover: false,
                    title: job.title.clone(),
                    note: job.note.clone(),
                });
            }
        });
    }
    for (v, path) in &job.exports {
        ui.weak(format!(
            "PDF salvo nesta sessão (versão {}): {path}",
            (*v).max(1)
        ));
    }
    ui.separator();
    ui.strong("Reimprimir com outro papel, fita, cor, tabela ou país (gera uma nova versão)");
    let base = vm.shown.options.paper.kind;
    let opts = ui_state.reprint.get_or_insert_with(|| {
        let mut o = vm.shown.options;
        o.paper = Paper::preset(base);
        o
    });
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("reprint-paper")
            .selected_text(paper_label(opts.paper.kind))
            .show_ui(ui, |ui| {
                for k in ALL_PAPERS {
                    if ui
                        .selectable_label(opts.paper.kind == k, paper_label(k))
                        .clicked()
                    {
                        opts.paper = Paper::preset(k);
                    }
                }
            });
        egui::ComboBox::from_id_salt("reprint-ribbon")
            .selected_text(format!("Fita {}", ribbon_label(opts.ribbon).to_lowercase()))
            .show_ui(ui, |ui| {
                for r in ALL_RIBBONS {
                    ui.selectable_value(&mut opts.ribbon, r, ribbon_label(r));
                }
            });
        egui::ComboBox::from_id_salt("reprint-ink")
            .selected_text(ink_label(opts.ink))
            .show_ui(ui, |ui| {
                for i in ALL_INKS {
                    ui.selectable_value(&mut opts.ink, i, ink_label(i));
                }
            });
        egui::ComboBox::from_id_salt("reprint-table")
            .selected_text(format!("Tabela {}", table_label(opts.table)))
            .show_ui(ui, |ui| {
                for t in ALL_TABLES {
                    ui.selectable_value(&mut opts.table, t, table_label(t));
                }
            });
        egui::ComboBox::from_id_salt("reprint-country")
            .selected_text(format!(
                "País {}",
                COUNTRIES[usize::from(opts.country.min(12))]
            ))
            .show_ui(ui, |ui| {
                for (i, name) in COUNTRIES.iter().enumerate() {
                    ui.selectable_value(&mut opts.country, i as u8, *name);
                }
            });
        let ok = !job.active && job.state == JobState::Finished;
        if button(
            ui,
            "Reimprimir",
            ok,
            "O trabalho em andamento só pode ser reimpresso depois de concluído.",
        ) {
            events.push(CenterEvent::Reprint {
                job: job.id.clone(),
                options: *opts,
            });
            show_preview = true;
        }
    });
    ui.weak(
        "Tabela e país são as chaves da impressora para esta versão (o programa ainda pode trocá-los \
         por comandos); reimprimir não altera os bytes gravados.",
    );
    ui.separator();
    delete_button(ui, job, ui_state);
    show_preview
}

fn delete_button(ui: &mut Ui, job: &JobEntry, ui_state: &mut CenterUi) {
    if button(
        ui,
        "Apagar trabalho…",
        !job.active,
        "O trabalho em andamento não pode ser apagado; use Cancelar trabalho.",
    ) {
        ui_state.confirm = Some(Confirm::Delete {
            job: job.id.clone(),
            title: job.title.clone(),
        });
    }
}

/// Modal-like windows: confirmations and the PDF form. Events carry the confirmed actions.
pub fn dialogs(ctx: &egui::Context, ui_state: &mut CenterUi, events: &mut Vec<CenterEvent>) {
    if let Some(c) = ui_state.confirm.clone() {
        let (title, text, yes) = match &c {
            Confirm::Cancel => (
                "Cancelar trabalho",
                "Cancelar o trabalho em andamento? O que já foi impresso será apagado do Arquivo."
                    .to_string(),
                "Cancelar trabalho",
            ),
            Confirm::Delete { title, .. } => (
                "Apagar trabalho",
                format!("Apagar \"{title}\" do Arquivo de impressões? Isto não pode ser desfeito."),
                "Apagar",
            ),
        };
        let mut close = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button(yes).clicked() {
                        events.push(match &c {
                            Confirm::Cancel => CenterEvent::CancelJob,
                            Confirm::Delete { job, .. } => CenterEvent::DeleteJob(job.clone()),
                        });
                        close = true;
                    }
                    if ui.button("Voltar").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            ui_state.confirm = None;
        }
    }
    if let Some(mut f) = ui_state.pdf.clone() {
        let mut close = false;
        let mut export = false;
        egui::Window::new("Salvar PDF")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Título:");
                    ui.add(egui::TextEdit::singleline(&mut f.title).desired_width(280.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Nota:");
                    ui.add(
                        egui::TextEdit::multiline(&mut f.note)
                            .desired_rows(2)
                            .desired_width(280.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Páginas:");
                    ui.radio_value(&mut f.all, true, format!("todas ({})", f.pages));
                    ui.radio_value(&mut f.all, false, "intervalo");
                });
                if !f.all {
                    let max = f.pages.max(1);
                    ui.horizontal(|ui| {
                        ui.label("de");
                        ui.add(egui::DragValue::new(&mut f.first).range(1..=max));
                        ui.label("até");
                        ui.add(egui::DragValue::new(&mut f.last).range(1..=max));
                    });
                    if f.last < f.first {
                        f.last = f.first;
                    }
                }
                ui.checkbox(&mut f.cover, "Incluir folha de rosto (capa)");
                ui.weak("Título e nota também ficam no Arquivo de impressões.");
                ui.horizontal(|ui| {
                    if ui.button("Escolher arquivo…").clicked() {
                        export = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                });
            });
        if export {
            events.push(CenterEvent::ExportPdf(PdfRequest {
                job: f.job.clone(),
                version: f.version,
                range: if f.all {
                    PageRange::All
                } else {
                    PageRange::Pages(f.first, f.last)
                },
                cover: f.cover,
                title: f.title.trim().to_string(),
                note: f.note.clone(),
            }));
            ui_state.pdf = None;
        } else if close {
            ui_state.pdf = None;
        } else {
            ui_state.pdf = Some(f);
        }
    }
}

/// Last messages (saved files, errors) at the bottom of the window.
pub fn message_bar(ui: &mut Ui, vm: &PrinterViewModel) {
    if vm.messages.is_empty() {
        return;
    }
    for m in vm.messages.iter().rev().take(3).rev() {
        if m.error {
            ui.colored_label(Color32::from_rgb(220, 60, 60), &m.text);
        } else {
            ui.label(&m.text);
        }
    }
}
