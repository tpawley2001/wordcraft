//! An embedded object's `\x03EPRINT` EMF is saved as a picture. Word cannot open a .docx whose
//! EMF has an `EMR_EXTCREATEFONTINDIRECTW` face name ([MS-EMF] 2.2.13 LogFont `FaceName`) of all
//! 32 UTF-16 units with no NUL, so the 32nd unit becomes NUL; nothing else changes.

use crate::ole::repair_emf_header;

fn rec(typ: u32, size: u32, first: u32) -> Vec<u8> {
    let mut r = vec![0u8; size as usize];
    r[0..4].copy_from_slice(&typ.to_le_bytes());
    r[4..8].copy_from_slice(&size.to_le_bytes());
    if size >= 12 {
        r[8..12].copy_from_slice(&first.to_le_bytes());
    }
    r
}

/// A correct EMR_HEADER (108 B) for a file of `n_bytes` and `n_records` with `n_handles`.
fn header(n_bytes: u32, n_records: u32, n_handles: u16) -> Vec<u8> {
    let mut h = rec(1, 108, 0);
    h[40..44].copy_from_slice(&0x464D_4520u32.to_le_bytes());
    h[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[48..52].copy_from_slice(&n_bytes.to_le_bytes());
    h[52..56].copy_from_slice(&n_records.to_le_bytes());
    h[56..58].copy_from_slice(&n_handles.to_le_bytes());
    h
}

fn eof() -> Vec<u8> {
    let mut e = rec(14, 20, 0);
    e[12..16].copy_from_slice(&16u32.to_le_bytes());
    e[16..20].copy_from_slice(&20u32.to_le_bytes());
    e
}

/// EMR_EXTCREATEFONTINDIRECTW of `size` bytes at handle `slot`, face name `face` (UTF-16, cut at 32 units).
fn font(slot: u32, size: u32, face: &str) -> Vec<u8> {
    let mut r = rec(82, size, slot);
    r[12..16].copy_from_slice(&(-16i32).to_le_bytes());
    r[28..32].copy_from_slice(&400i32.to_le_bytes());
    for (i, u) in face.encode_utf16().take(32).enumerate() {
        let at = 40 + i * 2;
        if at + 2 <= r.len() {
            r[at..at + 2].copy_from_slice(&u.to_le_bytes());
        }
    }
    r
}

/// A valid EMF: header, the given records, EOF, with correct header counts and handles.
fn emf(body: &[Vec<u8>]) -> Vec<u8> {
    let records: Vec<u8> = body.concat();
    let n_bytes = (108 + records.len() + 20) as u32;
    let n_records = (body.len() + 2) as u32;
    [header(n_bytes, n_records, (body.len() + 1) as u16), records, eof()].concat()
}

fn face(b: &[u8], record_at: usize) -> Vec<u16> {
    (0..32).map(|i| u16::from_le_bytes([b[record_at + 40 + i * 2], b[record_at + 41 + i * 2]])).collect()
}

const LONG: &str = "Liberation Serif;Times New Roman";

#[test]
fn a_32_unit_face_name_gets_a_nul_in_its_last_unit() {
    assert_eq!(LONG.encode_utf16().count(), 32);
    let mut b = emf(&[font(1, 332, LONG)]);
    let before = b.clone();
    repair_emf_header(&mut b);
    let want: Vec<u16> = LONG.encode_utf16().take(31).chain([0]).collect();
    assert_eq!(face(&b, 108), want);
    // Only the two bytes of the last face unit change.
    let at = 108 + 102;
    assert_eq!(b[..at], before[..at]);
    assert_eq!(b[at + 2..], before[at + 2..]);
}

#[test]
fn every_unterminated_face_name_is_terminated() {
    let mut b = emf(&[font(1, 332, LONG), font(2, 104, "Courier New;DejaVu Sans Mono;Ari")]);
    repair_emf_header(&mut b);
    assert_eq!(face(&b, 108)[31], 0);
    assert_eq!(face(&b, 108 + 332)[31], 0);
    assert_eq!(face(&b, 108 + 332)[30], u16::from(b'r'));
}

#[test]
fn terminated_names_and_short_records_are_left_byte_identical() {
    // `Arial` + NUL padding; a 31-unit name whose NUL is the 32nd unit; a record too short for a face name.
    let mut short = font(3, 92, "");
    short[40..92].fill(b'A');
    let b = emf(&[font(1, 332, "Arial"), font(2, 104, &LONG[..31]), short]);
    let mut got = b.clone();
    repair_emf_header(&mut got);
    assert_eq!(got, b);
}

#[test]
fn a_malformed_chain_is_left_unchanged() {
    // No EMR_EOF.
    let no_eof = [header(0, 0, 0), font(1, 332, LONG)].concat();
    // The font record claims to run past the end of the buffer.
    let mut overrun = emf(&[font(1, 332, LONG)]);
    overrun[108 + 4..108 + 8].copy_from_slice(&4000u32.to_le_bytes());
    for (name, b) in [("no eof", no_eof), ("overrun", overrun)] {
        let mut got = b.clone();
        repair_emf_header(&mut got);
        assert_eq!(got, b, "{name}");
    }
}
