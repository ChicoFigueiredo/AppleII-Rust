//! The Disk II controller card (AppleWin Disk.cpp, Disk.h, Disk2CardManager.cpp).
//!
//! Two drives per card. 5.25" images are read a track at a time into a nibble buffer. Plain
//! (non-WOZ) images follow AppleWin's nibble model, with its "enhanced" (full speed; the
//! default) and "authentic" (sector-skewed, spin-aware) modes. WOZ images run the bit-stream
//! Logic State Sequencer, including write support, quarter tracks and the copy-protection
//! jitter AppleWin emulates. All "random" behaviour comes from a seeded generator, so a run is
//! reproducible (ADR 0001).

use crate::format_track::FormatTrack;
use crate::image::{image_title, DiskImage, ImageError};
use crate::nibble::NIBBLES_PER_TRACK;
use apple2_core::card::{Card, CardCtx, SLOT_ROM_SIZE};
use std::any::Any;

pub const NUM_DRIVES: usize = 2;
/// 1M cycles = ~1 second of spin-down delay (Disk.h:284).
pub const SPINNING_CYCLES: u32 = 1_000_000;
pub const WRITELIGHT_CYCLES: u32 = 1_000_000;
/// ~0x2EC-0x990 cycles depending on the card, GH#864 (Disk.h:286).
const MOTOR_ON_UNTIL_LSS_STABLE_CYCLES: u64 = 0x2EC;
/// Token of the deferred-stepper synchronous event.
const STEPPER_EVENT: u16 = 0;
/// A magnet change takes effect this many cycles later (:614).
const STEPPER_DELAY_CYCLES: i32 = 10;
/// Per-card limit of queued [`DriveEvent`]s (the frontend is expected to drain them each frame).
const MAX_QUEUED_EVENTS: usize = 1024;

/// `Disk_Status_e`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskStatus {
    /// Motor is off.
    Off,
    Read,
    Write,
    /// Write-protected.
    Prot,
    /// No disk mounted.
    Empty,
    /// Motor was turned off, still spinning down.
    Spin,
}

/// What the drive hardware did, for sound and UI hooks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriveEvent {
    MotorOn { drive: usize },
    MotorOff { drive: usize },
    /// The head moved (phases are in half tracks, `phase / 2` = track).
    HeadStep { drive: usize, from_phase: f32, to_phase: f32 },
    DriveSelected { drive: usize },
    DiskInserted { drive: usize },
    DiskEjected { drive: usize },
}

/// `Logic State Sequencer` function (UTAIIe 9-14): bit 0 = write mode ($C08E/F), bit 1 = load
/// mode ($C08C/D).
const READ_SEQUENCING: u8 = 0;
const DATA_SHIFT_WRITE: u8 = 1;
const CHECK_WRITE_PROT_AND_INIT_WRITE: u8 = 2;
const DATA_LOAD_WRITE: u8 = 3;

/// Disk in a drive: image plus the track buffer and the head position inside it (`FloppyDisk`).
struct Floppy {
    image: Option<DiskImage>,
    name: String,
    full_name: String,
    write_protected: bool,
    byte: usize,
    nibbles: usize,
    bit_offset: u32,
    bit_count: u32,
    bit_mask: u8,
    extra_cycles: f64,
    track: Vec<u8>,
    track_data: bool,
    track_dirty: bool,
    longest_sync_ff_run: u32,
    longest_sync_ff_start: i32,
    initial_bit_offset: u32,
    revs: u32,
}

impl Floppy {
    fn empty() -> Self {
        Floppy {
            image: None,
            name: String::new(),
            full_name: String::new(),
            write_protected: false,
            byte: 0,
            nibbles: 0,
            bit_offset: 0,
            bit_count: 0,
            bit_mask: 0x80,
            extra_cycles: 0.0,
            track: Vec::new(),
            track_data: false,
            track_dirty: false,
            longest_sync_ff_run: 0,
            longest_sync_ff_start: -1,
            initial_bit_offset: 0,
            revs: 0,
        }
    }

    fn is_woz(&self) -> bool {
        self.image.as_ref().is_some_and(DiskImage::is_woz)
    }

    /// `UpdateBitStreamOffsets`.
    fn update_bit_stream_offsets(&mut self) {
        self.byte = (self.bit_offset / 8) as usize;
        self.bit_mask = 1 << (7 - (self.bit_offset & 7));
    }

    /// `UpdateBitStreamPosition`.
    fn update_bit_stream_position(&mut self, bit_cell_delta: u32) {
        if self.bit_count == 0 {
            return;
        }
        self.bit_offset = ((self.bit_offset as u64 + bit_cell_delta as u64) % self.bit_count as u64) as u32;
        self.update_bit_stream_offsets();
    }

    /// `IncBitStream`.
    fn inc_bit_stream(&mut self) {
        self.bit_mask >>= 1;
        if self.bit_mask == 0 {
            self.bit_mask = 0x80;
            self.byte += 1;
        }
        self.bit_offset += 1;
        if self.bit_offset == self.bit_count {
            self.bit_mask = 0x80;
            self.bit_offset = 0;
            self.byte = 0;
        }
        if self.bit_offset == self.initial_bit_offset {
            self.revs += 1;
        }
    }

    fn bit(&self) -> bool {
        self.track.get(self.byte).is_some_and(|b| b & self.bit_mask != 0)
    }
}

/// One drive: mechanics (`FloppyDrive`) plus its disk.
struct Drive {
    connected: bool,
    phase_precise: f32,
    phase: i32,
    last_stepper_cycle: u64,
    motor_on_cycle: u64,
    head_window: u8,
    spinning: u32,
    write_light: u32,
    disk: Floppy,
}

impl Drive {
    fn new() -> Self {
        Drive {
            connected: true,
            phase_precise: 0.0,
            phase: 0,
            last_stepper_cycle: 0,
            motor_on_cycle: 0,
            head_window: 0,
            spinning: 0,
            write_light: 0,
            disk: Floppy::empty(),
        }
    }
}

/// Seeded xorshift standing in for the C library `rand()` AppleWin uses (reproducible runs).
#[derive(Clone, Copy)]
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// `rand() < RAND_THRESHOLD(num, den)`.
    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.next() % den < num
    }
}

pub struct Disk2Card {
    firmware_16: [u8; SLOT_ROM_SIZE],
    firmware_13: [u8; SLOT_ROM_SIZE],
    is_13_sector_firmware: bool,
    force_13_sector_firmware: bool,
    curr_drive: usize,
    drives: [Drive; NUM_DRIVES],
    floppy_latch: u8,
    motor_on: bool,
    /// Magnet states (phases 0-3) of the selected drive.
    magnet_states: u8,
    disk_last_cycle: u64,
    disk_last_read_latch_cycle: u64,
    format_track: FormatTrack,
    enhance_disk: bool,
    // Logic State Sequencer (WOZ):
    shift_reg: u8,
    latch_delay: i32,
    write_started: bool,
    seq_func: u8,
    deferred_stepper_event: bool,
    deferred_stepper_address: u8,
    deferred_stepper_cycles: u64,
    /// Debug switch of AppleWin's `Disk2CardManager::SetStepperDefer`; on by default.
    defer_stepper: bool,
    // Jitter (GH#930):
    t00s00_pattern_idx: usize,
    found_t00s00_pattern: bool,
    rng: Rng,
    events: Vec<DriveEvent>,
}

/// NB. Non-standard 4&4, with Vol=0x00 and Chk=0x00 (only a few titles match). `m_T00S00Pattern`.
const T00S00_PATTERN: [u8; 12] = [0xD5, 0xAA, 0x96, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xDE];

impl Default for Disk2Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Disk2Card {
    pub fn new() -> Self {
        let mut f16 = [0u8; SLOT_ROM_SIZE];
        let mut f13 = [0u8; SLOT_ROM_SIZE];
        f16.copy_from_slice(apple2_roms::DISK2);
        f13.copy_from_slice(apple2_roms::DISK2_13SECTOR);
        let mut c = Disk2Card {
            firmware_16: f16,
            firmware_13: f13,
            is_13_sector_firmware: false,
            force_13_sector_firmware: false,
            curr_drive: 0,
            drives: [Drive::new(), Drive::new()],
            floppy_latch: 0,
            motor_on: false,
            magnet_states: 0,
            disk_last_cycle: 0,
            disk_last_read_latch_cycle: 0,
            format_track: FormatTrack::default(),
            enhance_disk: true,
            shift_reg: 0,
            latch_delay: 0,
            write_started: false,
            seq_func: READ_SEQUENCING,
            deferred_stepper_event: false,
            deferred_stepper_address: 0,
            deferred_stepper_cycles: 0,
            defer_stepper: true,
            t00s00_pattern_idx: 0,
            found_t00s00_pattern: false,
            rng: Rng(0x2545_F491),
            events: Vec::new(),
        };
        c.reset_switches();
        c.reset_logic_state_sequencer();
        c
    }

    // ---- Frontend API: media. ----

    /// Puts `image` in `drive` (0 or 1), ejecting what was there (its dirty track is flushed).
    /// If the other drive holds an image with the same non-empty pathname it is ejected first,
    /// so a disk can be moved between drives (`InsertDisk`, :827).
    pub fn insert_disk(&mut self, drive: usize, image: DiskImage) -> Option<DiskImage> {
        if drive >= NUM_DRIVES {
            return Some(image);
        }
        let previous = self.eject_disk(drive);
        let other = 1 - drive;
        if !image.pathname().is_empty() && self.drives[other].disk.image.as_ref().is_some_and(|i| i.pathname() == image.pathname()) {
            self.eject_disk(other);
        }
        let (name, full_name) = image_title(image.pathname());
        let alloc = NIBBLES_PER_TRACK.max(image.max_nibbles_per_track());
        let d = &mut self.drives[drive].disk;
        *d = Floppy::empty();
        d.write_protected = image.is_write_protected();
        d.name = name;
        d.full_name = full_name;
        d.track = vec![0; alloc];
        d.image = Some(image);
        self.push_event(DriveEvent::DiskInserted { drive });
        previous
    }

    /// Opens `path` and inserts it. `force_write_protected`: ignore the file's writability.
    pub fn insert_path(&mut self, drive: usize, path: &std::path::Path, force_write_protected: bool) -> Result<(), ImageError> {
        let image = DiskImage::open_path(path, crate::image::Class::Floppy, force_write_protected)?;
        self.insert_disk(drive, image);
        Ok(())
    }

    /// Removes the disk from `drive`, writing back the dirty track, and returns it.
    pub fn eject_disk(&mut self, drive: usize) -> Option<DiskImage> {
        if drive >= NUM_DRIVES || self.drives[drive].disk.image.is_none() {
            return None;
        }
        self.flush_current_track(drive);
        let old = std::mem::replace(&mut self.drives[drive].disk, Floppy::empty());
        self.push_event(DriveEvent::DiskEjected { drive });
        old.image
    }

    /// Ejects the disk and disconnects the drive (`UnplugDrive`): an unplugged drive reads 0x80.
    pub fn unplug_drive(&mut self, drive: usize) {
        self.eject_disk(drive);
        if drive < NUM_DRIVES {
            self.drives[drive].connected = false;
        }
    }

    pub fn plug_drive(&mut self, drive: usize) {
        if drive < NUM_DRIVES {
            self.drives[drive].connected = true;
        }
    }

    pub fn is_drive_connected(&self, drive: usize) -> bool {
        drive < NUM_DRIVES && self.drives[drive].connected
    }

    pub fn image(&self, drive: usize) -> Option<&DiskImage> {
        self.drives.get(drive)?.disk.image.as_ref()
    }

    pub fn image_mut(&mut self, drive: usize) -> Option<&mut DiskImage> {
        self.drives.get_mut(drive)?.disk.image.as_mut()
    }

    pub fn is_drive_empty(&self, drive: usize) -> bool {
        self.image(drive).is_none()
    }

    pub fn is_woz_image_in_drive(&self, drive: usize) -> bool {
        self.image(drive).is_some_and(DiskImage::is_woz)
    }

    /// File name without extension (`GetBaseName`).
    pub fn base_name(&self, drive: usize) -> &str {
        self.drives.get(drive).map_or("", |d| &d.disk.name)
    }

    /// File name with extension (`GetFullName`).
    pub fn full_name(&self, drive: usize) -> &str {
        self.drives.get(drive).map_or("", |d| &d.disk.full_name)
    }

    pub fn pathname(&self, drive: usize) -> &str {
        self.image(drive).map_or("", DiskImage::pathname)
    }

    /// Swaps the disks of the two drives after flushing both (`DriveSwap`). The frontend decides
    /// whether to warn when a drive is [`spinning`](Self::is_spinning).
    pub fn swap_disks(&mut self) {
        self.flush_current_track(0);
        self.flush_current_track(1);
        let (a, b) = self.drives.split_at_mut(1);
        std::mem::swap(&mut a[0].disk, &mut b[0].disk);
        // Re-read the track on the next latch access for the new floppy (GH#543).
        for d in &mut self.drives {
            d.disk.track_data = false;
        }
    }

    pub fn is_write_protected(&self, drive: usize) -> bool {
        self.drives.get(drive).is_none_or(|d| d.disk.write_protected)
    }

    pub fn set_write_protect(&mut self, drive: usize, wp: bool) {
        if let Some(d) = self.drives.get_mut(drive) {
            d.disk.write_protected = wp;
        }
    }

    /// Writes back the dirty tracks of both drives (call before saving or quitting).
    pub fn flush(&mut self) {
        self.flush_current_track(0);
        self.flush_current_track(1);
    }

    // ---- Frontend API: settings and status. ----

    pub fn enhance_disk(&self) -> bool {
        self.enhance_disk
    }

    /// Enhanced speed (default): sectors are not skewed and the host may run unthrottled while
    /// the motor is on. Authentic: spin-aware and timed (`m_enhanceDisk`).
    pub fn set_enhance_disk(&mut self, enhance: bool) {
        self.enhance_disk = enhance;
    }

    /// True while the emulation should run at full speed (`IsConditionForFullSpeed`).
    pub fn is_condition_for_full_speed(&self) -> bool {
        self.motor_on && self.enhance_disk
    }

    pub fn force_13_sector_firmware(&self) -> bool {
        self.force_13_sector_firmware
    }

    pub fn set_force_13_sector_firmware(&mut self, on: bool) {
        self.force_13_sector_firmware = on;
    }

    /// 13 or 16: the firmware currently in the slot ROM (`GetCurrentFirmware`).
    pub fn current_firmware(&self) -> u32 {
        if self.is_13_sector_firmware {
            13
        } else {
            16
        }
    }

    /// Chooses the slot ROM: forced 13-sector, or a WOZ in drive 1 whose INFO says 13-sector
    /// (`InitFirmware`, :2084). Done on every reset; call it after inserting a disk to apply it
    /// without resetting.
    pub fn init_firmware(&mut self) {
        self.is_13_sector_firmware = self.force_13_sector_firmware || self.drives[0].disk.image.as_ref().is_some_and(DiskImage::boot_sector_is_13);
    }

    /// Selected drive (`GetCurrentDrive`).
    pub fn current_drive(&self) -> usize {
        self.curr_drive
    }

    pub fn is_motor_on(&self) -> bool {
        self.motor_on
    }

    pub fn is_spinning(&self, drive: usize) -> bool {
        self.drives.get(drive).is_some_and(|d| d.spinning > 0)
    }

    /// Head position in half tracks, quarter-track precise for WOZ (`GetPhase`).
    pub fn phase(&self, drive: usize) -> f32 {
        self.drives.get(drive).map_or(0.0, |d| d.phase_precise)
    }

    /// Track under the head (`GetTrack`).
    pub fn track(&self, drive: usize) -> u32 {
        self.drives.get(drive).map_or(0, |d| DiskImage::phase_to_track(d.phase_precise))
    }

    /// "NN.nn" track string in hex (`GetCurrentTrackString`).
    pub fn track_string(&self, drive: usize) -> String {
        let p = self.phase(drive) / 2.0;
        let int = p as u32;
        format!("{:02X}.{:02}", int, ((p - int as f32) * 100.0 + 0.5) as u32)
    }

    /// LED state of a drive (`GetDriveLightStatus`).
    pub fn light_status(&self, drive: usize) -> DiskStatus {
        match self.drives.get(drive) {
            Some(d) if d.spinning > 0 => {
                if d.disk.write_protected {
                    DiskStatus::Prot
                } else if d.write_light > 0 {
                    DiskStatus::Write
                } else {
                    DiskStatus::Read
                }
            }
            Some(_) => DiskStatus::Off,
            None => DiskStatus::Empty,
        }
    }

    /// State of the selected drive for the status line (`GetCurrentState`).
    pub fn current_state(&self) -> DiskStatus {
        let d = &self.drives[self.curr_drive];
        if d.disk.image.is_none() {
            DiskStatus::Empty
        } else if !self.motor_on {
            if d.spinning > 0 {
                DiskStatus::Spin
            } else {
                DiskStatus::Off
            }
        } else if self.seq_func & 1 != 0 {
            if d.disk.write_protected {
                DiskStatus::Prot
            } else {
                DiskStatus::Write
            }
        } else {
            DiskStatus::Read
        }
    }

    /// Takes the queued hardware events (motor, head steps, ...). Drain this every frame.
    pub fn drain_events(&mut self) -> Vec<DriveEvent> {
        std::mem::take(&mut self.events)
    }

    fn push_event(&mut self, e: DriveEvent) {
        if self.events.len() < MAX_QUEUED_EVENTS {
            self.events.push(e);
        }
    }

    // ---- Internals (Disk.cpp). ----

    fn reset_switches(&mut self) {
        self.curr_drive = 0;
        self.motor_on = false;
        self.magnet_states = 0;
        self.seq_func = READ_SEQUENCING;
    }

    fn reset_logic_state_sequencer(&mut self) {
        self.shift_reg = 0;
        self.latch_delay = 0;
        self.write_started = false;
        self.t00s00_pattern_idx = 0;
        self.found_t00s00_pattern = false;
    }

    /// `CheckSpinning`.
    fn check_spinning(&mut self, state_changed: bool, now: u64) {
        let motor_on = self.motor_on;
        let d = &mut self.drives[self.curr_drive];
        let mode_changed = motor_on && d.spinning == 0;
        if motor_on && d.connected {
            d.spinning = SPINNING_CYCLES;
        }
        if motor_on && state_changed {
            d.motor_on_cycle = now;
        }
        if mode_changed {
            // Not spinning (off for 1 sec) -> on.
            self.disk_last_cycle = now;
        }
    }

    fn control_motor(&mut self, lo: u8, now: u64) {
        let new_state = lo & 1 != 0;
        let changed = new_state != self.motor_on;
        if !new_state {
            // DRIVES OFF forces the control flip-flops to clear (UTAIIe 9-12).
            self.magnet_states = 0;
        }
        if changed {
            self.motor_on = new_state;
            self.format_track.drive_not_writing_track();
            let drive = self.curr_drive;
            self.push_event(if new_state { DriveEvent::MotorOn { drive } } else { DriveEvent::MotorOff { drive } });
        }
        self.check_spinning(changed, now);
    }

    /// `Enable`: returns whether the (new) selected drive holds a WOZ image.
    fn enable(&mut self, lo: u8, now: u64) -> bool {
        let new_drive = (lo & 1) as usize;
        let changed = new_drive != self.curr_drive;
        self.curr_drive = new_drive;
        let other = &mut self.drives[1 - new_drive];
        other.spinning = 0;
        other.write_light = 0;
        self.check_spinning(changed, now);
        if changed {
            self.push_event(DriveEvent::DriveSelected { drive: new_drive });
        }
        self.drives[new_drive].disk.is_woz()
    }

    fn control_stepper(&mut self, lo: u8, now: u64, ctx: &mut CardCtx) {
        if !self.motor_on && self.drives[self.curr_drive].spinning == 0 {
            return; // GH#525
        }
        let bit = 1 << ((lo >> 1) & 3);
        if lo & 1 != 0 {
            self.magnet_states |= bit;
        } else {
            self.magnet_states &= !bit;
        }
        if !self.defer_stepper {
            self.deferred_stepper_address = lo;
            self.deferred_stepper_cycles = now;
            self.control_stepper_deferred();
            return;
        }
        if self.deferred_stepper_event {
            // Adjacent magnets turned off/on in a very short interval (GH#1110).
            ctx.cancel(STEPPER_EVENT);
            self.deferred_stepper_event = false;
            let delta = ((self.deferred_stepper_address & 7) as i32 - (lo & 7) as i32).abs();
            if (delta == 2 || delta == 6) && lo & 1 == 0 {
                // Two adjacent magnets off in quick succession don't move the cog (also DOS 3.2,
                // Pascal and ProDOS rapidly turning off all four).
                self.drives[self.curr_drive].last_stepper_cycle = now;
                return;
            }
            // Complete the deferred stepper event (eg. Glutton, EDD III).
            self.control_stepper_deferred();
        }
        self.deferred_stepper_address = lo;
        self.deferred_stepper_cycles = now;
        ctx.schedule(STEPPER_EVENT, STEPPER_DELAY_CYCLES);
        self.deferred_stepper_event = true;
    }

    fn control_stepper_deferred(&mut self) {
        self.deferred_stepper_event = false;
        let curr = self.curr_drive;
        let magnets = self.magnet_states;
        let is_woz = self.drives[curr].disk.is_woz();
        let pd = &mut self.drives[curr];
        // Move only when the magnet opposite the cog is off, towards an adjacent magnet that is
        // on, and not at all when both neighbours are on (a quarter track).
        let mut direction = 0;
        if magnets & (1 << ((pd.phase + 1) & 3)) != 0 {
            direction += 1;
        }
        if magnets & (1 << ((pd.phase + 3) & 3)) != 0 {
            direction -= 1;
        }
        let mut quarter = 0;
        if is_woz && matches!(magnets, 0xC | 0x6 | 0x3 | 0x9) {
            quarter = direction;
            direction = 0;
        }
        pd.phase = (pd.phase + direction).clamp(0, 79);
        let new_precise = (pd.phase as f32 + quarter as f32 * 0.5).max(0.0);
        pd.last_stepper_cycle = self.deferred_stepper_cycles;
        if new_precise != pd.phase_precise {
            let from = pd.phase_precise;
            self.flush_current_track(curr);
            let pd = &mut self.drives[curr];
            pd.phase_precise = new_precise;
            pd.disk.track_data = false;
            self.format_track.drive_not_writing_track();
            self.push_event(DriveEvent::HeadStep { drive: curr, from_phase: from, to_phase: new_precise });
        }
    }

    /// Writes back the dirty track buffer of `drive` (`FlushCurrentTrack`).
    fn flush_current_track(&mut self, drive: usize) {
        let d = &mut self.drives[drive];
        if !d.disk.track.is_empty() && d.disk.track_dirty {
            Self::write_track(d);
        }
    }

    fn write_track(d: &mut Drive) {
        let phase = d.phase_precise;
        let f = &mut d.disk;
        let Some(image) = f.image.as_mut() else {
            f.track_dirty = false;
            return;
        };
        if image.phase_to_track_limited(phase, false) >= image.num_tracks() || f.write_protected {
            return;
        }
        image.write_track(phase, &f.track[..f.nibbles], f.nibbles);
        f.track_dirty = false;
    }

    /// `ReadTrack`: fills the track buffer from the image for the current head position.
    fn read_track(&mut self, drive: usize, now: u64) {
        let phase = self.drives[drive].phase_precise;
        let (tracks, zero_ok, is_woz) = match self.drives[drive].disk.image.as_ref() {
            Some(i) => (i.num_tracks(), i.zero_tracks_valid(), i.is_woz()),
            None => return,
        };
        let track_no = DiskImage::phase_to_track(phase);
        if track_no >= tracks {
            if !zero_ok {
                self.drives[drive].disk.track_data = false;
            }
            return;
        }
        if is_woz {
            // Update bitStream position for the current track before re-calculating for the new one.
            let delta = self.bit_cell_delta(now);
            self.drives[drive].disk.update_bit_stream_position(delta);
            let f = &mut self.drives[drive].disk;
            if f.bit_count == 0 {
                f.bit_offset = 0; // only ever 0 on initial power on
                f.bit_count = 8;
            }
        }
        let enhance = self.enhance_disk;
        let f = &mut self.drives[drive].disk;
        let (current_pos, current_len) = (f.bit_offset, f.bit_count);
        let Some(image) = f.image.as_mut() else { return };
        let (nibbles, bits) = image.read_track(phase, &mut f.track, enhance);
        f.nibbles = nibbles;
        if !is_woz {
            f.byte = 0;
        } else {
            f.bit_count = bits;
            if f.nibbles == 0 || f.bit_count == 0 {
                f.nibbles = 1;
                f.bit_count = 8;
            }
            // Cross-track position, WOZ-1.01; "round-up" for sensitive cross-track sync checks (GH#1022).
            f.bit_offset = ((current_pos as u64 * f.bit_count as u64) / current_len.max(1) as u64) as u32 + 7;
            if f.bit_offset >= f.bit_count {
                f.bit_offset = 0;
            }
            f.update_bit_stream_offsets();
            f.extra_cycles = 0.0;
            self.drives[drive].head_window = 0;
            self.find_track_seam_woz(drive);
        }
        let f = &mut self.drives[drive].disk;
        f.track_data = f.nibbles != 0;
        f.initial_bit_offset = f.bit_offset;
        f.revs = 0;
    }

    /// `FindTrackSeamWOZ`: the longest run of FF/10 sync nibbles.
    fn find_track_seam_woz(&mut self, drive: usize) {
        let f = &mut self.drives[drive].disk;
        let old_bit_offset = f.bit_offset;
        let mut shift_reg: u8 = 0;
        let mut zero_count = 0;
        let mut start_bit_offset: i32 = -1;
        f.bit_offset = 0;
        f.update_bit_stream_offsets();
        let mut nibble_start: i32 = -1;
        let mut sync_start: i32 = -1;
        let mut sync_run = 0;
        let mut longest_start: i32 = -1;
        let mut longest_run = 0;
        f.longest_sync_ff_start = -1;
        // The revolution is at most bit_count bits; guard against a degenerate track.
        let mut guard = f.bit_count as u64 * 2 + 16;
        loop {
            let out = f.bit() as u8;
            f.inc_bit_stream();
            guard -= 1;
            if guard == 0 || (start_bit_offset < 0 && f.bit_offset == 0) || start_bit_offset == f.bit_offset as i32 {
                break;
            }
            if shift_reg & 0x80 != 0 {
                if out == 0 {
                    zero_count += 1; // zero: the LSS holds the nibble in the latch
                    continue;
                }
                // Start of the next nibble.
                if shift_reg == 0xFF && zero_count == 2 {
                    if sync_start < 0 {
                        sync_start = nibble_start;
                    }
                    sync_run += 1;
                }
                if (shift_reg != 0xFF || zero_count != 2) && sync_start >= 0 {
                    // The longest FF/2 run could straddle the end/start of the track's bit buffer.
                    if start_bit_offset < 0 {
                        start_bit_offset = nibble_start;
                    }
                    if longest_run < sync_run {
                        longest_start = sync_start;
                        longest_run = sync_run;
                    }
                    sync_start = -1;
                    sync_run = 0;
                }
                shift_reg = 0;
                zero_count = 0;
            }
            shift_reg = (shift_reg << 1) | out;
            if shift_reg == 0x01 {
                nibble_start = f.bit_offset as i32 - 1;
                if nibble_start < 0 {
                    nibble_start += f.bit_count as i32;
                }
            }
        }
        if longest_run != 0 {
            f.longest_sync_ff_start = longest_start;
            f.longest_sync_ff_run = longest_run as u32;
        }
        f.bit_offset = old_bit_offset;
        f.update_bit_stream_offsets();
    }

    /// `UpdateLatchForEmptyDrive`.
    fn update_latch_for_empty_drive(&mut self, now: u64) {
        let d = &self.drives[self.curr_drive];
        if !d.connected {
            self.floppy_latch = 0x80; // GH#864
        } else if now.wrapping_sub(d.motor_on_cycle) < MOTOR_ON_UNTIL_LSS_STABLE_CYCLES {
            self.floppy_latch = 0x80;
        } else {
            self.floppy_latch = self.rng.next() as u8; // GH#748
        }
    }

    /// `ReadWrite` ($C0EC,X for non-WOZ images): one nibble in or out.
    fn read_write(&mut self, now: u64) {
        let curr = self.curr_drive;
        if !self.drives[curr].disk.track_data && self.drives[curr].disk.image.is_some() {
            self.read_track(curr, now);
        }
        if !self.drives[curr].disk.track_data {
            return self.update_latch_for_empty_drive(now);
        }
        // Improve precision of "authentic" drive mode (GH#125).
        let mut spin_nibbles = 0u32;
        let spinning = self.drives[curr].spinning > 0;
        if !self.enhance_disk && spinning {
            let diff = now.wrapping_sub(self.disk_last_cycle) as u32;
            self.disk_last_cycle = now;
            if diff > 40 {
                // 40 cycles for a write of a 10-bit 0xFF sync byte, but divide by 32 (not 40).
                spin_nibbles = diff >> 5;
                let f = &mut self.drives[curr].disk;
                let wrap = spin_nibbles as usize % f.nibbles.max(1);
                f.byte += wrap;
                if f.byte >= f.nibbles {
                    f.byte -= f.nibbles;
                }
            }
        }
        let write_mode = self.seq_func & 1 != 0;
        if !write_mode {
            // Don't change the latch if the drive is off after the 1 second delay (UTAIIe 9-13).
            if !spinning {
                return; // GH#599
            }
            let read_diff = now.wrapping_sub(self.disk_last_read_latch_cycle) as u32;
            // Support partial nibble read if disk reads are very close (GH#582): 6 cycles for DOS
            // 3.3 $BD34 "read with delays to see if disk is spinning", 25+ fails Legacy of the
            // Ancients (GH#733); 31 is the max for a partial 8-bit nibble.
            const READ_ACCESS_THRESHOLD: u32 = 6;
            let f = &self.drives[curr].disk;
            if read_diff <= READ_ACCESS_THRESHOLD {
                let invalid_bits = 8 - (read_diff / 4);
                self.floppy_latch = f.track[f.byte].checked_shr(invalid_bits).unwrap_or(0);
                return; // don't update disk_last_read_latch_cycle & byte
            }
            self.floppy_latch = f.track[f.byte];
            self.disk_last_read_latch_cycle = now;
            self.format_track.decode_latch_nibble_read(self.floppy_latch);
        } else if !self.drives[curr].disk.write_protected {
            if !spinning {
                return; // if not spinning then only 1 bit-cell gets written?
            }
            let latch = self.floppy_latch;
            let f = &mut self.drives[curr].disk;
            f.track[f.byte] = latch;
            f.track_dirty = true;
            let (wp, byte, nibbles) = (f.write_protected, f.byte, f.nibbles);
            self.format_track.decode_latch_nibble_write(latch, spin_nibbles, wp, byte, nibbles);
        }
        // GH #1212: mirror so the shift register shows the last nibble.
        self.shift_reg = self.floppy_latch;
        let f = &mut self.drives[curr].disk;
        f.byte += 1;
        if f.byte >= f.nibbles {
            f.byte = 0;
        }
    }

    /// `GetBitCellDelta`: whole bit cells elapsed since the last call (carrying the remainder).
    fn bit_cell_delta(&mut self, now: u64) -> u32 {
        let curr = self.curr_drive;
        let timing = self.drives[curr].disk.image.as_ref().map_or(32, DiskImage::optimal_bit_timing);
        let floppy = &mut self.drives[curr].disk;
        // m_extraCycles retains accuracy: 0->9 and 9->20 give the same 5 cells as 0->20.
        let cycle_delta = now.saturating_sub(self.disk_last_cycle) as f64 + floppy.extra_cycles;
        let bit_time = 0.125 * timing as f64;
        let cells = (cycle_delta / bit_time).floor();
        floppy.extra_cycles = cycle_delta - cells * bit_time;
        self.disk_last_cycle = now;
        cells as u32
    }

    /// `PreJitterCheck`.
    fn pre_jitter_check(&mut self, phase: i32, latch: u8) {
        if phase != 0 || latch & 0x80 == 0 {
            return;
        }
        if T00S00_PATTERN.get(self.t00s00_pattern_idx) == Some(&latch) {
            self.t00s00_pattern_idx += 1;
            if self.t00s00_pattern_idx == T00S00_PATTERN.len() {
                self.found_t00s00_pattern = true; // the 6502 has just read the T$00,S$00 address prologue
            }
        } else {
            self.t00s00_pattern_idx = 0;
        }
    }

    /// `AddJitter` (GH#930): after T$00,S$00 randomly skip one bit-cell (Wasteland, Legacy of the
    /// Ancients).
    fn add_jitter(&mut self, phase: i32) {
        if phase == 0 && self.found_t00s00_pattern && self.rng.chance(1, 10) {
            self.drives[self.curr_drive].disk.inc_bit_stream();
        }
        self.t00s00_pattern_idx = 0;
        self.found_t00s00_pattern = false;
    }

    /// `AddTrackSeamJitter` (GH#1125): from T$21 up, randomly skip a bit-cell at the start of a
    /// long FF/10 run.
    fn add_track_seam_jitter(&mut self, phase_precise: f32) {
        let f = &self.drives[self.curr_drive].disk;
        if phase_precise >= 33.0 * 2.0 && f.longest_sync_ff_run > 110 && f.bit_offset as i32 == f.longest_sync_ff_start && self.rng.chance(5, 10) {
            self.drives[self.curr_drive].disk.inc_bit_stream();
        }
    }

    /// `DataLatchReadWriteWOZ`.
    fn data_latch_read_write_woz(&mut self, write: bool, now: u64) {
        let curr = self.curr_drive;
        if !self.drives[curr].disk.track_data && self.drives[curr].disk.image.is_some() {
            self.read_track(curr, now);
        }
        if !self.drives[curr].disk.track_data {
            return self.update_latch_for_empty_drive(now);
        }
        // Don't change the latch if the drive is off after the 1 second delay (UTAIIe 9-13).
        if self.drives[curr].spinning == 0 {
            return; // GH#599
        }
        // Skipping forward a large amount of bitcells leaves the stream out of sync, so run
        // enough bits through the sequencer to re-sync (GH#1020).
        const SIGNIFICANT_BIT_CELLS: u32 = 100;
        let mut delta = self.bit_cell_delta(now);
        let remainder;
        if delta <= SIGNIFICANT_BIT_CELLS {
            remainder = delta;
        } else {
            remainder = SIGNIFICANT_BIT_CELLS;
            delta -= SIGNIFICANT_BIT_CELLS;
            self.drives[curr].disk.update_bit_stream_position(delta);
            self.latch_delay = 0;
            self.drives[curr].head_window = 0;
            let phase = self.drives[curr].phase;
            self.add_jitter(phase);
        }
        if !write {
            if self.seq_func != READ_SEQUENCING {
                debug_assert_eq!(self.seq_func, CHECK_WRITE_PROT_AND_INIT_WRITE);
                self.drives[curr].disk.update_bit_stream_position(remainder);
                return;
            }
            self.data_latch_read_woz(remainder);
            let phase = self.drives[curr].phase;
            self.pre_jitter_check(phase, self.floppy_latch);
        } else {
            debug_assert_eq!(self.seq_func, DATA_LOAD_WRITE);
            self.data_load_write_woz(remainder);
        }
    }

    /// `DataLatchReadWOZ`: shift bit cells through the sequencer.
    fn data_latch_read_woz(&mut self, bit_cells: u32) {
        let curr = self.curr_drive;
        let phase_precise = self.drives[curr].phase_precise;
        for _ in 0..bit_cells {
            let drive = &mut self.drives[curr];
            let bit = drive.disk.bit() as u8;
            drive.head_window = (drive.head_window << 1) | bit;
            let hw = drive.head_window;
            // Within four zero cells the MC3470 returns random "fake" bits (~30% ones, WOZ-2.0).
            let output = if hw & 0xF != 0 { (hw >> 1) & 1 } else { self.rng.chance(3, 10) as u8 };
            self.drives[curr].disk.inc_bit_stream();
            self.add_track_seam_jitter(phase_precise);

            self.shift_reg = (self.shift_reg << 1) | output;
            if self.latch_delay != 0 {
                self.latch_delay -= 4;
                if self.latch_delay < 0 {
                    self.latch_delay = 0;
                }
                if self.shift_reg == 0 {
                    self.latch_delay += 4; // extend by 4us (so 7us again), GH#662
                }
            }
            if self.latch_delay == 0 {
                self.floppy_latch = self.shift_reg;
                if self.shift_reg & 0x80 != 0 {
                    self.latch_delay = 7;
                    self.shift_reg = 0;
                    self.format_track.decode_latch_nibble_read(self.floppy_latch);
                }
            }
        }
    }

    /// `DataLoadWriteWOZ`.
    fn data_load_write_woz(&mut self, bit_cells: u32) {
        let f = &mut self.drives[self.curr_drive].disk;
        if f.write_protected {
            f.update_bit_stream_position(bit_cells); // a bug in the 6502 code
            return;
        }
        if !self.write_started {
            f.update_bit_stream_position(bit_cells); // skip over bitCells before switching to write mode
        }
        self.write_started = true;
        self.shift_reg = self.floppy_latch;
        f.longest_sync_ff_start = -1; // invalidate the track seam location after a write
    }

    /// `DataShiftWriteWOZ`: bits shifted out to the disk since the last access.
    fn data_shift_write_woz(&mut self, now: u64) {
        let curr = self.curr_drive;
        let cells = self.bit_cell_delta(now);
        let spinning = self.drives[curr].spinning > 0;
        let f = &mut self.drives[curr].disk;
        if f.write_protected {
            f.update_bit_stream_position(cells);
            return;
        }
        if !spinning || !f.track_data {
            return; // GH#1126
        }
        for _ in 0..cells {
            let out = self.shift_reg & 0x80;
            self.shift_reg <<= 1;
            if let Some(n) = f.track.get_mut(f.byte) {
                *n &= !f.bit_mask;
                if out != 0 {
                    *n |= f.bit_mask;
                }
            }
            f.inc_bit_stream();
        }
        f.track_dirty = true;
    }

    /// `LoadWriteProtect` ($C08D,X).
    fn load_write_protect(&mut self, now: u64) {
        // Don't change the latch if the drive is off after the 1 second delay (UTAIIe 9-13).
        if self.drives[self.curr_drive].spinning == 0 {
            return; // GH#599
        }
        // The LSS saturates the data register before the CPU can read an intermediate value (GH#1078).
        self.floppy_latch = if self.drives[self.curr_drive].disk.write_protected { 0xFF } else { 0x00 };
        if self.write_started {
            return; // keep reset_logic_state_sequencer from resetting write_started
        }
        if self.drives[self.curr_drive].disk.is_woz() {
            let delta = self.bit_cell_delta(now);
            self.drives[self.curr_drive].disk.update_bit_stream_position(delta); // fix E7 copy-protection
            self.reset_logic_state_sequencer(); // "Set the sequencer to State 0" (UTAIIe 9-21)
        }
    }

    fn set_read_mode(&mut self) {
        let f = &mut self.drives[self.curr_drive].disk;
        let (byte, nibbles) = (f.byte, f.nibbles);
        self.format_track.drive_switched_to_read_mode(&mut f.track, byte, nibbles);
    }

    fn set_write_mode(&mut self) {
        self.format_track.drive_switched_to_write_mode();
        let d = &mut self.drives[self.curr_drive];
        d.write_light = WRITELIGHT_CYCLES;
    }

    /// `SetSequencerFunction`.
    fn set_sequencer_function(&mut self, lo: u8, now: u64) {
        if lo < 0xC {
            return;
        }
        let old = self.seq_func;
        match (lo & 3) ^ 2 {
            0 => self.seq_func &= !1, // $C08E,X
            1 => self.seq_func |= 1,  // $C08F,X
            2 => self.seq_func &= !2, // $C08C,X
            _ => self.seq_func |= 2,  // $C08D,X
        }
        if self.seq_func & 1 == 0 {
            self.write_started = false;
        }
        if old == CHECK_WRITE_PROT_AND_INIT_WRITE && self.seq_func != CHECK_WRITE_PROT_AND_INIT_WRITE && self.drives[self.curr_drive].disk.is_woz() {
            // Use up the remaining cycles before leaving this mode (write-protect check, bit-slip
            // copy protections such as E7).
            let delta = self.bit_cell_delta(now);
            self.drives[self.curr_drive].disk.update_bit_stream_position(delta);
        }
    }

    fn io(&mut self, lo: u8, value: Option<u8>, ctx: &mut CardCtx) -> u8 {
        let now = ctx.access_cycle;
        let mut is_woz = self.drives[self.curr_drive].disk.is_woz();
        if is_woz && self.seq_func == DATA_SHIFT_WRITE {
            self.data_shift_write_woz(now); // finish any previous write (end of sector write, $C0EE)
        }
        self.set_sequencer_function(lo, now);
        match lo {
            0..=7 => self.control_stepper(lo, now, ctx),
            8 | 9 => self.control_motor(lo, now),
            0xA | 0xB => is_woz = self.enable(lo, now),
            0xC => {
                if !is_woz {
                    self.read_write(now);
                }
            }
            0xD => self.load_write_protect(now),
            0xE => self.set_read_mode(),
            _ => self.set_write_mode(),
        }
        if let Some(d) = value {
            // Any address writes the latch via the sequencer LD command (74LS323 datasheet).
            if self.seq_func == DATA_LOAD_WRITE {
                self.floppy_latch = d;
                if is_woz {
                    self.data_latch_read_write_woz(true, now);
                }
            }
            return 0;
        }
        // Only even addresses return the latch (UTAIIe Table 9.1).
        if lo & 1 == 0 {
            if is_woz && self.seq_func != DATA_SHIFT_WRITE {
                self.data_latch_read_write_woz(false, now);
            }
            return self.floppy_latch;
        }
        ctx.floating_bus
    }

    // ---- Save-state. ----

    fn encode(&self) -> Vec<u8> {
        let mut o = Vec::new();
        let b = |o: &mut Vec<u8>, v: bool| o.push(v as u8);
        o.push(1); // version
        o.push(self.curr_drive as u8);
        o.push(self.magnet_states);
        b(&mut o, self.enhance_disk);
        o.push(self.floppy_latch);
        b(&mut o, self.motor_on);
        o.extend(self.disk_last_cycle.to_le_bytes());
        o.extend(self.disk_last_read_latch_cycle.to_le_bytes());
        o.push(self.shift_reg);
        o.extend(self.latch_delay.to_le_bytes());
        o.push(self.seq_func);
        b(&mut o, self.write_started);
        b(&mut o, self.deferred_stepper_event);
        o.push(self.deferred_stepper_address);
        o.extend(self.deferred_stepper_cycles.to_le_bytes());
        o.extend(self.rng.0.to_le_bytes());
        b(&mut o, self.is_13_sector_firmware);
        b(&mut o, self.force_13_sector_firmware);
        for d in &self.drives {
            b(&mut o, d.connected);
            o.extend(d.phase.to_le_bytes());
            o.extend(d.phase_precise.to_le_bytes());
            o.push(d.head_window);
            o.extend(d.last_stepper_cycle.to_le_bytes());
            o.extend(d.motor_on_cycle.to_le_bytes());
            o.extend(d.spinning.to_le_bytes());
            o.extend(d.write_light.to_le_bytes());
            let f = &d.disk;
            b(&mut o, f.write_protected);
            o.extend((f.byte as u32).to_le_bytes());
            o.extend((f.nibbles as u32).to_le_bytes());
            o.extend(f.bit_offset.to_le_bytes());
            o.extend(f.bit_count.to_le_bytes());
            o.extend(f.extra_cycles.to_le_bytes());
            b(&mut o, f.track_data);
            b(&mut o, f.track_dirty);
            o.extend((f.track.len() as u32).to_le_bytes());
            o.extend(&f.track);
        }
        o
    }

    fn decode(&mut self, s: &[u8]) -> Option<()> {
        struct R<'a>(&'a [u8]);
        impl R<'_> {
            fn take(&mut self, n: usize) -> Option<&[u8]> {
                let (a, b) = self.0.split_at_checked(n)?;
                self.0 = b;
                Some(a)
            }
            fn u8(&mut self) -> Option<u8> {
                Some(self.take(1)?[0])
            }
            fn bool(&mut self) -> Option<bool> {
                Some(self.u8()? != 0)
            }
            fn u32(&mut self) -> Option<u32> {
                Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
            }
            fn u64(&mut self) -> Option<u64> {
                Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
            }
        }
        let mut r = R(s);
        if r.u8()? != 1 {
            return None;
        }
        self.curr_drive = (r.u8()? as usize).min(1);
        self.magnet_states = r.u8()?;
        self.enhance_disk = r.bool()?;
        self.floppy_latch = r.u8()?;
        self.motor_on = r.bool()?;
        self.disk_last_cycle = r.u64()?;
        self.disk_last_read_latch_cycle = r.u64()?;
        self.shift_reg = r.u8()?;
        self.latch_delay = r.u32()? as i32;
        self.seq_func = r.u8()? & 3;
        self.write_started = r.bool()?;
        self.deferred_stepper_event = r.bool()?;
        self.deferred_stepper_address = r.u8()?;
        self.deferred_stepper_cycles = r.u64()?;
        self.rng = Rng(r.u32()?.max(1));
        self.is_13_sector_firmware = r.bool()?;
        self.force_13_sector_firmware = r.bool()?;
        for d in &mut self.drives {
            d.connected = r.bool()?;
            d.phase = r.u32()? as i32;
            d.phase_precise = f32::from_bits(r.u32()?);
            d.head_window = r.u8()? & 0xF;
            d.last_stepper_cycle = r.u64()?;
            d.motor_on_cycle = r.u64()?;
            d.spinning = r.u32()?;
            d.write_light = r.u32()?;
            let f = &mut d.disk;
            f.write_protected = r.bool()?;
            f.byte = r.u32()? as usize;
            f.nibbles = r.u32()? as usize;
            f.bit_offset = r.u32()?;
            f.bit_count = r.u32()?;
            f.extra_cycles = f64::from_bits(r.u64()?);
            let (data, dirty) = (r.bool()?, r.bool()?);
            let len = r.u32()? as usize;
            let track = r.take(len)?;
            if f.image.is_some() && f.track.len() >= len && (f.bit_count == 0 || f.bit_offset < f.bit_count) {
                f.track[..len].copy_from_slice(track);
                f.track_data = data;
                f.track_dirty = dirty;
                f.update_bit_stream_offsets_if_woz();
            } else {
                f.track_data = false;
                f.track_dirty = false;
                f.nibbles = 0;
            }
        }
        self.events.clear();
        Some(())
    }
}

impl Floppy {
    fn update_bit_stream_offsets_if_woz(&mut self) {
        if self.is_woz() {
            self.update_bit_stream_offsets(); // overwrites byte, inits bit_mask
        }
    }
}

impl Card for Disk2Card {
    fn name(&self) -> &'static str {
        "Disk II"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        self.io(lo, None, ctx)
    }

    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        self.io(lo, Some(value), ctx);
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        if self.is_13_sector_firmware {
            self.firmware_13[offset as usize]
        } else {
            self.firmware_16[offset as usize]
        }
    }

    fn reset(&mut self, power_cycle: bool, _ctx: &mut CardCtx) {
        // RESET' forces all switches off (UTAIIe Table 9.1).
        self.reset_switches();
        self.format_track.reset();
        self.reset_logic_state_sequencer();
        if power_cycle {
            // The drive head keeps its position; only the spin and the write light stop. The
            // machine cleared its scheduler and cycle counter.
            for d in &mut self.drives {
                d.spinning = 0;
                d.write_light = 0;
                d.motor_on_cycle = 0;
            }
            self.disk_last_cycle = 0;
            self.disk_last_read_latch_cycle = 0;
            self.deferred_stepper_event = false;
        }
        self.init_firmware();
    }

    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        let cycles = cycles.min(u32::MAX as u64) as u32;
        for i in (0..NUM_DRIVES).rev() {
            let motor_on = self.motor_on;
            let write_mode = self.seq_func & 1 != 0;
            let is_curr = self.curr_drive == i;
            let d = &mut self.drives[i];
            if d.spinning > 0 && !motor_on {
                d.spinning -= d.spinning.min(cycles);
            }
            if write_mode && is_curr && d.spinning > 0 {
                d.write_light = WRITELIGHT_CYCLES;
            } else if d.write_light > 0 {
                d.write_light -= d.write_light.min(cycles);
            }
            if !motor_on && d.spinning == 0 {
                // Motor off and not spinning: write back any dirty track (GH#1444).
                self.flush_current_track(i);
            }
        }
    }

    fn on_event(&mut self, token: u16, _ctx: &mut CardCtx) -> i32 {
        if token == STEPPER_EVENT {
            self.control_stepper_deferred();
        }
        0
    }

    fn save(&self) -> Vec<u8> {
        self.encode()
    }

    fn load(&mut self, state: &[u8]) -> bool {
        self.decode(state).is_some()
    }
}
