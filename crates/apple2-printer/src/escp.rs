//! Incremental ESC/P parser for the 9-pin profile (Epson LX-310). Bytes in, `Parsed` commands and
//! `Diagnostic`s out; any split of the stream (down to one byte per call) gives the same output
//! (T-010).
//!
//! State machine: `Normal` -> `Esc` -> `Params` (fixed parameter bytes) | `Paren` (ESC ( c nL nH
//! data) | `Delimited` (ESC D / ESC B up to NUL) -> `GraphicsPayload` (exactly the declared number
//! of columns; ESC/LF inside are data).
//!
//! Resynchronisation rules (R02/CA03), documented in `docs/printer/protocols/lx310.md`:
//! 1. ESC followed by an unknown printable byte drops exactly those two bytes (diagnostic); the
//!    next byte is text again.
//! 2. ESC followed by another ESC drops the first (diagnostic) and starts a new sequence.
//! 3. ESC followed by a control code that is not a command byte drops the lone ESC (diagnostic) and
//!    the control code is interpreted normally.
//! 4. A parameter out of range drops the sequence (diagnostic) after its fixed parameters; a
//!    graphics count above the configured limit drops the header only, so the payload is read as
//!    text rather than swallowed.
//! 5. `ESC D`/`ESC B` end at NUL, at a non-ascending value (like NUL) or when the manual's maximum
//!    number of tabs (32/16) has been read, so they never swallow unbounded text.
//! 6. At the end of the stream an incomplete sequence is reported (`finish`); an unfinished tab list
//!    and a graphics payload keep what was received.
//!
//! Memory is bounded: a graphics payload is at most `max_graphics_columns * 2` bytes, diagnostics
//! keep at most `DIAG_BYTES` bytes of the offending sequence.

use crate::command::{Command, Parsed, Pitch};
use crate::document::Diagnostic;

/// Bytes of an offending sequence kept in a diagnostic.
pub const DIAG_BYTES: usize = 16;

const ESC: u8 = 0x1B;

/// Parser limits (CA04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserLimits {
    /// Largest accepted `nL + 256 * nH` for a graphics command. The manual allows `nH <= 31`
    /// (8191); the default accepts the full 16-bit count so no declared payload is mis-read as text.
    pub max_graphics_columns: u32,
}

impl Default for ParserLimits {
    fn default() -> Self {
        ParserLimits {
            max_graphics_columns: 65_535,
        }
    }
}

#[derive(Clone, Debug)]
enum State {
    Normal,
    Esc,
    Params {
        cmd: u8,
        want: usize,
        got: Vec<u8>,
    },
    Paren {
        header: Vec<u8>,
        remaining: usize,
        payload: Vec<u8>,
    },
    Delimited {
        cmd: u8,
        values: Vec<u8>,
    },
    GraphicsPayload {
        density: u8,
        nine_pin: bool,
        columns: u16,
        want: usize,
        data: Vec<u8>,
    },
    /// ESC & 0 n m: the definitions of `left` characters are being consumed (and discarded).
    UserChars {
        nlq: bool,
        left: usize,
        cur: Vec<u8>,
    },
}

/// How the byte after ESC is interpreted.
enum Spec {
    Fixed(usize),
    Paren,
    Delimited,
    Unknown,
}

fn spec(b: u8) -> Spec {
    match b {
        // No parameters.
        b'@' | b'0' | b'1' | b'2' | b'4' | b'5' | b'E' | b'F' | b'G' | b'H' | b'O' | b'P'
        | b'M' | b'g' | b'T' | b'6' | b'7' | b'8' | b'9' | b'#' | b'=' | b'>' | b'<' | 0x0E
        | 0x0F => Spec::Fixed(0),
        // One parameter.
        b'3' | b'A' | b'J' | b'C' | b'N' | b'Q' | b'l' | b'!' | b't' | b'R' | b'x' | b'k'
        | b'W' | b'w' | b'-' | b'S' | b'p' | b'U' | b'y' | b' ' | b'%' | b'/' | b'a' | b'i'
        | b's' | b'r' | b'm' | b'I' | b'j' | 0x19 => Spec::Fixed(1),
        // Two parameters.
        b'$' | b'\\' | b'K' | b'L' | b'Y' | b'Z' | b'?' | b'e' | b'f' => Spec::Fixed(2),
        // Three parameters (graphics ones are followed by a payload).
        b'*' | b'^' | b':' | b'&' => Spec::Fixed(3),
        b'(' => Spec::Paren,
        b'D' | b'B' => Spec::Delimited,
        _ => Spec::Unknown,
    }
}

fn is_graphics(cmd: u8) -> bool {
    matches!(cmd, b'K' | b'L' | b'Y' | b'Z' | b'*' | b'^')
}

fn onoff(v: u8) -> Result<bool, &'static str> {
    match v {
        0 | 48 => Ok(false),
        1 | 49 => Ok(true),
        _ => Err("parâmetro fora da faixa (esperado 0, 1, 48 ou 49)"),
    }
}

/// Builds the typed command of a fixed-parameter ESC sequence, or says why it is invalid.
/// `Ok(None)` means the sequence is well formed but this profile does not implement it.
fn build(cmd: u8, p: &[u8]) -> Result<Option<Command>, &'static str> {
    let p0 = p.first().copied().unwrap_or(0);
    let c = match cmd {
        b'@' => Command::Reset,
        b'0' => Command::LineSpacing216(27),
        b'1' => Command::LineSpacing216(21),
        b'2' => Command::LineSpacing216(36),
        b'3' => Command::LineSpacing216(p0),
        b'A' => {
            if p0 > 85 {
                return Err("ESC A: n deve estar entre 0 e 85");
            }
            Command::LineSpacing216(p0 * 3)
        }
        b'J' => Command::AdvanceVertical216(p0),
        b'C' => {
            if p0 == 0 {
                let n = p.get(1).copied().unwrap_or(0);
                if !(1..=22).contains(&n) {
                    return Err("ESC C 0 n: n deve estar entre 1 e 22");
                }
                Command::PageLengthInches(n)
            } else if p0 <= 127 {
                Command::PageLengthLines(p0)
            } else {
                return Err("ESC C n: n deve estar entre 1 e 127");
            }
        }
        b'N' => {
            if !(1..=127).contains(&p0) {
                return Err("ESC N: n deve estar entre 1 e 127");
            }
            Command::BottomMargin(p0)
        }
        b'O' => Command::CancelBottomMargin,
        b'l' => Command::LeftMargin(p0),
        b'Q' => {
            if p0 == 0 {
                return Err("ESC Q: n deve estar entre 1 e 255");
            }
            Command::RightMargin(p0)
        }
        b'$' => {
            if p[1] > 127 {
                return Err("ESC $: nH deve estar entre 0 e 127");
            }
            Command::AbsoluteHorizontal(u16::from(p[0]) | (u16::from(p[1]) << 8))
        }
        b'\\' => {
            let v = u16::from(p[0]) | (u16::from(p[1]) << 8);
            Command::RelativeHorizontal(v as i16)
        }
        b'P' => Command::Pitch(Pitch::Cpi10),
        b'M' => Command::Pitch(Pitch::Cpi12),
        b'g' => Command::Pitch(Pitch::Cpi15),
        0x0F => return Ok(None), // ESC SI: not in the LX-310 list (use SI)
        b'E' => Command::Bold(true),
        b'F' => Command::Bold(false),
        b'G' => Command::DoubleStrike(true),
        b'H' => Command::DoubleStrike(false),
        b'4' => Command::Italic(true),
        b'5' => Command::Italic(false),
        b'-' => Command::Underline(onoff(p0)?),
        b'W' => Command::DoubleWidth(onoff(p0)?),
        b'!' => Command::MasterSelect(p0),
        b't' => {
            onoff(p0)?;
            Command::CharTable(p0 & 1)
        }
        b'R' => {
            if p0 > 12 {
                return Err("ESC R: n deve estar entre 0 e 12");
            }
            Command::International(p0)
        }
        b'x' => Command::Nlq(onoff(p0)?),
        b'k' => {
            if p0 > 1 {
                return Err("ESC k: n deve ser 0 ou 1");
            }
            Command::Typeface(p0)
        }
        b' ' => {
            if p0 > 127 {
                return Err("ESC SP: n deve estar entre 0 e 127");
            }
            Command::ExtraSpace(p0)
        }
        b'U' => Command::Unidirectional(onoff(p0)?),
        b'6' => Command::UpperControlCodes(true),
        b'7' => Command::UpperControlCodes(false),
        _ => return Ok(None),
    };
    Ok(Some(c))
}

pub struct Parser {
    limits: ParserLimits,
    state: State,
    /// Offset of the byte being processed.
    pos: u64,
    /// Offset of the first byte of the sequence in progress.
    seq_start: u64,
    /// First bytes of the sequence in progress (diagnostics only).
    seq: Vec<u8>,
    /// NLQ quality as last set by ESC x (ESC & data layout depends on it).
    nlq: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Parser::new(ParserLimits::default())
    }
}

impl Parser {
    pub fn new(limits: ParserLimits) -> Parser {
        Parser {
            limits,
            state: State::Normal,
            pos: 0,
            seq_start: 0,
            seq: Vec::new(),
            nlq: false,
        }
    }

    /// Forgets any sequence in progress (a new job).
    pub fn reset(&mut self) {
        self.state = State::Normal;
        self.pos = 0;
        self.seq_start = 0;
        self.seq.clear();
        self.nlq = false;
    }

    /// True while a multi-byte sequence is incomplete.
    pub fn in_sequence(&self) -> bool {
        !matches!(self.state, State::Normal)
    }

    /// Feeds `data`, the bytes of the job from `offset` on. Output is appended to `out`/`diags`.
    pub fn feed(
        &mut self,
        offset: u64,
        data: &[u8],
        out: &mut Vec<Parsed>,
        diags: &mut Vec<Diagnostic>,
    ) {
        self.pos = offset;
        for &b in data {
            self.step(b, out, diags);
            self.pos = self.pos.saturating_add(1);
        }
    }

    /// The stream ended: reports an incomplete sequence and flushes what it carries.
    pub fn finish(&mut self, out: &mut Vec<Parsed>, diags: &mut Vec<Diagnostic>) {
        let state = std::mem::replace(&mut self.state, State::Normal);
        match state {
            State::Normal => {}
            State::Esc => {
                self.diag(diags, "ESC isolado no fim do fluxo (sequência truncada)");
                self.unsupported(out);
            }
            State::Params { .. } | State::Paren { .. } => {
                self.diag(
                    diags,
                    "sequência ESC truncada no fim do fluxo (parâmetros incompletos)",
                );
                self.unsupported(out);
            }
            State::Delimited { cmd, values } => {
                self.diag(
                    diags,
                    "lista de tabulação sem NUL final no fim do fluxo (truncada); valores lidos foram aplicados",
                );
                out.push(Parsed {
                    offset: self.seq_start,
                    command: Self::tabs(cmd, values),
                });
            }
            State::UserChars { left, .. } => {
                self.diag(
                    diags,
                    &format!(
                        "ESC &: definição de caracteres do usuário truncada no fim do fluxo ({left} caractere(s) faltando)"
                    ),
                );
                self.unsupported(out);
            }
            State::GraphicsPayload {
                density,
                nine_pin,
                columns,
                want,
                mut data,
            } => {
                let per = if nine_pin { 2 } else { 1 };
                let got_cols = data.len() / per;
                data.truncate(got_cols * per);
                self.diag(
                    diags,
                    &format!(
                        "gráfico truncado: {} de {} colunas recebidas (esperados {} bytes)",
                        got_cols, columns, want
                    ),
                );
                out.push(Parsed {
                    offset: self.seq_start,
                    command: Command::Graphics {
                        density,
                        nine_pin,
                        columns: got_cols as u16,
                        data,
                    },
                });
            }
        }
        self.seq.clear();
    }

    fn tabs(cmd: u8, values: Vec<u8>) -> Command {
        if cmd == b'D' {
            Command::HorizontalTabs(values)
        } else {
            Command::VerticalTabs(values)
        }
    }

    fn diag(&self, diags: &mut Vec<Diagnostic>, message: &str) {
        diags.push(Diagnostic {
            offset: self.seq_start,
            bytes: self.seq.iter().copied().take(DIAG_BYTES).collect(),
            message: message.to_string(),
        });
    }

    fn unsupported(&mut self, out: &mut Vec<Parsed>) {
        out.push(Parsed {
            offset: self.seq_start,
            command: Command::Unsupported {
                bytes: self.seq.clone(),
            },
        });
    }

    fn seq_push(&mut self, b: u8) {
        if self.seq.len() < DIAG_BYTES {
            self.seq.push(b);
        }
    }

    fn single(&mut self, out: &mut Vec<Parsed>, command: Command) {
        out.push(Parsed {
            offset: self.pos,
            command,
        });
    }

    fn step(&mut self, b: u8, out: &mut Vec<Parsed>, diags: &mut Vec<Diagnostic>) {
        match std::mem::replace(&mut self.state, State::Normal) {
            State::Normal => self.normal(b, out, diags),
            State::Esc => self.after_esc(b, out, diags),
            State::Params { cmd, want, mut got } => {
                self.seq_push(b);
                got.push(b);
                // ESC C 0 takes one more parameter.
                let want = if cmd == b'C' && got.len() == 1 && b == 0 {
                    2
                } else {
                    want
                };
                if got.len() < want {
                    self.state = State::Params { cmd, want, got };
                } else {
                    self.params_done(cmd, &got, out, diags);
                }
            }
            State::Paren {
                mut header,
                mut remaining,
                mut payload,
            } => {
                self.seq_push(b);
                if header.len() < 3 {
                    header.push(b);
                    if header.len() == 3 {
                        remaining = usize::from(header[1]) | (usize::from(header[2]) << 8);
                    }
                } else {
                    if payload.len() < DIAG_BYTES {
                        payload.push(b);
                    }
                    remaining -= 1;
                }
                if header.len() == 3 && remaining == 0 {
                    self.paren_done(&header, &payload, out, diags);
                } else {
                    self.state = State::Paren {
                        header,
                        remaining,
                        payload,
                    };
                }
            }
            State::Delimited { cmd, mut values } => {
                self.seq_push(b);
                let max = if cmd == b'D' { 32 } else { 16 };
                let ends = b == 0 || values.last().is_some_and(|&last| b <= last);
                if ends {
                    out.push(Parsed {
                        offset: self.seq_start,
                        command: Self::tabs(cmd, values),
                    });
                    return;
                }
                values.push(b);
                if values.len() >= max {
                    out.push(Parsed {
                        offset: self.seq_start,
                        command: Self::tabs(cmd, values),
                    });
                } else {
                    self.state = State::Delimited { cmd, values };
                }
            }
            State::UserChars {
                nlq,
                mut left,
                mut cur,
            } => {
                cur.push(b);
                // Draft: a + 11 column bytes. NLQ: 0, a (width 1..12), 0, then 3 bytes per column.
                let need = if nlq {
                    if cur.len() < 2 {
                        3
                    } else {
                        3 + 3 * usize::from(cur[1].min(12))
                    }
                } else {
                    12
                };
                if cur.len() >= need {
                    left -= 1;
                    cur.clear();
                }
                if left == 0 {
                    self.diag(
                        diags,
                        "ESC &: caractere do usuário não suportado (definição consumida e descartada)",
                    );
                    self.unsupported(out);
                } else {
                    self.state = State::UserChars { nlq, left, cur };
                }
            }
            State::GraphicsPayload {
                density,
                nine_pin,
                columns,
                want,
                mut data,
            } => {
                data.push(b);
                if data.len() == want {
                    out.push(Parsed {
                        offset: self.seq_start,
                        command: Command::Graphics {
                            density,
                            nine_pin,
                            columns,
                            data,
                        },
                    });
                } else {
                    self.state = State::GraphicsPayload {
                        density,
                        nine_pin,
                        columns,
                        want,
                        data,
                    };
                }
            }
        }
    }

    fn normal(&mut self, b: u8, out: &mut Vec<Parsed>, diags: &mut Vec<Diagnostic>) {
        match b {
            ESC => {
                self.seq_start = self.pos;
                self.seq.clear();
                self.seq.push(ESC);
                self.state = State::Esc;
            }
            0x20..=0x7E | 0x80..=0xFF => self.single(out, Command::Char(b)),
            0x08 => self.single(out, Command::Backspace),
            0x09 => self.single(out, Command::HorizontalTab),
            0x0A => self.single(out, Command::LineFeed),
            0x0B => self.single(out, Command::VerticalTab),
            0x0C => self.single(out, Command::FormFeed),
            0x0D => self.single(out, Command::CarriageReturn),
            0x0E => self.single(out, Command::DoubleWidthLine(true)),
            0x0F => self.single(out, Command::Condensed(true)),
            0x12 => self.single(out, Command::Condensed(false)),
            0x14 => self.single(out, Command::DoubleWidthLine(false)),
            // NUL and BEL have no effect on the page; ignored by design (documented).
            0x00 | 0x07 => {}
            // Other control codes (CAN, DC1, DC3, DEL, unassigned) are not in the LX-310 ESC/P list.
            _ => {
                self.seq_start = self.pos;
                self.seq.clear();
                self.seq.push(b);
                self.diag(diags, "código de controle sem suporte ignorado");
                self.unsupported(out);
            }
        }
    }

    fn after_esc(&mut self, b: u8, out: &mut Vec<Parsed>, diags: &mut Vec<Diagnostic>) {
        if b == ESC {
            self.diag(diags, "ESC seguido de ESC: primeiro ESC descartado");
            self.unsupported(out);
            self.seq_start = self.pos;
            self.seq.clear();
            self.seq.push(ESC);
            self.state = State::Esc;
            return;
        }
        if b < 0x20 && !matches!(b, 0x0E | 0x0F | 0x19) {
            self.diag(diags, "ESC seguido de código de controle: ESC descartado");
            self.unsupported(out);
            self.normal(b, out, diags);
            return;
        }
        self.seq_push(b);
        match spec(b) {
            Spec::Fixed(0) => self.params_done(b, &[], out, diags),
            Spec::Fixed(n) => {
                self.state = State::Params {
                    cmd: b,
                    want: n,
                    got: Vec::new(),
                }
            }
            Spec::Paren => {
                self.state = State::Paren {
                    header: Vec::new(),
                    remaining: 0,
                    payload: Vec::new(),
                }
            }
            Spec::Delimited => {
                self.state = State::Delimited {
                    cmd: b,
                    values: Vec::new(),
                }
            }
            Spec::Unknown => {
                self.diag(
                    diags,
                    "sequência ESC desconhecida: ESC e o byte seguinte descartados",
                );
                self.unsupported(out);
            }
        }
    }

    fn params_done(
        &mut self,
        cmd: u8,
        p: &[u8],
        out: &mut Vec<Parsed>,
        diags: &mut Vec<Diagnostic>,
    ) {
        if is_graphics(cmd) {
            self.graphics_start(cmd, p, out, diags);
            return;
        }
        if cmd == b'&' {
            if p[0] != 0 || p[1] > p[2] {
                self.diag(
                    diags,
                    "ESC &: cabeçalho inválido (esperado NUL n m com n <= m): dados não consumidos",
                );
                self.unsupported(out);
                return;
            }
            self.state = State::UserChars {
                nlq: self.nlq,
                left: usize::from(p[2] - p[1]) + 1,
                cur: Vec::new(),
            };
            return;
        }
        if cmd == b'$' && p[1] > 127 {
            self.diag(diags, "ESC $: nH deve estar entre 0 e 127");
            self.unsupported(out);
            return;
        }
        match build(cmd, p) {
            Ok(Some(c)) => {
                match c {
                    Command::Nlq(v) => self.nlq = v,
                    Command::Reset => self.nlq = false,
                    _ => {}
                }
                out.push(Parsed {
                    offset: self.seq_start,
                    command: c,
                });
            }
            Ok(None) => {
                self.diag(diags, "comando ESC/P reconhecido, sem suporte neste perfil");
                self.unsupported(out);
            }
            Err(msg) => {
                self.diag(diags, msg);
                self.unsupported(out);
            }
        }
    }

    fn graphics_start(
        &mut self,
        cmd: u8,
        p: &[u8],
        out: &mut Vec<Parsed>,
        diags: &mut Vec<Diagnostic>,
    ) {
        // Header: ESC K/L/Y/Z nL nH | ESC * m nL nH | ESC ^ m nL nH.
        let (density, nine_pin, nl, nh) = match cmd {
            b'K' => (0, false, p[0], p[1]),
            b'L' => (1, false, p[0], p[1]),
            b'Y' => (2, false, p[0], p[1]),
            b'Z' => (3, false, p[0], p[1]),
            b'*' => (p[0], false, p[1], p[2]),
            _ => (p[0], true, p[1], p[2]),
        };
        let bad_mode = if nine_pin { density > 1 } else { density > 7 };
        if bad_mode {
            self.diag(
                diags,
                "modo de gráfico fora da faixa de 9 agulhas: cabeçalho descartado, dados não consumidos",
            );
            self.unsupported(out);
            return;
        }
        let columns = u32::from(nl) | (u32::from(nh) << 8);
        if columns > self.limits.max_graphics_columns {
            self.diag(
                diags,
                "contagem de colunas acima do limite configurado: cabeçalho descartado, dados não consumidos",
            );
            self.unsupported(out);
            return;
        }
        if columns > 8191 {
            self.diag(
                diags,
                "contagem de colunas acima de 8191 (nH > 31 fora da faixa do manual); consumida como declarada",
            );
        }
        let columns = columns as u16;
        if columns == 0 {
            out.push(Parsed {
                offset: self.seq_start,
                command: Command::Graphics {
                    density,
                    nine_pin,
                    columns: 0,
                    data: Vec::new(),
                },
            });
            return;
        }
        let want = usize::from(columns) * if nine_pin { 2 } else { 1 };
        self.state = State::GraphicsPayload {
            density,
            nine_pin,
            columns,
            want,
            data: Vec::with_capacity(want.min(4096)),
        };
    }

    fn paren_done(
        &mut self,
        header: &[u8],
        payload: &[u8],
        out: &mut Vec<Parsed>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let len = usize::from(header[1]) | (usize::from(header[2]) << 8);
        if header[0] == b't' && len == 3 && payload.len() >= 3 && onoff(payload[0]).is_ok() {
            out.push(Parsed {
                offset: self.seq_start,
                command: Command::AssignCharTable {
                    slot: payload[0] & 1,
                    id: payload[1],
                    variant: payload[2],
                },
            });
            return;
        }
        self.diag(
            diags,
            "ESC ( x sem suporte (ou parâmetros inválidos): carga consumida pelo comprimento declarado",
        );
        self.unsupported(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &[u8]) -> (Vec<Parsed>, Vec<Diagnostic>) {
        let mut p = Parser::default();
        let (mut o, mut d) = (Vec::new(), Vec::new());
        p.feed(0, data, &mut o, &mut d);
        p.finish(&mut o, &mut d);
        (o, d)
    }

    #[test]
    fn esc_amp_consumes_nlq_definitions_exactly() {
        // NLQ: two characters (width 2 -> 3 + 6 bytes; width 1 -> 3 + 3 bytes), data with ESC.
        let mut d = vec![0x1B, b'x', 1, 0x1B, b'&', 0, 58, 59];
        d.extend([0, 2, 0, 0x1B, 0x1B, 1, 2, 3, 4]);
        d.extend([0, 1, 0, 0x0A, 0x0C, 0x1B]);
        d.push(b'A');
        let (o, dg) = run(&d);
        assert_eq!(o.len(), 3, "{o:?}");
        assert_eq!(
            o[1].command,
            Command::Unsupported {
                bytes: d[3..8].to_vec()
            }
        );
        assert_eq!(o[2].command, Command::Char(b'A'));
        assert!(dg
            .iter()
            .any(|x| x.message.contains("caractere do usuário não suportado")));
    }

    #[test]
    fn esc_amp_consumes_draft_definitions_exactly() {
        // Draft (default): one character = a + 11 bytes.
        let mut d = vec![0x1B, b'&', 0, 60, 60, 128];
        d.extend([0x1B; 11]);
        d.push(b'B');
        let (o, _) = run(&d);
        assert_eq!(o.len(), 2, "{o:?}");
        assert_eq!(o[1].command, Command::Char(b'B'));
    }

    #[test]
    fn esc_percent_is_recognised_with_one_parameter_and_reported() {
        let (o, d) = run(&[0x1B, b'%', 1, b'A']);
        assert_eq!(o.len(), 2);
        assert!(matches!(o[0].command, Command::Unsupported { .. }));
        assert_eq!(o[1].command, Command::Char(b'A'));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn esc_amp_split_anywhere_is_equivalent() {
        let mut d = vec![0x1B, b'x', 1, 0x1B, b'&', 0, 61, 61, 0, 3, 0];
        d.extend(0..9u8);
        d.push(b'Z');
        let (whole, _) = run(&d);
        let mut p = Parser::default();
        let (mut o, mut dg) = (Vec::new(), Vec::new());
        for (i, b) in d.iter().enumerate() {
            p.feed(i as u64, &[*b], &mut o, &mut dg);
        }
        p.finish(&mut o, &mut dg);
        assert_eq!(o, whole);
    }

    #[test]
    fn graphics_consumes_exactly_declared_columns() {
        let (o, d) = run(&[0x1B, b'K', 2, 0, 0x1B, 0x0A, b'A']);
        assert!(d.is_empty());
        assert_eq!(o.len(), 2);
        assert_eq!(
            o[0].command,
            Command::Graphics {
                density: 0,
                nine_pin: false,
                columns: 2,
                data: vec![0x1B, 0x0A]
            }
        );
        assert_eq!(o[1].command, Command::Char(b'A'));
        assert_eq!(o[1].offset, 6);
    }
}
