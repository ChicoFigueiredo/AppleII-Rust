//! Monitor for the formatting of a track (AppleWin DiskFormatTrack.cpp). While DOS 3.3 `INIT` or
//! ProDOS `FORMAT` write a whole track, this watches the nibbles and, when the drive goes back
//! to read mode, zero-fills the rest of the (longer) track buffer so the old data cannot be
//! read as a second revolution.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrackState {
    Gap1,
    AddrField,
    Gap2Start,
    Gap2,
    DataField,
    Gap3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatTrack {
    vol_trk_sec_chk: [u8; 4],
    vol_trk_sec_chk_4and4: [u8; 8],
    written_sector_addr_fields: u16,
    /// `None` while waiting for the first write.
    write_track_start_index: Option<usize>,
    write_track_has_wrapped: bool,
    write_data_field_prologue_count: u8,
    address_prologue_is_dos32: bool,
    state: TrackState,
    last3: u32,
    idx_4and4: usize,
}

impl Default for FormatTrack {
    fn default() -> Self {
        let mut f = FormatTrack {
            vol_trk_sec_chk: [0; 4],
            vol_trk_sec_chk_4and4: [0; 8],
            written_sector_addr_fields: 0,
            write_track_start_index: Some(0),
            write_track_has_wrapped: false,
            write_data_field_prologue_count: 0,
            address_prologue_is_dos32: false,
            state: TrackState::Gap1,
            last3: 0,
            idx_4and4: 0,
        };
        f.reset();
        f
    }
}

impl FormatTrack {
    /// Controller reset (:54).
    pub fn reset(&mut self) {
        self.vol_trk_sec_chk = [0; 4];
        self.vol_trk_sec_chk_4and4 = [0; 8];
        self.state = TrackState::Gap1;
        self.last3 = 0;
        self.idx_4and4 = 0;
        self.drive_not_writing_track();
    }

    /// Spin of more than 2 bytes, head step, motor change or read mode after a complete track (:70).
    pub fn drive_not_writing_track(&mut self) {
        self.written_sector_addr_fields = 0;
        self.write_track_start_index = Some(0);
        self.write_track_has_wrapped = false;
        self.write_data_field_prologue_count = 0;
        self.address_prologue_is_dos32 = false;
    }

    fn update_on_write_latch(&mut self, spin_nibbles: u32, write_protected: bool, byte: usize, nibbles: usize) {
        if write_protected {
            return;
        }
        if self.written_sector_addr_fields == 0 {
            if self.write_track_start_index.is_none() {
                self.write_track_start_index = Some(byte);
            }
            return;
        }
        if self.write_track_has_wrapped {
            return;
        }
        if spin_nibbles > 2 {
            self.drive_not_writing_track();
            return;
        }
        let mut index = byte;
        let mut spin = spin_nibbles as i64;
        loop {
            if self.write_track_start_index == Some(index) {
                self.write_track_has_wrapped = true; // the disk has completed a revolution
                return;
            }
            index = (index + 1) % nibbles.max(1);
            let again = spin != 0;
            spin -= 1;
            if !again {
                break;
            }
        }
    }

    /// Switch to read mode: after a complete track, zero the part of the buffer not rewritten (:129).
    pub fn drive_switched_to_read_mode(&mut self, track: &mut [u8], byte: usize, nibbles: usize) {
        if self.address_prologue_is_dos32 {
            if self.written_sector_addr_fields != 0x1FFF {
                return;
            }
        } else if self.written_sector_addr_fields != 0xFFFF || self.write_data_field_prologue_count != 16 {
            return;
        }
        const SHORT_TRACK_LEN: usize = 0x18B0;
        let long_len = nibbles;
        if track.len() < long_len || long_len == 0 {
            return;
        }
        let start = self.write_track_start_index.unwrap_or(0);
        let end = byte;
        let mut written = if self.write_track_has_wrapped { long_len } else { 0 };
        written += if start <= end { end - start } else { (long_len - start) + end };
        written = written.min(SHORT_TRACK_LEN);
        let mut src = end as i64 - written as i64;
        if src < 0 {
            src += long_len as i64;
        }
        let src = src as usize;
        if src < end {
            track[..src].fill(0);
            track[end..long_len].fill(0);
        } else {
            track[end..src].fill(0);
        }
        self.drive_not_writing_track();
    }

    pub fn drive_switched_to_write_mode(&mut self) {
        if self.state != TrackState::Gap2Start {
            self.state = if self.written_sector_addr_fields == 0 { TrackState::Gap1 } else { TrackState::Gap3 };
        }
        self.last3 = 0;
        self.idx_4and4 = 0;
        if self.written_sector_addr_fields == 0 {
            self.write_track_start_index = None; // wait for the first write
        }
    }

    pub fn decode_latch_nibble_read(&mut self, latch: u8) {
        self.decode_latch_nibble(latch, false);
    }

    pub fn decode_latch_nibble_write(&mut self, latch: u8, spin_nibbles: u32, write_protected: bool, byte: usize, nibbles: usize) {
        self.decode_latch_nibble(latch, true);
        self.update_on_write_latch(spin_nibbles, write_protected, byte, nibbles);
    }

    fn decode_latch_nibble(&mut self, latch: u8, is_write: bool) {
        self.last3 = ((self.last3 << 8) | latch as u32) & 0xFF_FFFF;
        if self.state == TrackState::Gap2Start && is_write {
            self.state = TrackState::Gap2;
        }
        // Beneath Apple ProDOS 3-14: D5 and AA are reserved (never written as data).
        const ADDR_PROLOGUE_DOS33: u32 = 0xD5AA96;
        const ADDR_PROLOGUE_DOS32: u32 = 0xD5AAB5;
        if self.last3 == ADDR_PROLOGUE_DOS33 || self.last3 == ADDR_PROLOGUE_DOS32 {
            self.address_prologue_is_dos32 = self.last3 == ADDR_PROLOGUE_DOS32;
            self.state = TrackState::AddrField;
            self.idx_4and4 = 0;
            return;
        }
        // If AddrField with idx == 8 the epilogue is being written.
        if self.state == TrackState::AddrField && self.idx_4and4 < 8 {
            self.vol_trk_sec_chk_4and4[self.idx_4and4] = latch;
            self.idx_4and4 += 1;
            if self.idx_4and4 == 8 {
                for i in 0..4 {
                    self.vol_trk_sec_chk[i] = ((self.vol_trk_sec_chk_4and4[i * 2] & 0x55) << 1) | (self.vol_trk_sec_chk_4and4[i * 2 + 1] & 0x55);
                }
                if is_write {
                    let sector = self.vol_trk_sec_chk[2];
                    if sector > 15 {
                        return; // exotic formats with more than 16 sectors
                    }
                    self.written_sector_addr_fields |= 1 << sector;
                }
            }
            return;
        }
        if self.last3 == 0xDEAAEB && is_write {
            self.state = if self.state == TrackState::AddrField { TrackState::Gap2Start } else { TrackState::Gap3 };
            return;
        }
        if self.last3 == 0xD5AAAD {
            self.state = TrackState::DataField;
            if is_write {
                self.write_data_field_prologue_count += 1; // written every time a sector is written
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a full 16-sector DOS 3.3 format through the monitor, then goes back to read mode.
    #[test]
    fn a_complete_format_zero_fills_the_unwritten_tail() {
        let mut f = FormatTrack::default();
        let mut track = vec![0xAAu8; 0x1A00];
        f.drive_switched_to_write_mode();
        let mut byte = 0usize;
        let put = |f: &mut FormatTrack, v: u8, byte: &mut usize, track: &mut Vec<u8>| {
            track[*byte] = v;
            f.decode_latch_nibble_write(v, 0, false, *byte, 0x1A00);
            *byte = (*byte + 1) % 0x1A00;
        };
        for _ in 0..16 {
            put(&mut f, 0xFF, &mut byte, &mut track);
        }
        for s in 0..16u8 {
            for v in [0xD5, 0xAA, 0x96, 0xFF, 0xFE, 0xAA, 0xAA, 0xAA | (s >> 1 & 0x55), 0xAA | (s & 0x55), 0xFF, 0xFE, 0xDE, 0xAA, 0xEB] {
                put(&mut f, v, &mut byte, &mut track);
            }
            for v in [0xD5, 0xAA, 0xAD] {
                put(&mut f, v, &mut byte, &mut track);
            }
            for _ in 0..40 {
                put(&mut f, 0x96, &mut byte, &mut track);
            }
        }
        f.drive_switched_to_read_mode(&mut track, byte, 0x1A00);
        assert_eq!(track[byte], 0, "the part after the formatted data is cleared");
        assert_eq!(track[0], 0xFF, "the formatted part is kept");
    }

    #[test]
    fn an_incomplete_format_leaves_the_track_alone() {
        let mut f = FormatTrack::default();
        let mut track = vec![0xAAu8; 0x1A00];
        f.drive_switched_to_write_mode();
        f.decode_latch_nibble_write(0xFF, 0, false, 0, 0x1A00);
        f.drive_switched_to_read_mode(&mut track, 100, 0x1A00);
        assert!(track.iter().all(|&b| b == 0xAA));
    }
}
