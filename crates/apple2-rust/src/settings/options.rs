//! Per-card option dialogs behind the "..." buttons of the Slots page (`IDD_SSC`, `IDD_PRINTER`,
//! `IDD_FLOPPY_DISK_DRIVES`, `IDD_HARD_DISK_DRIVES`, `IDD_MOUSECARD`, `IDD_SATURN`,
//! `IDD_RAMWORKS3`, `IDD_MOCKINGBOARD`, `IDD_TFE_SETTINGS_DIALOG`).

use super::model::{self, SscForm};
use super::pages::{browse_save, combo, group, path_row};
use super::{HostLists, SettingsDialog, AUX};
use apple2_config::card::{
    AyChip, HdcFirmware, PrinterDestination, PrinterInk, PrinterPaper, PrinterRibbon, PrinterTable,
    PrinterTransport, SlotCardSpec, SoundSockets, Ssi263Chip,
};
use egui::{Context, Ui};

const AY_CHIPS: [(AyChip, &str); 5] = [
    (AyChip::Empty, "Empty"),
    (AyChip::Ay8910, "AY8910"),
    (AyChip::Ay8912, "AY8912"),
    (AyChip::Ay8913, "AY8913"),
    (AyChip::Ym2149, "YM2149"),
];
const SSI_CHIPS: [(Ssi263Chip, &str); 3] = [
    (Ssi263Chip::Empty, "Empty"),
    (Ssi263Chip::Ssi263P, "SSI263P"),
    (Ssi263Chip::Ssi263AP, "SSI263AP"),
];
const HDC_FW: [(HdcFirmware, &str); 4] = [
    (HdcFirmware::Default, "Default"),
    (HdcFirmware::SmartPort, "SmartPort"),
    (HdcFirmware::BlockMode2Devices, "Block mode (2 devices)"),
    (HdcFirmware::BlockMode4Devices, "Block mode (4 devices)"),
];

fn paper_label(k: PrinterPaper) -> &'static str {
    match k {
        PrinterPaper::GreenBar => "Green-bar continuous form 9.5\" x 11\"",
        PrinterPaper::BlueBar => "Blue-bar continuous form 9.5\" x 11\"",
        PrinterPaper::PlainContinuous => "Plain continuous 9.5\" x 11\"",
        PrinterPaper::A4 => "A4 sheet",
        PrinterPaper::Letter => "Letter sheet",
    }
}

fn table_label(t: PrinterTable) -> &'static str {
    match t {
        PrinterTable::Pc437 => "PC437",
        PrinterTable::Pc850 => "PC850",
        PrinterTable::Pc860 => "PC860",
        PrinterTable::Brascii => "BRASCII",
        PrinterTable::Abicomp => "ABICOMP",
        PrinterTable::Italic => "Italic",
    }
}

fn destination_label(d: PrinterDestination) -> &'static str {
    match d {
        PrinterDestination::VirtualAndFile => "Virtual printer + file",
        PrinterDestination::FileOnly => "File only",
        PrinterDestination::VirtualOnly => "Virtual printer only",
    }
}

fn title(spec: &SlotCardSpec) -> &'static str {
    match spec {
        SlotCardSpec::Disk2 { .. } => "Disk II interface",
        SlotCardSpec::GenericHdd { .. } => "Hard Disk Controller",
        SlotCardSpec::Ssc { .. } => "Super Serial Card",
        SlotCardSpec::GenericPrinter(_) => "Printer Card",
        SlotCardSpec::MouseInterface { .. } => "Mouse Card",
        SlotCardSpec::Saturn128K { .. } => "Saturn",
        SlotCardSpec::RamWorksIII { .. } => "RamWorks III",
        SlotCardSpec::MockingboardC(_) => "Mockingboard",
        SlotCardSpec::Phasor(_) => "Phasor",
        SlotCardSpec::Uthernet { .. } | SlotCardSpec::Uthernet2 { .. } => "Ethernet Settings",
        _ => "Card options",
    }
}

impl SettingsDialog {
    /// Draws the open card options dialog, if any.
    pub(super) fn draw_sub(&mut self, ctx: &Context) {
        let Some(mut sub) = self.sub.take() else {
            return;
        };
        let hosts = self.hosts().clone();
        let mut spec = if sub.slot == AUX {
            self.draft.aux.card.clone()
        } else {
            self.draft.slots.0[sub.slot].clone()
        };
        let mut ok = false;
        let mut cancel = false;
        egui::Window::new(title(&spec))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                card_ui(ui, &mut spec, &mut sub.ssc, &hosts);
                ui.separator();
                ui.horizontal(|ui| {
                    ok = ui.button("OK").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if cancel {
            spec = sub.backup.clone();
        }
        if sub.slot == AUX {
            self.draft.aux.card = spec;
        } else {
            self.draft.slots.0[sub.slot] = spec;
        }
        if !ok && !cancel {
            self.sub = Some(sub);
        }
    }
}

fn card_ui(ui: &mut Ui, spec: &mut SlotCardSpec, ssc: &mut Option<SscForm>, hosts: &HostLists) {
    match spec {
        SlotCardSpec::Disk2 {
            firmware_13_sector,
            drives,
        } => {
            group(ui, |ui| {
                ui.strong("Floppy Disk Drives");
                if ui.button("Swap").clicked() {
                    let a = drives[0].image.take();
                    drives[0].image = drives[1].image.take();
                    drives[1].image = a;
                }
                for (i, d) in drives.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("Floppy {}:", i + 1));
                        path_row(
                            ui,
                            "Disk image",
                            &mut d.image,
                            Some((
                                "Disk images",
                                &[
                                    "dsk", "do", "po", "nib", "woz", "2mg", "2img", "gz", "zip",
                                    "iie",
                                ],
                            )),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(52.0);
                        ui.checkbox(&mut d.write_protect, "Write protect");
                        ui.checkbox(&mut d.connected, "Drive connected");
                    });
                }
                // "Show status for slots 5 & 6" is global: it lives in the Configuration page,
                // not in this card dialog (GH#1490).
                ui.checkbox(firmware_13_sector, "13-sector Firmware");
            });
        }
        SlotCardSpec::GenericHdd { firmware, images } => {
            group(ui, |ui| {
                ui.strong("Hard Disk Drives");
                if ui.button("Swap 1 and 2").clicked() {
                    images.swap(0, 1);
                }
                for (i, img) in images.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("HDD {}:", i + 1));
                        path_row(
                            ui,
                            "Hard disk image",
                            img,
                            Some((
                                "Hard disk images",
                                &["hdv", "po", "2mg", "2img", "gz", "zip", "dsk"],
                            )),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Firmware type:");
                    combo(ui, "hdcfw", &HDC_FW, firmware);
                });
            });
        }
        SlotCardSpec::Ssc { .. } => {
            let Some(form) = ssc else { return };
            group(ui, |ui| {
                ui.strong("SSC");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut form.tcp, false, "Serial port");
                    ui.radio_value(&mut form.tcp, true, "TCP server");
                });
                if form.tcp {
                    ui.horizontal(|ui| {
                        ui.label("TCP port:");
                        let mut p = u32::from(form.tcp_port);
                        if ui
                            .add(egui::DragValue::new(&mut p).range(1..=65535))
                            .changed()
                        {
                            form.tcp_port = p as u16;
                        }
                    });
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 150, 40),
                        model::SSC_TCP_WARNING,
                    );
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Serial Port:");
                        let shown = if form.serial_port.is_empty() {
                            "(none)".to_string()
                        } else {
                            form.serial_port.clone()
                        };
                        egui::ComboBox::from_id_salt("serialport")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut form.serial_port, String::new(), "(none)");
                                for p in &hosts.serial_ports {
                                    ui.selectable_value(&mut form.serial_port, p.clone(), p);
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("or type a name:");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.serial_port).desired_width(180.0),
                        );
                    });
                }
                ui.checkbox(&mut form.support_dcd, "Support DCD (modem control lines)");
            });
            *spec = form.to_spec();
        }
        SlotCardSpec::GenericPrinter(p) => {
            ui.label("Printer dump filename:");
            ui.horizontal(|ui| {
                let mut text = p
                    .filename
                    .as_ref()
                    .map(|f| f.display().to_string())
                    .unwrap_or_default();
                if ui
                    .add(egui::TextEdit::singleline(&mut text).desired_width(260.0))
                    .changed()
                {
                    p.filename = (!text.is_empty()).then(|| text.clone().into());
                }
                if ui.button("Browse...").clicked() {
                    if let Some(f) = browse_save("Printer dump file", "Printer.txt") {
                        p.filename = Some(f);
                    }
                }
            });
            group(ui, |ui| {
                ui.strong("Virtual printer");
                ui.horizontal(|ui| {
                    ui.label("Send output to:");
                    for d in PrinterDestination::ALL {
                        ui.radio_value(&mut p.destination, d, destination_label(d));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Transport:");
                    ui.radio_value(&mut p.transport, PrinterTransport::Auto, "Automatic")
                        .on_hover_text(
                            "Text sent through the card's firmware (PR#1, PRINT, LIST) loses bit 7; \
                             programs writing the port directly (graphics) send all 8 bits",
                        );
                    ui.radio_value(&mut p.transport, PrinterTransport::EightBit, "8 bits");
                    ui.radio_value(
                        &mut p.transport,
                        PrinterTransport::SevenBitAppleWin,
                        "7 bits (AppleWin)",
                    );
                });
                ui.checkbox(
                    &mut p.auto_open_window,
                    "Open the printer window at the first byte printed",
                );
                ui.weak("The file above (Printer.txt) always receives 7-bit, filtered text.");
            });
            group(ui, |ui| {
                ui.strong("Printer profile: Epson LX-310");
                ui.weak("Paper and ribbon changes apply from the next print job.");
                egui::ComboBox::from_label("Paper")
                    .selected_text(paper_label(p.paper))
                    .show_ui(ui, |ui| {
                        for k in PrinterPaper::ALL {
                            ui.selectable_value(&mut p.paper, k, paper_label(k));
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Ribbon:");
                    ui.radio_value(&mut p.ribbon, PrinterRibbon::New, "New");
                    ui.radio_value(&mut p.ribbon, PrinterRibbon::Worn, "Worn");
                });
                ui.horizontal(|ui| {
                    ui.label("Ribbon colour:");
                    ui.radio_value(&mut p.ink, PrinterInk::Black, "Black");
                    ui.radio_value(&mut p.ink, PrinterInk::Blue, "Blue");
                    ui.radio_value(&mut p.ink, PrinterInk::Purple, "Purple");
                });
                egui::ComboBox::from_label("Character table")
                    .selected_text(table_label(p.table))
                    .show_ui(ui, |ui| {
                        for t in PrinterTable::ALL {
                            ui.selectable_value(&mut p.table, t, table_label(t));
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("International set (ESC R):");
                    let mut c = u32::from(p.country);
                    if ui.add(egui::DragValue::new(&mut c).range(0..=12)).changed() {
                        p.country = c as u8;
                    }
                });
                ui.weak("The program can change the table (ESC t, ESC R); ESC @ restores the printer's own.");
            });
            group(ui, |ui| {
                ui.strong("Advanced");
                ui.checkbox(&mut p.dump_to_printer, "Dump to printer (use with caution)");
                ui.checkbox(&mut p.convert_encoding, "Encoding conversion for clones");
                ui.checkbox(&mut p.filter_unprintable, "Filter unprintable characters");
                ui.checkbox(&mut p.append, "Append to print file");
                ui.horizontal(|ui| {
                    ui.label("Terminate printing after idle (in seconds):");
                    ui.add(egui::DragValue::new(&mut p.idle_limit).range(1..=3600));
                });
            });
        }
        SlotCardSpec::MouseInterface {
            show_crosshair,
            restrict_to_window,
        } => {
            group(ui, |ui| {
                ui.strong("UI");
                ui.checkbox(show_crosshair, "Show crosshairs in window's frame");
                ui.checkbox(restrict_to_window, "Restrict mouse to Apple window");
            });
        }
        SlotCardSpec::Saturn128K { banks } => {
            group(ui, |ui| {
                ui.strong("RAM Size (KB)");
                let mut idx = model::SATURN_BANK_STEPS
                    .iter()
                    .position(|b| b == banks)
                    .unwrap_or(3);
                let steps = model::SATURN_BANK_STEPS;
                if ui
                    .add(
                        egui::Slider::new(&mut idx, 0..=steps.len() - 1)
                            .custom_formatter(move |v, _| {
                                format!("{}KB", u32::from(steps[v as usize]) * 16)
                            })
                            .show_value(true),
                    )
                    .changed()
                {
                    *banks = steps[idx];
                }
            });
        }
        SlotCardSpec::RamWorksIII { banks } => {
            group(ui, |ui| {
                ui.strong("RAM Size");
                let steps = model::RW3_BANK_STEPS;
                let mut idx = steps.iter().position(|b| b == banks).unwrap_or(4);
                if ui
                    .add(
                        egui::Slider::new(&mut idx, 0..=steps.len() - 1).custom_formatter(
                            move |v, _| {
                                let kb = u32::from(steps[v as usize]) * 64;
                                if kb >= 1024 {
                                    format!("{}MB", kb / 1024)
                                } else {
                                    format!("{kb}KB")
                                }
                            },
                        ),
                    )
                    .changed()
                {
                    *banks = steps[idx];
                }
            });
        }
        SlotCardSpec::MockingboardC(s) => sockets_ui(ui, s, 2),
        SlotCardSpec::Phasor(s) => sockets_ui(ui, s, 4),
        SlotCardSpec::Uthernet { interface } => ethernet_ui(ui, interface, None, hosts),
        SlotCardSpec::Uthernet2 {
            interface,
            virtual_dns,
        } => ethernet_ui(ui, interface, Some(virtual_dns), hosts),
        _ => {
            ui.label("This card has no options.");
        }
    }
}

fn sockets_ui(ui: &mut Ui, s: &mut SoundSockets, ay_count: usize) {
    group(ui, |ui| {
        ui.strong("Speech");
        ui.horizontal(|ui| {
            ui.label("SSI263  $Cs40 (main):");
            combo(ui, "ssi1", &SSI_CHIPS, &mut s.ssi263[1]);
        });
        ui.horizontal(|ui| {
            ui.label("SSI263  $Cs20 (unused):");
            combo(ui, "ssi0", &SSI_CHIPS, &mut s.ssi263[0]);
        });
        ui.checkbox(&mut s.sc01, "SC-01");
    });
    group(ui, |ui| {
        ui.strong("Sound chips");
        for i in 0..ay_count {
            ui.horizontal(|ui| {
                ui.label(format!("AY socket {}:", i + 1));
                combo(ui, &format!("ay{i}"), &AY_CHIPS, &mut s.ay[i]);
            });
        }
    });
}

fn ethernet_ui(
    ui: &mut Ui,
    interface: &mut String,
    virtual_dns: Option<&mut bool>,
    hosts: &HostLists,
) {
    ui.horizontal(|ui| {
        ui.label("Interface");
        let shown = if interface.is_empty() {
            "(none)".to_string()
        } else {
            interface.clone()
        };
        egui::ComboBox::from_id_salt("tfeif")
            .width(260.0)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(interface, String::new(), "(none)");
                for (name, desc) in &hosts.interfaces {
                    let label = if desc.is_empty() {
                        name.clone()
                    } else {
                        format!("{name} - {desc}")
                    };
                    ui.selectable_value(interface, name.clone(), label);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("or type a name:");
        ui.add(egui::TextEdit::singleline(interface).desired_width(200.0));
    });
    if let Some(dns) = virtual_dns {
        ui.checkbox(dns, "Virtual DNS");
    }
    group(ui, |ui| {
        ui.strong("Npcap / libpcap");
        ui.label(
            "The Uthernet cards send raw Ethernet frames through Npcap (Windows) or libpcap (Linux, macOS). \
             Install it and run with enough privileges to capture, otherwise the card has no network.",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_card_has_a_title() {
        use apple2_config::CardType;
        for ty in [
            CardType::Disk2,
            CardType::GenericHdd,
            CardType::Ssc,
            CardType::GenericPrinter,
            CardType::MockingboardC,
            CardType::MouseInterface,
            CardType::Phasor,
            CardType::Saturn128K,
            CardType::Uthernet,
            CardType::Uthernet2,
            CardType::RamWorksIII,
        ] {
            assert_ne!(title(&SlotCardSpec::new(ty)), "Card options", "{ty:?}");
        }
    }
}
