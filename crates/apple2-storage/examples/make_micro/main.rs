//! Builds the oracle micro-test disks (HU-0301/0303): tiny boot sectors that exercise one Disk II
//! behaviour each, so that the Rust machine and the AppleWin oracle can be compared cycle by cycle
//! (`disk_follow`) and by the image they leave behind.
//!
//! usage: make_micro OUT_DIR      writes OUT_DIR/<name>.dsk.gz (140 KB images that are zero except
//!                                for the patterned sectors; <1 KB compressed) and WOZ/HDV/2MG
//!                                variants. The output is deterministic.

mod asm;

use apple2_storage::image::{Class, DiskImage};
use apple2_storage::woz;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `DISK2.rom` boot: loads track 0 sector 0 to $0800 and jumps to $0801 with X = slot * 16.
const SLOT_X: u8 = 0x60;

/// Patterned DOS-order image: every sector has distinct, non-zero content.
fn patterned_dsk() -> Vec<u8> {
    let mut d = vec![0u8; 143_360];
    for t in 0..35usize {
        for s in 0..16usize {
            for i in 0..256usize {
                d[(t * 16 + s) * 256 + i] =
                    (((t * 16 + s) * 7 + i * 3) as u8) | if i % 5 == 0 { 0x80 } else { 0 };
            }
        }
    }
    d
}

fn boot_sector(src: &str) -> Vec<u8> {
    let (org, code) = asm::assemble(src).unwrap_or_else(|e| panic!("assembly failed: {e}"));
    assert_eq!(org, 0x0801, "boot programs start at $0801");
    let mut s = vec![1u8]; // number of sectors the ROM loads
    s.extend(code);
    assert!(s.len() <= 256, "boot sector is {} bytes", s.len());
    s.resize(256, 0);
    s
}

fn dsk_with(src: &str) -> Vec<u8> {
    let mut d = patterned_dsk();
    d[..256].copy_from_slice(&boot_sector(src));
    d
}

fn gz(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// `WOZ2` with the tracks of a `.dsk` (nibblized) at the listed track numbers.
fn woz_from_dsk(dsk: &[u8], tracks: &[u32]) -> Vec<u8> {
    let mut dimg = DiskImage::from_bytes("src.do", dsk.to_vec()).unwrap();
    let mut wimg = DiskImage::from_bytes("m.woz", woz::create_empty_woz2()).unwrap();
    for &t in tracks {
        let mut buf = vec![0u8; 0x4000];
        let (n, _) = dimg.read_track(t as f32 * 2.0, &mut buf, true);
        wimg.write_track(t as f32 * 2.0, &buf[..n], n);
    }
    wimg.data().to_vec()
}

fn delay_sub() -> &'static str {
    // 5 cycles per Y step: dly with Y = n takes about 5n + 12 cycles
    "dly: dey\n bne dly\n rts\n"
}

fn programs() -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();

    // #303: Q6L returns variable values even if the drive is off.
    v.push((
        "latch-motor-off",
        format!(
            r#".org $0801
 lda $c08e,x
 ldy #8
a1: lda $c08c,x
 sta $0300,y
 dey
 bne a1
 lda $c088,x
 lda #0
 sta $06
 sta $07
 lda #3
 sta $08
d1: dec $06
 bne d1
 dec $07
 bne d1
 dec $08
 bne d1
 ldy #8
a2: lda $c08c,x
 sta $0310,y
 dey
 bne a2
 lda $c08c,x
 sta $0320
 lda $c08e,x
 sta $0321
 jmp *
"#
        ),
    ));

    // #672/#673: every even address of the controller returns the data latch.
    let mut even = String::from(".org $0801\n lda $c08e,x\n");
    for (i, a) in [0x80u16, 0x82, 0x84, 0x86, 0x8A, 0x8C, 0x8E, 0x8C, 0x80, 0x8C].iter().enumerate() {
        even.push_str(&format!(" lda $c0{a:02x},x\n sta $03{:02x}\n nop\n nop\n", i));
    }
    // and the odd ones return the floating bus (video scanner)
    for (i, a) in [0x81u16, 0x83, 0x85, 0x87, 0x8B, 0x8D, 0x8F].iter().enumerate() {
        even.push_str(&format!(" lda $c0{a:02x},x\n sta $03{:02x}\n", 0x20 + i));
    }
    even.push_str(" jmp *\n");
    v.push(("even-latch", even));

    // #389: write-protect sense ($C08D then $C08E: bit 7 = write protected).
    v.push((
        "wp-sense",
        r#".org $0801
 lda $c08d,x
 lda $c08e,x
 sta $0300
 lda $c08d,x
 lda $c08e,x
 sta $0301
 lda $c08c,x
 sta $0302
 lda $c08d,x
 lda $c08e,x
 sta $0303
 jmp *
"#
        .into(),
    ));

    // #582: partial nibble reads when two reads are very close (threshold 6 cycles).
    let mut pn = String::from(".org $0801\n lda $c08e,x\n");
    for k in 0..6 {
        pn.push_str(&format!("s{k}: lda $c08c,x\n bpl s{k}\n"));
        for _ in 0..k {
            pn.push_str(" nop\n");
        }
        pn.push_str(&format!(" lda $c08c,x\n sta $03{:02x}\n lda $c08c,x\n sta $03{:02x}\n", k * 2, k * 2 + 1));
    }
    pn.push_str(" jmp *\n");
    v.push(("partial-nibble", pn));

    // #821: head beyond the last track and back (ReadTrack past the image).
    v.push((
        "seek-beyond",
        format!(
            r#".org $0801
 lda $c08e,x
 lda #10
 sta $06
 ldy #20
 sty $07
l1:
 lda $c083,x
 ldy $07
 jsr dly
 lda $c080,x
 ldy $07
 jsr dly
 lda $c085,x
 ldy $07
 jsr dly
 lda $c082,x
 ldy $07
 jsr dly
 lda $c087,x
 ldy $07
 jsr dly
 lda $c084,x
 ldy $07
 jsr dly
 lda $c081,x
 ldy $07
 jsr dly
 lda $c086,x
 ldy $07
 jsr dly
 dec $06
 bne l1
 ldy #16
r1: lda $c08c,x
 sta $0300,y
 ldy #3
 jsr dly
 ldy $08
 inc $08
 cpy #16
 bne r1
 lda #10
 sta $06
 lda #200
 sta $07
l2:
 lda $c081,x
 ldy $07
 jsr dly
 lda $c086,x
 ldy $07
 jsr dly
 lda $c087,x
 ldy $07
 jsr dly
 lda $c080,x
 ldy $07
 jsr dly
 lda $c085,x
 ldy $07
 jsr dly
 lda $c082,x
 ldy $07
 jsr dly
 lda $c083,x
 ldy $07
 jsr dly
 lda $c084,x
 ldy $07
 jsr dly
 dec $06
 bne l2
 lda $c08c,x
 sta $0320
 jmp *
{}"#,
            delay_sub()
        ),
    ));

    // #24/#202: rotation period (three D5 AA 96 address prologues in a row, repeatedly).
    v.push((
        "rotation",
        r#".org $0801
 lda $c08e,x
rot:
w1: lda $c08c,x
 bpl w1
 cmp #$d5
 bne w1
w2: lda $c08c,x
 bpl w2
 cmp #$aa
 bne w1
w3: lda $c08c,x
 bpl w3
 cmp #$96
 bne w1
 inc $06
 lda $06
 cmp #40
 bne rot
 jmp *
"#
        .into(),
    ));
    v
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    println!("{} ({} bytes)", p.display(), bytes.len());
    p
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: make_micro OUT_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let _ = (Class::Floppy, SLOT_X);
    for (name, src) in programs() {
        let dsk = dsk_with(&src);
        write_file(&out, &format!("{name}.do.gz"), &gz(&dsk));
        if matches!(name, "even-latch" | "partial-nibble" | "rotation") {
            let w = woz_from_dsk(&dsk, &[0, 1, 2, 3]);
            write_file(&out, &format!("{name}.woz"), &w);
        }
    }
}
