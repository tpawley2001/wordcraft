//! A whole `.doc` whose list levels hold Unicode number text ([MS-DOC] §2.9.150 LVL `xst`,
//! §2.9.147 LSTF, §2.9.151 LVLF): the opened document keeps "第%1章", "●" and a character
//! outside the Basic Multilingual Plane (a UTF-16 surrogate pair) as Word shows them.

use crate::read;
use crate::tests::{FibSpec, cfb_file, fib_bytes};

/// Where the text piece sits in the WordDocument stream (past the FIB).
const TEXT_FC: u32 = 0x600;

/// One LVL: start 1, the given nfc, placeholders at the given one-based offsets, number text
/// as UTF-16 units, no grpprls.
fn lvl(nfc: u8, placeholders: &[u8], units: &[u16]) -> Vec<u8> {
    let mut v = vec![0u8; 28];
    v[0..4].copy_from_slice(&1u32.to_le_bytes());
    v[4] = nfc;
    for (slot, at) in v[6..15].iter_mut().zip(placeholders) {
        *slot = *at;
    }
    v.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for u in units {
        v.extend_from_slice(&u.to_le_bytes());
    }
    v
}

/// A PlfLst with one multi-level list of the nine given levels, and a PlfLfo bound to it.
fn lists(levels: &[Vec<u8>]) -> (Vec<u8>, Vec<u8>) {
    let lsid = 0x3000i32;
    let mut lst = 1u16.to_le_bytes().to_vec();
    lst.extend_from_slice(&lsid.to_le_bytes());
    lst.extend_from_slice(&0u32.to_le_bytes()); // tplc
    for _ in 0..9 {
        lst.extend_from_slice(&0x0FFFu16.to_le_bytes()); // no style links
    }
    lst.extend_from_slice(&[0, 0]); // flags, grfhic
    for l in levels {
        lst.extend_from_slice(l);
    }
    let mut lfo = 1u32.to_le_bytes().to_vec(); // lfoMac
    lfo.extend_from_slice(&lsid.to_le_bytes());
    lfo.extend_from_slice(&[0u8; 12]); // unused1/2, clfolvl, ibstFltAutoNum, grfhic, unused3
    lfo.extend_from_slice(&0u32.to_le_bytes()); // LFOData.cp
    (lst, lfo)
}

/// A document of one paragraph whose Table stream carries the given PlfLst and PlfLfo.
fn doc_with_lists(lst: &[u8], lfo: &[u8]) -> Vec<u8> {
    let text = "one\r";
    let ccp = text.chars().count() as u32;
    let mut word = vec![0u8; TEXT_FC as usize];
    for u in text.encode_utf16() {
        word.extend_from_slice(&u.to_le_bytes());
    }
    // Clx: one uncompressed piece covering the text.
    let mut plc = Vec::new();
    plc.extend_from_slice(&0u32.to_le_bytes());
    plc.extend_from_slice(&ccp.to_le_bytes());
    plc.extend_from_slice(&[0, 0]);
    plc.extend_from_slice(&TEXT_FC.to_le_bytes());
    plc.extend_from_slice(&0u16.to_le_bytes());
    let mut clx = vec![0x02u8];
    clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    clx.extend_from_slice(&plc);
    let mut table = vec![0u8; 0x40];
    let clx_at = table.len() as u32;
    table.extend_from_slice(&clx);
    let lst_at = table.len() as u32;
    table.extend_from_slice(lst);
    let lfo_at = table.len() as u32;
    table.extend_from_slice(lfo);
    let pairs = vec![(33, clx_at, clx.len() as u32), (73, lst_at, lst.len() as u32), (74, lfo_at, lfo.len() as u32)];
    let fib = fib_bytes(&FibSpec { ccp_text: ccp, pairs, ..Default::default() });
    word[..fib.len()].copy_from_slice(&fib);
    cfb_file(&[("WordDocument", &word), ("0Table", &table)])
}

#[test]
fn unicode_number_text_survives_opening() {
    // Level 1: 第%1章 (decimal, placeholder at offset 2); level 2: a ● bullet (nfc 0x17, as the
    // real file stores it in MS Mincho); level 3: "%3" followed by U+1F600 as a surrogate pair.
    let mut levels = vec![lvl(0x00, &[2], &[0x7B2C, 0x0000, 0x7AE0]), lvl(0x17, &[], &[0x25CF]), lvl(0x00, &[1], &[0x0002, 0xD83D, 0xDE00])];
    levels.extend((3..9).map(|i| lvl(0x00, &[1], &[i, 0x002E])));
    let (lst, lfo) = lists(&levels);
    let doc = read(&doc_with_lists(&lst, &lfo)).expect("opens");
    let text = |lvl: u8| doc.numbering.level(1, lvl).map(|l| l.text.clone());
    assert_eq!(text(0).as_deref(), Some("\u{7B2C}%1\u{7AE0}"));
    assert_eq!(text(1).as_deref(), Some("\u{25CF}"));
    assert_eq!(text(2).as_deref(), Some("%3\u{1F600}"));
    assert_eq!(text(3).as_deref(), Some("%4."));
}

#[test]
fn a_lone_surrogate_becomes_a_bullet() {
    let mut levels = vec![lvl(0x17, &[], &[0xD83D]), lvl(0x17, &[], &[0xDE00, 0x0041])];
    levels.extend((2..9).map(|i| lvl(0x00, &[1], &[i, 0x002E])));
    let (lst, lfo) = lists(&levels);
    let doc = read(&doc_with_lists(&lst, &lfo)).expect("opens");
    let text = |lvl: u8| doc.numbering.level(1, lvl).map(|l| l.text.clone());
    assert_eq!(text(0).as_deref(), Some("\u{2022}"));
    assert_eq!(text(1).as_deref(), Some("\u{2022}A"));
}

#[test]
fn a_first_line_indented_level_keeps_its_list_tab() {
    // Legal numbering: left 0, first line +720 twips, a list tab stop at 1080 twips.
    let papx: &[u8] = &[
        0x0F, 0x84, 0x00, 0x00, 0x11, 0x84, 0xD0, 0x02, 0x15, 0xC6, 0x05, 0x00, 0x01, 0x38, 0x04, 0x06, 0x5E, 0x84, 0x00, 0x00, 0x60, 0x84, 0xD0,
        0x02,
    ];
    let mut first = lvl(0x00, &[1], &[0x0000, 0x002E]);
    first[25] = papx.len() as u8;
    let xst = first.split_off(28);
    first.extend_from_slice(papx);
    first.extend_from_slice(&xst);
    let mut levels = vec![first];
    levels.extend((1..9).map(|i| lvl(0x00, &[1], &[i, 0x002E])));
    let (lst, lfo) = lists(&levels);
    let doc = read(&doc_with_lists(&lst, &lfo)).expect("opens");
    let l = doc.numbering.level(1, 0).expect("level 1");
    assert_eq!((l.indent, l.hanging, l.tab), (0.0, -36.0, Some(54.0)));
    assert_eq!(l.text, "%1.");
}
