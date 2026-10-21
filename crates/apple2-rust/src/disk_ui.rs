//! Disk dialogs: the Ctrl+F3 / Ctrl+F4 drive menu (insert, eject, write-protect, recent images,
//! create a new disk, swap) and the hard disk section of the same menu.
//!
//! Like the rest of `ui`, pure drawing: it reports [`UiEvent`]s. The new-disk contents come from
//! `apple2_storage::new_disk` (AppleWin's `New_DOSProDOS_Disk` / `New_Blank_Disk`).

use crate::actions::Action;
use crate::backend::DriveStatus;
use crate::ui::UiEvent;
use apple2_config::{Settings, SlotCardSpec};
use apple2_storage::new_disk::{new_blank_disk, new_dos33_disk, new_prodos_disk, ProDosFiles};
use egui::Context;
use std::path::{Path, PathBuf};

pub const MAX_RECENT: usize = 10;
const FLOPPY_SIZE: usize = 143_360;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewDiskKind {
    /// Zeroed data disk with AppleWin's "THIS IS AN EMPTY DATA DISK" boot sector.
    Blank,
    Dos33,
    ProDos,
}

impl NewDiskKind {
    pub const ALL: [NewDiskKind; 3] = [NewDiskKind::Blank, NewDiskKind::Dos33, NewDiskKind::ProDos];

    pub fn label(self) -> &'static str {
        match self {
            NewDiskKind::Blank => "Blank data disk",
            NewDiskKind::Dos33 => "DOS 3.3 system disk",
            NewDiskKind::ProDos => "ProDOS disk",
        }
    }

    /// File extension: DOS 3.3 is in DOS order, ProDOS in ProDOS order.
    pub fn extension(self) -> &'static str {
        match self {
            NewDiskKind::Blank | NewDiskKind::Dos33 => "dsk",
            NewDiskKind::ProDos => "po",
        }
    }

    /// Sizes offered (bytes, label). DOS 3.3 is a 140K floppy only.
    pub fn sizes(self) -> &'static [(usize, &'static str)] {
        match self {
            NewDiskKind::Dos33 => &[(FLOPPY_SIZE, "140K floppy")],
            _ => &[
                (FLOPPY_SIZE, "140K floppy"),
                (819_200, "800K (3.5\")"),
                (5 * 1024 * 1024, "5MB hard disk"),
                (32 * 1024 * 1024, "32MB hard disk"),
            ],
        }
    }
}

/// The bytes of a new disk image.
pub fn new_disk_bytes(kind: NewDiskKind, size: usize, files: ProDosFiles) -> Vec<u8> {
    match kind {
        NewDiskKind::Blank => new_blank_disk(size, size > 1024 * 1024),
        NewDiskKind::Dos33 => new_dos33_disk(size),
        NewDiskKind::ProDos => new_prodos_disk(size, files),
    }
}

/// Which files the Advanced page asks to copy onto new ProDOS disks.
pub fn prodos_files(s: &Settings) -> ProDosFiles {
    ProDosFiles {
        bitsy_boot: s.storage.new_disk_copy_bitsy_boot,
        bitsy_bye: s.storage.new_disk_copy_bitsy_bye,
        basic: s.storage.new_disk_copy_basic,
        prodos: s.storage.new_disk_copy_prodos,
    }
}

/// Creates the image file. Refuses to overwrite nothing silently: the save dialog already asked.
pub fn write_new_disk(
    path: &Path,
    kind: NewDiskKind,
    size: usize,
    files: ProDosFiles,
) -> std::io::Result<()> {
    std::fs::write(path, new_disk_bytes(kind, size, files))
}

/// Moves `path` to the front of the recent list (no duplicates, at most [`MAX_RECENT`]).
pub fn recent_push(list: &mut Vec<PathBuf>, path: &Path) {
    list.retain(|p| p != path);
    list.insert(0, path.to_path_buf());
    list.truncate(MAX_RECENT);
}

fn recent_file() -> Option<PathBuf> {
    apple2_config::store::config_dir().map(|d| d.join("recent_disks.txt"))
}

pub fn parse_recent(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(MAX_RECENT)
        .map(PathBuf::from)
        .collect()
}

pub fn format_recent(list: &[PathBuf]) -> String {
    list.iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

struct CreateState {
    kind: NewDiskKind,
    size: usize,
}

pub struct DiskUi {
    pub recent: Vec<PathBuf>,
    create: Option<CreateState>,
    /// Result of the last create attempt, shown in the menu.
    note: String,
}

impl Default for DiskUi {
    fn default() -> Self {
        let recent = recent_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| parse_recent(&t))
            .unwrap_or_default();
        DiskUi {
            recent,
            create: None,
            note: String::new(),
        }
    }
}

impl DiskUi {
    /// Remembers an inserted image (also called by the F3/F4 picker).
    pub fn note_inserted(&mut self, path: &Path) {
        recent_push(&mut self.recent, path);
        if let Some(f) = recent_file() {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(f, format_recent(&self.recent));
        }
    }

    /// Draws the menu of drive `*menu`; closes it by setting `*menu = None`.
    pub fn drive_menu(
        &mut self,
        ctx: &Context,
        menu: &mut Option<usize>,
        drives: &[DriveStatus; 2],
        settings: &Settings,
        ev: &mut Vec<UiEvent>,
    ) {
        let Some(d) = *menu else { return };
        let mut open = true;
        let mut close = false;
        let st = &drives[d];
        egui::Window::new(format!("Drive {}", d + 1))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                match &st.image {
                    Some(name) => ui.label(format!("Disk: {name}")),
                    None => ui.label("Disk: (empty)"),
                };
                if ui.button("Insert disk image...").clicked() {
                    ev.push(UiEvent::Action(Action::DriveSelect(d as u8)));
                    close = true;
                }
                if ui
                    .button("Insert disk image (read only)...")
                    .on_hover_text("Open as Read Only: the disk is mounted write-protected")
                    .clicked()
                {
                    ev.push(UiEvent::PickDiskReadOnly(d));
                    close = true;
                }
                ui.add_enabled_ui(st.image.is_some(), |ui| {
                    if ui.button("Eject").clicked() {
                        ev.push(UiEvent::EjectDisk(d));
                        close = true;
                    }
                    let mut wp = st.write_protected;
                    if ui.checkbox(&mut wp, "Write protected").changed() {
                        ev.push(UiEvent::SetWriteProtect { drive: d, on: wp });
                    }
                });
                if ui.button("Swap drives").clicked() {
                    ev.push(UiEvent::Action(Action::SwapDrives));
                    close = true;
                }
                ui.separator();
                ui.menu_button("Insert recent", |ui| {
                    if self.recent.is_empty() {
                        ui.label("(none)");
                    }
                    for p in self.recent.clone() {
                        let name = p.file_name().map_or_else(
                            || p.display().to_string(),
                            |n| n.to_string_lossy().into_owned(),
                        );
                        if ui
                            .button(name)
                            .on_hover_text(p.display().to_string())
                            .clicked()
                        {
                            ev.push(UiEvent::InsertDisk { drive: d, path: p });
                            ui.close_menu();
                            close = true;
                        }
                    }
                });
                ui.separator();
                self.create_ui(ui, d, settings, ev, &mut close);
                if !self.note.is_empty() {
                    ui.label(&self.note);
                }
                hdd_section(ui, settings, ev);
            });
        if !open || close {
            *menu = None;
            self.create = None;
        }
    }

    fn create_ui(
        &mut self,
        ui: &mut egui::Ui,
        d: usize,
        settings: &Settings,
        ev: &mut Vec<UiEvent>,
        close: &mut bool,
    ) {
        if self.create.is_none() {
            if ui.button("Create new disk...").clicked() {
                self.create = Some(CreateState {
                    kind: NewDiskKind::Blank,
                    size: FLOPPY_SIZE,
                });
            }
            return;
        }
        let Some(c) = self.create.as_mut() else {
            return;
        };
        ui.strong("Create new disk");
        for k in NewDiskKind::ALL {
            if ui.radio_value(&mut c.kind, k, k.label()).clicked()
                && !k.sizes().iter().any(|(s, _)| *s == c.size)
            {
                c.size = FLOPPY_SIZE;
            }
        }
        let sizes = c.kind.sizes();
        egui::ComboBox::from_id_salt("newdisksize")
            .selected_text(
                sizes
                    .iter()
                    .find(|(s, _)| *s == c.size)
                    .map_or("140K floppy", |(_, n)| *n),
            )
            .show_ui(ui, |ui| {
                for (s, n) in sizes {
                    ui.selectable_value(&mut c.size, *s, *n);
                }
            });
        let (kind, size) = (c.kind, c.size);
        ui.horizontal(|ui| {
            if ui.button("Create and insert").clicked() {
                let name = format!("NewDisk.{}", kind.extension());
                let mut dlg = rfd::FileDialog::new()
                    .set_title("New disk image")
                    .set_file_name(&name);
                if let Some(dir) = &settings.storage.disk_dir {
                    dlg = dlg.set_directory(dir);
                }
                if let Some(path) = dlg.save_file() {
                    match write_new_disk(&path, kind, size, prodos_files(settings)) {
                        Ok(()) => {
                            ev.push(UiEvent::InsertDisk { drive: d, path });
                            self.note.clear();
                            *close = true;
                        }
                        Err(e) => self.note = format!("Cannot create {}: {e}", path.display()),
                    }
                }
            }
            if ui.button("Cancel").clicked() {
                self.create = None;
            }
        });
    }
}

/// Hard disk controller units of the applied settings. Changing an image is hot (AppleWin
/// `HarddiskInterfaceCard::Insert`): it goes through `UiEvent::ApplyLive` and the machine keeps
/// running (see [`crate::hotswap`]).
fn hdd_section(ui: &mut egui::Ui, settings: &Settings, ev: &mut Vec<UiEvent>) {
    let Some((slot, images)) = settings
        .slots
        .0
        .iter()
        .enumerate()
        .find_map(|(i, c)| match c {
            SlotCardSpec::GenericHdd { images, .. } => Some((i, images)),
            _ => None,
        })
    else {
        return;
    };
    ui.separator();
    ui.strong(format!("Hard disks (slot {slot})"));
    for (u, img) in images.iter().enumerate() {
        ui.horizontal(|ui| {
            let name = img
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or("(empty)".to_string(), |n| n.to_string_lossy().into_owned());
            ui.label(format!("HDD {}: {name}", u + 1));
            if ui.small_button("Select...").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Hard disk image")
                    .add_filter(
                        "Hard disk images",
                        &["hdv", "po", "2mg", "2img", "gz", "zip", "dsk"],
                    )
                    .pick_file()
                {
                    ev.push(UiEvent::ApplyLive(Box::new(with_hdd(
                        settings,
                        slot,
                        u,
                        Some(p),
                    ))));
                }
            }
            if img.is_some() && ui.small_button("Unplug").clicked() {
                ev.push(UiEvent::ApplyLive(Box::new(with_hdd(
                    settings, slot, u, None,
                ))));
            }
        });
    }
}

/// `settings` with HDD `unit` of the controller in `slot` set to `image`.
pub fn with_hdd(settings: &Settings, slot: usize, unit: usize, image: Option<PathBuf>) -> Settings {
    let mut s = settings.clone();
    if let SlotCardSpec::GenericHdd { images, .. } = &mut s.slots.0[slot] {
        images[unit] = image;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_config::{CardType, SlotCardSpec};

    #[test]
    fn recent_list_is_mru_unique_and_capped() {
        let mut l = Vec::new();
        for i in 0..15 {
            recent_push(&mut l, Path::new(&format!("d{i}.dsk")));
        }
        assert_eq!(l.len(), MAX_RECENT);
        assert_eq!(l[0], Path::new("d14.dsk"));
        recent_push(&mut l, Path::new("d10.dsk"));
        assert_eq!(l[0], Path::new("d10.dsk"));
        assert_eq!(l.iter().filter(|p| *p == Path::new("d10.dsk")).count(), 1);
        assert_eq!(l.len(), MAX_RECENT);
    }

    #[test]
    fn recent_text_round_trips() {
        let l = vec![PathBuf::from("/a/b.dsk"), PathBuf::from("c d.po")];
        assert_eq!(parse_recent(&format_recent(&l)), l);
        assert!(parse_recent("\n \n").is_empty());
    }

    #[test]
    fn new_disks_have_the_expected_size_and_content() {
        let none = ProDosFiles::default();
        let blank = new_disk_bytes(NewDiskKind::Blank, FLOPPY_SIZE, none);
        assert_eq!(blank.len(), FLOPPY_SIZE);
        assert!(blank[..256].iter().any(|b| *b != 0), "boot sector present");
        let dos = new_disk_bytes(NewDiskKind::Dos33, FLOPPY_SIZE, none);
        assert_eq!(dos.len(), FLOPPY_SIZE);
        assert_eq!(dos[0x11 * 4096 + 3], 3, "VTOC says DOS 3.3");
        let po = new_disk_bytes(NewDiskKind::ProDos, FLOPPY_SIZE, none);
        assert_eq!(po.len(), FLOPPY_SIZE);
        assert_eq!(&po[2 * 512 + 5..2 * 512 + 10], b"BLANK");
        let big = new_disk_bytes(NewDiskKind::ProDos, 5 * 1024 * 1024, none);
        assert_eq!(big.len(), 5 * 1024 * 1024);
    }

    #[test]
    fn dos33_only_offers_floppy_size() {
        assert_eq!(NewDiskKind::Dos33.sizes().len(), 1);
        assert!(NewDiskKind::ProDos.sizes().len() > 1);
        assert_eq!(NewDiskKind::ProDos.extension(), "po");
        assert_eq!(NewDiskKind::Dos33.extension(), "dsk");
    }

    #[test]
    fn written_image_loads_back() {
        let dir = std::env::temp_dir().join(format!("apple2-rust-disk-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("n.po");
        write_new_disk(&p, NewDiskKind::ProDos, FLOPPY_SIZE, ProDosFiles::default()).unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().len() as usize, FLOPPY_SIZE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prodos_file_choices_follow_settings() {
        let mut s = Settings::default();
        s.storage.new_disk_copy_prodos = true;
        s.storage.new_disk_copy_basic = true;
        let f = prodos_files(&s);
        assert!(f.prodos && f.basic && !f.bitsy_boot && !f.bitsy_bye);
        let with = new_disk_bytes(NewDiskKind::ProDos, FLOPPY_SIZE, f);
        let without = new_disk_bytes(NewDiskKind::ProDos, FLOPPY_SIZE, ProDosFiles::default());
        assert_ne!(with, without);
    }

    #[test]
    fn hdd_helper_edits_only_that_unit() {
        let mut s = Settings::default();
        s.slots.0[7] = SlotCardSpec::new(CardType::GenericHdd);
        let t = with_hdd(&s, 7, 2, Some("x.hdv".into()));
        match &t.slots.0[7] {
            SlotCardSpec::GenericHdd { images, .. } => {
                assert_eq!(images[2].as_deref(), Some(Path::new("x.hdv")));
                assert!(images[0].is_none());
            }
            _ => unreachable!(),
        }
        assert_eq!(with_hdd(&t, 7, 2, None), s);
    }
}
