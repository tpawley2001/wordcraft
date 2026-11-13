//! `Copts60.fNoTabForInd` in the DOP of a Word 97-2003 file ([MS-DOC] §2.7.2; bit 0 of the u16 at
//! DOP offset 84) is read into the document settings (serialized as `noTabHangInd`), the .doc
//! form of `w:noTabHangInd`.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
/// Where the test DOP starts in the Table stream (past the Clx).
const DOP_AT: u32 = 0x100;
/// Size of a Dop97 ([MS-DOC] §2.7).
const DOP97: usize = 500;

/// Whether a one-paragraph document whose Table stream holds `dop` opens with the flag set.
fn flag(dop: &[u8]) -> bool {
    let text = "Hello\r";
    let mut word = vec![0u8; super::TEXT_FC as usize];
    word.extend(text.bytes());
    let plc = plcpcd(&[Piece { compressed: true, text: text.into() }], super::TEXT_FC);
    let mut clx = vec![0x02u8];
    clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    clx.extend_from_slice(&plc);
    let mut table = vec![0u8; super::CLX_AT as usize];
    table.extend_from_slice(&clx);
    table.resize(DOP_AT as usize, 0);
    table.extend_from_slice(dop);
    let pairs = vec![(33, super::CLX_AT, clx.len() as u32), (DOP_PAIR, DOP_AT, dop.len() as u32)];
    let fib = fib_bytes(&FibSpec { ccp_text: text.chars().count() as u32, pairs, ..Default::default() });
    word[..fib.len()].copy_from_slice(&fib);
    let doc = read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens");
    assert_eq!(serde_json::to_value(&doc.settings).expect("settings serialize")["noTabHangInd"].as_bool().unwrap_or(false), doc.settings.no_tab_hang_ind);
    doc.settings.no_tab_hang_ind
}

/// A DOP of `len` bytes whose u16 at offset 84 (`Copts60`) is `copts60`.
fn dop(len: usize, copts60: u16) -> Vec<u8> {
    let mut d = vec![0u8; len];
    if len >= 86 {
        d[84..86].copy_from_slice(&copts60.to_le_bytes());
    }
    d
}

#[test]
fn the_flag_is_read_from_bit_0() {
    assert!(flag(&dop(DOP97, 0x0001)));
}

#[test]
fn the_flag_is_read_among_other_options() {
    assert!(flag(&dop(DOP97, 0x2009)));
}

#[test]
fn no_bit_no_flag() {
    assert!(!flag(&dop(DOP97, 0x0000)));
}

#[test]
fn the_other_bits_do_not_set_it() {
    assert!(!flag(&dop(DOP97, 0xFFFE)));
}

#[test]
fn a_dop_too_short_for_the_options_does_not_set_it() {
    let mut short = vec![0u8; 84];
    short[83] = 0x01;
    assert!(!flag(&short));
}
