//! The font table's family class ([MS-DOC] §2.9.82 FFN, `ff` = bits 4-6 of the first byte)
//! reaches `Settings::roman_fonts`, the list layout reads to draw a missing roman font in a
//! serif face instead of the generic sans substitutes.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcSttbfFfn`/`lcbSttbfFfn` ([MS-DOC] §2.5.6).
const FFN_PAIR: usize = 15;
/// Where the test font table starts in the Table stream (past the Clx).
const FFN_AT: u32 = 0x100;

/// FFID bytes: `prq` in bits 0-1, `fTrueType` bit 2, `ff` in bits 4-6.
const ROMAN: u8 = 0x10;
const ROMAN_VARIABLE: u8 = 0x12;
const ROMAN_VARIABLE_TRUETYPE: u8 = 0x16;
const SWISS_VARIABLE: u8 = 0x22;
const MODERN_FIXED: u8 = 0x31;

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

/// One FFN: the 39-byte header (FFID, wWeight, chs, ixchSzAlt, panose, fs), the primary name and
/// the alternate name, when there is one, at the character index stored in `ixchSzAlt`.
fn ffn(ffid: u8, name: &str, alt: Option<&str>) -> Vec<u8> {
    let mut v = vec![0u8; 39];
    v[0] = ffid;
    v[1..3].copy_from_slice(&400u16.to_le_bytes());
    let primary = utf16z(name);
    if alt.is_some() {
        v[4] = (primary.len() / 2) as u8;
    }
    v.extend_from_slice(&primary);
    if let Some(a) = alt {
        v.extend_from_slice(&utf16z(a));
    }
    v
}

/// A non-extended SttbfFfn ([MS-DOC] §2.9.286) of ready-made FFNs: cData, cbExtra = 0, then a
/// one-byte cchData before each FFN.
fn sttbf_ffn(ffns: &[Vec<u8>]) -> Vec<u8> {
    let mut v = (ffns.len() as u16).to_le_bytes().to_vec();
    v.extend_from_slice(&0u16.to_le_bytes());
    for f in ffns {
        assert!(f.len() <= 255, "an FFN of a non-extended table fits one length byte");
        v.push(f.len() as u8);
        v.extend_from_slice(f);
    }
    v
}

/// The bytes of a one-paragraph document whose Table stream holds `font_table` (if any) at
/// `FFN_AT`; `lcb` overrides the stored length of the table.
fn doc_bytes(font_table: Option<&[u8]>, lcb: Option<u32>) -> Vec<u8> {
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
    if let Some(t) = font_table {
        assert!(table.len() <= FFN_AT as usize, "the Clx ends before the font table");
        table.resize(FFN_AT as usize, 0);
        table.extend_from_slice(t);
        pairs.push((FFN_PAIR, FFN_AT, lcb.unwrap_or(t.len() as u32)));
    }
    let fib = fib_bytes(&FibSpec { ccp_text: text.chars().count() as u32, pairs, ..Default::default() });
    word[..fib.len()].copy_from_slice(&fib);
    cfb_file(&[("WordDocument", &word), ("0Table", &table)])
}

/// `Settings::roman_fonts` of a document with these FFNs as its font table.
fn roman_fonts(ffns: &[Vec<u8>]) -> Vec<String> {
    read(&doc_bytes(Some(&sttbf_ffn(ffns)), None)).expect("opens").settings.roman_fonts
}

#[test]
fn roman_fonts_are_the_fonts_of_the_roman_family() {
    let got = roman_fonts(&[
        ffn(ROMAN_VARIABLE_TRUETYPE, "Times New Roman", None),
        ffn(ROMAN_VARIABLE, "Zzyzx Roman", None),
        ffn(SWISS_VARIABLE, "Zzyzx Sans", None),
        ffn(MODERN_FIXED, "Zzyzx Mono", Some("Courier New")),
        ffn(ROMAN_VARIABLE, "CG Times", None),
        ffn(0x00, "Zzyzx Dontcare", None),
        ffn(0x52, "Zzyzx Decorative", None),
    ]);
    assert_eq!(got, ["Times New Roman", "Zzyzx Roman", "CG Times"], "roman fonts only, in table order");
}

#[test]
fn only_the_family_bits_decide() {
    // prq, fTrueType and the two unused bits (3 and 7) don't change the family.
    for ffid in [0x10u8, 0x11, 0x12, 0x14, 0x16, 0x18, 0x90, 0x9F] {
        assert_eq!(roman_fonts(&[ffn(ffid, "Zzyzx Roman", None)]), ["Zzyzx Roman"], "FFID {ffid:#04x} is roman");
    }
    for ffid in [0x00u8, 0x02, 0x20, 0x26, 0x30, 0x40, 0x50, 0x60, 0x70, 0xA2] {
        assert!(roman_fonts(&[ffn(ffid, "Zzyzx Other", None)]).is_empty(), "FFID {ffid:#04x} is not roman");
    }
}

#[test]
fn a_roman_font_with_an_alternate_is_listed_under_its_primary_name() {
    let got = roman_fonts(&[ffn(ROMAN, "Zzyzx Alt", Some("Arial")), ffn(SWISS_VARIABLE, "Zzyzx Sans", Some("Zzyzx Roman"))]);
    assert_eq!(got, ["Zzyzx Alt"], "alternate names are never listed");
}

#[test]
fn an_alternate_index_past_the_data_is_no_alternate() {
    let mut f = ffn(ROMAN_VARIABLE, "Zzyzx Far", None);
    f[4] = 200; // ixchSzAlt: 200 characters into a 10-character name field
    assert_eq!(roman_fonts(&[f, ffn(ROMAN, "Zzyzx After", None)]), ["Zzyzx Far", "Zzyzx After"]);
}

#[test]
fn an_ffn_cut_short_adds_nothing_and_the_next_one_is_still_read() {
    let cut = ffn(ROMAN, "Zzyzx Cut", None)[..20].to_vec(); // header only in part: no name
    assert_eq!(roman_fonts(&[cut, ffn(ROMAN_VARIABLE, "Zzyzx After", None)]), ["Zzyzx After"]);
    // Shorter than the header's first five bytes, and empty.
    assert_eq!(roman_fonts(&[vec![ROMAN], Vec::new(), vec![ROMAN, 0, 0, 0], ffn(ROMAN, "Zzyzx After", None)]), ["Zzyzx After"]);
    // A roman FFN whose name is empty.
    assert!(roman_fonts(&[ffn(ROMAN, "", None)]).is_empty());
}

#[test]
fn a_huge_font_table_is_capped() {
    let ffns: Vec<Vec<u8>> = (0..1100).map(|i| ffn(ROMAN, &format!("Zz{i}"), None)).collect();
    let got = roman_fonts(&ffns);
    assert_eq!(got.first().map(String::as_str), Some("Zz0"));
    assert!(got.len() <= 2000, "{} names", got.len());
    assert!(got.iter().all(|n| n.len() <= 256));
}

#[test]
fn no_font_table_or_a_broken_one_leaves_the_list_empty() {
    assert!(read(&doc_bytes(None, None)).expect("opens").settings.roman_fonts.is_empty());
    let table = sttbf_ffn(&[ffn(ROMAN, "Zzyzx Roman", None)]);
    // The stored length runs past the end of the Table stream.
    assert!(read(&doc_bytes(Some(&table), Some(0x00FF_FFFF))).expect("opens").settings.roman_fonts.is_empty());
    // The table ends inside the roman FFN's header: nothing to list, nothing to panic on.
    assert!(read(&doc_bytes(Some(&table), Some(8))).expect("opens").settings.roman_fonts.is_empty());
}

/// An FFN with this charset (`chs`) and PANOSE.
fn ffn_classed(name: &str, chs: u8, panose: [u8; 10]) -> Vec<u8> {
    let mut v = ffn(ROMAN_VARIABLE, name, None);
    v[3] = chs;
    v[5..15].copy_from_slice(&panose);
    v
}

#[test]
fn symbol_class_fonts_reach_the_document_settings() {
    // D7: a converted document's icon font (symbol charset, pictorial PANOSE) is marked, under
    // its own name in lowercase; a text font and a symbol-charset font with no PANOSE are not.
    let table = sttbf_ffn(&[
        ffn_classed("Times New Roman", 0, [2, 2, 6, 3, 5, 4, 5, 2, 3, 4]),
        ffn_classed("WP IconicSymbolsA", 2, [5, 1, 1, 1, 1, 1, 1, 1, 1, 1]),
        ffn_classed("Zzyzx Nopanose", 2, [0; 10]),
    ]);
    let d = read(&doc_bytes(Some(&table), None)).expect("opens");
    assert_eq!(d.settings.symbol_class_fonts, ["wp iconicsymbolsa"]);
    // Without a font table, none.
    assert!(read(&doc_bytes(None, None)).expect("opens").settings.symbol_class_fonts.is_empty());
}
