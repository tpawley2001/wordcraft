//! The compatibility options read from the DOP ([MS-DOC] §2.7) of a Word 97-2003 file.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
/// Where the test DOP starts in the Table stream (past the Clx).
const DOP_AT: u32 = 0x100;
/// Sizes of the DOP versions ([MS-DOC] §2.7): Dop95, Dop97, Dop2000, Dop2003.
const DOP95: usize = 88;
const DOP97: usize = 500;
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

/// A DOP of `len` bytes with `Copts60.fExpShRtn`, `Copts.fDontUseHTMLParagraphAutoSpacing` and
/// `Copts.fAllowSpaceOfSameStyleInTable` set as given (where the DOP is long enough to hold them).
fn dop(len: usize, exp_sh_rtn: bool, no_html_spacing: bool, same_style_space: bool) -> Vec<u8> {
    let mut d = vec![0u8; len];
    if exp_sh_rtn && len > 85 {
        d[85] |= 0x20; // Copts60 at 84, bit 13
    }
    let mut flags = 0u32;
    if no_html_spacing {
        flags |= 1 << 2;
    }
    if same_style_space {
        flags |= 1 << 21;
    }
    if len >= 516 {
        d[512..516].copy_from_slice(&flags.to_le_bytes()); // Dop2000.copts at 508, + copts80
    }
    d
}

#[test]
fn no_dop_keeps_defaults() {
    let doc = doc_with_dop(None);
    let s = &doc.settings;
    assert_eq!(s.compat_mode, 11);
    assert!(!s.do_not_expand_shift_return && !s.sum_paragraph_spacing && !s.allow_space_of_same_style_in_table);
}

#[test]
fn expand_shift_return_flag_is_read() {
    assert!(doc_with_dop(Some(&dop(DOP2003, true, false, false))).settings.do_not_expand_shift_return);
    assert!(!doc_with_dop(Some(&dop(DOP2003, false, true, true))).settings.do_not_expand_shift_return);
    // A Dop95-sized DOP already holds it.
    assert!(doc_with_dop(Some(&dop(88, true, false, false))).settings.do_not_expand_shift_return);
}

#[test]
fn html_auto_spacing_flag_is_read() {
    assert!(doc_with_dop(Some(&dop(DOP2000, false, true, false))).settings.sum_paragraph_spacing);
    assert!(!doc_with_dop(Some(&dop(DOP2003, true, false, true))).settings.sum_paragraph_spacing);
    // A Dop2000 with the bit clear collapses (HTML auto spacing).
    assert!(!doc_with_dop(Some(&dop(DOP2000, false, false, false))).settings.sum_paragraph_spacing);
}

#[test]
fn same_style_space_in_table_flag_is_read() {
    assert!(doc_with_dop(Some(&dop(DOP2003, false, false, true))).settings.allow_space_of_same_style_in_table);
    assert!(!doc_with_dop(Some(&dop(DOP2003, true, true, false))).settings.allow_space_of_same_style_in_table);
    // A Dop2000 or Dop2002 predates the option: the bit is not trusted.
    assert!(!doc_with_dop(Some(&dop(DOP2000, false, false, true))).settings.allow_space_of_same_style_in_table);
}

#[test]
fn short_dop_sums_paragraph_spacing() {
    // A Dop97 (shorter than Dop2000) predates HTML paragraph auto spacing: Word adds the space after
    // a paragraph to the space before the next. The Copts60 flag is still read; the Word 2003 option
    // stays default.
    let s = doc_with_dop(Some(&dop(DOP97, true, false, false))).settings;
    assert!(s.do_not_expand_shift_return);
    assert!(s.sum_paragraph_spacing);
    assert!(!s.allow_space_of_same_style_in_table);
    let s = doc_with_dop(Some(&dop(DOP97, false, false, false))).settings;
    assert!(!s.do_not_expand_shift_return && s.sum_paragraph_spacing);
    // A Dop95-sized DOP (Word 95) sums too.
    let s = doc_with_dop(Some(&dop(DOP95, false, false, false))).settings;
    assert!(s.sum_paragraph_spacing && !s.allow_space_of_same_style_in_table);
    let s = doc_with_dop(Some(&dop(DOP2000 - 1, false, false, false))).settings;
    assert!(s.sum_paragraph_spacing);
    // A DOP too short even for Dop95 keeps every default.
    let s = doc_with_dop(Some(&[0xFF; 60])).settings;
    assert!(!s.do_not_expand_shift_return && !s.sum_paragraph_spacing && !s.allow_space_of_same_style_in_table);
    assert_eq!(s.compat_mode, 11);
}

#[test]
fn dop_running_past_the_table_stream_keeps_defaults() {
    // fcDop inside the stream but lcbDop runs past its end (and an overflowing pair).
    let d = dop(DOP2003, true, true, true);
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
    table.extend_from_slice(&d);
    for (fc, lcb) in [(DOP_AT, DOP2003 as u32 + 1), (u32::MAX, u32::MAX), (DOP_AT, u32::MAX)] {
        let pairs = vec![(33, super::CLX_AT, clx.len() as u32), (DOP_PAIR, fc, lcb)];
        let fib = fib_bytes(&FibSpec { ccp_text: 6, pairs, ..Default::default() });
        word[..fib.len()].copy_from_slice(&fib);
        let s = read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens").settings;
        assert!(!s.do_not_expand_shift_return && !s.sum_paragraph_spacing && !s.allow_space_of_same_style_in_table);
        assert_eq!(s.compat_mode, 11);
    }
}

/// `Copts80.f2ptExtLeadingOnly` ([MS-DOC] §2.7.3): bit G of the u16 at Dop offset 86.
#[test]
fn ext_leading_2pt_only_flag_is_read() {
    let mut d = dop(DOP2003, false, true, false);
    d[86] |= 0x40;
    assert!(doc_with_dop(Some(&d)).settings.ext_leading_2pt_only);
    // A Dop95-sized DOP already holds Copts80.
    let mut d = dop(DOP95, false, false, false);
    d[86] |= 0x40;
    assert!(doc_with_dop(Some(&d)).settings.ext_leading_2pt_only);
    assert!(!doc_with_dop(None).settings.ext_leading_2pt_only);
}

#[test]
fn ext_leading_2pt_only_needs_its_own_bit() {
    // fDontMakeSpaceForUL (E, 0x0010), fTruncFontHeight (H, 0x0080) and fUsePrinterMetrics (P, 0x8000)
    // set, G clear: Word keeps the font's own line height in these files.
    let mut d = dop(DOP2003, false, true, false);
    d[86] |= 0x10 | 0x80;
    d[87] |= 0x80;
    assert!(!doc_with_dop(Some(&d)).settings.ext_leading_2pt_only);
    // A DOP too short for Copts80 leaves it off.
    let mut d = vec![0u8; 86];
    d[84] = 0xff;
    assert!(!doc_with_dop(Some(&d)).settings.ext_leading_2pt_only);
}

/// A Dop2003 whose `Copts60` word is `copts60`.
fn dop_copts60(copts60: u16) -> Vec<u8> {
    let mut d = vec![0u8; DOP2003];
    d[84..86].copy_from_slice(&copts60.to_le_bytes());
    d
}

#[test]
fn a1_wrap_trailing_spaces_flag_is_read() {
    assert!(doc_with_dop(Some(&dop_copts60(0xf029))).settings.wrap_trail_spaces);
}

#[test]
fn a2_without_the_flag_trailing_spaces_hang() {
    assert!(!doc_with_dop(Some(&dop_copts60(0xf021))).settings.wrap_trail_spaces);
}

#[test]
fn a3_a_dop_too_short_for_copts60_keeps_the_default() {
    let mut d = dop_copts60(0xf029);
    d.truncate(84);
    assert!(!doc_with_dop(Some(&d)).settings.wrap_trail_spaces);
    assert!(!doc_with_dop(None).settings.wrap_trail_spaces);
}
