//! A picture stored as an OfficeArt EMF BLIP ([MS-ODRAW] 2.2.24) gets the same repair as an
//! embedded object's EMF: header counts from the record chain and a NUL in any font face name
//! that fills all 32 UTF-16 units ([MS-EMF] 2.2.13). Word cannot open a .docx whose EMF picture
//! has an unterminated 32-unit face name. WMF BLIPs and malformed EMFs are left as stored.

use crate::officeart::{self, Decode, Rec};

const LONG: &str = "Liberation Serif;Times New Roman";

fn emr(typ: u32, size: u32, first: u32) -> Vec<u8> {
    let mut r = vec![0u8; size as usize];
    r[0..4].copy_from_slice(&typ.to_le_bytes());
    r[4..8].copy_from_slice(&size.to_le_bytes());
    r[8..12].copy_from_slice(&first.to_le_bytes());
    r
}

/// Header (108 B) + one 332-byte EMR_EXTCREATEFONTINDIRECTW with face `LONG` + EMR_EOF, with the
/// header's nRecords one short (2 instead of 3), as in the corpus file.
fn emf(with_eof: bool) -> Vec<u8> {
    let mut font = emr(82, 332, 1);
    for (i, u) in LONG.encode_utf16().enumerate() {
        font[40 + i * 2..42 + i * 2].copy_from_slice(&u.to_le_bytes());
    }
    let mut eof = emr(14, 20, 0);
    eof[12..16].copy_from_slice(&16u32.to_le_bytes());
    eof[16..20].copy_from_slice(&20u32.to_le_bytes());
    let mut h = emr(1, 108, 0);
    h[40..44].copy_from_slice(&0x464D_4520u32.to_le_bytes());
    h[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[48..52].copy_from_slice(&460u32.to_le_bytes());
    h[52..56].copy_from_slice(&2u32.to_le_bytes());
    h[56..58].copy_from_slice(&2u16.to_le_bytes());
    if with_eof { [h, font, eof].concat() } else { [h, font].concat() }
}

/// The OfficeArtMetafileHeader + stream body of a metafile BLIP (16-byte UID, even instance).
fn blip_body(cb_size: usize, stream: &[u8], compressed: bool) -> Vec<u8> {
    let mut hdr = vec![0u8; 34];
    hdr[0..4].copy_from_slice(&(cb_size as u32).to_le_bytes());
    hdr[28..32].copy_from_slice(&(stream.len() as u32).to_le_bytes());
    hdr[32] = if compressed { 0 } else { 0xFE };
    hdr[33] = 0xFE;
    [[0u8; 16].as_slice(), &hdr, stream].concat()
}

fn decode(typ: u16, inst: u16, body: &[u8]) -> Option<Vec<u8>> {
    let rec = Rec { inst, typ, body };
    let mut work = Decode::new(64 << 20);
    officeart::blip(&rec, 16 << 20, &mut work).map(|b| b.bytes)
}

fn face(b: &[u8]) -> Vec<u16> {
    (0..32).map(|i| u16::from_le_bytes([b[148 + i * 2], b[149 + i * 2]])).collect()
}

fn repaired() -> Vec<u8> {
    let mut want = emf(true);
    want[52..56].copy_from_slice(&3u32.to_le_bytes());
    want[108 + 102..108 + 104].fill(0);
    want
}

#[test]
fn an_uncompressed_emf_blip_is_repaired() {
    let raw = emf(true);
    let got = decode(0xF01A, 0x3D4, &blip_body(raw.len(), &raw, false)).expect("EMF BLIP decodes");
    assert_eq!(got, repaired());
    let want: Vec<u16> = LONG.encode_utf16().take(31).chain([0]).collect();
    assert_eq!(face(&got), want);
    assert_eq!(u32::from_le_bytes(got[52..56].try_into().unwrap()), 3, "nRecords counts header, font and EOF");
    assert_eq!(u32::from_le_bytes(got[48..52].try_into().unwrap()), got.len() as u32, "nBytes");
}

#[test]
fn a_compressed_emf_blip_is_repaired() {
    let raw = emf(true);
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    let got = decode(0xF01A, 0x3D4, &blip_body(raw.len(), &packed, true)).expect("EMF BLIP decodes");
    assert_eq!(got, repaired());
}

#[test]
fn a_wmf_blip_is_returned_as_stored() {
    let raw = emf(true);
    let got = decode(0xF01B, 0x216, &blip_body(raw.len(), &raw, false)).expect("WMF BLIP decodes");
    assert_eq!(got, raw);
}

#[test]
fn an_emf_blip_without_eof_is_returned_as_stored() {
    let raw = emf(false);
    let got = decode(0xF01A, 0x3D4, &blip_body(raw.len(), &raw, false)).expect("EMF BLIP decodes");
    assert_eq!(got, raw);
}
