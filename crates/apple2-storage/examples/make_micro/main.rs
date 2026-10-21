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

/// Head movement of `tracks` tracks outward from track 0 (two half-steps per track), with `delay`
/// as the Y count of `dly` between phase changes.
fn seek_out(tracks: u32, delay: u32) -> String {
    let mut o = String::new();
    for k in 0..tracks {
        let c = (2 * k) % 4; // current phase
        let on = |p: u32| 0xC081 + 2 * (p % 4);
        let off = |p: u32| 0xC080 + 2 * (p % 4);
        for (addr, _) in [(on(c + 1), 0), (off(c), 0), (on(c + 2), 0), (off(c + 1), 0)] {
            o.push_str(&format!(" lda ${:04x},x\n ldy #{delay}\n jsr dly\n", addr));
        }
    }
    o
}

/// Writes a zero-data sector (`target`, physical) of the current track the way RWTS does: find
/// the address field, then sync, D5 AA AD, 343 nibbles, DE AA EB. `motor_off` ends with the
/// motor off (so the drive stops spinning and the track is flushed); `tracks` seeks first.
fn write_sector(target: u8, tracks: u32, motor_off: bool) -> String {
    write_sector_on(target, tracks, motor_off, "")
}

fn write_sector_on(target: u8, tracks: u32, motor_off: bool, prologue: &str) -> String {
    let seek = format!("{prologue}{}", seek_out(tracks, 100));
    format!(
        r#".org $0801
 lda #{target}
 sta $0a
{seek} lda $c08e,x
find:
 jsr rn
 cmp #$d5
 bne find
 jsr rn
 cmp #$aa
 bne find
 jsr rn
 cmp #$96
 bne find
 jsr rn
 jsr rn
 jsr rn
 jsr rn
 jsr rn
 sec
 rol
 sta $06
 jsr rn
 and $06
 cmp $0a
 bne find
 jsr rn
 jsr rn
 jsr rn
 jsr rn
 jsr rn
 lda $c08d,x
 lda $c08e,x
 bmi done
 lda #$ff
 sta $c08f,x
 cmp $c08c,x
 ldy #5
sy: lda #$ff
 jsr wb
 dey
 bne sy
 lda #$d5
 jsr wb
 lda #$aa
 jsr wb
 lda #$ad
 jsr wb
 ldy #0
d1: lda #$96
 jsr wb
 dey
 bne d1
 ldy #87
d2: lda #$96
 jsr wb
 dey
 bne d2
 lda #$de
 jsr wb
 lda #$aa
 jsr wb
 lda #$eb
 jsr wb
 lda #$ff
 jsr wb
 lda $c08e,x
 lda $c08c,x
done:
{}
 jmp *
rn: lda $c08c,x
 bpl rn
 rts
wb: sta $c08d,x
 cmp $c08c,x
 rts
{}"#,
        if motor_off { " lda $c088,x" } else { "" },
        delay_sub()
    )
}

/// `ops` are phase switches `(phase, on)`; after each one the program waits and reads 8 latch bytes
/// into $0300+ (quarter-track positions of a WOZ show up as different data).
fn quarter_steps(ops: &[(u32, bool)]) -> String {
    let mut o = String::from(" ldy #0\n sty $09\n");
    for (ph, on) in ops {
        let addr = 0xC080 + 2 * ph + u32::from(*on);
        o.push_str(&format!(
            " lda ${addr:04x},x\n ldy #60\n jsr dly\n jsr rd8\n"
        ));
    }
    o
}

/// Reads 64 latch bytes after seeking `tracks` out.
fn read_track_program(tracks: u32) -> String {
    format!(
        r#".org $0801
 lda $c08e,x
{} ldy #0
r: lda $c08c,x
 sta $0300,y
 nop
 nop
 nop
 iny
 cpy #64
 bne r
 jmp *
{}"#,
        seek_out(tracks, 100),
        delay_sub()
    )
}

fn programs() -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    v.push(("read-t1", read_track_program(1)));
    // #1110: half and quarter track positions (adjacent phases on together are a quarter track)
    v.push((
        "quarter",
        format!(
            r#".org $0801
 lda $c08e,x
{}
 jmp *
rd8: ldy #8
q: lda $c08c,x
 ldy $09
 sta $0300,y
 inc $09
 ldy #8
 nop
 nop
 dey
 bne q
 rts
{}"#,
            quarter_steps(&[
                (1, true),
                (0, false),
                (2, true),
                (1, false),
                (2, false),
                (3, true),
                (2, true),
                (3, false),
                (1, true),
                (2, false),
                (1, false),
                (0, true),
            ]),
            delay_sub()
        ),
    ));
    // #664: boot from drive 1, write to drive 2
    v.push((
        "write-sector-d2",
        write_sector_on(5, 0, true, "lda $c08b,x\n"),
    ));
    // #649: read the first nibbles of drive 2
    v.push((
        "read-d2",
        String::from(
            r#".org $0801
 lda $c08e,x
 lda $c08b,x
 ldy #0
r: lda $c08c,x
 sta $0300,y
 nop
 nop
 nop
 iny
 cpy #96
 bne r
 lda $c08a,x
 ldy #0
r2: lda $c08c,x
 sta $0400,y
 nop
 nop
 nop
 iny
 cpy #32
 bne r2
 jmp *
"#,
        ),
    ));
    v.push(("write-sector", write_sector(5, 0, true)));
    v.push(("write-sector-motor-on", write_sector(5, 0, false)));
    v.push(("write-sector-t1", write_sector(3, 1, true)));

    // #303: Q6L returns variable values even if the drive is off.
    v.push((
        "latch-motor-off",
        String::from(
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
"#,
        ),
    ));

    // #672/#673: every even address of the controller returns the data latch.
    let mut even = String::from(".org $0801\n lda $c08e,x\n");
    for (i, a) in [
        0x80u16, 0x82, 0x84, 0x86, 0x8A, 0x8C, 0x8E, 0x8C, 0x80, 0x8C,
    ]
    .iter()
    .enumerate()
    {
        even.push_str(&format!(
            " lda $c0{a:02x},x\n sta $03{:02x}\n nop\n nop\n",
            i
        ));
    }
    // and the odd ones return the floating bus (video scanner)
    for (i, a) in [0x81u16, 0x83, 0x85, 0x87, 0x8B, 0x8D, 0x8F]
        .iter()
        .enumerate()
    {
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
        pn.push_str(&format!(
            " lda $c08c,x\n sta $03{:02x}\n lda $c08c,x\n sta $03{:02x}\n",
            k * 2,
            k * 2 + 1
        ));
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

/// Boot programs of the hard disk (block 0 is loaded at $0800 by the slot 7 firmware, which jumps
/// to $0801; up to 512 bytes).
fn hdd_programs() -> Vec<(&'static str, String)> {
    let common = r#"
call: jmp ($00fa)
rb: stx $46
 sty $47
 lda #$00
 sta $44
 lda #$20
 sta $45
 lda #1
 sta $42
 jsr call
 php
 sta $0310
 lda $2000
 eor $21ff
 sta $0320
 plp
 rts
wb: stx $46
 sty $47
 lda #$00
 sta $44
 lda #$20
 sta $45
 lda #2
 sta $42
 jsr call
 php
 sta $0311
 plp
 rts
"#;
    let mut v = Vec::new();
    // #1165 timing, sequential and random block I/O, status/capacity
    v.push((
        "hdd-seq",
        format!(
            r#".org $0801
 lda $c7ff
 sta $fa
 lda #$c7
 sta $fb
 lda #$70
 sta $43
 lda #0
 sta $42
 jsr call
 stx $0300
 sty $0301
 ldx #1
sq: ldy #0
 jsr rb
 inx
 cpx #7
 bne sq
 ldy #0
f: tya
 sta $2000,y
 eor #$ff
 sta $2100,y
 iny
 bne f
 ldx #9
 ldy #0
 jsr wb
 ldx #12
 ldy #0
 jsr wb
 ldx #9
 ldy #0
 jsr rb
 ldx #12
 ldy #0
 jsr rb
 lda #$b7
 sta $06
 lda #24
 sta $08
rr: lda $06
 asl
 bcc nr
 eor #$1d
nr: sta $06
 and #$3f
 tax
 ldy #0
 jsr rb
 dec $08
 bne rr
 ldx #$ff
 ldy #$ff
 jsr rb
 ldx #$ff
 ldy #$ff
 jsr wb
 jmp *
{common}"#
        ),
    ));
    // #1007: DMA that touches I/O or ROM space, and the 64 KiB wrap
    let boundary_cases = [
        (0xBF, 0x00, "r"),
        (0xBE, 0x00, "r"),
        (0xC1, 0x00, "r"),
        (0xFF, 0x80, "r"),
        (0xFF, 0x80, "w"),
        (0xBF, 0x00, "w"),
        (0xFE, 0x00, "r"),
        (0xBF, 0xF0, "r"),
    ];
    let mut b = String::from(
        r#".org $0801
 lda $c7ff
 sta $fa
 lda #$c7
 sta $fb
 lda #$70
 sta $43
"#,
    );
    for (i, (hi, lo, op)) in boundary_cases.iter().enumerate() {
        let cmd = if *op == "r" { 1 } else { 2 };
        b.push_str(&format!(
            r#" lda #{lo}
 sta $44
 lda #{hi}
 sta $45
 lda #{cmd}
 sta $42
 lda #{blk}
 sta $46
 lda #0
 sta $47
 jsr call
 php
 sta $0300+{i}
 plp
 lda $2000
 eor $21ff
 sta $0310+{i}
"#,
            blk = 1 + (i % 3)
        ));
    }
    b.push_str(" jmp *\ncall: jmp ($00fa)\n");
    v.push(("hdd-boundary", b));
    // #319: SmartPort call at $08FD must return to $0903
    v.push((
        "hdd-smartport-page",
        r#".org $0801
 lda #$00
 sta $06
 jmp go
.fill 241,$ea
go:
 ldx #$11
 ldy #$22
 jsr $c70d
 .byte $00
 .word plist
 ldx #$55
 lda $06
 jmp *
plist: .byte 3,0,$00,$30,0
"#
        .into(),
    ));
    let _ = common;
    v
}

fn hdv_with(src: &str, blocks: usize) -> Vec<u8> {
    let (org, code) = asm::assemble(src).unwrap_or_else(|e| panic!("hdd assembly failed: {e}"));
    assert_eq!(org, 0x0801);
    let mut img = vec![0u8; blocks * 512];
    for b in 0..blocks {
        for i in 0..512 {
            img[b * 512 + i] = ((b * 37 + i * 5) as u8) | if i % 7 == 0 { 0x80 } else { 0 };
        }
    }
    assert!(
        code.len() < 512,
        "hdd boot block is {} bytes",
        code.len() + 1
    );
    img[..512].fill(0);
    img[0] = 1;
    img[1..1 + code.len()].copy_from_slice(&code);
    img
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    println!("{} ({} bytes)", p.display(), bytes.len());
    p
}

/// `2MG` wrapper: 64-byte header (format 0 DOS order, 1 ProDOS order) around a 140K image.
fn wrap_2mg(data: &[u8], format: u32) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[0..4].copy_from_slice(b"2IMG");
    h[4..8].copy_from_slice(b"XGS!");
    h[8..10].copy_from_slice(&64u16.to_le_bytes());
    h[10..12].copy_from_slice(&1u16.to_le_bytes());
    h[12..16].copy_from_slice(&format.to_le_bytes());
    h[20..24].copy_from_slice(&((data.len() / 512) as u32).to_le_bytes());
    h[24..28].copy_from_slice(&64u32.to_le_bytes());
    h[28..32].copy_from_slice(&(data.len() as u32).to_le_bytes());
    h.extend_from_slice(data);
    h
}

fn woz_set_largest(w: &mut [u8], blocks: u16) {
    // INFO v2: "largest track" (blocks) at INFO offset 44; the INFO payload starts at byte 20
    w[20 + 44..20 + 46].copy_from_slice(&blocks.to_le_bytes());
    woz::update_crc(w);
}

/// WOZ with the nibblized tracks of `dsk` at quarter-track positions `q` (0.25 track each).
fn woz_quarters(dsk: &[u8], quarters: &[u32]) -> Vec<u8> {
    let mut dimg = DiskImage::from_bytes("src.do", dsk.to_vec()).unwrap();
    let mut wimg = DiskImage::from_bytes("m.woz", woz::create_empty_woz2()).unwrap();
    for &q in quarters {
        // distinct data per quarter: the nibblized track of the whole-track below, rotated and
        // marked so that every position reads differently
        let mut buf = vec![0u8; 0x4000];
        let (n, _) = dimg.read_track((q / 4) as f32 * 2.0, &mut buf, true);
        let mut t = buf[..n].to_vec();
        t.rotate_left(q as usize * 37);
        for b in t.iter_mut().skip(100).step_by(64) {
            *b = 0x80 | (q as u8 * 5);
        }
        wimg.write_track(q as f32 / 2.0, &t, n);
    }
    wimg.data().to_vec()
}

fn extras(out: &Path) {
    let gz_write = |name: &str, data: &[u8]| {
        write_file(out, name, &gz(data));
    };
    // WOZ variants of the track-reading program
    let rt1 = dsk_with(&read_track_program(1));
    write_file(out, "read-t1.woz", &woz_from_dsk(&rt1, &[0, 1, 2]));
    // #745: a track bigger than the default 13 blocks; INFO says so (Largest Track)
    {
        let mut dimg = DiskImage::from_bytes("src.do", rt1.clone()).unwrap();
        let mut wimg = DiskImage::from_bytes("m.woz", woz::create_empty_woz2()).unwrap();
        let mut buf = vec![0u8; 0x4000];
        let (n, _) = dimg.read_track(0.0, &mut buf, true);
        wimg.write_track(0.0, &buf[..n], n);
        let (n1, _) = dimg.read_track(2.0, &mut buf, true);
        let mut big = buf[..n1].to_vec();
        big.resize(7200, 0xFF);
        wimg.write_track(2.0, &big, 7200);
        let mut w = wimg.data().to_vec();
        woz_set_largest(&mut w, 15);
        write_file(out, "read-t1-bigtrack.woz", &w);
    }
    // #695: a track with no flux transitions (MC3470 noise)
    {
        let mut dimg = DiskImage::from_bytes("src.do", rt1.clone()).unwrap();
        let mut wimg = DiskImage::from_bytes("m.woz", woz::create_empty_woz2()).unwrap();
        let mut buf = vec![0u8; 0x4000];
        let (n, _) = dimg.read_track(0.0, &mut buf, true);
        wimg.write_track(0.0, &buf[..n], n);
        wimg.write_track(2.0, &vec![0u8; 6656], 6656);
        write_file(out, "read-t1-noflux.woz", wimg.data());
    }
    // #1110: quarter tracks
    let q = dsk_with(
        &programs()
            .into_iter()
            .find(|(n, _)| *n == "quarter")
            .unwrap()
            .1,
    );
    write_file(
        out,
        "quarter.woz",
        &woz_quarters(&q, &(0..=16).collect::<Vec<_>>()),
    );
    // #664: WOZ in drive 1 (boot, selects drive 2) and a writable DSK in drive 2
    let d2 = dsk_with(
        &programs()
            .into_iter()
            .find(|(n, _)| *n == "write-sector-d2")
            .unwrap()
            .1,
    );
    write_file(
        out,
        "write-sector-d2.woz",
        &woz_from_dsk(&d2, &[0, 1, 2, 3]),
    );
    gz_write("patterned.do.gz", &patterned_dsk());
    // #649: .2mg wrapped images in both drives
    let rd2 = dsk_with(
        &programs()
            .into_iter()
            .find(|(n, _)| *n == "read-d2")
            .unwrap()
            .1,
    );
    gz_write("read-d2.do.2mg.gz", &wrap_2mg(&rd2, 0));
    gz_write("patterned.po.2mg.gz", &wrap_2mg(&patterned_dsk(), 1));
    // #186: ProDOS-order 2IMG (Microzine-like): ProDOS 2.4.3 in a .2mg with the ProDOS-order flag
    if let Ok(po) = std::fs::read("assets/disks/ProDOS_2_4_3.po") {
        gz_write("prodos243-po.2mg.gz", &wrap_2mg(&po, 1));
    }
    // #1402: WOZ whose TRKS chunk id is [0,0,0,0] (the file is otherwise valid, CRC fixed)
    {
        let mut w = woz_from_dsk(&rt1, &[0, 1]);
        let mut pos = 12;
        while pos + 8 <= w.len() {
            if &w[pos..pos + 4] == b"TRKS" {
                w[pos..pos + 4].fill(0);
                break;
            }
            let size =
                u32::from_le_bytes([w[pos + 4], w[pos + 5], w[pos + 6], w[pos + 7]]) as usize;
            pos += 8 + size;
        }
        woz::update_crc(&mut w);
        write_file(out, "bad-trks.woz", &w);
    }
    // #494: a zero-length .do (AppleWin creates a blank 140K image, GH#506)
    write_file(out, "zero.do", &[]);
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: make_micro OUT_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let _ = (Class::Floppy, SLOT_X);
    for (name, src) in programs() {
        let dsk = dsk_with(&src);
        if matches!(name, "quarter" | "read-d2" | "write-sector-d2") {
            continue;
        }
        if name == "write-sector" {
            // one image is stored raw (the plain-file write path); the others compressed
            write_file(&out, &format!("{name}.do"), &dsk);
        }
        write_file(&out, &format!("{name}.do.gz"), &gz(&dsk));
        if matches!(
            name,
            "even-latch" | "partial-nibble" | "rotation" | "write-sector"
        ) {
            let w = woz_from_dsk(&dsk, &[0, 1, 2, 3]);
            write_file(&out, &format!("{name}.woz"), &w);
        }
    }
    extras(&out);
    for (name, src) in hdd_programs() {
        let img = hdv_with(&src, 64);
        write_file(&out, &format!("{name}.hdv.gz"), &gz(&img));
    }
}
