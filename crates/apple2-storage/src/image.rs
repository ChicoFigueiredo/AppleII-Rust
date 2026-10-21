//! Disk images: format detection, track and block access, write-back (AppleWin DiskImage.cpp
//! and DiskImageHelper.cpp). Formats: DO/DSK, PO, 6656-nibble NIB, 6384-nibble NB2, WOZ 1/2,
//! SimSystem IIe, APL/PRG program images (boot only), HDV, and the 2MG/2IMG and MacBinary
//! wrappers; `.gz` and `.zip` containers are unpacked on open and repacked on write.

use crate::nibble::*;
use crate::store::{FileStore, Store};
use crate::woz::{self, WozLayout};
use std::fmt;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::path::Path;

pub const HD_BLOCK_SIZE: usize = 512;
pub const UNIDISK35_800K_SIZE: usize = 800 * 1024;
pub const HARDDISK_32M_SIZE: usize = HD_BLOCK_SIZE * 65536;
pub const TRACKS_STANDARD: u32 = 35;
/// Allow up to a 40-track `.dsk` image (160KB), DiskImage.h:42-44.
pub const TRACKS_MAX: u32 = 40;
const HDR_2IMG_SIZE: usize = 64;
const MACBINARY_HDR_SIZE: usize = 128;

/// `eImageType` (DiskImageHelper.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    /// DOS-order 140K (`.do`, `.dsk`).
    Do,
    /// ProDOS-order 140K (`.po`).
    Po,
    /// Nibblized, 6656 bytes per track.
    Nib1,
    /// Nibblized, 6384 bytes per track (`.nb2`).
    Nib2,
    Hdv,
    /// SimSystem IIe (`.iie`).
    Iie,
    /// Raw program (`.apl`), boot only.
    Apl,
    /// Program with an "!LG" header (`.prg`), boot only.
    Prg,
    Woz1,
    Woz2,
}

impl ImageKind {
    pub fn is_woz(self) -> bool {
        matches!(self, ImageKind::Woz1 | ImageKind::Woz2)
    }

    /// `GetRejectExtensions()`: extensions that exclude this type from detection.
    fn reject(self) -> &'static str {
        match self {
            ImageKind::Do => ".nib;.iie;.po;.prg",
            ImageKind::Po => ".do;.iie;.nib;.prg;.woz",
            ImageKind::Nib1 => ".do;.iie;.po;.prg;.woz",
            ImageKind::Nib2 => ".do;.iie;.po;.prg;.woz;.2mg;.2img",
            ImageKind::Hdv => ".do;.iie;.prg",
            ImageKind::Iie => ".do.;.nib;.po;.prg;.woz;.2mg;.2img",
            ImageKind::Apl | ImageKind::Prg => ".do;.dsk;.iie;.nib;.po;.woz;.2mg;.2img",
            ImageKind::Woz1 | ImageKind::Woz2 => ".do;.dsk;.nib;.iie;.po;.prg",
        }
    }

    /// Read/write supported (everything but APL and PRG).
    fn allow_rw(self) -> bool {
        !matches!(self, ImageKind::Apl | ImageKind::Prg)
    }
}

/// `ImageError_e`.
#[derive(Debug)]
pub enum ImageError {
    BadSize,
    BadFile,
    /// Format not recognized.
    Unsupported,
    /// A UniDisk 3.5"/IIgs/hard-disk image offered to a floppy drive.
    UnsupportedHdv,
    Gz,
    Zip,
    /// Zero-length write-protected file: cannot be formatted.
    ZeroLengthWriteProtected,
    Io(io::Error),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::BadSize => write!(f, "the disk image is an unsupported size"),
            ImageError::BadFile => write!(f, "the OS can't access the file"),
            ImageError::Unsupported => write!(f, "the disk image format is not recognized"),
            ImageError::UnsupportedHdv => write!(
                f,
                "this UniDisk 3.5/Apple IIGS/hard-disk image is not supported in a floppy drive"
            ),
            ImageError::Gz | ImageError::Zip => {
                write!(f, "the compressed disk image is corrupt/unsupported")
            }
            ImageError::ZeroLengthWriteProtected => {
                write!(f, "unsupported zero-length write-protected file")
            }
            ImageError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// How the image sits in its file.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Container {
    Plain,
    Gz,
    Zip { name: String },
}

/// Kind of drive an image is opened for (`bExpectFloppy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Floppy,
    HardDisk,
}

impl Class {
    fn max_image_size(self) -> usize {
        match self {
            // Pathologic largest 5.25 image: a WOZ with FLUX tracks for each 0.25 track.
            Class::Floppy => 100 * 35 * 4 * 512,
            Class::HardDisk => HARDDISK_32M_SIZE + HDR_2IMG_SIZE,
        }
    }
}

/// Lower-cased extension with its dot, the way `GetCharLowerExt` finds it.
fn lower_ext(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rfind('.')
        .map_or(base, |i| &base[i..])
        .to_ascii_lowercase()
}

/// `GetImageTitle`: (image name without extension, full name).
pub fn image_title(path: &str) -> (String, String) {
    let base: String = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .chars()
        .take(127)
        .collect();
    let full = if !base.chars().any(char::is_lowercase) && base.chars().count() > 2 {
        let mut it = base.chars();
        it.next().map(String::from).unwrap_or_default() + &it.as_str().to_lowercase()
    } else {
        base
    };
    let name = match full.rfind('.') {
        Some(i) if i > 0 => full[..i].to_string(),
        _ => full.clone(),
    };
    (name, full)
}

/// Result of detection: where the disk data sits and what it is.
struct Detected {
    kind: ImageKind,
    offset: usize,
    size: usize,
    num_tracks: u32,
    volume: u8,
    locked: bool,
    sim_order: [u8; 16],
    woz: Option<WozLayout>,
}

fn u16le(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32le(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

#[derive(PartialEq, Eq)]
enum Det {
    Mismatch,
    Possible,
    Match,
}

/// `CImageBase::IsValidImageSize` (:586): the track count of a 140KB-ish image, if valid.
fn valid_floppy_size(size: usize) -> Option<u32> {
    const T: usize = TRACK_DENIBBLIZED_SIZE;
    if size > TRACKS_MAX as usize * T {
        return None;
    }
    let ok = if size >= (TRACKS_STANDARD as usize + 1) * T {
        (size - TRACKS_STANDARD as usize * T).is_multiple_of(T)
    } else {
        (143105..=143364).contains(&size) || size == 143403 || size == 143488
    };
    ok.then_some((size / T) as u32)
}

/// Sector-link checks of `CDoImage::Detect` / `CPoImage::Detect` (:656, :724). `image` is at
/// least 143105 bytes, so every offset below is in range.
fn detect_order(image: &[u8], prodos_order: bool) -> Det {
    let w = |at: usize| u16le(image, at);
    if !prodos_order {
        if (1..=15usize).all(|l| image[0x11002 + (l << 8)] as usize == l - 1) {
            return Det::Match;
        }
        if (2..=5usize).all(|l| {
            w((l << 9) + 0x100) as usize == if l == 5 { 0 } else { 6 - l }
                && w((l << 9) + 0x102) as usize == if l == 2 { 0 } else { 8 - l }
        }) {
            return Det::Match;
        }
    } else {
        if (5..=13usize).all(|l| image[0x11002 + (l << 8)] as usize == 14 - l) {
            return Det::Match;
        }
        if (2..=5usize).all(|l| {
            w(l << 9) as usize == if l == 2 { 0 } else { l - 1 }
                && w((l << 9) + 2) as usize == if l == 5 { 0 } else { l + 1 }
        }) {
            return Det::Match;
        }
    }
    Det::Possible
}

/// Header probing shared by both classes.
struct Wrap {
    offset: usize,
    size: usize,
    /// 2MG header was recognised (and accepted).
    two_img: bool,
    volume: u8,
    locked: bool,
    /// 2MG image format: 0 DOS 3.3, 1 ProDOS, 2 NIB.
    format: u32,
}

/// `CMacBinaryHelper::DetectHdr` (:1419).
fn skip_macbinary(image: &[u8], w: &mut Wrap) {
    if w.size > MACBINARY_HDR_SIZE
        && image[w.offset] == 0
        && image[w.offset + 1] < 120
        && image[w.offset + image[w.offset + 1] as usize + 2] == 0
        && image[w.offset + 0x7A] == 0x81
        && image[w.offset + 0x7B] == 0x81
    {
        w.offset += MACBINARY_HDR_SIZE;
        w.size -= MACBINARY_HDR_SIZE;
    }
}

/// `C2IMGHelper::DetectHdr` (:1440). Like AppleWin it advances past the header even when a later
/// check rejects the image.
fn detect_2img(image: &[u8], w: &mut Wrap, floppy: bool) {
    let h = &image[w.offset..];
    if w.size < HDR_2IMG_SIZE || &h[0..4] != b"2IMG" || u16le(h, 8) as usize != HDR_2IMG_SIZE {
        return;
    }
    if u16le(h, 10) > 1 {
        return; // GH#317: some writers store 0 for the version
    }
    let data_len = u32le(h, 28) as usize;
    if w.size < HDR_2IMG_SIZE + data_len {
        return;
    }
    let (format, flags, blocks) = (u32le(h, 12), u32le(h, 16), u32le(h, 20) as usize);
    w.offset += HDR_2IMG_SIZE;
    w.size = data_len;
    let ok = match format {
        0 => data_len >= TRACKS_STANDARD as usize * TRACK_DENIBBLIZED_SIZE,
        1 => {
            w.size = if data_len != 0 {
                data_len
            } else {
                blocks * HD_BLOCK_SIZE
            };
            !floppy
                || (w.size >= TRACKS_STANDARD as usize * TRACK_DENIBBLIZED_SIZE
                    && w.size <= TRACKS_MAX as usize * TRACK_DENIBBLIZED_SIZE)
        }
        2 => data_len == TRACKS_STANDARD as usize * NIBBLES_PER_TRACK_NIB,
        _ => false,
    };
    if ok {
        w.two_img = true;
        w.format = format;
        w.locked = flags & 0x8000_0000 != 0;
        w.volume = if format == 0 && flags & 0x100 != 0 {
            flags as u8
        } else {
            DEFAULT_VOLUME_NUMBER
        };
    }
}

fn detect_floppy(image: &[u8], ext: &str) -> Option<Detected> {
    let mut w = Wrap {
        offset: 0,
        size: image.len(),
        two_img: false,
        volume: DEFAULT_VOLUME_NUMBER,
        locked: false,
        format: 3,
    };
    skip_macbinary(image, &mut w);
    detect_2img(image, &mut w, true);
    let body = &image[w.offset.min(image.len())..];
    let body = &body[..w.size.min(body.len())];

    let order = [
        ImageKind::Woz1,
        ImageKind::Woz2,
        ImageKind::Do,
        ImageKind::Po,
        ImageKind::Nib1,
        ImageKind::Nib2,
        ImageKind::Hdv,
        ImageKind::Iie,
        ImageKind::Apl,
        ImageKind::Prg,
    ];
    let mut found: Option<(ImageKind, u32)> = None;
    let mut possible: Option<(ImageKind, u32)> = None;

    let mut kind_from_2img = None;
    if w.two_img {
        let k = match w.format {
            0 => Some(ImageKind::Do),
            1 => Some(ImageKind::Po),
            _ => None,
        };
        if let Some(k) = k {
            if let Some(n) = valid_floppy_size(w.size) {
                kind_from_2img = Some((k, n));
            }
        }
    }
    if let Some(k) = kind_from_2img {
        found = Some(k);
    } else {
        for kind in order {
            if !ext.is_empty() && kind.reject().contains(ext) {
                continue;
            }
            let (res, tracks) = detect_kind(kind, body, ext);
            match res {
                Det::Match => {
                    found = Some((kind, tracks));
                    break;
                }
                Det::Possible if possible.is_none() => possible = Some((kind, tracks)),
                _ => {}
            }
        }
    }
    let (kind, mut num_tracks) = found.or(possible)?;
    let mut d = Detected {
        kind,
        offset: w.offset,
        size: w.size,
        num_tracks,
        volume: DEFAULT_VOLUME_NUMBER,
        locked: false,
        sim_order: [0; 16],
        woz: None,
    };
    if kind.is_woz() {
        // CRC is advisory (a mismatch asks the user in AppleWin); trust the content.
        d.woz = Some(woz::process_chunks(image)?);
        d.num_tracks = woz::MAX_TRACKS_5_25;
        d.offset = d.woz.map_or(0, |l| l.trks);
    } else {
        if kind.allow_rw() && num_tracks == 0 {
            num_tracks = if w.size > 0 { TRACKS_STANDARD } else { 0 };
            d.num_tracks = num_tracks;
        }
        if w.two_img {
            d.volume = w.volume;
            d.locked = w.locked;
        }
        if kind == ImageKind::Iie {
            // The sector order of the header, converted (CIIeImage::ConvertSectorOrder, :993).
            let src = &image[14..30];
            for l in 0..16 {
                d.sim_order[l] = (0..16).rev().find(|&i| src[i] as usize == l).unwrap_or(0) as u8;
            }
        }
    }
    Some(d)
}

/// `Detect()` of one image type. Returns the verdict and the number of tracks it sets.
fn detect_kind(kind: ImageKind, image: &[u8], ext: &str) -> (Det, u32) {
    let size = image.len();
    match kind {
        ImageKind::Woz1 | ImageKind::Woz2 => {
            let id: &[u8; 4] = if kind == ImageKind::Woz1 {
                woz::ID1_WOZ1
            } else {
                woz::ID1_WOZ2
            };
            if size >= 12 && &image[0..4] == id && image[4..8] == woz::ID2 {
                (Det::Match, woz::MAX_TRACKS_5_25)
            } else {
                (Det::Mismatch, 0)
            }
        }
        ImageKind::Do | ImageKind::Po => match valid_floppy_size(size) {
            Some(n) => (detect_order(image, kind == ImageKind::Po), n),
            None => (Det::Mismatch, 0),
        },
        ImageKind::Nib1 => {
            const T: usize = NIBBLES_PER_TRACK_NIB;
            if size < T * TRACKS_STANDARD as usize
                || !size.is_multiple_of(T)
                || size > T * TRACKS_MAX as usize
            {
                (Det::Mismatch, 0)
            } else {
                (Det::Match, (size / T) as u32)
            }
        }
        ImageKind::Nib2 => {
            if size == 6384 * TRACKS_STANDARD as usize {
                (Det::Match, TRACKS_STANDARD)
            } else {
                (Det::Mismatch, 0)
            }
        }
        ImageKind::Hdv => {
            let tracks = (size / TRACK_DENIBBLIZED_SIZE) as u32;
            if ext == ".hdv" || size >= UNIDISK35_800K_SIZE {
                (Det::Match, tracks)
            } else {
                (Det::Mismatch, tracks)
            }
        }
        ImageKind::Iie => {
            if size > 14 && &image[0..13] == b"SIMSYSTEM_IIE" && image[13] <= 3 {
                (Det::Match, TRACKS_STANDARD)
            } else {
                (Det::Mismatch, 0)
            }
        }
        ImageKind::Apl => {
            if size < 4 {
                return (Det::Mismatch, 0);
            }
            let len = u16le(image, 2) as usize;
            let ok = len + 4 == size || len + 4 + ((256 - ((len + 4) & 255)) & 255) == size;
            (if ok { Det::Possible } else { Det::Mismatch }, 0)
        }
        ImageKind::Prg => (
            if size >= 4 && image[0..4] == [0x0A, 0x47, 0x4C, 0x21] {
                Det::Match
            } else {
                Det::Mismatch
            },
            0,
        ),
    }
}

/// Detection for a hard disk: HDV only (`CHardDiskImageHelper::Detect`, :2207).
fn detect_hard_disk(image: &[u8], ext: &str) -> Option<Detected> {
    let mut w = Wrap {
        offset: 0,
        size: image.len(),
        two_img: false,
        volume: DEFAULT_VOLUME_NUMBER,
        locked: false,
        format: 3,
    };
    detect_2img(image, &mut w, false);
    let body = &image[w.offset.min(image.len())..];
    let body = &body[..w.size.min(body.len())];
    if ImageKind::Hdv.reject().contains(ext) && !ext.is_empty() {
        return None;
    }
    let (res, tracks) = detect_kind(ImageKind::Hdv, body, ext);
    (res == Det::Match).then_some(Detected {
        kind: ImageKind::Hdv,
        offset: w.offset,
        size: w.size,
        num_tracks: tracks,
        volume: DEFAULT_VOLUME_NUMBER,
        locked: w.two_img && w.locked,
        sim_order: [0; 16],
        woz: None,
    })
}

/// An opened disk image, entirely in memory, with optional write-back to a [`Store`].
pub struct DiskImage {
    kind: ImageKind,
    /// The file content (after unpacking a `.gz`/`.zip`), headers included.
    data: Vec<u8>,
    /// Where the disk data starts inside `data` (`uOffset`).
    offset: usize,
    /// Size of the disk data (`uImageSize`).
    image_size: usize,
    write_protected: bool,
    num_tracks: u32,
    volume: u8,
    container: Container,
    store: Option<Box<dyn Store>>,
    pathname: String,
    woz: Option<WozLayout>,
    max_nibbles_per_track: usize,
    sim_order: [u8; 16],
    num_valid_images_in_zip: u32,
    woz_empty_track: Vec<u8>,
    io_error: Option<String>,
}

/// Deterministic stand-in for AppleWin's seeded `rand()` (`CWOZImageHelper`, :1123): an empty WOZ
/// track is ~30% one bits.
fn woz_empty_track() -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    (0..woz::EMPTY_TRACK_SIZE)
        .map(|_| (0..8).fold(0u8, |n, j| if next() % 10 < 3 { n | 1 << j } else { n }))
        .collect()
}

impl DiskImage {
    fn from_detected(
        d: Detected,
        data: Vec<u8>,
        container: Container,
        pathname: &str,
        class: Class,
    ) -> Self {
        let woz = d.woz;
        DiskImage {
            kind: d.kind,
            max_nibbles_per_track: if class == Class::HardDisk {
                0
            } else {
                woz.map_or(NIBBLES_PER_TRACK, |l| l.max_nibbles_per_track)
            },
            write_protected: d.locked || woz.is_some_and(|l| l.write_protected),
            data,
            offset: d.offset,
            image_size: d.size,
            num_tracks: d.num_tracks,
            volume: d.volume,
            container,
            store: None,
            pathname: pathname.to_string(),
            woz,
            sim_order: d.sim_order,
            num_valid_images_in_zip: 0,
            woz_empty_track: if d.kind.is_woz() {
                woz_empty_track()
            } else {
                Vec::new()
            },
            io_error: None,
        }
    }

    /// Opens a floppy image from its file content. `name` supplies the extension (and `.gz`/`.zip`
    /// wrappers). An empty `bytes` creates a blank image of the type the extension names, like
    /// opening a zero-length file in AppleWin (`.do`/`.dsk`, `.nib`, `.woz`).
    pub fn from_bytes(name: &str, bytes: Vec<u8>) -> Result<DiskImage, ImageError> {
        Self::open_as(Class::Floppy, name, bytes)
    }

    /// Like [`DiskImage::from_bytes`] for a hard-disk (`.hdv`, `.2mg`, ...) image.
    pub fn hard_disk_from_bytes(name: &str, bytes: Vec<u8>) -> Result<DiskImage, ImageError> {
        Self::open_as(Class::HardDisk, name, bytes)
    }

    pub fn open_as(class: Class, name: &str, bytes: Vec<u8>) -> Result<DiskImage, ImageError> {
        let lower = name.to_ascii_lowercase();
        let image = if lower.len() > 3 && lower.ends_with(".gz") {
            Self::open_gz(class, name, &bytes)?
        } else if lower.len() > 4 && lower.ends_with(".zip") {
            Self::open_zip(class, name, &bytes)?
        } else if bytes.is_empty() {
            Self::blank(class, name)?
        } else {
            Self::open_plain(class, name, bytes)?
        };
        if class == Class::Floppy {
            if image.kind == ImageKind::Hdv {
                return Err(ImageError::UnsupportedHdv);
            }
            if image.num_tracks == 0 && !matches!(image.kind, ImageKind::Apl | ImageKind::Prg) {
                return Err(ImageError::Unsupported);
            }
        }
        Ok(image)
    }

    fn detect(class: Class, data: &[u8], ext: &str) -> Option<Detected> {
        match class {
            Class::Floppy => detect_floppy(data, ext),
            Class::HardDisk => detect_hard_disk(data, ext),
        }
    }

    fn open_plain(class: Class, name: &str, data: Vec<u8>) -> Result<Self, ImageError> {
        if data.len() > class.max_image_size() {
            return Err(ImageError::BadSize);
        }
        let d = Self::detect(class, &data, &lower_ext(name)).ok_or(ImageError::Unsupported)?;
        Ok(Self::from_detected(d, data, Container::Plain, name, class))
    }

    /// A new image for a zero-length file (`GetImageForCreation`, :2161): `.do`/`.dsk` 140K zeros,
    /// `.nib` 35 tracks of invalid-nibble fill, `.woz` an empty WOZ2.
    pub fn blank(class: Class, name: &str) -> Result<Self, ImageError> {
        let ext = lower_ext(name);
        let data = match (class, ext.as_str()) {
            (Class::Floppy, ".do" | ".dsk") => {
                vec![0u8; TRACKS_STANDARD as usize * TRACK_DENIBBLIZED_SIZE]
            }
            (Class::Floppy, ".nib") => (0..TRACKS_STANDARD as usize * NIBBLES_PER_TRACK_NIB)
                .map(|i| 0x80 | (i & 1) as u8)
                .collect(),
            (Class::Floppy, ".woz") => woz::create_empty_woz2(),
            _ => return Err(ImageError::Unsupported),
        };
        let d = Self::detect(class, &data, &ext).ok_or(ImageError::Unsupported)?;
        Ok(Self::from_detected(d, data, Container::Plain, name, class))
    }

    fn open_gz(class: Class, name: &str, bytes: &[u8]) -> Result<Self, ImageError> {
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take(class.max_image_size() as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| ImageError::Gz)?;
        if data.is_empty() || data.len() > class.max_image_size() {
            return Err(ImageError::BadSize);
        }
        let ext = lower_ext(&name[..name.len() - 3]);
        let d = Self::detect(class, &data, &ext).ok_or(ImageError::Unsupported)?;
        if matches!(d.kind, ImageKind::Apl | ImageKind::Iie | ImageKind::Prg) {
            return Err(ImageError::Unsupported);
        }
        Ok(Self::from_detected(d, data, Container::Gz, name, class))
    }

    fn open_zip(class: Class, name: &str, bytes: &[u8]) -> Result<Self, ImageError> {
        let mut archive =
            zip::ZipArchive::new(io::Cursor::new(bytes)).map_err(|_| ImageError::Zip)?;
        let entries = archive.len();
        let mut first: Option<(Detected, Vec<u8>, String)> = None;
        let mut valid = 0;
        for i in 0..entries {
            let mut f = archive.by_index(i).map_err(|_| ImageError::Zip)?;
            if f.size() as usize > class.max_image_size() {
                return Err(ImageError::BadSize);
            }
            if f.size() == 0 {
                continue; // directory or empty file
            }
            let entry = f.name().to_string();
            let mut data = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut data)
                .map_err(|_| ImageError::Unsupported)?;
            if let Some(d) = Self::detect(class, &data, &lower_ext(&entry)) {
                valid += 1;
                if first.is_none() {
                    first = Some((d, data, entry));
                }
            }
        }
        let (d, data, entry) = first.ok_or(ImageError::Unsupported)?;
        if matches!(d.kind, ImageKind::Apl | ImageKind::Iie | ImageKind::Prg) {
            return Err(ImageError::Unsupported);
        }
        let mut image = Self::from_detected(d, data, Container::Zip { name: entry }, name, class);
        if entries > 1 {
            image.write_protected = true; // multi-file zips are read-only
        }
        image.num_valid_images_in_zip = valid;
        Ok(image)
    }

    /// Reads and opens a file. Write-back goes to the file unless it is read-only (then the image
    /// is write-protected) or `force_write_protected`. A zero-length file is initialised as a
    /// blank image of the type its extension names.
    pub fn open_path(
        path: &Path,
        class: Class,
        force_write_protected: bool,
    ) -> Result<DiskImage, ImageError> {
        let name = path.to_string_lossy().to_string();
        let meta = std::fs::metadata(path).map_err(ImageError::Io)?;
        let readonly = force_write_protected || meta.permissions().readonly();
        let bytes = std::fs::read(path).map_err(ImageError::Io)?;
        let zero = bytes.is_empty();
        if zero && readonly {
            return Err(ImageError::ZeroLengthWriteProtected);
        }
        let mut image = Self::open_as(class, &name, bytes)?;
        image.write_protected |= readonly;
        if !image.write_protected {
            let mut store = FileStore::open(path).map_err(ImageError::Io)?;
            if zero {
                store.replace_all(&image.data).map_err(ImageError::Io)?; // resize to the full size (GH#506)
            }
            image.store = Some(Box::new(store));
        }
        Ok(image)
    }

    /// Attaches the destination for writes (replacing any previous one).
    pub fn with_store(mut self, store: Box<dyn Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn set_store(&mut self, store: Option<Box<dyn Store>>) {
        self.store = store;
    }

    pub fn kind(&self) -> ImageKind {
        self.kind
    }

    pub fn is_woz(&self) -> bool {
        self.kind.is_woz()
    }

    pub fn pathname(&self) -> &str {
        &self.pathname
    }

    pub fn is_write_protected(&self) -> bool {
        self.write_protected
    }

    pub fn set_write_protected(&mut self, wp: bool) {
        self.write_protected = wp;
    }

    pub fn num_tracks(&self) -> u32 {
        self.num_tracks
    }

    /// Size of the disk data, excluding 2MG/MacBinary headers (`ImageGetImageSize`).
    pub fn image_size(&self) -> usize {
        self.image_size
    }

    /// The file content (after unpacking), for saving or inspection.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn max_nibbles_per_track(&self) -> usize {
        self.max_nibbles_per_track
    }

    /// WOZ bit-cell time in 125ns units (32 = 4us); 32 for every other format.
    pub fn optimal_bit_timing(&self) -> u8 {
        self.woz
            .map_or(woz::OPTIMAL_BIT_TIMING_5_25, |l| l.optimal_bit_timing)
    }

    /// WOZ INFO says the boot sector is 13-sector only (`ImageIsBootSectorFormatSector13`).
    pub fn boot_sector_is_13(&self) -> bool {
        self.woz
            .is_some_and(|l| l.boot_sector_format == woz::BOOT_SECTOR_13)
    }

    /// APL/PRG "zero tracks" are valid (`ImageIsZeroTracksValidForThisType`).
    pub fn zero_tracks_valid(&self) -> bool {
        matches!(self.kind, ImageKind::Apl | ImageKind::Prg)
    }

    /// Number of valid images in a zip archive (more than one: AppleWin warns that only the first
    /// is used, and the image is write-protected).
    pub fn num_valid_images_in_zip(&self) -> u32 {
        self.num_valid_images_in_zip
    }

    pub fn name_in_zip(&self) -> Option<&str> {
        match &self.container {
            Container::Zip { name } => Some(name),
            _ => None,
        }
    }

    /// First write error since the last call (the emulation itself cannot report it).
    pub fn take_io_error(&mut self) -> Option<String> {
        self.io_error.take()
    }

    /// `PhaseToTrack`: round half tracks down.
    pub fn phase_to_track(phase: f32) -> u32 {
        (phase.ceil() as u32) >> 1
    }

    /// `ImagePhaseToTrack` (`limit`: clamp to the image's last track).
    pub fn phase_to_track_limited(&self, phase: f32, limit: bool) -> u32 {
        let t = Self::phase_to_track(phase);
        if limit {
            if self.num_tracks == 0 {
                0
            } else {
                t.min(self.num_tracks - 1)
            }
        } else {
            t
        }
    }

    /// Program to load for an APL or PRG image: (address, bytes) (`CAplImage::Boot`, `CPrgImage::Boot`).
    /// Booting write-protects the image.
    pub fn boot_program(&mut self) -> Option<(u16, Vec<u8>)> {
        let d = &self.data;
        let (addr, bytes) = match self.kind {
            ImageKind::Apl if d.len() >= 4 => {
                let (addr, len) = (u16le(d, 0), u16le(d, 2) as usize);
                (addr, d.get(4..4 + len)?.to_vec())
            }
            ImageKind::Prg if d.len() >= 128 => {
                let (addr, len) = (u16le(d, 5), (u16le(d, 7) as usize) << 1);
                (addr, d.get(128..128 + len)?.to_vec())
            }
            _ => return None,
        };
        let end = addr.wrapping_add(bytes.len() as u16);
        if end <= addr || addr >= 0xC000 || addr as usize + bytes.len() > 0xC000 {
            return None;
        }
        self.write_protected = true;
        Some((addr, bytes))
    }

    // ---- Track access (ImageReadTrack / ImageWriteTrack). ----

    /// Reads the track under `phase` (0..79, +-0.5 for WOZ quarter tracks) into `buf` (at least
    /// [`NIBBLES_PER_TRACK`] bytes, or `max_nibbles_per_track`). Returns (nibbles, bit count);
    /// the bit count is only meaningful for WOZ. `enhance` is AppleWin's enhanced disk speed:
    /// when off, 140K tracks are rotated by their physical skew.
    pub fn read_track(&mut self, phase: f32, buf: &mut [u8], enhance: bool) -> (usize, u32) {
        let phase = phase.max(0.0);
        let track = Self::phase_to_track(phase) as usize;
        match self.kind {
            ImageKind::Do | ImageKind::Po => {
                let order = if self.kind == ImageKind::Do {
                    &DOS_ORDER
                } else {
                    &PRODOS_ORDER
                };
                let start = self.offset + track * TRACK_DENIBBLIZED_SIZE;
                let Some(src) = self.data.get(start..start + TRACK_DENIBBLIZED_SIZE) else {
                    return (0, 0);
                };
                let n = nibblize_track(src, order, self.volume, track as u8, buf);
                if !enhance {
                    skew_track(track as u8, &mut buf[..n]);
                }
                (n, 0)
            }
            ImageKind::Nib1 | ImageKind::Nib2 => {
                let size = if self.kind == ImageKind::Nib1 {
                    NIBBLES_PER_TRACK_NIB
                } else {
                    6384
                };
                let start = self.offset + track * size;
                match self.data.get(start..start + size) {
                    Some(src) => {
                        buf[..size].copy_from_slice(src);
                        (size, 0)
                    }
                    None => (0, 0),
                }
            }
            ImageKind::Iie => self.read_track_iie(track, buf),
            ImageKind::Woz1 | ImageKind::Woz2 => self.read_track_woz(phase, buf),
            ImageKind::Apl | ImageKind::Prg | ImageKind::Hdv => {
                // Not readable: noise, as AppleWin returns for non-RW images.
                let n = self.max_nibbles_per_track.max(1).min(buf.len());
                let mut x: u32 = 0xC0FF_EE11;
                for b in buf[..n].iter_mut() {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    *b = x as u8;
                }
                (n, 0)
            }
        }
    }

    fn read_track_iie(&self, track: usize, buf: &mut [u8]) -> (usize, u32) {
        let d = &self.data;
        if d.len() < 88 {
            return (0, 0);
        }
        if d[13] <= 2 {
            let start = track * TRACK_DENIBBLIZED_SIZE + 30;
            let mut work = [0u8; TRACK_DENIBBLIZED_SIZE];
            let avail = d.len().saturating_sub(start).min(TRACK_DENIBBLIZED_SIZE);
            if start < d.len() {
                work[..avail].copy_from_slice(&d[start..start + avail]);
            }
            (
                nibblize_track(&work, &self.sim_order, self.volume, track as u8, buf),
                0,
            )
        } else {
            let n = u16le(d, track * 2 + 14) as usize;
            let off = 88
                + (0..track)
                    .map(|t| u16le(d, t * 2 + 14) as usize)
                    .sum::<usize>();
            buf[..n].fill(0);
            if let Some(src) = d.get(off..off + n) {
                buf[..n].copy_from_slice(src);
            }
            (n, 0)
        }
    }

    fn read_track_woz(&self, phase: f32, buf: &mut [u8]) -> (usize, u32) {
        let l = self.woz.expect("WOZ layout");
        let idx = woz::tmap_entry(&self.data, &l, (phase * 2.0) as usize);
        let empty = |buf: &mut [u8]| {
            buf[..woz::EMPTY_TRACK_SIZE].copy_from_slice(&self.woz_empty_track);
            (woz::EMPTY_TRACK_SIZE, woz::EMPTY_TRACK_SIZE as u32 * 8)
        };
        if idx == woz::TMAP_TRACK_EMPTY {
            return empty(buf);
        }
        if self.kind == ImageKind::Woz1 {
            let start = self.offset + idx as usize * woz::WOZ1_TRACK_SIZE;
            let Some(src) = self.data.get(start..start + woz::WOZ1_TRACK_SIZE) else {
                return empty(buf);
            };
            buf[..woz::WOZ1_TRACK_SIZE].copy_from_slice(src);
            let t = woz::WOZ1_TRK_OFFSET;
            (u16le(src, t) as usize, u16le(src, t + 2) as u32) // bytesUsed, bitCount
        } else {
            let Some((start, _blocks, bits)) = woz::trk2(&self.data, &l, idx as usize) else {
                return empty(buf);
            };
            let nibbles = (bits as usize).div_ceil(8);
            let start = start as usize * woz::BLOCK_SIZE;
            if nibbles
                > self
                    .max_nibbles_per_track
                    .max(NIBBLES_PER_TRACK)
                    .min(buf.len())
            {
                return empty(buf); // more than the largest track the INFO chunk announced
            }
            match self.data.get(start..start + nibbles) {
                Some(src) => {
                    buf[..nibbles].copy_from_slice(src);
                    (nibbles, bits)
                }
                None => empty(buf),
            }
        }
    }

    /// Writes back the track under `phase` from its `nibbles` nibbles in `buf`. Does nothing if
    /// the image is write-protected or read-only.
    pub fn write_track(&mut self, phase: f32, buf: &[u8], nibbles: usize) {
        if self.write_protected || !self.kind.allow_rw() {
            return;
        }
        let phase = phase.max(0.0);
        let track = Self::phase_to_track(phase) as usize;
        match self.kind {
            ImageKind::Do | ImageKind::Po => {
                let order = if self.kind == ImageKind::Do {
                    &DOS_ORDER
                } else {
                    &PRODOS_ORDER
                };
                let work = denibblize_track(&buf[..nibbles], order);
                let start = self.offset + track * TRACK_DENIBBLIZED_SIZE;
                self.write_data(start, &work);
            }
            ImageKind::Nib1 | ImageKind::Nib2 => {
                let size = if self.kind == ImageKind::Nib1 {
                    NIBBLES_PER_TRACK_NIB
                } else {
                    6384
                };
                if nibbles == size {
                    let start = self.offset + track * size;
                    self.write_data(start, &buf[..size]);
                }
            }
            ImageKind::Woz1 => self.write_track_woz1(phase, buf, nibbles),
            ImageKind::Woz2 => self.write_track_woz2(phase, buf, nibbles),
            ImageKind::Iie | ImageKind::Hdv | ImageKind::Apl | ImageKind::Prg => {}
        }
    }

    /// Copies `bytes` into the image at absolute `start` and persists them.
    fn write_data(&mut self, start: usize, bytes: &[u8]) -> bool {
        let Some(dst) = self.data.get_mut(start..start + bytes.len()) else {
            return false;
        };
        dst.copy_from_slice(bytes);
        self.persist(start..start + bytes.len());
        true
    }

    /// Sends `range` of `data` to the store (the whole container for gzip/zip).
    fn persist(&mut self, range: Range<usize>) {
        let res = match (&self.container, self.store.as_mut()) {
            (_, None) => return,
            (Container::Plain, Some(s)) => s.write_at(range.start as u64, &self.data[range]),
            (Container::Gz, Some(s)) => {
                let mut enc =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(&self.data)
                    .and_then(|_| enc.finish())
                    .and_then(|z| s.replace_all(&z))
            }
            (Container::Zip { name }, Some(s)) => {
                let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
                let opts = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                w.start_file(name.as_str(), opts)
                    .map_err(io::Error::other)
                    .and_then(|_| w.write_all(&self.data))
                    .and_then(|_| w.finish().map_err(io::Error::other))
                    .and_then(|c| s.replace_all(&c.into_inner()))
            }
        };
        if let Err(e) = res {
            self.io_error = Some(e.to_string());
        }
    }

    // ---- WOZ writes (:1195, :1326). ----

    fn tmap_assign(&mut self, l: &WozLayout, quarter: usize, index: u8) {
        let t = &mut self.data[l.tmap..l.tmap + woz::MAX_QUARTER_TRACKS_5_25];
        t[quarter] = index;
        if quarter >= 1 {
            t[quarter - 1] = index; // WOZ spec: a track is also visible from the neighbouring quarter tracks
        }
        if quarter + 1 < woz::MAX_QUARTER_TRACKS_5_25 {
            t[quarter + 1] = index;
        }
    }

    fn next_free_index(&self, l: &WozLayout) -> u8 {
        self.data[l.tmap..l.tmap + woz::MAX_QUARTER_TRACKS_5_25]
            .iter()
            .filter(|&&b| b != woz::TMAP_TRACK_EMPTY)
            .max()
            .map_or(0, |m| m + 1)
    }

    /// Grows the TRKS chunk by `len` zero bytes (inserted at its end, before any trailing chunk)
    /// and re-reads the layout. Returns the file offset of the new space.
    fn grow_trks(&mut self, l: &WozLayout, len: usize) -> Option<usize> {
        let at = l.trks + l.trks_size;
        let new_size = (l.trks_size + len) as u32;
        self.data.splice(at..at, std::iter::repeat_n(0u8, len));
        self.data[l.trks - 4..l.trks].copy_from_slice(&new_size.to_le_bytes());
        self.woz = woz::process_chunks(&self.data);
        self.woz.map(|_| at)
    }

    fn woz_finish(&mut self, whole_file: bool, track: Range<usize>) {
        woz::update_crc(&mut self.data);
        self.image_size = self.data.len();
        if whole_file {
            self.persist(0..self.data.len());
        } else {
            self.persist(track);
            self.persist(0..woz::HEADER_SIZE);
        }
        self.woz = woz::process_chunks(&self.data);
    }

    fn write_track_woz1(&mut self, phase: f32, buf: &[u8], nibbles: usize) {
        let Some(l) = self.woz else { return };
        if nibbles > woz::WOZ1_TRK_OFFSET || buf.len() < woz::WOZ1_TRACK_SIZE {
            return;
        }
        let quarter = ((phase * 2.0) as usize).min(woz::MAX_QUARTER_TRACKS_5_25 - 1);
        let mut idx = woz::tmap_entry(&self.data, &l, quarter);
        let mut block = buf[..woz::WOZ1_TRACK_SIZE].to_vec();
        let mut grew = false;
        let mut l = l;
        if idx == woz::TMAP_TRACK_EMPTY {
            idx = self.next_free_index(&l);
            self.tmap_assign(&l, quarter, idx);
            block[nibbles..].fill(0);
            let t = woz::WOZ1_TRK_OFFSET;
            block[t..t + 2].copy_from_slice(&(nibbles as u16).to_le_bytes());
            block[t + 2..t + 4].copy_from_slice(&((nibbles * 8) as u16).to_le_bytes());
            let Some(new_start) = self.grow_trks(&l, woz::WOZ1_TRACK_SIZE) else {
                return;
            };
            // The new track is appended after the existing ones: index == number of tracks.
            let expect = l.trks + idx as usize * woz::WOZ1_TRACK_SIZE;
            if new_start != expect {
                return; // tracks not stored in index order: leave the image alone
            }
            l = self.woz.expect("layout");
            grew = true;
        }
        let start = l.trks + idx as usize * woz::WOZ1_TRACK_SIZE;
        let Some(dst) = self.data.get_mut(start..start + woz::WOZ1_TRACK_SIZE) else {
            return;
        };
        dst.copy_from_slice(&block);
        self.woz_finish(grew, start..start + woz::WOZ1_TRACK_SIZE);
    }

    fn write_track_woz2(&mut self, phase: f32, buf: &[u8], nibbles: usize) {
        let Some(mut l) = self.woz else { return };
        let quarter = ((phase * 2.0) as usize).min(woz::MAX_QUARTER_TRACKS_5_25 - 1);
        let mut idx = woz::tmap_entry(&self.data, &l, quarter);
        let mut grew = false;
        if idx == woz::TMAP_TRACK_EMPTY {
            idx = self.next_free_index(&l);
            self.tmap_assign(&l, quarter, idx);
            let ext = (nibbles + woz::BLOCK_SIZE - 1) & !(woz::BLOCK_SIZE - 1);
            // Pad so that the new bits start on a block boundary.
            let at = l.trks + l.trks_size;
            let pad = (woz::BLOCK_SIZE - at % woz::BLOCK_SIZE) % woz::BLOCK_SIZE;
            let Some(new_start) = self.grow_trks(&l, pad + ext) else {
                return;
            };
            let start_block = (new_start + pad) / woz::BLOCK_SIZE;
            l = self.woz.expect("layout");
            let rec = l.trks + idx as usize * 8;
            self.data[rec..rec + 2].copy_from_slice(&(start_block as u16).to_le_bytes());
            self.data[rec + 2..rec + 4]
                .copy_from_slice(&((ext / woz::BLOCK_SIZE) as u16).to_le_bytes());
            self.data[rec + 4..rec + 8].copy_from_slice(&((nibbles * 8) as u32).to_le_bytes());
            grew = true;
        }
        let Some((start_block, _, bits)) = woz::trk2(&self.data, &l, idx as usize) else {
            return;
        };
        if (bits as usize).div_ceil(8) != nibbles {
            return; // track size changed: WOZ writes keep the bit count
        }
        let start = start_block as usize * woz::BLOCK_SIZE;
        let Some(dst) = self.data.get_mut(start..start + nibbles) else {
            return;
        };
        dst.copy_from_slice(&buf[..nibbles]);
        self.woz_finish(grew, start..start + nibbles);
    }

    // ---- Block access for hard disks (ImageReadBlock / ImageWriteBlock). ----

    /// Reads 512-byte block `block`; false past the end of the image.
    pub fn read_block(&self, block: u32, out: &mut [u8; HD_BLOCK_SIZE]) -> bool {
        let start = self.offset + block as usize * HD_BLOCK_SIZE;
        match self.data.get(start..start + HD_BLOCK_SIZE) {
            Some(src) if start + HD_BLOCK_SIZE <= self.offset + self.image_size => {
                out.copy_from_slice(src);
                true
            }
            _ => false,
        }
    }

    /// Writes block `block`, growing the image if it is past the end. False if write-protected.
    pub fn write_block(&mut self, block: u32, src: &[u8; HD_BLOCK_SIZE]) -> bool {
        if self.write_protected {
            return false;
        }
        let start = self.offset + block as usize * HD_BLOCK_SIZE;
        let end = start + HD_BLOCK_SIZE;
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        if end > self.offset + self.image_size {
            self.image_size = end - self.offset;
        }
        self.data[start..end].copy_from_slice(src);
        self.persist(start..end);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_follows_get_image_title() {
        assert_eq!(
            image_title("C:\\disks\\DOS33.DSK"),
            ("Dos33".to_string(), "Dos33.dsk".to_string())
        );
        assert_eq!(
            image_title("/a/b/Game.dsk"),
            ("Game".to_string(), "Game.dsk".to_string())
        );
        assert_eq!(image_title("XY"), ("XY".to_string(), "XY".to_string()));
    }

    #[test]
    fn floppy_sizes_follow_is_valid_image_size() {
        for ok in [143360usize, 143105, 143364, 143403, 143488, 163840] {
            assert!(valid_floppy_size(ok).is_some(), "{ok}");
        }
        for bad in [143104, 143365, 143400, 147456 + 1, 4096 * 41] {
            assert!(valid_floppy_size(bad).is_none(), "{bad}");
        }
        assert_eq!(valid_floppy_size(143360), Some(35));
    }

    #[test]
    fn extension_is_the_last_dot_lowercased() {
        assert_eq!(lower_ext("a/b/X.DSK"), ".dsk");
        assert_eq!(lower_ext("x"), "x");
    }
}
