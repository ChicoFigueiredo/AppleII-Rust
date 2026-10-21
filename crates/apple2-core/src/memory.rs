//! Memory map and paging (AppleWin Memory.cpp `UpdatePaging`, `MemSetPaging`).
//!
//! AppleWin keeps a 64K "mem" image with copy-back; this port instead keeps one read table and
//! one write table with an entry per 256-byte CPU page, naming a backing segment and the offset
//! of the page inside it. The tables are rebuilt only when a paging flag or the RamWorks bank
//! changes, so every access is an O(1) lookup without allocation.

use crate::{AuxCardKind, Model};
use serde::{Deserialize, Serialize};

/// `g_memmode` flags (Memory.h:8-23). The bit values are persisted in save-states.
pub mod mf {
    pub const STORE80: u32 = 0x0001;
    pub const ALTZP: u32 = 0x0002;
    pub const AUXREAD: u32 = 0x0004;
    pub const AUXWRITE: u32 = 0x0008;
    pub const BANK2: u32 = 0x0010;
    pub const HIGHRAM: u32 = 0x0020;
    pub const HIRES: u32 = 0x0040;
    pub const PAGE2: u32 = 0x0080;
    pub const SLOTC3ROM: u32 = 0x0100;
    pub const INTCXROM: u32 = 0x0200;
    pub const WRITERAM: u32 = 0x0400;
    pub const IOUDIS: u32 = 0x0800;
    pub const ALTROM0: u32 = 0x1000;
    pub const ALTROM1: u32 = 0x2000;
    /// Flags owned by the Language Card (slice 3): AppleWin `MF_LANGCARD_MASK`.
    pub const LANGCARD_MASK: u32 = BANK2 | HIGHRAM | WRITERAM;
    /// `LanguageCardUnit::kMemModeInitialState` (LanguageCard.cpp:62).
    pub const INITIAL: u32 = BANK2 | WRITERAM;
}

const SYS_ROM_SIZE: usize = 0x3000;
const CX_ROM_SIZE: usize = 0x1000;
const LC_SIZE: usize = 0x4000;
const BANK_SIZE: usize = 0x10000;
const MAX_BANKS: u16 = 256;

/// Backing segment of a CPU page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seg {
    /// Writes are discarded.
    None,
    /// Reads return the floating bus (empty aux slot).
    Floating,
    Main,
    Aux,
    /// Main 16K Language Card RAM.
    Lc,
    Rom,
    /// $C000-$CFFF: the board dispatches I/O and Cx ROM.
    Io,
}

#[derive(Clone, Copy)]
struct Page {
    seg: Seg,
    base: u32,
}

const NONE: Page = Page {
    seg: Seg::None,
    base: 0,
};

/// Serializable RAM and paging state (the ROM comes from the configuration).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryState {
    pub main: Vec<u8>,
    pub aux: Vec<u8>,
    pub lc: Vec<u8>,
    pub lc_region: usize,
    pub memmode: u32,
    pub aux_bank: u16,
}

pub struct Memory {
    main: Vec<u8>,
    /// All aux RAM: 0 bytes (empty slot), 1K, or `banks` x 64K.
    aux: Vec<u8>,
    /// 16K regions of Language Card RAM: 0 is the //e built-in / slot-0 card, then Saturn banks.
    lc: Vec<u8>,
    /// Region currently behind $D000-$FFFF (AppleWin `g_pMemMainLanguageCard`).
    lc_region: usize,
    /// 12K pages of system ROM (several only on the Base64A).
    rom: Vec<u8>,
    rom_pages: usize,
    /// Internal Cx ROM ($C000-$CFFF, 4K); empty on the ][ family.
    cx_rom: Vec<u8>,
    aux_kind: AuxCardKind,
    banks: u16,
    aux_bank: u16,
    memmode: u32,
    initial_memmode: u32,
    is_e: bool,
    is_base64a: bool,
    /// VidHD on a ][/][+ (`memVidHD`): CPU writes to RAM are mirrored into `vidhd_aux`.
    dual_write_aux: bool,
    /// 64K "aux" of the ][ family, allocated on first use (AppleWin always allocates `memaux`).
    vidhd_aux: Vec<u8>,
    rd: [Page; 256],
    wr: [Page; 256],
}

impl Memory {
    /// `rom` is the model's image: 16K //e = 4K internal Cx ROM + 12K; 48K Base64A = 4 pages of
    /// 12K; 12K otherwise (Memory.cpp:2105-2118). `has_lc`: ][-family with a Language Card in
    /// slot 0, else ROM only (LanguageCard.cpp:626).
    pub fn new(model: Model, rom: &[u8], aux: AuxCardKind, has_lc: bool) -> Self {
        let (cx_rom, sys) = if rom.len() == CX_ROM_SIZE + SYS_ROM_SIZE {
            (rom[..CX_ROM_SIZE].to_vec(), &rom[CX_ROM_SIZE..])
        } else if rom.len() > SYS_ROM_SIZE && !rom.len().is_multiple_of(SYS_ROM_SIZE) {
            (Vec::new(), &rom[rom.len() - SYS_ROM_SIZE..])
        } else {
            (Vec::new(), rom)
        };
        let mut rom = sys.to_vec();
        rom.resize(rom.len().div_ceil(SYS_ROM_SIZE).max(1) * SYS_ROM_SIZE, 0);
        let is_e = !model.is_apple2();
        let (aux_len, banks) = match aux {
            AuxCardKind::Empty => (0, 1),
            AuxCardKind::Col80_1K => (0x400, 1),
            AuxCardKind::Extended80Col => (BANK_SIZE, 1),
            AuxCardKind::RamWorksIII { banks } => {
                let b = banks.clamp(1, MAX_BANKS);
                (BANK_SIZE * b as usize, b)
            }
        };
        let initial_memmode = if is_e || has_lc { mf::INITIAL } else { 0 };
        let mut m = Memory {
            main: vec![0; BANK_SIZE],
            aux: vec![0; aux_len],
            lc: vec![0; LC_SIZE],
            lc_region: 0,
            rom_pages: rom.len() / SYS_ROM_SIZE,
            rom,
            cx_rom,
            aux_kind: aux,
            banks,
            aux_bank: 0,
            memmode: initial_memmode,
            initial_memmode,
            is_e,
            is_base64a: model == Model::Base64A,
            dual_write_aux: false,
            vidhd_aux: Vec::new(),
            rd: [NONE; 256],
            wr: [NONE; 256],
        };
        m.update_paging();
        m
    }

    /// Power-cycle: RAM cleared, flags back to the model default, RamWorks bank 0 (GH#1414).
    pub fn power_cycle(&mut self) {
        self.main.fill(0);
        self.aux.fill(0);
        self.lc.fill(0);
        self.lc_region = 0;
        self.aux_bank = 0;
        self.memmode = self.initial_memmode;
        self.update_paging();
    }

    /// Ctrl-Reset. //e: `MemResetPaging` -> `LanguageCardManager::Reset(false)` sets the whole
    /// memmode to the initial state (all MMU switches off; LanguageCard.cpp:612-629), the RamWorks
    /// bank is kept (Utilities.cpp:578). ][ family: nothing, except the Base64A which goes back
    /// to ROM page 0 (`MemAnnunciatorReset`, Memory.cpp:2705).
    pub fn reset(&mut self) {
        if self.is_e {
            self.memmode = mf::INITIAL;
        } else if self.is_base64a {
            self.memmode &= !(mf::ALTROM0 | mf::ALTROM1);
        } else {
            return;
        }
        self.update_paging();
    }

    /// Makes room for `n` 16K Language Card RAM regions (region 0 always exists).
    pub fn alloc_lc_regions(&mut self, n: usize) {
        self.lc.resize(n.max(1) * LC_SIZE, 0);
        self.lc_region = self.lc_region.min(n.max(1) - 1);
        self.update_paging();
    }

    pub fn lc_region(&self) -> usize {
        self.lc_region
    }

    /// Selects the LC RAM region behind $D000-$FFFF (`SetMemMainLanguageCard`, Memory.cpp:454).
    pub fn set_lc_region(&mut self, region: usize) {
        if region != self.lc_region && region * LC_SIZE < self.lc.len() {
            self.lc_region = region;
            self.update_paging();
        }
    }

    /// Replaces the `LANGCARD_MASK` flags (`SetMemMode` of the LC manager, LanguageCard.cpp:~640).
    pub fn set_lc_flags(&mut self, bits: u32) {
        self.set_memmode((self.memmode & !mf::LANGCARD_MASK) | (bits & mf::LANGCARD_MASK));
    }

    pub fn memmode(&self) -> u32 {
        self.memmode
    }

    /// Replaces all flags (direct setter used by the LC switches of slice 3 and by tests).
    pub fn set_memmode(&mut self, mode: u32) {
        if mode != self.memmode {
            self.memmode = mode;
            self.update_paging();
        }
    }

    pub fn is_apple2e_or_above(&self) -> bool {
        self.is_e
    }

    pub fn aux_kind(&self) -> AuxCardKind {
        self.aux_kind
    }

    /// Selected RamWorks bank (0 for the other cards).
    pub fn aux_bank(&self) -> u16 {
        self.aux_bank
    }

    /// `MemSetPaging` (Memory.cpp:2523) for the offset `lo` inside $C0xx; `value` is the byte
    /// written (RamWorks bank). Reads and writes alike toggle $C054-$C057; the //e switches are
    /// ignored on the ][ family; $C058-$C05B select the Base64A ROM page.
    pub fn set_paging(&mut self, lo: u8, value: u8) {
        let bit = |m: u32, on: bool| {
            if on {
                self.memmode | m
            } else {
                self.memmode & !m
            }
        };
        let on = lo & 1 != 0;
        let mut mode = self.memmode;
        if self.is_e {
            mode = match lo {
                0x00 | 0x01 => bit(mf::STORE80, on),
                0x02 | 0x03 => bit(mf::AUXREAD, on),
                0x04 | 0x05 => bit(mf::AUXWRITE, on),
                0x06 | 0x07 => bit(mf::INTCXROM, on),
                0x08 | 0x09 => bit(mf::ALTZP, on),
                0x0A | 0x0B => bit(mf::SLOTC3ROM, on),
                0x54 | 0x55 => bit(mf::PAGE2, on),
                0x56 | 0x57 => bit(mf::HIRES, on),
                0x71 | 0x73 => {
                    if (value as u16) < self.banks && value as u16 != self.aux_bank {
                        self.aux_bank = value as u16;
                        self.update_paging();
                    }
                    mode
                }
                _ => mode,
            };
        } else if matches!(lo, 0x54..=0x57) {
            // No MMU, but the video-side PAGE2/HIRES state is kept here too.
            mode = if lo < 0x56 {
                bit(mf::PAGE2, on)
            } else {
                bit(mf::HIRES, on)
            };
        }
        if self.is_base64a {
            mode = match lo {
                0x58 | 0x59 => (mode & !mf::ALTROM0) | if on { mf::ALTROM0 } else { 0 },
                0x5A | 0x5B => (mode & !mf::ALTROM1) | if on { mf::ALTROM1 } else { 0 },
                _ => mode,
            };
        }
        self.set_memmode(mode);
    }

    /// CPU read; `None` means the floating bus (empty aux slot). $C000-$CFFF is the board's job;
    /// here it yields the internal Cx ROM or `None`.
    #[inline]
    pub fn read(&self, addr: u16) -> Option<u8> {
        let p = self.rd[(addr >> 8) as usize];
        let i = p.base as usize + (addr & 0xFF) as usize;
        match p.seg {
            Seg::Main => Some(self.main[i]),
            Seg::Aux => Some(self.aux[i]),
            Seg::Lc => Some(self.lc[i]),
            Seg::Rom => Some(self.rom[i]),
            Seg::Io => self.read_cx(addr),
            Seg::Floating | Seg::None => None,
        }
    }

    /// A CPU write: [`Memory::write`], also mirrored into the ][-family aux RAM while
    /// [`Memory::set_dual_write_aux`] is on (`if (memVidHD) *(memVidHD + addr) = a`, only for pages
    /// with `memwrite != NULL`, cpu_general.inl:112).
    #[inline]
    pub fn cpu_write(&mut self, addr: u16, value: u8) {
        if self.dual_write_aux && self.is_writable(addr) {
            self.vidhd_aux[addr as usize] = value;
        }
        self.write(addr, value);
    }

    /// Enables the VidHD dual write (`memVidHD = IsWriteAux() ? memaux : NULL`). Only the ][ family
    /// has it: a //e has real aux paging, so this is ignored there.
    pub fn set_dual_write_aux(&mut self, on: bool) {
        if self.is_e {
            return;
        }
        if on && self.vidhd_aux.is_empty() {
            self.vidhd_aux = vec![0; 0x10000];
        }
        self.dual_write_aux = on;
    }

    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        let p = self.wr[(addr >> 8) as usize];
        let i = p.base as usize + (addr & 0xFF) as usize;
        match p.seg {
            Seg::Main => self.main[i] = value,
            Seg::Aux => self.aux[i] = value,
            Seg::Lc => self.lc[i] = value,
            Seg::Rom | Seg::Io | Seg::Floating | Seg::None => {}
        }
    }

    /// True if a CPU write to `addr` lands in RAM (AppleWin `memwrite[page] != NULL`); false for
    /// ROM, I/O space and the write-protected Language Card.
    pub fn is_writable(&self, addr: u16) -> bool {
        matches!(
            self.wr[(addr >> 8) as usize].seg,
            Seg::Main | Seg::Aux | Seg::Lc
        )
    }

    /// Internal Cx ROM at $C100-$CFFF (Memory.cpp:1367-1387): used for everything when INTCXROM,
    /// and for $C3xx unless SLOTC3ROM. `None` when a slot owns the space (see `Board::cx_access`) or the model
    /// has no internal ROM.
    pub fn read_cx(&self, addr: u16) -> Option<u8> {
        let internal = self.memmode & mf::INTCXROM != 0
            || (addr >> 8 == 0xC3 && self.memmode & mf::SLOTC3ROM == 0);
        if internal {
            self.internal_cx(addr)
        } else {
            None
        }
    }

    /// Byte of the internal Cx ROM at `addr` (`$C100-$CFFF`) whatever the switches say; `None`
    /// on models without one.
    pub fn internal_cx(&self, addr: u16) -> Option<u8> {
        if !(0xC100..=0xCFFF).contains(&addr) {
            return None;
        }
        self.cx_rom.get((addr - 0xC000) as usize).copied()
    }

    /// Main RAM, ignoring paging.
    pub fn peek_main(&self, addr: u16) -> u8 {
        self.main[addr as usize]
    }

    /// `MemGetMainPtr(0)[addr]` (Memory.cpp:1734-1738, used by `ReadFloatingBus`, :2460-2470): the
    /// check that picks the CPU read image `mem` over `memmain` looks at page 0 only, so with
    /// ALTZP=0 the whole index goes through the read image, and RAMRD (or 80STORE+PAGE2) makes the
    /// video scanner "see" the auxiliary RAM; with ALTZP=1 it is main RAM. AUD-C-050: the real
    /// hardware reads main, but AppleWin is the oracle.
    pub fn peek_floating_image(&self, addr: u16) -> u8 {
        if self.rd[0].seg == Seg::Main {
            let p = self.rd[(addr >> 8) as usize];
            if p.seg == Seg::Aux {
                return self.aux[p.base as usize + (addr & 0xFF) as usize];
            }
        }
        self.main[addr as usize]
    }

    pub fn poke_main(&mut self, addr: u16, value: u8) {
        self.main[addr as usize] = value;
    }

    /// Aux RAM of the current bank, ignoring paging (0 / discarded for an empty slot).
    pub fn peek_aux(&self, addr: u16) -> u8 {
        if !self.is_e && !self.vidhd_aux.is_empty() {
            return self.vidhd_aux[addr as usize];
        }
        self.aux_index(addr as u32).map_or(0, |i| self.aux[i])
    }

    pub fn poke_aux(&mut self, addr: u16, value: u8) {
        if let Some(i) = self.aux_index(addr as u32) {
            self.aux[i] = value;
        }
    }

    pub fn state(&self) -> MemoryState {
        MemoryState {
            main: self.main.clone(),
            aux: self.aux.clone(),
            lc: self.lc.clone(),
            lc_region: self.lc_region,
            memmode: self.memmode,
            aux_bank: self.aux_bank,
        }
    }

    /// Whether `s` was taken from a memory of the same sizes.
    pub fn fits(&self, s: &MemoryState) -> bool {
        s.main.len() == self.main.len()
            && s.aux.len() == self.aux.len()
            && s.lc.len() == self.lc.len()
            && s.aux_bank < self.banks
            && s.lc_region * LC_SIZE < self.lc.len()
    }

    /// Restores a state taken from a memory of the same configuration; false if sizes differ.
    pub fn restore(&mut self, s: &MemoryState) -> bool {
        if !self.fits(s) {
            return false;
        }
        self.main.copy_from_slice(&s.main);
        self.aux.copy_from_slice(&s.aux);
        self.lc.copy_from_slice(&s.lc);
        self.lc_region = s.lc_region;
        self.memmode = s.memmode;
        self.aux_bank = s.aux_bank;
        self.update_paging();
        true
    }

    fn aux_index(&self, off: u32) -> Option<usize> {
        match self.aux_kind {
            AuxCardKind::Empty => None,
            // 1K card: every aux access mirrors into the same 1K (Memory.cpp:1536-1585).
            AuxCardKind::Col80_1K => Some((off & 0x3FF) as usize),
            _ => Some(self.aux_bank as usize * BANK_SIZE + off as usize),
        }
    }

    /// Page of aux RAM starting at aux offset `off` (page aligned), for reading or writing.
    fn aux_page(&self, off: u32, write: bool) -> Page {
        match self.aux_kind {
            AuxCardKind::Empty if write => NONE,
            AuxCardKind::Empty => Page {
                seg: Seg::Floating,
                base: 0,
            },
            AuxCardKind::Col80_1K => Page {
                seg: Seg::Aux,
                base: off & 0x300,
            },
            _ => Page {
                seg: Seg::Aux,
                base: self.aux_bank as u32 * BANK_SIZE as u32 + off,
            },
        }
    }

    /// `UpdatePaging` (Memory.cpp:1331): rebuilds both page tables from the flags.
    fn update_paging(&mut self) {
        let f = self.memmode;
        let has = |m: u32| f & m != 0;
        let main = |p: u32| Page {
            seg: Seg::Main,
            base: p << 8,
        };
        let mut rd = [NONE; 256];
        let mut wr = [NONE; 256];
        for p in 0..0xC0u32 {
            let (r, w) = if p < 2 && !has(mf::ALTZP) {
                (main(p), main(p))
            } else if p < 2 {
                (self.aux_page(p << 8, false), self.aux_page(p << 8, true))
            } else {
                (
                    if has(mf::AUXREAD) {
                        self.aux_page(p << 8, false)
                    } else {
                        main(p)
                    },
                    if has(mf::AUXWRITE) {
                        self.aux_page(p << 8, true)
                    } else {
                        main(p)
                    },
                )
            };
            rd[p as usize] = r;
            wr[p as usize] = w;
        }
        // 80STORE: PAGE2 (and HIRES for the hi-res pages) override AUXREAD/AUXWRITE.
        if has(mf::STORE80) {
            let hires_pages = if has(mf::HIRES) { 0x20..0x40 } else { 0..0 };
            for p in (4..8).chain(hires_pages) {
                let (r, w) = if has(mf::PAGE2) {
                    (self.aux_page(p << 8, false), self.aux_page(p << 8, true))
                } else {
                    (main(p), main(p))
                };
                rd[p as usize] = r;
                wr[p as usize] = w;
            }
        }
        for p in 0xC0..0xD0 {
            rd[p] = Page {
                seg: Seg::Io,
                base: 0,
            };
            wr[p] = rd[p];
        }
        // $D000-$FFFF: Language Card RAM (aux when ALTZP) or ROM (+ Base64A page offset).
        let rom_page =
            ((has(mf::ALTROM0) as usize) | (has(mf::ALTROM1) as usize) << 1) % self.rom_pages;
        for p in 0xD0..0x100u32 {
            let bank_off = if p < 0xE0 && !has(mf::BANK2) {
                0x1000
            } else {
                0
            };
            let ram = |aux: bool, write: bool| {
                if aux {
                    self.aux_page((p << 8) - bank_off, write)
                } else {
                    Page {
                        seg: Seg::Lc,
                        base: (self.lc_region * LC_SIZE) as u32 + ((p - 0xC0) << 8) - bank_off,
                    }
                }
            };
            let altzp = has(mf::ALTZP);
            rd[p as usize] = if has(mf::HIGHRAM) {
                ram(altzp, false)
            } else {
                Page {
                    seg: Seg::Rom,
                    base: (rom_page * SYS_ROM_SIZE) as u32 + ((p - 0xD0) << 8),
                }
            };
            wr[p as usize] = if has(mf::WRITERAM) {
                ram(altzp, true)
            } else {
                NONE
            };
        }
        self.rd = rd;
        self.wr = wr;
    }
}
