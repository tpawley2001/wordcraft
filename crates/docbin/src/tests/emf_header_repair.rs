//! An embedded object's `\x03EPRINT` EMF is saved as a picture, so its `EMR_HEADER` must be one
//! a strict EMF reader accepts ([MS-EMF] 2.3.4.2): `nHandles` covers every object index the
//! records use (index 0 is the metafile itself), `sReserved` is 0, and `nRecords` / `nBytes` match
//! the records up to `EMR_EOF`. Some writers swap `nHandles` and `sReserved` (0 and 2) and count
//! one record short; Word cannot open a .docx holding such a picture.

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

/// EMR_HEADER (108 B) with the given counts and handle fields.
fn header(n_bytes: u32, n_records: u32, n_handles: u16, reserved: u16) -> Vec<u8> {
    let mut h = rec(1, 108, 0);
    h[40..44].copy_from_slice(&0x464D_4520u32.to_le_bytes());
    h[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[48..52].copy_from_slice(&n_bytes.to_le_bytes());
    h[52..56].copy_from_slice(&n_records.to_le_bytes());
    h[56..58].copy_from_slice(&n_handles.to_le_bytes());
    h[58..60].copy_from_slice(&reserved.to_le_bytes());
    h
}

fn eof() -> Vec<u8> {
    let mut e = rec(14, 20, 0);
    e[12..16].copy_from_slice(&16u32.to_le_bytes()); // offPalEntries
    e[16..20].copy_from_slice(&20u32.to_le_bytes()); // nSizeLast
    e
}

/// Font at index 1, select it, pen at index 3, select a stock object, delete the font, EOF.
fn body() -> Vec<u8> {
    [rec(82, 332, 1), rec(37, 12, 1), rec(38, 28, 3), rec(37, 12, 0x8000_0007), rec(40, 12, 1), eof()].concat()
}

fn emf(n_bytes: u32, n_records: u32, n_handles: u16, reserved: u16) -> Vec<u8> {
    [header(n_bytes, n_records, n_handles, reserved), body()].concat()
}

fn fields(b: &[u8]) -> (u32, u32, u16, u16) {
    (
        u32::from_le_bytes(b[52..56].try_into().unwrap()),
        u32::from_le_bytes(b[48..52].try_into().unwrap()),
        u16::from_le_bytes(b[56..58].try_into().unwrap()),
        u16::from_le_bytes(b[58..60].try_into().unwrap()),
    )
}

#[test]
fn swapped_handle_count_and_short_record_count_are_repaired() {
    let mut b = emf(520, 6, 0, 2);
    assert_eq!(b.len(), 524);
    let before = b.clone();
    repair_emf_header(&mut b);
    assert_eq!(fields(&b), (7, 524, 4, 0));
    // Nothing outside the four header fields changes.
    assert_eq!(b[..48], before[..48]);
    assert_eq!(b[60..], before[60..]);
}

#[test]
fn a_correct_header_is_left_byte_identical_and_handles_never_shrink() {
    let mut b = emf(524, 7, 10, 0);
    let before = b.clone();
    repair_emf_header(&mut b);
    assert_eq!(b, before);
}

#[test]
fn stock_objects_alone_need_one_handle() {
    let mut b = [header(0, 0, 0, 0), rec(37, 12, 0x8000_0007), eof()].concat();
    repair_emf_header(&mut b);
    assert_eq!(fields(&b), (3, 140, 1, 0));
}

#[test]
fn malformed_record_chains_are_left_unchanged() {
    // A record whose size is 6 (less than 8, not a multiple of 4).
    let mut bad_size = [header(0, 0, 0, 2), rec(37, 12, 1), eof()].concat();
    bad_size[112..116].copy_from_slice(&6u32.to_le_bytes());
    // The last record runs past the end of the buffer.
    let mut overrun = emf(520, 6, 0, 2);
    let n = overrun.len();
    overrun[n - 16..n - 12].copy_from_slice(&64u32.to_le_bytes());
    // No EMR_EOF at all.
    let no_eof = [header(0, 0, 0, 2), rec(82, 332, 1)].concat();
    for (name, b) in [("size 6", bad_size), ("overrun", overrun), ("no eof", no_eof)] {
        let mut got = b.clone();
        repair_emf_header(&mut got);
        assert_eq!(got, b, "{name}");
    }
}
