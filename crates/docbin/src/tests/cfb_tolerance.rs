//! Compound-file defects Word tolerates ([MS-CFB] §2.3, §2.6.1): FAT entries for sectors past
//! the end of the file left as ENDOFCHAIN, and unused (type 0) directory entries holding junk.
//! Fixtures are the synthetic `.doc` from `text_doc`, patched byte by byte.

use std::borrow::Cow;

use super::{Piece, body_texts, read, text_doc};

const FREE: u32 = 0xFFFF_FFFF;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Sector size, sector count after the header, and the byte offset of sector `n`.
fn geometry(b: &[u8]) -> (usize, usize) {
    let sector_len = 1usize << u16_at(b, 0x1E);
    (sector_len, b.len().div_ceil(sector_len) - 1)
}
fn sector_at(b: &[u8], n: u32) -> usize {
    let (sector_len, _) = geometry(b);
    (n as usize + 1) * sector_len
}
/// Byte offset of FAT entry `i` (the fixture has a single FAT sector, listed first in the DIFAT).
fn fat_entry_at(b: &[u8], i: usize) -> usize {
    sector_at(b, u32_at(b, 0x4C)) + i * 4
}
fn fat_entries(b: &[u8]) -> usize {
    geometry(b).0 / 4
}
/// Byte offsets of the 128-byte entries of the first directory sector.
fn dir_entries(b: &[u8]) -> Vec<usize> {
    let start = sector_at(b, u32_at(b, 0x30));
    (0..geometry(b).0 / 128).map(|k| start + k * 128).collect()
}
fn entry_name(b: &[u8], at: usize) -> String {
    let len = (u16_at(b, at + 64) as usize).min(64);
    let units: Vec<u16> = b[at..at + len].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units).trim_end_matches('\0').to_string()
}

fn fixture() -> Vec<u8> {
    text_doc(&[Piece { compressed: false, text: "Alpha\rBeta\r".into() }], &[], 0)
}

#[test]
fn fat_past_eof_endofchain() {
    let mut f = fixture();
    let (_, sectors) = geometry(&f);
    assert!(sectors < fat_entries(&f), "fixture must have spare FAT entries");
    for i in sectors..fat_entries(&f) {
        let at = fat_entry_at(&f, i);
        assert_eq!(u32_at(&f, at), FREE);
        put_u32(&mut f, at, END_OF_CHAIN);
    }
    let doc = read(&f).expect("Word opens a file whose FAT marks missing sectors ENDOFCHAIN");
    assert_eq!(body_texts(&doc), ["Alpha", "Beta"]);
}

#[test]
fn unused_entry_garbage() {
    let mut f = fixture();
    let last = *dir_entries(&f).last().unwrap();
    assert_eq!(f[last + 66], 0, "the last directory entry of the fixture is unused");
    for (k, byte) in f[last..last + 64].iter_mut().enumerate() {
        *byte = (k as u8).wrapping_mul(37) | 1;
    }
    f[last + 64..last + 66].copy_from_slice(&14336u16.to_le_bytes());
    put_u32(&mut f, last + 116, 26112);
    put_u32(&mut f, last + 120, 29696);
    let doc = read(&f).expect("Word ignores unused directory entries");
    assert_eq!(body_texts(&doc), ["Alpha", "Beta"]);
}

#[test]
fn used_entry_bad_name() {
    let mut f = fixture();
    let word = dir_entries(&f).into_iter().find(|&at| entry_name(&f, at) == "WordDocument").expect("WordDocument entry");
    f[word + 64..word + 66].copy_from_slice(&14336u16.to_le_bytes());
    assert!(read(&f).is_err(), "a stream entry with a bad name length is not repaired");
}

#[test]
fn clean_file_borrowed() {
    let f = fixture();
    assert!(matches!(wordcraft_doc::cfb_repair::repair(&f), Cow::Borrowed(_)));
}

#[test]
fn repair_never_grows() {
    let mut f = fixture();
    let (_, sectors) = geometry(&f);
    for i in sectors..fat_entries(&f) {
        let at = fat_entry_at(&f, i);
        put_u32(&mut f, at, END_OF_CHAIN);
    }
    let fixed = wordcraft_doc::cfb_repair::repair(&f);
    assert!(matches!(fixed, Cow::Owned(_)));
    assert_eq!(fixed.len(), f.len());
}
