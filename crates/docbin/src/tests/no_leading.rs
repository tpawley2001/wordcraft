//! `Copts80.fNoExtLeading` ([MS-DOC] §2.7.3: bit D, mask 0x0008, of the u16 at Dop offset 86) is
//! the compatibility option "Don't add leading (extra space) between rows of text"; it is read
//! into `Settings::no_leading`.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
/// Where the test DOP starts in the Table stream (past the Clx).
const DOP_AT: u32 = 0x100;
/// Sizes of the DOP versions ([MS-DOC] §2.7): Dop95, Dop2000, Dop2003.
const DOP95: usize = 88;
const DOP2000: usize = 544;
const DOP2003: usize = 616;

/// A one-paragraph document whose Table stream holds `dop` (if any) at `DOP_AT`.
fn doc_with_dop(dop: Option<&[u8]>) -> wordcraft_doc::Document {
    let text = "Hello\r";
    let mut word = vec![0u8; super::TEXT_FC as usize];
    word.extend(text.bytes());
    let plc = plcpcd(&[Piece { compressed: true, text: text.into() }], super::TEXT_FC);
    let mut clx = vec![0x02u8];
    clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    clx.extend_from_slice(&plc);
    let mut table = vec![0u8; super::CLX_AT as usize];
    table.extend_from_slice(&clx);
    let mut pairs = vec![(33, super::CLX_AT, clx.len() as u32)];
    if let Some(dop) = dop {
        table.resize(DOP_AT as usize, 0);
        table.extend_from_slice(dop);
        pairs.push((DOP_PAIR, DOP_AT, dop.len() as u32));
    }
    let fib = fib_bytes(&FibSpec { ccp_text: text.chars().count() as u32, pairs, ..Default::default() });
    word[..fib.len()].copy_from_slice(&fib);
    read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens")
}

#[test]
fn no_leading_flag_is_read() {
    for len in [DOP95, DOP2000, DOP2003] {
        let mut d = vec![0u8; len];
        d[86] |= 0x08;
        assert!(doc_with_dop(Some(&d)).settings.no_leading, "DOP of {len} bytes");
    }
}

/// The flags of a real Word for Mac file: Copts60 0xF580, Copts80's second word 0x843B.
#[test]
fn no_leading_flag_is_read_among_the_other_options() {
    let mut d = vec![0u8; DOP2003];
    d[84..88].copy_from_slice(&[0x80, 0xF5, 0x3B, 0x84]);
    let s = doc_with_dop(Some(&d)).settings;
    assert!(s.no_leading);
    assert!(!s.ext_leading_2pt_only);
}

#[test]
fn no_leading_needs_its_own_bit() {
    // Every other bit of the word set, D clear.
    let mut d = vec![0u8; DOP2003];
    d[86] = 0xF7;
    d[87] = 0xFF;
    assert!(!doc_with_dop(Some(&d)).settings.no_leading);
    // The first word of Copts80 (Copts60) does not hold it.
    let mut d = vec![0u8; DOP2003];
    d[84] = 0xFF;
    d[85] = 0xFF;
    assert!(!doc_with_dop(Some(&d)).settings.no_leading);
    // No DOP, or one too short for Copts80: off.
    assert!(!doc_with_dop(None).settings.no_leading);
    let mut d = vec![0u8; 86];
    d[84] = 0xFF;
    assert!(!doc_with_dop(Some(&d)).settings.no_leading);
}
