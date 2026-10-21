//! The generic hard disk controller (AppleWin Harddisk.cpp, firmware `HDC-SmartPort.bin`,
//! `Hddrvr.bin`, `Hddrvr-v2.bin`).
//!
//! Memory map (IO addr + slot*$10), "hddrvr" v1/v2 and SmartPort firmware:
//! ```text
//! C080 (r)   EXECUTE AND RETURN STATUS (SmartPort: later reads just return STATUS)
//! C081 (r)   STATUS: b7 busy, b0 error
//! C082 (r/w) COMMAND            BLK $00 status, $01 read, $02 write, $03 format; SP $80-$82
//! C083 (r/w) UNIT NUMBER        BLK DSSS0000, SP $00,$01...
//! C084/5     MEMORY BUFFER lo/hi
//! C086-8     BLOCK NUMBER (SP: C086 is the status code for the Status command)
//! C089 (r)   image size in blocks, lo      (w) 6-deep FIFO: command, unit, mem(2), block(2)
//! C08A (r)   image size in blocks, hi      (w) 7-deep FIFO for SmartPort, first byte OR $80
//! ```
//! DMA to and from the Apple's memory goes through [`CardCtx::dma_write`], so it respects the
//! current paging (GH#1319) and fails on I/O space and ROM (GH#1007).

use crate::disk2::DiskStatus;
use crate::image::{image_title, Class, DiskImage, ImageError, HD_BLOCK_SIZE};
use apple2_core::card::{Card, CardCtx, SLOT_ROM_SIZE};
use apple2_core::Model;
use std::any::Any;

/// Drives a controller can hold (SmartPort units 1-8).
pub const NUM_HARDDISKS: usize = 8;
/// Maximum blocks reported: 24-bit SmartPort block numbers (Harddisk.h:46).
pub const MAX_NUM_BLOCKS: u32 = 0x007F_FFFF;
const CYCLES_FOR_DMA_RW_BLOCK: u64 = HD_BLOCK_SIZE as u64;
const CYCLES_FOR_FORMATTING_1_BLOCK: u64 = 100; // arbitrary

// ProDOS block and SmartPort commands.
const BLK_CMD_STATUS: u8 = 0x00;
const BLK_CMD_READ: u8 = 0x01;
const BLK_CMD_WRITE: u8 = 0x02;
const BLK_CMD_FORMAT: u8 = 0x03;
const SP_CMD_BASE: u8 = 0x80;
const SP_CMD_STATUS: u8 = SP_CMD_BASE;
const SP_CMD_READBLOCK: u8 = SP_CMD_BASE + 0x01;
const SP_CMD_WRITEBLOCK: u8 = SP_CMD_BASE + 0x02;
const SP_CMD_FORMAT: u8 = SP_CMD_BASE + 0x03;
/// AppleWin vendor-specific command: "report the busy status again".
const SP_CMD_BUSY_STATUS: u8 = SP_CMD_BASE + 0x3F;
const SP_STATUS_STATUS: u8 = 0x00;
const SP_STATUS_GETDIB: u8 = 0x03;

const DEVICE_OK: u8 = 0x00;
const BADCTL: u8 = 0x21;
const DEVICE_IO_ERROR: u8 = 0x27;
const DEVICE_NOT_CONNECTED: u8 = 0x28;
const NOWRITE: u8 = 0x2B;
const ERRORCODE_MASK: u8 = 0x3F;
const STATUS_ERROR: u8 = 0x01;
const STATUS_BUSY: u8 = 0x80;
const fn status_error(err: u8) -> u8 {
    (err << 1) | STATUS_ERROR
}

/// Firmware revision reported in SmartPort device information blocks (the AppleWin version
/// numbers there; only informational).
const FW_VERSION: (u8, u8) = (130, 0);

/// Which slot ROM to run (`HdcMode` and the `-hdc-firmware` options).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdcMode {
    /// SmartPort on the enhanced //e, block mode elsewhere.
    Default,
    SmartPort,
    /// Block mode, 2 devices.
    BlockMode2Devices,
    /// Block mode, 4 devices.
    BlockMode4Devices,
}

/// Original firmware revisions (command-line overrides in AppleWin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdcFirmware {
    SmartPort,
    HddrvrV1,
    HddrvrV2,
}

struct Unit {
    image: Option<DiskImage>,
    name: String,
    full_name: String,
    error: u8,
    memblock: u16,
    diskblock: u32,
    buf_ptr: u16,
    buf: [u8; HD_BLOCK_SIZE],
    status_next: DiskStatus,
    status_prev: DiskStatus,
}

impl Unit {
    fn new() -> Self {
        Unit {
            image: None,
            name: String::new(),
            full_name: String::new(),
            error: 0,
            memblock: 0,
            diskblock: 0,
            buf_ptr: 0,
            buf: [0; HD_BLOCK_SIZE],
            status_next: DiskStatus::Off,
            status_prev: DiskStatus::Off,
        }
    }

    fn loaded(&self) -> bool {
        self.image.is_some()
    }
}

pub struct HardDiskCard {
    slot: u8,
    firmware: [u8; SLOT_ROM_SIZE],
    is_firmware_v1_or_2: bool,
    units: [Unit; NUM_HARDDISKS],
    /// SmartPort unit 0: always "loaded".
    controller: Unit,
    unit_num: u8,
    command: u8,
    fifo_idx: u8,
    status_code: u8,
    not_busy_cycle: u64,
    user_num_blocks: u32,
}

impl HardDiskCard {
    /// Controller for `slot` in a machine of `model` (the firmware is adjusted for the model:
    /// block mode on anything but the enhanced //e unless SmartPort is forced, and a Shift-key
    /// button test on the ][ and ][+, GH#1234).
    pub fn new(slot: u8, model: Model, mode: HdcMode) -> Self {
        Self::with_firmware(slot, model, mode, HdcFirmware::SmartPort)
    }

    pub fn with_firmware(slot: u8, model: Model, mode: HdcMode, fw: HdcFirmware) -> Self {
        let rom = match fw {
            HdcFirmware::SmartPort => apple2_roms::HDC_SMARTPORT_BIN,
            HdcFirmware::HddrvrV1 => apple2_roms::HDDRVR_BIN,
            HdcFirmware::HddrvrV2 => apple2_roms::HDDRVR_V2_BIN,
        };
        let mut firmware = [0u8; SLOT_ROM_SIZE];
        firmware.copy_from_slice(rom);
        if matches!(
            mode,
            HdcMode::BlockMode2Devices | HdcMode::BlockMode4Devices
        ) {
            firmware[0x07] = 0x3C; // block mode (not SmartPort)
            let drives: u8 = if mode == HdcMode::BlockMode2Devices {
                1
            } else {
                3
            };
            firmware[0xFE] = (firmware[0xFE] & 0xCF) | (drives << 4); // 2 or 4 drives
        } else if model != Model::Apple2eEnhanced && mode != HdcMode::SmartPort {
            firmware[0x07] = 0x3C;
        }
        if model.is_apple2() && model != Model::Apple2e {
            // With no joystick PB0 floats high (the button looks pressed): detect the Shift key
            // instead, BIT $C061; BMI -> BIT $C063; BPL.
            for i in 0..SLOT_ROM_SIZE - 3 {
                if firmware[i..i + 4] == [0x2C, 0x61, 0xC0, 0x30] {
                    firmware[i + 1] = 0x63;
                    firmware[i + 3] = 0x10;
                    break;
                }
            }
        }
        let mut controller = Unit::new();
        controller.name = "SmartPort".into();
        HardDiskCard {
            slot,
            firmware,
            is_firmware_v1_or_2: fw != HdcFirmware::SmartPort,
            units: std::array::from_fn(|_| Unit::new()),
            controller,
            unit_num: slot << 4, // HARDDISK_1 << 7 | slot << 4
            command: 0,
            fifo_idx: 0,
            status_code: 0,
            not_busy_cycle: 0,
            user_num_blocks: 0,
        }
    }

    // ---- Frontend API. ----

    /// Puts `image` in hard disk `drive` (0..8), unplugging what was there and the same image
    /// from another drive. Returns the previous image.
    pub fn insert(&mut self, drive: usize, image: DiskImage) -> Option<DiskImage> {
        if drive >= NUM_HARDDISKS {
            return Some(image);
        }
        let previous = self.unplug(drive);
        for i in 0..NUM_HARDDISKS {
            if i != drive
                && !image.pathname().is_empty()
                && self.units[i]
                    .image
                    .as_ref()
                    .is_some_and(|o| o.pathname() == image.pathname())
            {
                self.unplug(i);
            }
        }
        let (name, full_name) = image_title(image.pathname());
        let u = &mut self.units[drive];
        u.name = name;
        u.full_name = full_name;
        u.image = Some(image);
        u.status_next = DiskStatus::Off;
        u.status_prev = DiskStatus::Off;
        previous
    }

    /// Opens an `.hdv`/`.po`/`.2mg`/... file and inserts it (`Insert`). Hard disk images are never
    /// created.
    pub fn insert_path(&mut self, drive: usize, path: &std::path::Path) -> Result<(), ImageError> {
        let image = DiskImage::open_path(path, Class::HardDisk, false)?;
        self.insert(drive, image);
        Ok(())
    }

    pub fn unplug(&mut self, drive: usize) -> Option<DiskImage> {
        let u = self.units.get_mut(drive)?;
        u.name.clear();
        u.full_name.clear();
        u.image.take()
    }

    pub fn image(&self, drive: usize) -> Option<&DiskImage> {
        self.units.get(drive)?.image.as_ref()
    }

    pub fn image_mut(&mut self, drive: usize) -> Option<&mut DiskImage> {
        self.units.get_mut(drive)?.image.as_mut()
    }

    pub fn base_name(&self, drive: usize) -> &str {
        self.units.get(drive).map_or("", |u| &u.name)
    }

    pub fn full_name(&self, drive: usize) -> &str {
        self.units.get(drive).map_or("", |u| &u.full_name)
    }

    /// Swaps hard disks 1 and 2 (`ImageSwap`).
    pub fn swap(&mut self) {
        self.units.swap(0, 1);
    }

    /// Overrides the size reported to ProDOS (the `-harddisknumblocks` option); 0 = real size.
    pub fn set_user_num_blocks(&mut self, blocks: u32) {
        self.user_num_blocks = blocks;
    }

    /// LED state of `drive` (`GetLightStatus`/`UpdateLightStatus`): reads and writes show
    /// while the DMA is "busy".
    pub fn light_status(&self, drive: usize) -> DiskStatus {
        self.units
            .get(drive)
            .map_or(DiskStatus::Empty, |u| u.status_prev)
    }

    /// LED of the unit addressed last.
    pub fn current_light_status(&self) -> DiskStatus {
        let unit = if self.command & SP_CMD_BASE != 0 {
            self.unit_num as usize
        } else {
            self.pro_dos_block_device_unit()
        };
        self.units
            .get(unit)
            .map_or(DiskStatus::Off, |u| u.status_prev)
    }

    /// Writes back nothing (blocks are written through at once); here for symmetry with the
    /// floppy card.
    pub fn flush(&mut self) {}

    // ---- Internals (Harddisk.cpp). ----

    fn image_size_in_blocks(&self, unit: usize, is_16bit: bool) -> u32 {
        if self.user_num_blocks != 0 {
            return self.user_num_blocks;
        }
        let size = if unit == usize::MAX {
            0
        } else {
            self.units[unit]
                .image
                .as_ref()
                .map_or(0, DiskImage::image_size)
        };
        let mut n = (size / HD_BLOCK_SIZE) as u32;
        n = n.min(MAX_NUM_BLOCKS);
        if is_16bit {
            n = n.min(0xFFFF);
        }
        n
    }

    /// `FixupUnitNum` (Harddisk.cpp): older firmwares can write the unit register with slot 0.
    /// `LoadSnapshot` calls it for states older than v5 (after [`Self::apply_snapshot`]).
    pub fn fixup_unit_num(&mut self) {
        // Older firmwares can write unitNum with 0x00.
        if self.is_firmware_v1_or_2 && self.unit_num >> 4 != self.slot {
            self.unit_num = (self.unit_num & 0x8F) | (self.slot << 4);
        }
    }

    fn num_connected_devices(&self) -> u8 {
        // Scan backwards for the index of the last attached HDD.
        self.units
            .iter()
            .rposition(Unit::loaded)
            .map_or(0, |i| i as u8 + 1)
    }

    fn pro_dos_block_device_unit(&self) -> usize {
        let slot_from_unit = (self.unit_num >> 4) & 7;
        let offset = if slot_from_unit == self.slot { 0 } else { 2 };
        offset + (self.unit_num >> 7) as usize // bit 7 = drive select
    }

    /// Unit addressed by the current command/unit registers: `Some(index)` into `units`, with
    /// `usize::MAX` meaning the SmartPort controller; None if there is no such device.
    fn get_unit(&self) -> Option<usize> {
        if self.command & SP_CMD_BASE == 0 {
            return Some(self.pro_dos_block_device_unit());
        }
        let u = self.unit_num;
        if u as usize > NUM_HARDDISKS || u > self.num_connected_devices() {
            return None;
        }
        Some(if u == 0 { usize::MAX } else { u as usize - 1 })
    }

    fn unit_mut(&mut self, idx: usize) -> &mut Unit {
        if idx == usize::MAX {
            &mut self.controller
        } else {
            &mut self.units[idx]
        }
    }

    fn update_light_status(&mut self, idx: usize) {
        let u = self.unit_mut(idx);
        u.status_prev = u.status_next;
    }

    fn cmd_status(&mut self, idx: usize, now: u64) -> u8 {
        let busy = now <= self.not_busy_cycle;
        let u = self.unit_mut(idx);
        let mut r = 0;
        if u.error != 0 {
            r = STATUS_ERROR; // the firmware requires b0=1 for an error
        }
        if busy {
            r |= STATUS_BUSY; // b7=1 while doing the r/w DMA
        } else {
            u.status_next = DiskStatus::Off;
        }
        r | ((u.error & ERRORCODE_MASK) << 1)
    }

    fn cmd_execute(&mut self, idx: usize, ctx: &mut CardCtx) -> u8 {
        let now = ctx.access_cycle;
        let command = self.command;
        let loaded = self.unit_mut(idx).loaded() || idx == usize::MAX;
        if !loaded && command != BLK_CMD_STATUS && command != SP_CMD_STATUS {
            let u = self.unit_mut(idx);
            u.status_next = DiskStatus::Off;
            u.error = DEVICE_NOT_CONNECTED; // GH#452
            return self.cmd_status(idx, now);
        }
        if (command == SP_CMD_READBLOCK || command == SP_CMD_WRITEBLOCK)
            && self.unit_mut(idx).diskblock > MAX_NUM_BLOCKS
        {
            let u = self.unit_mut(idx);
            u.status_next = DiskStatus::Off;
            u.error = DEVICE_IO_ERROR;
            return self.cmd_status(idx, now);
        }
        self.unit_mut(idx).error = DEVICE_OK;
        match command {
            BLK_CMD_STATUS => {
                if idx != usize::MAX
                    && self.units[idx]
                        .image
                        .as_ref()
                        .map_or(0, DiskImage::image_size)
                        == 0
                {
                    self.unit_mut(idx).error = DEVICE_IO_ERROR;
                }
            }
            SP_CMD_BUSY_STATUS => {}
            SP_CMD_STATUS => {
                let e = self.smart_port_cmd_status(idx, ctx);
                self.unit_mut(idx).error = e;
            }
            BLK_CMD_READ | SP_CMD_READBLOCK => self.cmd_read(idx, now, ctx),
            BLK_CMD_WRITE | SP_CMD_WRITEBLOCK => self.cmd_write(idx, now, ctx),
            BLK_CMD_FORMAT | SP_CMD_FORMAT => self.cmd_format(idx, now),
            _ => self.unit_mut(idx).error = DEVICE_IO_ERROR,
        }
        self.cmd_status(idx, now)
    }

    fn cmd_read(&mut self, idx: usize, now: u64, ctx: &mut CardCtx) {
        let u = self.unit_mut(idx);
        u.status_next = DiskStatus::Read;
        let (block, mem) = (u.diskblock, u.memblock);
        let ok = idx != usize::MAX
            && (block as u64 * HD_BLOCK_SIZE as u64)
                < self.units[idx]
                    .image
                    .as_ref()
                    .map_or(0, DiskImage::image_size) as u64;
        let mut done = false;
        if ok {
            let u = &mut self.units[idx];
            let mut buf = [0u8; HD_BLOCK_SIZE];
            if u.image
                .as_ref()
                .is_some_and(|i| i.read_block(block, &mut buf))
            {
                u.buf = buf;
                u.buf_ptr = 0;
                // The MMU may map reads and writes to different memory: DMA writes as the CPU would.
                done = true;
                for (i, b) in buf.iter().enumerate() {
                    if !ctx.dma_write(mem.wrapping_add(i as u16), *b) {
                        done = false;
                        break;
                    }
                }
            }
        }
        if done {
            self.unit_mut(idx).error = DEVICE_OK;
            self.not_busy_cycle = now + CYCLES_FOR_DMA_RW_BLOCK;
        } else {
            self.unit_mut(idx).error = DEVICE_IO_ERROR;
        }
    }

    fn cmd_write(&mut self, idx: usize, now: u64, ctx: &mut CardCtx) {
        if idx == usize::MAX {
            self.unit_mut(idx).error = DEVICE_IO_ERROR;
            return;
        }
        let (block, mem) = (self.units[idx].diskblock, self.units[idx].memblock);
        let write_protected = self.units[idx]
            .image
            .as_ref()
            .is_none_or(DiskImage::is_write_protected);
        if write_protected {
            let u = &mut self.units[idx];
            u.status_next = DiskStatus::Prot;
            u.error = NOWRITE;
            return;
        }
        self.units[idx].status_next = DiskStatus::Write;
        let mut ok = true;
        let image_size = self.units[idx]
            .image
            .as_ref()
            .map_or(0, DiskImage::image_size);
        if block as usize * HD_BLOCK_SIZE >= image_size {
            // Writing past the end grows the image with zero blocks.
            let zero = [0u8; HD_BLOCK_SIZE];
            let img = self.units[idx].image.as_mut().expect("loaded");
            for b in (image_size / HD_BLOCK_SIZE) as u32..block {
                ok &= img.write_block(b, &zero);
            }
        }
        // Trap accesses that overlap I/O memory (GH#1007): the 512 bytes must not touch $C000-$CFFF.
        let end = mem as u32 + HD_BLOCK_SIZE as u32 - 1;
        if (mem < 0xC000 && end >= 0xC000) || (mem >> 12) == 0xC {
            ok = false;
        }
        let mut buf = [0u8; HD_BLOCK_SIZE];
        if ok {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = ctx.dma_read(mem.wrapping_add(i as u16));
            }
            ok = self.units[idx]
                .image
                .as_mut()
                .expect("loaded")
                .write_block(block, &buf);
        }
        if ok {
            self.units[idx].error = DEVICE_OK;
            self.not_busy_cycle = now + CYCLES_FOR_DMA_RW_BLOCK;
        } else {
            self.units[idx].error = DEVICE_IO_ERROR;
        }
    }

    fn cmd_format(&mut self, idx: usize, now: u64) {
        if idx == usize::MAX {
            self.unit_mut(idx).error = DEVICE_IO_ERROR;
            return;
        }
        if self.units[idx]
            .image
            .as_ref()
            .is_none_or(DiskImage::is_write_protected)
        {
            self.units[idx].error = NOWRITE;
            return;
        }
        let blocks = self.image_size_in_blocks(idx, false);
        let zero = [0u8; HD_BLOCK_SIZE];
        self.not_busy_cycle = now;
        let mut ok = true;
        for b in 0..blocks {
            ok = self.units[idx]
                .image
                .as_mut()
                .expect("loaded")
                .write_block(b, &zero);
            if !ok {
                break;
            }
            self.not_busy_cycle += CYCLES_FOR_FORMATTING_1_BLOCK;
        }
        self.units[idx].error = if ok { DEVICE_OK } else { DEVICE_IO_ERROR };
    }

    fn id_string(status: &mut Vec<u8>, id: &str) {
        let n = id.len().min(16);
        status.push(n as u8);
        status.extend(id.bytes().take(n));
        status.extend(std::iter::repeat_n(b' ', 16 - n)); // padded with ASCII spaces
    }

    /// SmartPort Status command (`SmartPortCmdStatus`).
    fn smart_port_cmd_status(&mut self, idx: usize, ctx: &mut CardCtx) -> u8 {
        let mut status: Vec<u8> = Vec::new();
        let mut r = DEVICE_OK;
        let code = self.status_code;
        if self.unit_num == 0 {
            // Unit 0: the SmartPort controller.
            match code {
                SP_STATUS_STATUS | SP_STATUS_GETDIB => {
                    status.push(self.num_connected_devices());
                    status.extend([0; 7]);
                    if code == SP_STATUS_GETDIB {
                        Self::id_string(&mut status, "AppleWin SP");
                        status.extend([0x00, 0x00, FW_VERSION.0, FW_VERSION.1]);
                        // memory expansion card
                    }
                }
                _ => r = BADCTL,
            }
        } else {
            match code {
                SP_STATUS_STATUS | SP_STATUS_GETDIB => {
                    let unit = self.unit_num as usize - 1;
                    let loaded = self.units[unit].loaded();
                    // b7 block device, b6 write allowed, b5 read allowed, b4 online, b3 format allowed, b2 write protected.
                    let mut general: u8 = if loaded { 0xF8 } else { 0xE8 };
                    if self.units[idx]
                        .image
                        .as_ref()
                        .is_some_and(DiskImage::is_write_protected)
                    {
                        general |= 1 << 2;
                    }
                    status.push(general);
                    let blocks = if loaded {
                        self.image_size_in_blocks(idx, false)
                    } else {
                        0
                    };
                    status.extend([blocks as u8, (blocks >> 8) as u8, (blocks >> 16) as u8]);
                    if code == SP_STATUS_GETDIB {
                        let id = format!("AppleWin SP D#{:02}", self.unit_num);
                        Self::id_string(&mut status, &id);
                        status.extend([0x02, 0x20, FW_VERSION.0, FW_VERSION.1]);
                        // hard disk
                    }
                }
                _ => r = BADCTL,
            }
        }
        if r != DEVICE_OK {
            return r;
        }
        let mem = self.unit_mut(idx).memblock;
        // The status list must not hit I/O space or ROM.
        let mut page = (mem >> 8) as u8;
        let end_page = (mem.wrapping_add(status.len() as u16) >> 8) as u8;
        loop {
            if !ctx.dma_writable((page as u16) << 8) {
                return BADCTL;
            }
            if page == end_page {
                break;
            }
            page = page.wrapping_add(1);
        }
        for (i, b) in status.iter().enumerate() {
            ctx.dma_write(mem.wrapping_add(i as u16), *b);
        }
        DEVICE_OK
    }

    fn encode(&self) -> Vec<u8> {
        let mut o = vec![
            1u8,
            self.unit_num,
            self.command,
            self.fifo_idx,
            self.status_code,
        ];
        o.extend(self.not_busy_cycle.to_le_bytes());
        for u in self.units.iter().chain(std::iter::once(&self.controller)) {
            o.push(u.error);
            o.extend(u.memblock.to_le_bytes());
            o.extend(u.diskblock.to_le_bytes());
            o.extend(u.buf_ptr.to_le_bytes());
            o.push(u.status_next as u8);
            o.push(u.status_prev as u8);
        }
        o
    }

    fn decode(&mut self, s: &[u8]) -> Option<()> {
        const STATUSES: [DiskStatus; 6] = [
            DiskStatus::Off,
            DiskStatus::Read,
            DiskStatus::Write,
            DiskStatus::Prot,
            DiskStatus::Empty,
            DiskStatus::Spin,
        ];
        if s.len() != 5 + 8 + (NUM_HARDDISKS + 1) * 11 || s[0] != 1 {
            return None;
        }
        (self.unit_num, self.command, self.fifo_idx, self.status_code) =
            (s[1], s[2], s[3] % 7, s[4]);
        self.not_busy_cycle = u64::from_le_bytes(s[5..13].try_into().ok()?);
        for (i, u) in self
            .units
            .iter_mut()
            .chain(std::iter::once(&mut self.controller))
            .enumerate()
        {
            let c = &s[13 + i * 11..13 + (i + 1) * 11];
            u.error = c[0];
            u.memblock = u16::from_le_bytes([c[1], c[2]]);
            u.diskblock = u32::from_le_bytes([c[3], c[4], c[5], c[6]]);
            u.buf_ptr = u16::from_le_bytes([c[7], c[8]]).min(HD_BLOCK_SIZE as u16 - 1);
            u.status_next = *STATUSES.get(c[9] as usize)?;
            u.status_prev = *STATUSES.get(c[10] as usize)?;
        }
        Some(())
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        let now = ctx.access_cycle;
        let mut idx = None;
        if lo != 2 && lo != 3 {
            // GetUnit() depends on command & unit_num.
            match self.get_unit() {
                Some(i) => idx = Some(i),
                None => return status_error(DEVICE_NOT_CONNECTED),
            }
        }
        let unit = idx.unwrap_or(0);
        let r = match lo {
            0x0 => {
                let r = self.cmd_execute(unit, ctx);
                // Subsequent reads from IO addr 0 just execute 'Status'.
                self.command = if self.command & SP_CMD_BASE != 0 {
                    SP_CMD_BUSY_STATUS
                } else {
                    BLK_CMD_STATUS
                };
                self.fifo_idx = 0;
                r
            }
            0x1 => self.cmd_status(unit, now),
            0x2 => self.command,
            0x3 => self.unit_num,
            0x4 => self.unit_mut(unit).memblock as u8,
            0x5 => (self.unit_mut(unit).memblock >> 8) as u8,
            0x6 => self.unit_mut(unit).diskblock as u8,
            0x7 => (self.unit_mut(unit).diskblock >> 8) as u8,
            0x8 => {
                // Legacy: continue to support this port for old HDC firmware.
                let u = self.unit_mut(unit);
                let r = u.buf[u.buf_ptr as usize];
                if (u.buf_ptr as usize) < HD_BLOCK_SIZE - 1 {
                    u.buf_ptr += 1;
                }
                r
            }
            0x9 | 0xA => {
                if unit != usize::MAX && self.units[unit].loaded() {
                    let blocks = self.image_size_in_blocks(unit, true);
                    if lo == 0x9 {
                        blocks as u8
                    } else {
                        (blocks >> 8) as u8
                    }
                } else {
                    0
                }
            }
            _ => {
                self.unit_mut(unit).status_next = DiskStatus::Off;
                ctx.floating_bus
            }
        };
        self.update_light_status(unit);
        r
    }

    fn io_write(&mut self, lo: u8, d: u8, ctx: &mut CardCtx) {
        let _ = ctx;
        let mut lo = lo;
        let mut d = d;
        let mut idx = None;
        if lo != 2 && lo != 3 && !((lo == 9 || lo == 0xA) && self.fifo_idx < 2) {
            match self.get_unit() {
                Some(i) => idx = Some(i),
                None => {
                    // Make sure fifo_idx returns to 0 even if the unit is out of range (ProDOS 8
                    // v2.5.0 Bitsy Bye: TAB through S7 volumes).
                    let fifo_size = if lo == 9 { 6 } else { 7 };
                    self.fifo_idx = (self.fifo_idx + 1) % fifo_size;
                    return;
                }
            }
        }
        if lo == 9 || lo == 0xA {
            // BLK or SP command FIFO.
            if lo == 0xA && self.fifo_idx == 0 {
                d |= SP_CMD_BASE;
            }
            let fifo_size = if lo == 9 { 6 } else { 7 };
            lo = 2 + self.fifo_idx;
            self.fifo_idx = (self.fifo_idx + 1) % fifo_size;
        }
        // The unit may have changed by this write; registers below act on the unit chosen before.
        let unit = idx.unwrap_or(0);
        match lo {
            0x0 | 0x1 => {} // read-only registers
            0x2 => self.command = d,
            0x3 => {
                self.unit_num = d;
                self.fixup_unit_num();
            }
            0x4 => {
                let u = self.unit_mut(unit);
                u.memblock = (u.memblock & 0xFF00) | d as u16;
            }
            0x5 => {
                let u = self.unit_mut(unit);
                u.memblock = (u.memblock & 0x00FF) | (d as u16) << 8;
            }
            0x6 => {
                if self.command != SP_CMD_STATUS {
                    let u = self.unit_mut(unit);
                    u.diskblock = (u.diskblock & 0xFF_FF00) | d as u32;
                } else {
                    self.status_code = d;
                }
            }
            0x7 => {
                let u = self.unit_mut(unit);
                u.diskblock = (u.diskblock & 0xFF_00FF) | (d as u32) << 8;
            }
            0x8 => {
                if self.command & SP_CMD_BASE != 0 {
                    let u = self.unit_mut(unit);
                    u.diskblock = (u.diskblock & 0x00_FFFF) | (d as u32) << 16;
                }
            }
            _ => self.unit_mut(unit).status_next = DiskStatus::Off,
        }
        if idx.is_some() {
            if self.command & SP_CMD_BASE == 0 {
                self.unit_mut(unit).diskblock &= 0x00_FFFF; // BLK commands are only 16-bit
            }
            self.update_light_status(unit);
        }
    }
}

/// One hard disk unit in an AppleWin save-state (`SaveSnapshotHDDUnit`, Harddisk.cpp:1310).
#[derive(Clone, Debug, PartialEq)]
pub struct HddUnitSnapshot {
    pub filename: String,
    pub pathname: String,
    pub error: u8,
    pub memblock: u16,
    pub diskblock: u32,
    pub status_next: u8,
    pub status_prev: u8,
    pub buf_ptr: u16,
    pub buf: Vec<u8>,
}

/// The controller state AppleWin saves (`HarddiskInterfaceCard::SaveSnapshot`, :1332). Units
/// that hold an image only, indexed from 0 (AppleWin names them `Unit1`...).
#[derive(Clone, Debug, PartialEq)]
pub struct HddSnapshot {
    pub unit_num: u8,
    pub command: u8,
    pub not_busy_cycle: u64,
    pub status_code: u8,
    pub fifo_idx: u8,
    pub firmware: Vec<u8>,
    pub units: Vec<(usize, HddUnitSnapshot)>,
}

impl HardDiskCard {
    /// State in AppleWin's save-state terms.
    pub fn snapshot(&self) -> HddSnapshot {
        HddSnapshot {
            unit_num: self.unit_num,
            command: self.command,
            not_busy_cycle: self.not_busy_cycle,
            status_code: self.status_code,
            fifo_idx: self.fifo_idx,
            firmware: self.firmware.to_vec(),
            units: self
                .units
                .iter()
                .enumerate()
                .filter_map(|(i, u)| {
                    let img = u.image.as_ref()?;
                    Some((
                        i,
                        HddUnitSnapshot {
                            filename: u.full_name.clone(),
                            pathname: img.pathname().to_string(),
                            error: u.error,
                            memblock: u.memblock,
                            diskblock: u.diskblock,
                            status_next: u.status_next as u8,
                            status_prev: u.status_prev as u8,
                            buf_ptr: u.buf_ptr,
                            buf: u.buf.to_vec(),
                        },
                    ))
                })
                .collect(),
        }
    }

    /// Applies a [`HddSnapshot`] (`LoadSnapshot`, Harddisk.cpp:1440). Insert the images first;
    /// units without an image are skipped. The firmware stays the card's own (AppleWin keeps the
    /// saved one only for old revisions).
    pub fn apply_snapshot(&mut self, s: &HddSnapshot) {
        const STATUSES: [DiskStatus; 6] = [
            DiskStatus::Off,
            DiskStatus::Read,
            DiskStatus::Write,
            DiskStatus::Prot,
            DiskStatus::Empty,
            DiskStatus::Spin,
        ];
        self.unit_num = s.unit_num;
        self.command = s.command;
        self.not_busy_cycle = s.not_busy_cycle;
        self.status_code = s.status_code;
        self.fifo_idx = s.fifo_idx % 7;
        for (i, us) in &s.units {
            let Some(u) = self.units.get_mut(*i) else {
                continue;
            };
            if u.image.is_none() {
                continue;
            }
            u.error = us.error;
            u.memblock = us.memblock;
            u.diskblock = us.diskblock;
            u.status_next = STATUSES
                .get(us.status_next as usize)
                .copied()
                .unwrap_or(DiskStatus::Off);
            u.status_prev = STATUSES
                .get(us.status_prev as usize)
                .copied()
                .unwrap_or(DiskStatus::Off);
            u.buf_ptr = us.buf_ptr.min(HD_BLOCK_SIZE as u16 - 1);
            let n = us.buf.len().min(HD_BLOCK_SIZE);
            u.buf[..n].copy_from_slice(&us.buf[..n]);
        }
    }
}

impl Card for HardDiskCard {
    fn name(&self) -> &'static str {
        "Hard Disk Controller"
    }

    fn wants_opcode_info(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        HardDiskCard::io_read(self, lo, ctx)
    }

    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        HardDiskCard::io_write(self, lo, value, ctx);
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.firmware[offset as usize]
    }

    fn reset(&mut self, power_cycle: bool, _ctx: &mut CardCtx) {
        for u in &mut self.units {
            u.error = 0;
        }
        self.fifo_idx = 0;
        if power_cycle {
            self.not_busy_cycle = 0; // the machine's cycle counter restarted
        }
    }

    fn save(&self) -> Vec<u8> {
        self.encode()
    }

    fn load(&mut self, state: &[u8]) -> bool {
        self.decode(state).is_some()
    }
}
