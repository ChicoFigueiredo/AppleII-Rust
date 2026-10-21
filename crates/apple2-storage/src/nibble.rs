//! 6-and-2 nibblization of a 16-sector track (AppleWin DiskImageHelper.cpp `CImageBase`
//! `Code62`/`Decode62`/`NibblizeTrack`/`DenibblizeTrack`/`SkewTrack`, :298-582).

/// `#Sectors x Sector-size` (DiskImage.h `TRACK_DENIBBLIZED_SIZE`).
pub const TRACK_DENIBBLIZED_SIZE: usize = 16 * 256;
pub const NUM_SECTORS: usize = 16;
/// `NIBBLES_PER_TRACK_NIB` (DiskDefs.h:3).
pub const NIBBLES_PER_TRACK_NIB: usize = 0x1A00;
/// `NIBBLES_PER_TRACK_WOZ2` = the largest track buffer (DiskDefs.h:4-5).
pub const NIBBLES_PER_TRACK: usize = 0x1A18;
pub const DEFAULT_VOLUME_NUMBER: u8 = 254;

/// `ms_DiskByte` (:87).
pub const DISK_BYTE: [u8; 0x40] = [
    0x96, 0x97, 0x9A, 0x9B, 0x9D, 0x9E, 0x9F, 0xA6, 0xA7, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB2, 0xB3,
    0xB4, 0xB5, 0xB6, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xCB, 0xCD, 0xCE, 0xCF, 0xD3,
    0xD6, 0xD7, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC,
    0xED, 0xEE, 0xEF, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
];

/// Logical sector -> physical position in the file, for ProDOS- and DOS-ordered images (:99).
pub const PRODOS_ORDER: [u8; 16] = [
    0x00, 0x08, 0x01, 0x09, 0x02, 0x0A, 0x03, 0x0B, 0x04, 0x0C, 0x05, 0x0D, 0x06, 0x0E, 0x07, 0x0F,
];
pub const DOS_ORDER: [u8; 16] = [
    0x00, 0x07, 0x0E, 0x06, 0x0D, 0x05, 0x0C, 0x04, 0x0B, 0x03, 0x0A, 0x02, 0x09, 0x01, 0x08, 0x0F,
];

/// Encodes the 256 bytes of `sector` (`data` holds the 16 sectors of the track in file order) as
/// 343 disk nibbles (:298).
fn code62(data: &[u8], sector: usize) -> [u8; 343] {
    let base = &data[sector << 8..(sector << 8) + 256];
    let mut six = [0u8; 342];
    let mut offset: u8 = 0xAC;
    let mut i = 0;
    while offset != 0x02 {
        let mut value = 0u8;
        for step in [0x56u8, 0x56, 0x53] {
            let a = base[offset as usize];
            value = (value << 2) | ((a & 1) << 1) | ((a & 2) >> 1);
            offset = offset.wrapping_sub(step);
        }
        six[i] = value << 2;
        i += 1;
    }
    six[i - 2] &= 0x3F;
    six[i - 1] &= 0x3F;
    six[0x56..].copy_from_slice(base);
    let mut out = [0u8; 343];
    let mut saved = 0u8;
    for (o, s) in out.iter_mut().zip(&six) {
        *o = DISK_BYTE[((saved ^ *s) >> 2) as usize];
        saved = *s;
    }
    out[342] = DISK_BYTE[(saved >> 2) as usize];
    out
}

fn six_bit_table() -> [u8; 0x80] {
    let mut t = [0u8; 0x80];
    for (i, b) in DISK_BYTE.iter().enumerate() {
        t[(*b - 0x80) as usize] = (i as u8) << 2;
    }
    t
}

/// Decodes 343 nibbles into the 256 bytes of a sector (:359).
fn decode62(nibbles: &[u8], table: &[u8; 0x80], image: &mut [u8]) {
    let mut six = [0u8; 342];
    for (s, n) in six.iter_mut().zip(nibbles) {
        *s = table[(*n & 0x7F) as usize];
    }
    let mut saved = 0u8;
    for s in six.iter_mut() {
        *s ^= saved;
        saved = *s;
    }
    let (low, data) = six.split_at(0x56);
    let mut offset: u8 = 0xAC;
    let mut li = 0;
    while offset != 0x02 {
        let l = low[li];
        if offset >= 0xAC {
            image[offset as usize] =
                (data[offset as usize] & 0xFC) | ((l & 0x80) >> 7) | ((l & 0x40) >> 5);
        }
        offset = offset.wrapping_sub(0x56);
        image[offset as usize] =
            (data[offset as usize] & 0xFC) | ((l & 0x20) >> 5) | ((l & 0x10) >> 3);
        offset = offset.wrapping_sub(0x56);
        image[offset as usize] =
            (data[offset as usize] & 0xFC) | ((l & 0x08) >> 3) | ((l & 0x04) >> 1);
        offset = offset.wrapping_sub(0x53);
        li += 1;
    }
}

/// Builds the nibble stream of track `track` from its 16 sectors (`NibblizeTrack`, :508) into
/// `out` and returns its length (0x18F0 for a 16-sector track). `order` maps logical sectors to
/// file positions.
pub fn nibblize_track(
    data: &[u8],
    order: &[u8; 16],
    volume: u8,
    track: u8,
    out: &mut [u8],
) -> usize {
    let a = |v: u8| ((v >> 1) & 0x55) | 0xAA;
    let b = |v: u8| (v & 0x55) | 0xAA;
    let mut n = 0;
    let mut put = |bytes: &[u8], out: &mut [u8]| {
        out[n..n + bytes.len()].copy_from_slice(bytes);
        n += bytes.len();
    };
    put(&[0xFF; 48], out);
    for sector in 0..16u8 {
        let chk = volume ^ track ^ sector;
        put(
            &[
                0xD5,
                0xAA,
                0x96,
                a(volume),
                b(volume),
                a(track),
                b(track),
                a(sector),
                b(sector),
                a(chk),
                b(chk),
                0xDE,
                0xAA,
                0xEB,
            ],
            out,
        );
        put(&[0xFF; 6], out);
        put(&[0xD5, 0xAA, 0xAD], out);
        put(&code62(data, order[sector as usize] as usize), out);
        put(&[0xDE, 0xAA, 0xEB], out);
        put(&[0xFF; 27], out);
    }
    n
}

/// Reverses [`nibblize_track`] on `nibbles` (`DenibblizeTrack`, :431); sectors not found stay 0.
pub fn denibblize_track(nibbles: &[u8], order: &[u8; 16]) -> [u8; TRACK_DENIBBLIZED_SIZE] {
    let table = six_bit_table();
    let mut work = [0u8; TRACK_DENIBBLIZED_SIZE];
    let count = nibbles.len();
    if count == 0 {
        return work;
    }
    let mut offset = 0;
    let mut sector: i32 = -1;
    for _ in 0..NUM_SECTORS * 2 + 1 {
        let mut byteval = [0u8; 3];
        let mut bytenum = 0;
        let mut left = count;
        while left > 0 && bytenum < 3 {
            left -= 1;
            let v = nibbles[offset];
            offset += 1;
            if bytenum > 0 {
                byteval[bytenum] = v;
                bytenum += 1;
            } else if v == 0xD5 {
                bytenum = 1;
            }
            if offset >= count {
                offset = 0;
            }
        }
        if bytenum == 3 && byteval[1] == 0xAA {
            let mut field = [0u8; 384];
            let mut t = offset;
            for f in field.iter_mut() {
                *f = nibbles[t];
                t += 1;
                if t >= count {
                    t = 0;
                }
            }
            if byteval[2] == 0x96 {
                sector = (((field[4] & 0x55) << 1) | (field[5] & 0x55)) as i32;
            } else if byteval[2] == 0xAD {
                if (0..16).contains(&sector) {
                    let pos = (order[sector as usize] as usize) << 8;
                    decode62(&field, &table, &mut work[pos..pos + 256]);
                }
                sector = 0;
            }
        }
    }
    work
}

/// Rotates a nibblized track to emulate the physical sector skew of an "authentic speed" disk
/// (`SkewTrack`, :576).
pub fn skew_track(track: u8, buf: &mut [u8]) {
    let skew = (track as usize * 768) % buf.len();
    buf.rotate_left(skew);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_track() -> Vec<u8> {
        (0..TRACK_DENIBBLIZED_SIZE)
            .map(|i| (i * 7 + i / 256) as u8)
            .collect()
    }

    #[test]
    fn nibblized_track_has_the_dos33_layout() {
        let data = sample_track();
        let mut out = [0u8; NIBBLES_PER_TRACK];
        let n = nibblize_track(&data, &DOS_ORDER, 254, 3, &mut out);
        assert_eq!(n, 6384); // a 16-sector track, as in a 6384-byte NB2 image
        assert!(out[..48].iter().all(|&b| b == 0xFF));
        // Sector 0's address field: volume 254, track 3, sector 0, checksum 0xFD, in 4-and-4.
        assert_eq!(
            &out[48..62],
            &[0xD5, 0xAA, 0x96, 0xFF, 0xFE, 0xAB, 0xAB, 0xAA, 0xAA, 0xFE, 0xFF, 0xDE, 0xAA, 0xEB]
        );
        assert!(
            out[..n].iter().all(|&b| b & 0x80 != 0),
            "every nibble has the high bit set"
        );
    }

    #[test]
    fn denibblize_inverts_nibblize_for_both_orders() {
        for order in [DOS_ORDER, PRODOS_ORDER] {
            let data = sample_track();
            let mut out = [0u8; NIBBLES_PER_TRACK];
            let n = nibblize_track(&data, &order, 254, 17, &mut out);
            assert_eq!(&denibblize_track(&out[..n], &order)[..], &data[..]);
        }
    }

    #[test]
    fn denibblize_survives_a_rotated_track() {
        let data = sample_track();
        let mut out = [0u8; NIBBLES_PER_TRACK];
        let n = nibblize_track(&data, &DOS_ORDER, 254, 5, &mut out);
        let mut rotated = out[..n].to_vec();
        skew_track(5, &mut rotated);
        assert_ne!(rotated[..], out[..n]);
        assert_eq!(&denibblize_track(&rotated, &DOS_ORDER)[..], &data[..]);
    }
}
