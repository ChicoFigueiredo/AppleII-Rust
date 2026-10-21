//! Creating and formatting disk images (AppleWin ProDOS_Utils.cpp and ProDOS_FileSystem.h): the
//! "New disk" options (blank with AppleWin's boot sector, DOS 3.3 with the OS, ProDOS with the
//! bootstrap and optional BITSY.BOOT / QUIT.SYSTEM / BASIC.SYSTEM / PRODOS files) and the
//! "Format" actions. Everything works on byte vectors; saving them is the caller's business.
//! The OS files in `firmware/` are AppleWin's `firmware/OS` and `firmware/BootSector`.

const BOOT_SECTOR: &[u8] = include_bytes!("../firmware/bootsector.bin");
const DOS33_OS: &[u8] = include_bytes!("../firmware/dos33c.bin");
const PRODOS_BOOT: &[u8] = include_bytes!("../firmware/bootsector_prodos243.bin");
const PRODOS_243: &[u8] = include_bytes!("../firmware/prodos243.bin");
const BASIC_SYSTEM: &[u8] = include_bytes!("../firmware/basic17.system.bin");
const BITSY_BOOT: &[u8] = include_bytes!("../firmware/bitsy.boot.bin");
const QUIT_SYSTEM: &[u8] = include_bytes!("../firmware/quit.system.bin");

const TRACK_SIZE: usize = 16 * 256;
const TRACKS_MAX: usize = 40;
const HARDDISK_32M_SIZE: usize = 512 * 65536;
const DEFAULT_VOLUME_NUMBER: u8 = 254;

const BLOCK: usize = 512;
const ROOT_BLOCK: usize = 2;
const MAX_BLOCKS: usize = 0xFFFF;
const MAX_FILENAME: usize = 15;

const ACCESS_D: u8 = 0x80;
const ACCESS_N: u8 = 0x40;
const ACCESS_B: u8 = 0x20;
const ACCESS_W: u8 = 0x02;
const ACCESS_R: u8 = 0x01;

const KIND_DEL: u8 = 0x0;
const KIND_SEED: u8 = 0x1;
const KIND_SAPL: u8 = 0x2;
const KIND_ROOT: u8 = 0xF;

/// Sector order of an image file; the ProDOS formatter works in ProDOS order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectorOrder {
    Dos33,
    ProDos,
}

impl SectorOrder {
    /// By extension: `.dsk`/`.do` DOS order, `.po`/`.hdv` ProDOS order, else unknown.
    pub fn from_extension(path: &str) -> Option<Self> {
        let ext = path.rfind('.').map(|i| path[i..].to_ascii_lowercase())?;
        match ext.as_str() {
            ".dsk" | ".do" => Some(SectorOrder::Dos33),
            ".po" | ".hdv" => Some(SectorOrder::ProDos),
            _ => None,
        }
    }
}

/// Which files `new_prodos_disk` copies onto the disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProDosFiles {
    pub bitsy_boot: bool,
    pub bitsy_bye: bool,
    pub basic: bool,
    pub prodos: bool,
}

/// Map physical <-> logical (`g_aInterleave_DSK`).
const INTERLEAVE: [usize; 16] = [
    0x0, 0xE, 0xD, 0xC, 0xB, 0xA, 0x9, 0x8, 0x7, 0x6, 0x5, 0x4, 0x3, 0x2, 0x1, 0xF,
];

/// Swizzles DOS-order sectors to ProDOS order in place (only for DOS-ordered images).
fn forward_interleave(disk: &mut [u8], order: SectorOrder) {
    if order != SectorOrder::Dos33 {
        return;
    }
    let src = disk.to_vec();
    for t in 0..disk.len() / TRACK_SIZE {
        for (s, &phys) in INTERLEAVE.iter().enumerate() {
            let (from, to) = (t * TRACK_SIZE + phys * 256, t * TRACK_SIZE + s * 256);
            disk[to..to + 256].copy_from_slice(&src[from..from + 256]);
        }
    }
}

/// The inverse of [`forward_interleave`].
fn reverse_interleave(disk: &mut [u8], order: SectorOrder) {
    if order != SectorOrder::Dos33 {
        return;
    }
    let src = disk.to_vec();
    for t in 0..disk.len() / TRACK_SIZE {
        for (s, &phys) in INTERLEAVE.iter().enumerate() {
            let (from, to) = (t * TRACK_SIZE + s * 256, t * TRACK_SIZE + phys * 256);
            disk[to..to + 256].copy_from_slice(&src[from..from + 256]);
        }
    }
}

fn get16(d: &[u8], at: usize) -> usize {
    d[at] as usize | (d[at + 1] as usize) << 8
}

fn put16(d: &mut [u8], at: usize, v: usize) {
    d[at] = v as u8;
    d[at + 1] = (v >> 8) as u8;
}

fn put24(d: &mut [u8], at: usize, v: usize) {
    put16(d, at, v);
    d[at + 2] = (v >> 16) as u8;
}

/// `ProDOS_PackDate`: year (two digits), month 1-12, day.
pub fn pack_date(year: u16, month: u16, day: u16) -> u16 {
    ((year & 0x7F) << 9) | ((month & 0x0F) << 5) | (day & 0x1F)
}

pub fn pack_time(hours: u16, minutes: u16) -> u16 {
    ((hours & 0x1F) << 8) | (minutes & 0x3F)
}

/// The parts of a ProDOS volume header the formatter needs (`ProDOS_VolumeHeader_t`).
#[derive(Default)]
struct Volume {
    kind: u8,
    name: Vec<u8>,
    access: u8,
    entry_len: u8,
    entry_num: u8,
    file_count: usize,
    bitmap_block: usize,
    total_blocks: usize,
}

impl Volume {
    /// `ProDOS_GetVolumeHeader`.
    fn read(d: &[u8], block: usize) -> Volume {
        let base = block * BLOCK + 4; // skip the prev/next directory links
        let len = (d[base] & 0xF) as usize;
        Volume {
            kind: d[base] >> 4,
            name: d[base + 1..base + 1 + len].to_vec(),
            access: d[base + 30],
            entry_len: d[base + 31],
            entry_num: d[base + 32],
            file_count: get16(d, base + 33),
            bitmap_block: get16(d, base + 35),
            total_blocks: get16(d, base + 37),
        }
    }

    /// `ProDOS_SetVolumeHeader`.
    fn write(&self, d: &mut [u8], block: usize) {
        let base = block * BLOCK + 4;
        d[base] = (self.kind << 4) | (self.name.len() as u8 & 0xF);
        d[base + 1..base + 1 + self.name.len()].copy_from_slice(&self.name);
        d[base + 30] = self.access;
        d[base + 31] = self.entry_len;
        d[base + 32] = self.entry_num;
        put16(d, base + 33, self.file_count);
        put16(d, base + 35, self.bitmap_block);
        put16(d, base + 37, self.total_blocks);
    }

    /// `ProDOS_BlockInitFree`: marks every block free; returns the number of bitmap blocks.
    fn init_free(&mut self, d: &mut [u8]) -> usize {
        let at = self.bitmap_block * BLOCK;
        let blocks = d.len().div_ceil(BLOCK);
        let size = blocks.div_ceil(8);
        d[at..at + size].fill(0xFF);
        self.total_blocks = blocks.min(MAX_BLOCKS); // GH#1469: 32-MB volumes
        size.div_ceil(BLOCK)
    }

    /// `ProDOS_BlockSetUsed`.
    fn set_used(&self, d: &mut [u8], block: usize) {
        d[self.bitmap_block * BLOCK + block / 8] &= !(0x80 >> (block % 8));
    }

    /// `ProDOS_BlockGetFirstFree`: 0 if the disk is full.
    fn first_free(&self, d: &[u8]) -> usize {
        let at = self.bitmap_block * BLOCK;
        let bytes = d.len().div_ceil(BLOCK).div_ceil(8);
        for byte in 0..bytes {
            let v = d[at + byte];
            if v != 0 {
                return byte * 8 + v.leading_zeros() as usize;
            }
        }
        0
    }
}

/// A file entry (`ProDOS_FileHeader_t`).
struct FileEntry {
    kind: u8,
    name: &'static str,
    file_type: u8,
    inode: usize,
    blocks: usize,
    size: usize,
    date: u16,
    time: u16,
    cur_ver: u8,
    min_ver: u8,
    access: u8,
    aux: u16,
    mod_date: u16,
    mod_time: u16,
    dir_block: usize,
}

impl FileEntry {
    /// `ProDOS_PutFileHeader`.
    fn write(&self, d: &mut [u8], at: usize) {
        d[at] = (self.kind << 4) | (self.name.len() as u8 & 0xF);
        d[at + 1..at + 1 + self.name.len()].copy_from_slice(self.name.as_bytes());
        d[at + 16] = self.file_type;
        put16(d, at + 17, self.inode);
        put16(d, at + 19, self.blocks);
        put24(d, at + 21, self.size);
        put16(d, at + 24, self.date as usize);
        put16(d, at + 26, self.time as usize);
        d[at + 28] = self.cur_ver;
        d[at + 29] = self.min_ver;
        d[at + 30] = self.access;
        put16(d, at + 31, self.aux as usize);
        put16(d, at + 33, self.mod_date as usize);
        put16(d, at + 35, self.mod_time as usize);
        put16(d, at + 37, self.dir_block);
    }
}

/// `ProDOS_DirGetFirstFreeEntryOffset`: 0 if the directory is full.
fn first_free_dir_entry(d: &[u8], vol: &Volume, mut base: usize) -> usize {
    let mut offset = base + 4;
    loop {
        let next = get16(d, base + 2);
        for _ in 0..vol.entry_num {
            if d[offset] >> 4 == KIND_DEL || d[offset] & 0xF == 0 {
                return offset;
            }
            offset += vol.entry_len as usize;
        }
        if next == 0 {
            return 0;
        }
        base = next * BLOCK;
        offset = base + 4;
    }
}

/// `Util_ProDOS_IsFileBlockSparse`.
fn is_block_sparse(offset: usize, data: &[u8]) -> bool {
    if offset >= data.len() {
        return false;
    }
    let end = if offset + BLOCK > data.len() {
        offset + data.len() % BLOCK
    } else {
        offset + BLOCK
    };
    data[offset..end].iter().all(|&b| b == 0)
}

/// `Util_ProDOS_FormatFileSystem`: an empty volume (boot blocks kept). `disk` is in ProDOS order.
fn format_prodos_fs(disk: &mut [u8], volume_name: &str) {
    disk[BLOCK * 2..].fill(0);
    let root_dir_blocks = 4;
    let mut vol = Volume {
        bitmap_block: ROOT_BLOCK + root_dir_blocks,
        ..Volume::default()
    };
    let bitmap_blocks = vol.init_free(disk);
    for b in 0..ROOT_BLOCK {
        vol.set_used(disk, b);
    }
    let mut prev = 0;
    for i in 0..root_dir_blocks {
        let next = vol.first_free(disk);
        vol.set_used(disk, next);
        // Doubly linked list: [0] = prev, [2] = next (set on the next allocation).
        put16(disk, next * BLOCK, prev);
        put16(disk, next * BLOCK + 2, 0);
        if i > 0 {
            put16(disk, prev * BLOCK + 2, next);
        }
        prev = next;
    }
    for _ in 0..bitmap_blocks {
        let b = vol.first_free(disk);
        vol.set_used(disk, b);
    }
    vol.entry_len = 0x27;
    vol.entry_num = (BLOCK / 0x27) as u8;
    vol.file_count = 0;
    vol.kind = KIND_ROOT;
    vol.name = volume_name
        .trim_start_matches('/')
        .bytes()
        .take(MAX_FILENAME)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    vol.access = ACCESS_D | ACCESS_N | ACCESS_B | ACCESS_W | ACCESS_R;
    vol.write(disk, ROOT_BLOCK);
}

/// `Util_ProDOS_AddFile`: copies `data` into the root directory. SEED and SAPL files only (up to
/// 128K); false if the disk or directory is full or the file is too big.
fn add_file(disk: &mut [u8], data: &[u8], mut meta: FileEntry, allow_sparse: bool) -> bool {
    let base = ROOT_BLOCK * BLOCK;
    let dir_block = ROOT_BLOCK;
    let mut vol = Volume::read(disk, dir_block);
    let free = first_free_dir_entry(disk, &vol, base);
    if free == 0 {
        return false;
    }
    let size = data.len();
    let blocks_data = size.div_ceil(BLOCK);
    let (kind, blocks_index) = if size <= BLOCK {
        (KIND_SEED, 0)
    } else if size > 256 * BLOCK {
        return false; // tree files: not implemented in AppleWin either
    } else {
        (KIND_SAPL, 1)
    };
    let mut blocks_total = blocks_index;
    let (mut node, mut index_base) = (0, 0);
    if blocks_index == 1 {
        let b = vol.first_free(disk);
        if b == 0 {
            return false;
        }
        node = b;
        index_base = b * BLOCK;
        vol.set_used(disk, b);
    }
    let slack = size % BLOCK;
    let last_size = if slack != 0 { slack } else { BLOCK };
    for i in 0..blocks_data {
        let block = vol.first_free(disk);
        if block == 0 {
            return false;
        }
        if i == 0 && kind == KIND_SEED {
            node = block;
        }
        let last = i == blocks_data - 1;
        let sparse = is_block_sparse(i * BLOCK, data);
        if sparse && allow_sparse {
            if kind == KIND_SAPL {
                disk[index_base + i] = 0;
                disk[index_base + i + 256] = 0;
            }
        } else {
            let n = if last { last_size } else { BLOCK };
            disk[block * BLOCK..block * BLOCK + n].copy_from_slice(&data[i * BLOCK..i * BLOCK + n]);
            blocks_total += 1;
            vol.set_used(disk, block);
            if kind == KIND_SAPL && index_base != 0 {
                disk[index_base + i] = block as u8;
                disk[index_base + i + 256] = (block >> 8) as u8;
            }
        }
    }
    meta.kind = kind;
    meta.inode = node;
    meta.blocks = blocks_total;
    meta.size = size;
    meta.dir_block = dir_block;
    meta.write(disk, free);
    vol.file_count += 1;
    vol.write(disk, dir_block);
    true
}

fn sys_file(
    name: &'static str,
    date: (u16, u16, u16),
    time: (u16, u16),
    mod_date: (u16, u16, u16),
    mod_time: (u16, u16),
    full_access: bool,
    versions: (u8, u8),
) -> FileEntry {
    FileEntry {
        kind: 0,
        name,
        file_type: 0xFF, // SYS
        inode: 0,
        blocks: 0,
        size: 0,
        date: pack_date(date.0, date.1, date.2),
        time: pack_time(time.0, time.1),
        cur_ver: versions.0,
        min_ver: versions.1,
        access: if full_access {
            ACCESS_D | ACCESS_N | ACCESS_B | ACCESS_W | ACCESS_R
        } else {
            ACCESS_B | ACCESS_R
        },
        aux: if full_access { 0 } else { 0x2000 },
        mod_date: pack_date(mod_date.0, mod_date.1, mod_date.2),
        mod_time: pack_time(mod_time.0, mod_time.1),
        dir_block: 0,
    }
}

/// A new ProDOS volume ("BLANK") in ProDOS order, `size` bytes (`New_DOSProDOS_Disk`, ProDOS
/// branch): bootstrap blocks, empty file system and the chosen files.
pub fn new_prodos_disk(size: usize, files: ProDosFiles) -> Vec<u8> {
    let mut disk = vec![0u8; size.max(BLOCK * 16)];
    let name = "BLANK";
    format_prodos_fs(&mut disk, name);
    disk[..PRODOS_BOOT.len()].copy_from_slice(PRODOS_BOOT);
    if files.bitsy_boot {
        add_file(
            &mut disk,
            BITSY_BOOT,
            sys_file(
                "BITSY.BOOT",
                (18, 1, 13),
                (9, 9),
                (16, 9, 15),
                (9, 49),
                false,
                (0x24, 0),
            ),
            false,
        );
    }
    if files.bitsy_bye {
        add_file(
            &mut disk,
            QUIT_SYSTEM,
            sys_file(
                "QUIT.SYSTEM",
                (18, 1, 13),
                (9, 9),
                (16, 9, 15),
                (9, 41),
                false,
                (0x24, 0),
            ),
            false,
        );
    }
    if files.basic {
        add_file(
            &mut disk,
            BASIC_SYSTEM,
            sys_file(
                "BASIC.SYSTEM",
                (18, 1, 13),
                (9, 9),
                (16, 8, 30),
                (7, 56),
                false,
                (0x24, 0),
            ),
            true,
        );
    }
    if files.prodos {
        add_file(
            &mut disk,
            PRODOS_243,
            sys_file(
                "PRODOS",
                (23, 12, 30),
                (2, 43),
                (23, 12, 30),
                (2, 43),
                true,
                (0x00, 0x80),
            ),
            true,
        );
    }
    disk
}

/// `Util_DOS33_SetTrackSectorUsage`: free-sector bitmap of `track` in the VTOC (1 = free).
fn dos33_set_track_usage(vtoc: &mut [u8], track: usize, sectors_free: u16) {
    let at = 0x38 + track * 4;
    vtoc[at] = (sectors_free >> 8) as u8;
    vtoc[at + 1] = sectors_free as u8;
    vtoc[at + 2] = 0;
    vtoc[at + 3] = 0;
}

/// `Util_DOS33_FormatFileSystem`: VTOC, empty catalog chain and free-sector maps (DOS order).
fn format_dos33_fs(disk: &mut [u8], vtoc_track: usize) {
    let tracks = disk.len() / TRACK_SIZE;
    let sector = |t: usize, s: usize| t * TRACK_SIZE + s * 256;
    // Catalog sectors link downwards: 15 -> 14 -> ... -> 1.
    for s in (2..=0xF).rev() {
        let at = sector(vtoc_track, s);
        disk[at + 1] = vtoc_track as u8;
        disk[at + 2] = s as u8 - 1;
    }
    let at = sector(vtoc_track, 1);
    disk[at + 1] = 0;
    disk[at + 2] = 0;
    let v = sector(vtoc_track, 0);
    disk[v + 0x1] = vtoc_track as u8; // CATALOG = T11
    disk[v + 0x2] = 0xF; // CATALOG = S0F
    disk[v + 0x3] = 0x3; // DOS 3.3
    disk[v + 0x6] = DEFAULT_VOLUME_NUMBER;
    disk[v + 0x27] = ((256usize - 12) / 2) as u8; // track/sector pairs per T/S list
    disk[v + 0x30] = vtoc_track as u8; // last track allocated
    disk[v + 0x31] = 1; // direction
    disk[v + 0x34] = tracks as u8;
    disk[v + 0x35] = 16; // sectors per track
    disk[v + 0x36] = 0x00;
    disk[v + 0x37] = 0x01; // 256 bytes per sector
    let vtoc = &mut disk[v..v + 256];
    for t in 0..tracks {
        // T00 can never be free (DOS 3.3 has a BNE instead of a BPL); nor the VTOC track.
        let free = if t == 0 || t == vtoc_track {
            0x0000
        } else {
            0xFFFF
        };
        dos33_set_track_usage(vtoc, t, free);
    }
}

/// A new DOS 3.3 disk in DOS order with the operating system on tracks 0-2 (`New_DOSProDOS_Disk`,
/// DOS 3.3 branch). `size` is 34-40 tracks of 4096 bytes (140K by default).
pub fn new_dos33_disk(size: usize) -> Vec<u8> {
    let mut disk = vec![0u8; size.clamp(34 * TRACK_SIZE, TRACKS_MAX * TRACK_SIZE)];
    let vtoc_track = 0x11;
    format_dos33_fs(&mut disk, vtoc_track);
    disk[..DOS33_OS.len()].copy_from_slice(DOS33_OS);
    // DOS 3.3 resides on tracks 0, 1 and 2; track 0 was reserved by the format.
    let v = vtoc_track * TRACK_SIZE;
    dos33_set_track_usage(&mut disk[v..v + 256], 1, 0);
    dos33_set_track_usage(&mut disk[v..v + 256], 2, 0);
    disk
}

/// A new blank data disk: `size` zero bytes with AppleWin's boot sector (it prints "THIS IS AN
/// EMPTY DATA DISK", or "HARD DISK") in the first sector (`New_Blank_Disk`).
pub fn new_blank_disk(size: usize, is_hard_disk: bool) -> Vec<u8> {
    let mut disk = vec![0u8; size.max(256)];
    let mut boot = BOOT_SECTOR.to_vec();
    if is_hard_disk {
        let text = boot[0xFF] as usize; // the last byte holds the offset of the message
        if text > 0 && text + 4 <= boot.len() {
            for (i, c) in b"HARD".iter().enumerate() {
                boot[text + i] = 0x80 | c;
            }
        }
    }
    disk[..boot.len()].copy_from_slice(&boot);
    disk
}

/// `Format_ProDOS_Disk`: writes an empty ProDOS file system ("BLANK") on an existing image of up
/// to 32MB. `order` is the file's sector order. Returns false if the image is too big.
pub fn format_prodos_disk(image: &mut [u8], order: SectorOrder) -> bool {
    if image.len() > HARDDISK_32M_SIZE || image.len() < BLOCK * 16 {
        return false;
    }
    forward_interleave(image, order);
    format_prodos_fs(image, "BLANK");
    reverse_interleave(image, order);
    true
}

/// `Format_DOS33_Disk`: writes an empty DOS 3.3 file system (VTOC and catalog) on an existing
/// 34-40 track image (DOS order). The boot tracks are untouched.
pub fn format_dos33_disk(image: &mut [u8]) -> bool {
    if !(34 * TRACK_SIZE..=TRACKS_MAX * TRACK_SIZE).contains(&image.len()) {
        return false;
    }
    format_dos33_fs(image, 0x11);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prodos_volume_has_a_root_directory_and_bitmap() {
        let d = new_prodos_disk(143360, ProDosFiles::default());
        assert_eq!(d.len(), 143360);
        let v = Volume::read(&d, 2);
        assert_eq!(
            (
                v.kind,
                &v.name[..],
                v.entry_len,
                v.entry_num,
                v.file_count,
                v.bitmap_block,
                v.total_blocks
            ),
            (KIND_ROOT, &b"BLANK"[..], 0x27, 13, 0, 6, 280)
        );
        // Blocks 0-6 are used, then the first free block is 7.
        assert_eq!(v.first_free(&d), 7);
        // Root directory chain 2 -> 3 -> 4 -> 5.
        assert_eq!(
            [
                get16(&d, 1024),
                get16(&d, 1026),
                get16(&d, 1536 + 2),
                get16(&d, 2560),
                get16(&d, 2560 + 2)
            ],
            [0, 3, 4, 4, 0]
        );
    }

    #[test]
    fn files_are_added_with_index_blocks_and_marked_used() {
        let all = ProDosFiles {
            bitsy_boot: true,
            bitsy_bye: true,
            basic: true,
            prodos: true,
        };
        let d = new_prodos_disk(143360, all);
        let v = Volume::read(&d, 2);
        assert_eq!(v.file_count, 4);
        // BITSY.BOOT is a seedling (1 block); BASIC.SYSTEM a sapling of 20 blocks + index.
        let e1 = 2 * 512 + 4 + 0x27;
        assert_eq!(d[e1] >> 4, KIND_SEED);
        assert_eq!(&d[e1 + 1..e1 + 11], b"BITSY.BOOT");
        assert_eq!(get16(&d, e1 + 19), 1);
        let e3 = 2 * 512 + 4 + 3 * 0x27;
        assert_eq!(d[e3] >> 4, KIND_SAPL);
        assert_eq!(&d[e3 + 1..e3 + 13], b"BASIC.SYSTEM");
        assert_eq!(get16(&d, e3 + 19), 21);
        // The index block lists the data blocks, which hold the file.
        let index = get16(&d, e3 + 17) * 512;
        let first = d[index] as usize | (d[index + 256] as usize) << 8;
        assert_eq!(&d[first * 512..first * 512 + 64], &BASIC_SYSTEM[..64]);
    }

    #[test]
    fn dos33_disk_has_the_vtoc_and_system_tracks() {
        let d = new_dos33_disk(143360);
        let v = 0x11 * 4096;
        assert_eq!(
            [
                d[v + 1],
                d[v + 2],
                d[v + 3],
                d[v + 6],
                d[v + 0x34],
                d[v + 0x35]
            ],
            [0x11, 0x0F, 3, 254, 35, 16]
        );
        assert_eq!(&d[..DOS33_OS.len()], DOS33_OS);
        let free = |t: usize| (d[v + 0x38 + t * 4] as u16) << 8 | d[v + 0x39 + t * 4] as u16;
        assert_eq!(
            [free(0), free(1), free(2), free(3), free(17), free(34)],
            [0, 0, 0, 0xFFFF, 0, 0xFFFF]
        );
    }

    #[test]
    fn blank_disks_carry_the_boot_sector_message() {
        let f = new_blank_disk(143360, false);
        let h = new_blank_disk(143360, true);
        assert_eq!(&f[..256], BOOT_SECTOR);
        assert_ne!(&h[..256], &f[..256]);
        assert!(f[256..].iter().all(|&b| b == 0));
    }

    #[test]
    fn dos_ordered_prodos_formatting_round_trips_the_interleave() {
        let mut image = vec![0xAAu8; 143360];
        assert!(format_prodos_disk(&mut image, SectorOrder::Dos33));
        let mut po = image.clone();
        forward_interleave(&mut po, SectorOrder::Dos33);
        assert_eq!(Volume::read(&po, 2).name, b"BLANK");
        assert!(!format_prodos_disk(
            &mut vec![0u8; HARDDISK_32M_SIZE + 512],
            SectorOrder::ProDos
        ));
        assert_eq!(
            SectorOrder::from_extension("A.DSK"),
            Some(SectorOrder::Dos33)
        );
        assert_eq!(
            SectorOrder::from_extension("a.hdv"),
            Some(SectorOrder::ProDos)
        );
        assert_eq!(SectorOrder::from_extension("a.bin"), None);
    }
}
