//! Firmware images embedded from `roms/` (ADR 0002). Sizes follow AppleWin Memory.cpp.

#![forbid(unsafe_code)]

pub const APPLE2: &[u8] = include_bytes!("../../../roms/Apple2.rom");
pub const APPLE2_JPLUS: &[u8] = include_bytes!("../../../roms/Apple2_JPlus.rom");
pub const APPLE2_JPLUS_VIDEO: &[u8] = include_bytes!("../../../roms/Apple2_JPlus_Video.rom");
pub const APPLE2_PLUS: &[u8] = include_bytes!("../../../roms/Apple2_Plus.rom");
pub const APPLE2_VIDEO: &[u8] = include_bytes!("../../../roms/Apple2_Video.rom");
pub const APPLE2E: &[u8] = include_bytes!("../../../roms/Apple2e.rom");
pub const APPLE2E_ENHANCED: &[u8] = include_bytes!("../../../roms/Apple2e_Enhanced.rom");
pub const APPLE2E_ENHANCED_VIDEO: &[u8] =
    include_bytes!("../../../roms/Apple2e_Enhanced_Video.rom");
pub const BASE64A: &[u8] = include_bytes!("../../../roms/Base64A.rom");
pub const BASE64A_GERMAN_VIDEO: &[u8] = include_bytes!("../../../roms/Base64A_German_Video.rom");
pub const DISK2_13SECTOR: &[u8] = include_bytes!("../../../roms/DISK2-13sector.rom");
pub const DISK2: &[u8] = include_bytes!("../../../roms/DISK2.rom");
pub const FREEZES_NON_AUTOSTART_F8_ROM: &[u8] =
    include_bytes!("../../../roms/Freezes_Non-autostart_F8_Rom.rom");
pub const HDC_SMARTPORT_BIN: &[u8] = include_bytes!("../../../roms/HDC-SmartPort.bin");
pub const HDDRVR_V2_BIN: &[u8] = include_bytes!("../../../roms/Hddrvr-v2.bin");
pub const HDDRVR_BIN: &[u8] = include_bytes!("../../../roms/Hddrvr.bin");
pub const MOCKINGBOARD_D: &[u8] = include_bytes!("../../../roms/Mockingboard-D.rom");
pub const MOUSEINTERFACE: &[u8] = include_bytes!("../../../roms/MouseInterface.rom");
pub const PRAVETS82: &[u8] = include_bytes!("../../../roms/PRAVETS82.ROM");
pub const PRAVETS8C: &[u8] = include_bytes!("../../../roms/PRAVETS8C.ROM");
pub const PRAVETS8M: &[u8] = include_bytes!("../../../roms/PRAVETS8M.ROM");
pub const PARALLEL: &[u8] = include_bytes!("../../../roms/Parallel.rom");
pub const SSC: &[u8] = include_bytes!("../../../roms/SSC.rom");
pub const TK3000E: &[u8] = include_bytes!("../../../roms/TK3000e.rom");
pub const TKCLOCK: &[u8] = include_bytes!("../../../roms/TKClock.rom");
pub const THUNDERCLOCKPLUS: &[u8] = include_bytes!("../../../roms/ThunderClockPlus.rom");

/// SHA-256 of `data` (FIPS 180-4), for ROM provenance. Self-contained to keep this crate free of
/// dependencies.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, c) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (o, x) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        o.copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// [`sha256`] as 64 lowercase hex digits.
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    sha256(data)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // two-block message (padding crosses a block boundary)
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn embedded_roms_have_expected_sizes() {
        assert_eq!(APPLE2.len(), 12288);
        assert_eq!(APPLE2_JPLUS.len(), 12288);
        assert_eq!(APPLE2_JPLUS_VIDEO.len(), 2048);
        assert_eq!(APPLE2_PLUS.len(), 12288);
        assert_eq!(APPLE2_VIDEO.len(), 2048);
        assert_eq!(APPLE2E.len(), 16384);
        assert_eq!(APPLE2E_ENHANCED.len(), 16384);
        assert_eq!(APPLE2E_ENHANCED_VIDEO.len(), 4096);
        assert_eq!(BASE64A.len(), 49152);
        assert_eq!(BASE64A_GERMAN_VIDEO.len(), 4096);
        assert_eq!(DISK2_13SECTOR.len(), 256);
        assert_eq!(DISK2.len(), 256);
        assert_eq!(FREEZES_NON_AUTOSTART_F8_ROM.len(), 2048);
        assert_eq!(HDC_SMARTPORT_BIN.len(), 256);
        assert_eq!(HDDRVR_V2_BIN.len(), 256);
        assert_eq!(HDDRVR_BIN.len(), 256);
        assert_eq!(MOCKINGBOARD_D.len(), 2048);
        assert_eq!(MOUSEINTERFACE.len(), 2048);
        assert_eq!(PRAVETS82.len(), 12288);
        assert_eq!(PRAVETS8C.len(), 16384);
        assert_eq!(PRAVETS8M.len(), 12288);
        assert_eq!(PARALLEL.len(), 256);
        assert_eq!(SSC.len(), 2048);
        assert_eq!(TK3000E.len(), 16384);
        assert_eq!(TKCLOCK.len(), 2304);
        assert_eq!(THUNDERCLOCKPLUS.len(), 2048);
    }
}
