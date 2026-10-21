//! WOZ 1/2 container parsing (AppleWin DiskImageHelper.h:202-357 `CWOZHelper`, .cpp:1519-1586,
//! :2285-2333). Only the 5.25" INFO/TMAP/TRKS chunks are used (WRIT/META/FLUX are skipped), as
//! in AppleWin.

use flate2::Crc;

pub const ID1_WOZ1: &[u8; 4] = b"WOZ1";
pub const ID1_WOZ2: &[u8; 4] = b"WOZ2";
pub const ID2: [u8; 4] = [0xFF, 0x0A, 0x0D, 0x0A];
pub const HEADER_SIZE: usize = 12;
pub const MAX_TRACKS_5_25: u32 = 40;
pub const MAX_QUARTER_TRACKS_5_25: usize = 160;
pub const WOZ1_TRACK_SIZE: usize = 6656;
/// Offset of the `TRKv1` record inside a WOZ1 track block.
pub const WOZ1_TRK_OFFSET: usize = 6646;
pub const EMPTY_TRACK_SIZE: usize = 6400;
pub const BLOCK_SIZE: usize = 512;
pub const TMAP_TRACK_EMPTY: u8 = 0xFF;
pub const TRK_DEFAULT_BLOCK_COUNT_5_25: u16 = 13;
pub const OPTIMAL_BIT_TIMING_5_25: u8 = 32;
pub const BOOT_SECTOR_13: u8 = 2;
const INFO_CHUNK_SIZE: usize = 60;

fn u16le(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32le(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Where the interesting chunks sit in the file, plus the INFO fields the emulation needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WozLayout {
    pub version: u8,
    pub write_protected: bool,
    pub optimal_bit_timing: u8,
    pub boot_sector_format: u8,
    pub max_nibbles_per_track: usize,
    /// File offset of the 160-byte TMAP.
    pub tmap: usize,
    /// File offset of the TRKS chunk data (`pImageInfo->uOffset`).
    pub trks: usize,
    /// Size of the TRKS chunk data.
    pub trks_size: usize,
}

/// `CWOZHelper::ProcessChunks` + `GetOptimalBitTiming/GetMaxNibblesPerTrack/GetBootSectorFormat`:
/// None if the chunks are malformed or the disk is not 5.25".
pub fn process_chunks(data: &[u8]) -> Option<WozLayout> {
    let mut pos = HEADER_SIZE;
    let (mut info, mut tmap, mut trks) = (None, None, None);
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32le(data, pos + 4) as usize;
        pos += 8;
        if pos + size > data.len() {
            return None;
        }
        match id {
            b"INFO" => {
                if info.is_some() || size < 44 || data[pos + 1] != 1 {
                    return None; // duplicate, too short, or not a 5.25" disk
                }
                info = Some(pos);
            }
            b"TMAP" => {
                if tmap.is_some() || size < MAX_QUARTER_TRACKS_5_25 {
                    return None;
                }
                tmap = Some(pos);
            }
            b"TRKS" => {
                if trks.is_some() {
                    return None;
                }
                trks = Some((pos, size));
            }
            _ => {} // WRIT, META, FLUX and unknown chunks
        }
        pos += size;
    }
    let (info, tmap, (trks, trks_size)) = (info?, tmap?, trks?);
    let version = data[info];
    let v2 = version >= 2 && data.len() >= info + 46;
    Some(WozLayout {
        version,
        write_protected: data[info + 2] == 1,
        optimal_bit_timing: if v2 {
            data[info + 39]
        } else {
            OPTIMAL_BIT_TIMING_5_25
        },
        boot_sector_format: if v2 { data[info + 38] } else { 0 },
        max_nibbles_per_track: if v2 {
            u16le(data, info + 44) as usize * BLOCK_SIZE
        } else {
            WOZ1_TRACK_SIZE
        },
        tmap,
        trks,
        trks_size,
    })
}

/// CRC-32 of everything after the 12-byte header (WOZ spec).
pub fn body_crc(data: &[u8]) -> u32 {
    let mut c = Crc::new();
    c.update(&data[HEADER_SIZE..]);
    c.sum()
}

/// Stores the CRC in the header.
pub fn update_crc(data: &mut [u8]) {
    let crc = body_crc(data);
    data[8..12].copy_from_slice(&crc.to_le_bytes());
}

/// TMAP entry for quarter track `quarter` (`phase * 2`).
pub fn tmap_entry(data: &[u8], l: &WozLayout, quarter: usize) -> u8 {
    data.get(l.tmap + quarter)
        .copied()
        .filter(|_| quarter < MAX_QUARTER_TRACKS_5_25)
        .unwrap_or(TMAP_TRACK_EMPTY)
}

/// `TRKv2` record `index`: (start block, block count, bit count).
pub fn trk2(data: &[u8], l: &WozLayout, index: usize) -> Option<(u32, u32, u32)> {
    let at = l.trks + index * 8;
    (index < MAX_QUARTER_TRACKS_5_25 && at + 8 <= data.len()).then(|| {
        (
            u16le(data, at) as u32,
            u16le(data, at + 2) as u32,
            u32le(data, at + 4),
        )
    })
}

/// `CWOZHelper::CreateEmptyDisk`: an empty 5.25" WOZ2 (3 blocks, every track empty).
pub fn create_empty_woz2() -> Vec<u8> {
    let mut d = vec![0u8; 3 * BLOCK_SIZE];
    d[0..4].copy_from_slice(ID1_WOZ2);
    d[4..8].copy_from_slice(&ID2);
    let put = |d: &mut Vec<u8>, at: usize, id: &[u8; 4], size: usize| {
        d[at..at + 4].copy_from_slice(id);
        d[at + 4..at + 8].copy_from_slice(&(size as u32).to_le_bytes());
    };
    put(&mut d, 12, b"INFO", INFO_CHUNK_SIZE);
    let i = 20;
    d[i] = 2; // version
    d[i + 1] = 1; // 5.25"
    d[i + 4] = 1; // cleaned
    let creator = b"AppleII-Rust";
    d[i + 5..i + 37].fill(b' ');
    d[i + 5..i + 5 + creator.len()].copy_from_slice(creator);
    d[i + 37] = 1; // sides
    d[i + 38] = 0; // boot sector format unknown
    d[i + 39] = OPTIMAL_BIT_TIMING_5_25;
    d[i + 44..i + 46].copy_from_slice(&TRK_DEFAULT_BLOCK_COUNT_5_25.to_le_bytes());
    put(&mut d, 80, b"TMAP", MAX_QUARTER_TRACKS_5_25);
    d[88..88 + MAX_QUARTER_TRACKS_5_25].fill(TMAP_TRACK_EMPTY);
    put(&mut d, 248, b"TRKS", 1280);
    update_crc(&mut d);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_disk_parses_and_has_the_documented_offsets() {
        let d = create_empty_woz2();
        assert_eq!(d.len(), 1536);
        let l = process_chunks(&d).unwrap();
        assert_eq!((l.tmap, l.trks, l.trks_size), (88, 256, 1280));
        assert_eq!(
            (l.version, l.optimal_bit_timing, l.max_nibbles_per_track),
            (2, 32, 13 * 512)
        );
        assert_eq!(tmap_entry(&d, &l, 0), TMAP_TRACK_EMPTY);
        assert_eq!(u32le(&d, 8), body_crc(&d));
    }

    #[test]
    fn rejects_duplicate_and_non_525_chunks() {
        let mut d = create_empty_woz2();
        d[21] = 2; // 3.5" disk
        assert!(process_chunks(&d).is_none());
        let mut d = create_empty_woz2();
        d.truncate(100);
        assert!(process_chunks(&d).is_none());
    }
}
