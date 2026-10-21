//! Auditoria wave0-core, area 5 (HU-0301/0302/0303): imagens malformadas, imagens enormes,
//! contêineres gz/zip hostis. Nada aqui pode dar panic nem alocar sem limite.
//!
//! Oraculo: DiskImageHelper.cpp (C++, SHA a8f0d1d) e GH#1402 (WOZ com chunk TRKS zerado).

use apple2_storage::image::{Class, DiskImage, ImageError};
use apple2_storage::woz;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn find_chunk(d: &[u8], id: &[u8; 4]) -> usize {
    let mut pos = 12;
    while pos + 8 <= d.len() {
        if &d[pos..pos + 4] == id {
            return pos;
        }
        let size = u32::from_le_bytes([d[pos + 4], d[pos + 5], d[pos + 6], d[pos + 7]]) as usize;
        pos += 8 + size;
    }
    panic!("chunk {id:?} nao encontrado");
}

/// Usa a imagem como o emulador: le varias trilhas e grava uma. Devolve true se deu panic.
fn exercise_panics(name: &str, bytes: Vec<u8>) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        if let Ok(mut img) = DiskImage::from_bytes(name, bytes) {
            let mut buf = vec![0u8; 0x4000];
            for q in 0..160 {
                let _ = img.read_track(q as f32 / 4.0, &mut buf, true);
            }
            img.write_track(0.0, &[0xD5, 0xAA, 0x96], 3);
            img.write_track(17.0, &[0xFF; 64], 64);
            let _ = img.boot_program();
        }
    }))
    .is_err()
}

#[test]
fn woz_with_zeroed_trks_chunk_id_is_refused_gh1402() {
    // GH#1402: um WOZ "em branco" com o id do chunk TRKS = 0,0,0,0 (CRC valido) passou a ser
    // recusado com mensagem. Aqui: process_chunks exige os tres chunks, entao tem de dar erro.
    let mut d = woz::create_empty_woz2();
    let at = find_chunk(&d, b"TRKS");
    d[at..at + 4].fill(0);
    woz::update_crc(&mut d);
    let r = DiskImage::from_bytes("blank.woz", d);
    assert!(r.is_err(), "WOZ sem chunk TRKS deveria ser recusado");
}

#[test]
fn woz_missing_each_required_chunk_is_refused() {
    for id in [b"INFO", b"TMAP", b"TRKS"] {
        let mut d = woz::create_empty_woz2();
        let at = find_chunk(&d, id);
        d[at..at + 4].copy_from_slice(b"JUNK");
        woz::update_crc(&mut d);
        assert!(
            DiskImage::from_bytes("x.woz", d).is_err(),
            "sem {}",
            String::from_utf8_lossy(id)
        );
    }
}

#[test]
fn malformed_woz_never_panics_on_open_read_or_write() {
    let good = woz::create_empty_woz2();
    let mut panics = Vec::new();
    // truncamentos
    for cut in (0..good.len()).step_by(7) {
        if exercise_panics("t.woz", good[..cut].to_vec()) {
            panics.push(format!("truncado em {cut}"));
        }
    }
    // cada byte do cabecalho, INFO e inicio do TMAP/TRKS com valores extremos
    for at in 0..400.min(good.len()) {
        for v in [0x00u8, 0x01, 0x7F, 0xFF] {
            let mut d = good.clone();
            d[at] = v;
            if at < 256 {
                woz::update_crc(&mut d);
            }
            if exercise_panics("m.woz", d) {
                panics.push(format!("byte {at} = {v:#x}"));
            }
        }
    }
    assert!(
        panics.is_empty(),
        "{} panics, ex.: {:?}",
        panics.len(),
        &panics[..panics.len().min(8)]
    );
}

#[test]
fn woz_track_records_pointing_outside_the_file_never_panic() {
    let mut d = woz::create_empty_woz2();
    let trks = find_chunk(&d, b"TRKS") + 8;
    let tmap = find_chunk(&d, b"TMAP") + 8;
    d[tmap] = 0; // quarter track 0 -> TRKS record 0
    d[trks..trks + 2].copy_from_slice(&0xFFF0u16.to_le_bytes()); // start block
    d[trks + 2..trks + 4].copy_from_slice(&0xFFFFu16.to_le_bytes()); // block count
    d[trks + 4..trks + 8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // bit count
    woz::update_crc(&mut d);
    assert!(!exercise_panics("far.woz", d));
}

#[test]
fn dsk_nib_po_do_with_odd_sizes_never_panic() {
    let mut panics = Vec::new();
    for ext in ["dsk", "do", "po", "nib", "2mg", "hdv", "apl", "prg", "iie"] {
        for size in [
            1usize,
            2,
            255,
            256,
            511,
            512,
            4095,
            143359,
            143360,
            143361,
            232960,
            232961,
            819200,
            1 << 20,
        ] {
            let name = format!("x.{ext}");
            if exercise_panics(&name, vec![0xA5; size]) {
                panics.push(format!("{name} {size}"));
            }
        }
    }
    assert!(panics.is_empty(), "panics: {panics:?}");
}

// ---- tamanho / conteineres hostis ----

#[test]
fn oversized_plain_floppy_is_refused() {
    let r = DiskImage::from_bytes("big.dsk", vec![0; 64 << 20]);
    assert!(matches!(r, Err(ImageError::BadSize)));
}

#[test]
fn gzip_bomb_is_bounded() {
    // 256 MiB de zeros comprimem para ~250 KB. O leitor limita a max_image_size()+1.
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let chunk = vec![0u8; 1 << 20];
    for _ in 0..256 {
        enc.write_all(&chunk).unwrap();
    }
    let gz = enc.finish().unwrap();
    assert!(gz.len() < 2 << 20);
    let r = DiskImage::from_bytes("bomb.dsk.gz", gz.clone());
    assert!(matches!(r, Err(ImageError::BadSize)), "{:?}", r.err());
    let r = DiskImage::open_as(Class::HardDisk, "bomb.hdv.gz", gz);
    assert!(matches!(r, Err(ImageError::BadSize)), "{:?}", r.err());
}

fn zip_with(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn zip_bomb_with_honest_header_is_refused() {
    let z = zip_with(&[("a.dsk", vec![0u8; 128 << 20])]);
    assert!(z.len() < 1 << 20);
    let r = DiskImage::from_bytes("bomb.zip", z);
    assert!(matches!(r, Err(ImageError::BadSize)), "{:?}", r.err());
}

#[test]
fn zip_whose_header_lies_about_the_size_stays_bounded() {
    // Cabecalho local e central dizem 143360 bytes, mas o fluxo deflate expande a 128 MiB.
    let mut z = zip_with(&[("a.dsk", vec![0u8; 128 << 20])]);
    let lie = 143_360u32.to_le_bytes();
    // local file header: uncompressed size em +22; central directory: +24
    let local = z.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    z[local + 22..local + 26].copy_from_slice(&lie);
    let central = z.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    z[central + 24..central + 28].copy_from_slice(&lie);
    let res = catch_unwind(AssertUnwindSafe(|| DiskImage::from_bytes("lie.zip", z)));
    // Aceitar Ok ou Err, mas sem panic; o teste termina rapido e sem OOM se o leitor limita.
    assert!(res.is_ok(), "panic ao abrir zip com tamanho falso");
}

#[test]
fn zip_entry_names_with_traversal_are_never_used_as_paths() {
    // O nome do membro so e guardado para regravar o zip; nada e extraido para o disco.
    let dsk = vec![0u8; 143_360];
    let z = zip_with(&[("../../../tmp/evil.dsk", dsk)]);
    let img = DiskImage::from_bytes("t.zip", z).expect("abre");
    assert_eq!(img.name_in_zip(), Some("../../../tmp/evil.dsk"));
    assert!(!std::path::Path::new("/tmp/evil.dsk").exists());
}
