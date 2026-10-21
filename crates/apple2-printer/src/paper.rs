//! Papel and Fita (GLOSSARY): size, printable area and look. Only the PDF/preview look depends on
//! the decoration; the dot positions depend only on size and margins (R01).

use crate::units;
use serde::{Deserialize, Serialize};

/// Paper presets. Default: continuous green-bar form (ADR 0008 decision of the owner).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperKind {
    /// 9.5" x 11" fanfold with tractor holes, perforation and green bars (80 columns).
    #[default]
    GreenBar,
    /// Same form with blue bars.
    BlueBar,
    /// Plain continuous 9.5" x 11" without bars.
    PlainContinuous,
    A4,
    Letter,
}

/// Look of the ink (Fita). Never changes dot positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ribbon {
    /// Crisp, uniform round dots.
    #[default]
    New,
    /// Irregular intensity, deterministic per job (seeded from the job's raw-bytes hash).
    Worn,
}

/// Ribbon colour. Colour profiles may also offer `Black` as "preto e branco".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InkColor {
    #[default]
    Black,
    Blue,
    Purple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paper {
    pub kind: PaperKind,
    /// Size in units.
    pub width: u32,
    pub height: u32,
}

impl Paper {
    pub fn preset(kind: PaperKind) -> Paper {
        let (width, height) = match kind {
            PaperKind::GreenBar | PaperKind::BlueBar | PaperKind::PlainContinuous => {
                (units::steps(19, 2), units::steps(11, 1))
            }
            PaperKind::A4 => (units::from_mm(210.0), units::from_mm(297.0)),
            PaperKind::Letter => (units::steps(17, 2), units::steps(11, 1)),
        };
        Paper {
            kind,
            width,
            height,
        }
    }

    pub fn is_continuous(&self) -> bool {
        matches!(
            self.kind,
            PaperKind::GreenBar | PaperKind::BlueBar | PaperKind::PlainContinuous
        )
    }
}

impl Default for Paper {
    fn default() -> Self {
        Paper::preset(PaperKind::default())
    }
}

/// Everything about how a job is rendered that the user chooses (frozen when the job starts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    pub paper: Paper,
    pub ribbon: Ribbon,
    pub ink: InkColor,
}
