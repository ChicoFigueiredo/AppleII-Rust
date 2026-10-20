//! Uthernet I: a CS8900A Ethernet controller in I/O mode (Uthernet1.cpp, adapted by AppleWin from
//! VICE's TFE support).
//!
//! The 16 bytes at `$C080+slot*16` are the eight 16-bit I/O ports of the chip; most traffic goes
//! through the 4 KB *PacketPage* memory behind the PacketPage pointer/data ports. Only the
//! registers the TFE driver needs have side effects (listed at Uthernet1.h:23-36).

use crate::network::{crc32, NetworkBackend, MAX_RXLENGTH, MAX_TXLENGTH, MIN_TXLENGTH};
use apple2_core::card::CardCtx;
use apple2_core::Card;

const IO_REGISTERS: usize = 0x10;
const PACKETPAGE_SIZE: usize = 0x1000;

// I/O ports (Uthernet1.h:49-56).
const ADDR_RXTXDATA: u8 = 0x00;
const ADDR_RXTXDATA2: u8 = 0x02;
const ADDR_TXCMD: u8 = 0x04;
const ADDR_TXLENGTH: u8 = 0x06;
const ADDR_INTSTQUEUE: u8 = 0x08;
const ADDR_PP_PTR: u8 = 0x0A;
const ADDR_PP_DATA: u8 = 0x0C;
const ADDR_PP_DATA2: u8 = 0x0E;

// PacketPage addresses (:59-114).
pub const PP_PRODUCTID: u16 = 0x0000;
const PP_IOBASE: u16 = 0x0020;
const PP_INTNO: u16 = 0x0022;
const PP_DMA_CHAN: u16 = 0x0024;
const PP_STATUS_EVENT: u16 = 0x0120;
const PP_TXCMD: u16 = 0x0144;
const PP_TXLENGTH: u16 = 0x0146;
const PP_LOG_ADDR_FILTER: u16 = 0x0150;
pub const PP_MAC_ADDR: u16 = 0x0158;
pub const PP_RXSTATUS: u16 = 0x0400;
pub const PP_RXLENGTH: u16 = 0x0402;
pub const PP_RX_FRAMELOC: u16 = 0x0404;
const PP_TX_FRAMELOC: u16 = 0x0A00;
const PP_CC_RXCFG: u16 = 0x0102;
pub const PP_CC_RXCTL: u16 = 0x0104;
const PP_CC_TXCFG: u16 = 0x0106;
const PP_CC_TXCMD: u16 = 0x0108;
const PP_CC_BUFCFG: u16 = 0x010A;
const PP_CC_LINECTL: u16 = 0x0112;
const PP_CC_SELFCTL: u16 = 0x0114;
const PP_CC_BUSCTL: u16 = 0x0116;
const PP_CC_TESTCTL: u16 = 0x0118;
const PP_SE_ISQ: u16 = 0x0120;
pub const PP_SE_RXEVENT: u16 = 0x0124;
const PP_SE_TXEVENT: u16 = 0x0128;
const PP_SE_BUFEVENT: u16 = 0x012C;
const PP_SE_RXMISS: u16 = 0x0130;
const PP_SE_TXCOL: u16 = 0x0132;
const PP_SE_LINEST: u16 = 0x0134;
const PP_SE_SELFST: u16 = 0x0136;
pub const PP_SE_BUSST: u16 = 0x0138;
const PP_SE_TDR: u16 = 0x013C;

pub struct Uthernet1 {
    backend: Box<dyn NetworkBackend>,
    ia_mac: [u8; 6],
    hash_mask: [u32; 2],
    recv_broadcast: bool,
    recv_mac: bool,
    recv_multicast: bool,
    recv_correct: bool,
    recv_promiscuous: bool,
    recv_hashfilter: bool,
    tfe: [u8; IO_REGISTERS],
    txcollect_buffer: u16,
    rx_buffer: u16,
    inside_frameloc: bool,
    pp: Box<[u8; PACKETPAGE_SIZE]>,
    pp_ptr: u16,
}

/// What `tfe_should_accept` found out about a frame.
#[derive(Default)]
struct RxInfo {
    hashed: bool,
    hash_index: u32,
    correct_mac: bool,
    broadcast: bool,
    multicast: bool,
}

impl Uthernet1 {
    pub fn new(backend: Box<dyn NetworkBackend>) -> Self {
        let mut c = Uthernet1 {
            backend,
            ia_mac: [0; 6],
            hash_mask: [0; 2],
            recv_broadcast: false,
            recv_mac: false,
            recv_multicast: false,
            recv_correct: false,
            recv_promiscuous: false,
            recv_hashfilter: false,
            tfe: [0; IO_REGISTERS],
            txcollect_buffer: PP_TX_FRAMELOC,
            rx_buffer: PP_RXSTATUS,
            inside_frameloc: false,
            pp: Box::new([0; PACKETPAGE_SIZE]),
            pp_ptr: 0,
        };
        c.init();
        c
    }

    /// `Init` (:192-262): power-on register values from the CS8900A data sheet (page 19 unless noted).
    fn init(&mut self) {
        self.ia_mac = [0; 6];
        self.hash_mask = [0; 2];
        self.recv_broadcast = false;
        self.recv_mac = false;
        self.recv_multicast = false;
        self.recv_correct = false;
        self.recv_promiscuous = false;
        self.recv_hashfilter = false;
        self.tfe = [0; IO_REGISTERS];
        self.txcollect_buffer = PP_TX_FRAMELOC;
        self.rx_buffer = PP_RXSTATUS;
        self.inside_frameloc = false;
        self.pp.fill(0);
        self.pp_ptr = 0;
        self.set_pp32(PP_PRODUCTID, 0x0700_630E); // 0E630007 for Rev. B, reversed order
        self.set_pp16(PP_IOBASE, 0x0300);
        self.set_pp16(PP_INTNO, 0x0004);
        self.set_pp16(PP_DMA_CHAN, 0x0003);
        for (a, v) in [
            (PP_CC_RXCFG, 0x0003),
            (PP_CC_RXCTL, 0x0005),
            (PP_CC_TXCFG, 0x0007),
            (PP_CC_TXCMD, 0x0009),
            (PP_CC_BUFCFG, 0x000B),
            (PP_CC_LINECTL, 0x0013),
            (PP_CC_SELFCTL, 0x0015),
            (PP_CC_BUSCTL, 0x0017),
            (PP_CC_TESTCTL, 0x0019),
            (PP_SE_ISQ, 0x0000),
            (PP_SE_RXEVENT, 0x0004),
            (PP_SE_TXEVENT, 0x0008),
            (PP_SE_BUFEVENT, 0x000C),
            (PP_SE_RXMISS, 0x0010),
            (PP_SE_TXCOL, 0x0012),
            (PP_SE_LINEST, 0x0014),
            (PP_SE_SELFST, 0x0016),
            (PP_SE_BUSST, 0x0018),
            (PP_SE_TDR, 0x001C),
        ] {
            self.set_pp16(a, v);
        }
    }

    fn pp16(&self, a: u16) -> u16 {
        let a = a as usize & (PACKETPAGE_SIZE - 1);
        u16::from_le_bytes([self.pp[a], self.pp[(a + 1) & (PACKETPAGE_SIZE - 1)]])
    }

    fn set_pp16(&mut self, a: u16, v: u16) {
        let a = a as usize & (PACKETPAGE_SIZE - 1);
        self.pp[a] = v as u8;
        self.pp[(a + 1) & (PACKETPAGE_SIZE - 1)] = (v >> 8) as u8;
    }

    fn set_pp32(&mut self, a: u16, v: u32) {
        self.set_pp16(a, v as u16);
        self.set_pp16(a + 2, (v >> 16) as u16);
    }

    fn pp8(&self, a: u16) -> u8 {
        self.pp[a as usize & (PACKETPAGE_SIZE - 1)]
    }

    fn set_pp8(&mut self, a: u16, v: u8) {
        self.pp[a as usize & (PACKETPAGE_SIZE - 1)] = v;
    }

    fn tfe16(&self, a: u8) -> u16 {
        u16::from_le_bytes([self.tfe[a as usize], self.tfe[a as usize + 1]])
    }

    fn set_tfe16(&mut self, a: u8, v: u16) {
        self.tfe[a as usize] = v as u8;
        self.tfe[a as usize + 1] = (v >> 8) as u8;
    }

    /// The MAC address the guest programmed.
    pub fn mac(&self) -> [u8; 6] {
        self.ia_mac
    }

    /// `tfe_sideeffects_write_pp_on_txframe` (:265-310): the last octet of a frame starts the send.
    fn sideeffects_write_pp_on_txframe(&mut self, ppaddress: u16) {
        let txlen = self.pp16(PP_TXLENGTH);
        if ppaddress != PP_TX_FRAMELOC.wrapping_add(txlen).wrapping_sub(1) {
            return;
        }
        let txcmd = self.pp16(PP_TXCMD);
        let busst = self.pp16(PP_SE_BUSST);
        let txlen = txlen as usize;
        let inhibit_crc = txcmd & 0x1000 != 0;
        if txlen > MAX_TXLENGTH || (txlen > MAX_TXLENGTH - 4 && !inhibit_crc) || txlen < MIN_TXLENGTH {
            return; // not allowed: ignored
        }
        self.set_pp16(PP_SE_BUSST, busst & !0x180);
        let start = PP_TX_FRAMELOC as usize;
        let frame = self.pp[start..start + txlen].to_vec();
        // FORCE / ONECOLL / INHIBITCRC / TXPADDIS are not used by any existing backend (:972).
        self.backend.transmit(&frame);
        self.txcollect_buffer = PP_TX_FRAMELOC;
    }

    /// `tfe_sideeffects_write_pp` (:315-434), called after the octets are written.
    fn sideeffects_write_pp(&mut self, ppaddress: u16, odd: usize) {
        let mut content = self.pp16(ppaddress);
        match ppaddress {
            PP_CC_RXCFG => {
                if content & 0x40 != 0 {
                    content &= !0x40; // "act once" bit
                    self.set_pp16(ppaddress, content);
                }
            }
            PP_CC_RXCTL => {
                self.recv_broadcast = content & 0x0800 != 0;
                self.recv_mac = content & 0x0400 != 0;
                self.recv_multicast = content & 0x0200 != 0;
                self.recv_correct = content & 0x0100 != 0;
                self.recv_promiscuous = content & 0x0080 != 0;
                self.recv_hashfilter = content & 0x0040 != 0;
            }
            PP_TXCMD => {
                // The transmit status command gets the last transmit command.
                let v = self.pp16(PP_TXCMD);
                self.set_pp16(PP_CC_TXCMD, v);
                self.txcollect_buffer = PP_TX_FRAMELOC;
            }
            PP_TXLENGTH => {
                let txlength = self.pp16(PP_TXLENGTH) as usize;
                let txcommand = self.pp16(PP_TXCMD);
                let busst = self.pp16(PP_SE_BUSST);
                if txlength > MAX_TXLENGTH || (txlength > MAX_TXLENGTH - 4 && txcommand & 0x1000 == 0) {
                    self.set_pp16(PP_SE_BUSST, (busst | 0x80) & !0x100); // too big: error
                } else {
                    self.set_pp16(PP_SE_BUSST, (busst & !0x80) | 0x100); // ready for the next frame
                }
            }
            PP_LOG_ADDR_FILTER | 0x0152 | 0x0154 | 0x0156 => {
                let pos = 8 * (ppaddress - PP_LOG_ADDR_FILTER) as u32 + 8 * odd as u32;
                let word = (pos >= 32) as usize;
                let shift = pos & 31;
                self.hash_mask[word] &= !(0xFF << shift);
                self.hash_mask[word] |= (self.pp8(ppaddress + odd as u16) as u32) << shift;
            }
            PP_MAC_ADDR | 0x015A | 0x015C => {
                self.ia_mac[(ppaddress - PP_MAC_ADDR) as usize + odd] = self.pp8(ppaddress + odd as u16);
            }
            // PP_SE_RXEVENT and PP_SE_BUSST are read-only: the write is accepted but has no effect.
            _ => {}
        }
    }

    /// `tfe_sideeffects_read_pp` (:439-476), called before the octets are read.
    fn sideeffects_read_pp(&mut self, ppaddress: u16) {
        if ppaddress == PP_SE_RXEVENT {
            // Reading this before all octets of the frame are read performs an "implied skip".
            let v = self.receive();
            self.set_pp16(PP_RXSTATUS, v);
            self.set_pp16(PP_SE_RXEVENT, v);
        }
    }

    /// `tfe_proceed_rx_buffer` (:479-542).
    fn proceed_rx_buffer(&mut self, odd: bool) {
        let mut proceed = false;
        if self.rx_buffer == PP_RX_FRAMELOC.wrapping_add(self.pp16(PP_RXLENGTH)) {
            // Everything available has been read: start again.
            self.rx_buffer = PP_RXSTATUS;
            self.inside_frameloc = false;
        } else {
            match self.rx_buffer {
                PP_RXSTATUS => {
                    if odd {
                        proceed = true;
                        self.inside_frameloc = false;
                    }
                }
                PP_RXLENGTH => proceed = odd,
                PP_RX_FRAMELOC => {
                    if !odd {
                        if self.inside_frameloc {
                            proceed = true;
                        } else {
                            self.inside_frameloc = true;
                        }
                    }
                }
                _ => proceed = !odd,
            }
        }
        if proceed {
            let v = self.pp16(self.rx_buffer);
            self.set_tfe16(ADDR_RXTXDATA, v);
            self.rx_buffer = self.rx_buffer.wrapping_add(2);
        }
    }

    /// `tfe_should_accept` (:781-860).
    fn should_accept(&self, buffer: &[u8]) -> (bool, RxInfo) {
        let mut info = RxInfo::default();
        let da = &buffer[..6];
        if da == self.ia_mac {
            info.correct_mac = true;
            if self.recv_mac || self.recv_promiscuous {
                return (true, info);
            }
        }
        if da == [0xFF; 6] {
            info.broadcast = true; // broadcasts cannot be accepted by the hash filter
            return (self.recv_broadcast || self.recv_promiscuous, info);
        }
        let hashreg = (!crc32(da) >> 26) & 0x3F;
        info.hashed = self.hash_mask[(hashreg >= 32) as usize] & (1 << (hashreg & 0x1F)) != 0;
        if info.hashed {
            info.hash_index = hashreg;
            if buffer[0] & 0x80 != 0 {
                info.multicast = true;
                info.hashed = false; // a multicast address matching the hash clears the hashed bit
                return (self.recv_multicast || self.recv_promiscuous, info);
            }
            return (self.recv_hashfilter || self.recv_promiscuous, info);
        }
        (self.recv_promiscuous, info)
    }

    /// `tfe_receive` (:867-961): fetches the next acceptable frame into the PacketPage and returns
    /// the RxEvent status word (0x0004 = nothing).
    fn receive(&mut self) -> u16 {
        let mut ret_val = 0x0004u16;
        while let Some(mut frame) = self.backend.receive() {
            if frame.len() < 6 {
                continue;
            }
            if frame.len() & 1 != 0 {
                frame.push(0); // length has to be even
            }
            let (accept, info) = self.should_accept(&frame);
            if !accept {
                continue; // look for another one
            }
            let len = frame.len();
            ret_val |= 0x0100; // RxOK
            if info.multicast {
                ret_val |= 0x0200;
            } else if info.hashed {
                ret_val |= 0x0040;
            }
            if info.hashed {
                ret_val |= (info.hash_index as u16) << 9; // the 2nd, special format
            } else {
                if info.correct_mac {
                    ret_val |= 0x0400;
                }
                if info.broadcast {
                    ret_val |= 0x0800;
                }
                if len < crate::network::MIN_RXLENGTH {
                    ret_val |= 0x2000;
                }
                if len > MAX_RXLENGTH {
                    ret_val |= 0x4000;
                }
            }
            let len = len.min(MAX_RXLENGTH); // octets beyond the maximum are discarded
            self.set_pp16(PP_RXLENGTH, len as u16);
            for (i, &b) in frame[..len].iter().enumerate() {
                self.set_pp8(PP_RX_FRAMELOC + i as u16, b);
            }
            self.rx_buffer = PP_RXSTATUS; // reading starts with RxStatus and RxLength (4.10.9)
            break;
        }
        ret_val
    }

    /// `tfe_read` (:545-621).
    pub fn tfe_read(&mut self, io: u8) -> u8 {
        match io {
            ADDR_PP_DATA | 0x0D => {
                let pp = self.pp_ptr & (PACKETPAGE_SIZE as u16 - 1);
                self.sideeffects_read_pp(pp);
                let v = self.pp16(pp);
                self.set_tfe16(ADDR_PP_DATA, v);
            }
            ADDR_INTSTQUEUE | 0x09 => {
                let v = self.pp16(PP_STATUS_EVENT);
                self.set_tfe16(ADDR_INTSTQUEUE, v);
            }
            ADDR_RXTXDATA | 0x01 => self.proceed_rx_buffer(io & 1 != 0),
            _ => {} // TXCMD/TXLENGTH (write-only) and the 32-bit ports return the raw register
        }
        self.tfe[io as usize]
    }

    /// `tfe_store` (:623-760).
    pub fn tfe_store(&mut self, io: u8, byte: u8) {
        match io {
            ADDR_RXTXDATA | 0x01 => {
                let at = self.txcollect_buffer;
                self.set_pp8(at, byte);
                self.txcollect_buffer = at.wrapping_add(1);
                self.sideeffects_write_pp_on_txframe(at);
            }
            ADDR_INTSTQUEUE | 0x09 | ADDR_RXTXDATA2 | 0x03 | ADDR_PP_DATA2 | 0x0F => {} // read-only / unsupported
            ADDR_TXCMD | 0x05 => {
                self.tfe[io as usize] = byte;
                self.set_pp8(io as u16 - ADDR_TXCMD as u16 + PP_TXCMD, byte);
                self.sideeffects_write_pp(PP_TXCMD, (io - ADDR_TXCMD) as usize);
            }
            ADDR_TXLENGTH | 0x07 => {
                self.tfe[io as usize] = byte;
                self.set_pp8(io as u16 - ADDR_TXLENGTH as u16 + PP_TXLENGTH, byte);
                self.sideeffects_write_pp(PP_TXLENGTH, (io - ADDR_TXLENGTH) as usize);
            }
            ADDR_PP_DATA | 0x0D => {
                // Keep the visible word equal to the real value, then overlay the written byte.
                let v = self.pp16(self.pp_ptr);
                self.set_tfe16(ADDR_PP_DATA, v);
                self.tfe[io as usize] = byte;
                let pp = self.pp_ptr & (PACKETPAGE_SIZE as u16 - 1);
                let word = self.tfe16(ADDR_PP_DATA);
                self.set_pp16(pp, word);
                self.sideeffects_write_pp(pp, (io - ADDR_PP_DATA) as usize);
            }
            ADDR_PP_PTR | 0x0B => {
                self.tfe[io as usize] = byte;
                self.pp_ptr = self.tfe16(ADDR_PP_PTR) & !1; // an odd address is "corrected" downwards
            }
            _ => {}
        }
    }
}

impl Card for Uthernet1 {
    fn name(&self) -> &'static str {
        "Uthernet"
    }

    fn io_read(&mut self, lo: u8, _ctx: &mut CardCtx) -> u8 {
        self.tfe_read(lo)
    }

    fn io_write(&mut self, lo: u8, value: u8, _ctx: &mut CardCtx) {
        self.tfe_store(lo, value);
    }

    /// `TfeIoCxxx` is `IO_Null`: no slot ROM, the floating bus shows through.
    fn slot_rom_read(&mut self, _offset: u8, ctx: &mut CardCtx) -> u8 {
        ctx.floating_bus
    }

    fn reset(&mut self, power_cycle: bool, _ctx: &mut CardCtx) {
        if power_cycle {
            self.init();
        }
    }

    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        self.backend.update(cycles);
    }

    fn save(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(IO_REGISTERS + PACKETPAGE_SIZE + 6);
        v.extend_from_slice(&self.tfe);
        v.extend_from_slice(&self.pp[..]);
        v.extend_from_slice(&self.txcollect_buffer.to_le_bytes());
        v.extend_from_slice(&self.rx_buffer.to_le_bytes());
        v
    }

    /// `LoadSnapshot` (:1092-1136): the derived filter state is recomputed from the PacketPage.
    fn load(&mut self, state: &[u8]) -> bool {
        if state.len() != IO_REGISTERS + PACKETPAGE_SIZE + 4 {
            return false;
        }
        self.tfe.copy_from_slice(&state[..IO_REGISTERS]);
        self.pp.copy_from_slice(&state[IO_REGISTERS..IO_REGISTERS + PACKETPAGE_SIZE]);
        let t = IO_REGISTERS + PACKETPAGE_SIZE;
        self.txcollect_buffer = u16::from_le_bytes([state[t], state[t + 1]]);
        self.rx_buffer = u16::from_le_bytes([state[t + 2], state[t + 3]]);
        self.pp_ptr = self.tfe16(ADDR_PP_PTR) & !1;
        self.sideeffects_write_pp(PP_CC_RXCTL, 0);
        for i in 0..8u16 {
            self.sideeffects_write_pp((PP_LOG_ADDR_FILTER + i) & !1, (i & 1) as usize);
        }
        for i in 0..6u16 {
            self.sideeffects_write_pp((PP_MAC_ADDR + i) & !1, (i & 1) as usize);
        }
        true
    }
}
