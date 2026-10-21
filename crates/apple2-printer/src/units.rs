//! Canonical physical unit: 1/2160 inch, integer, on both axes.
//!
//! Every LX-310 step is a whole number of units: 1/60" = 36, 1/72" = 30, 1/80" = 27, 1/90" = 24,
//! 1/120" = 18, 1/240" = 9 (horizontal dot densities), 1/216" = 10 (vertical step), and one PDF
//! point (1/72") = 30. Positions never accumulate rounding error (R01, T-016).

/// Units per inch.
pub const PER_INCH: u32 = 2160;
/// Units per PDF point (1/72").
pub const PER_POINT: u32 = 30;
/// Units per 1/216" (ESC 3 / ESC J step).
pub const PER_216: u32 = 10;

/// Units for `n` steps of `1/den` inch, when `den` divides 2160.
pub const fn steps(n: u32, den: u32) -> u32 {
    n * (PER_INCH / den)
}

/// Millimetres to units (rounded), for paper sizes given in mm.
pub fn from_mm(mm: f64) -> u32 {
    (mm / 25.4 * PER_INCH as f64).round() as u32
}
