//! Document-wide footnote and endnote options in the DOP of Word 97 and Word 2000 files ([MS-DOC]
//! §2.7.2 DopBase: fpc, rncFtn, nFtn, rncEdn, nEdn, epc; §2.7.4 Dop97: nfcFtnRef, nfcEdnRef). They
//! count only when nFib <= 0x00D9; newer files keep the options in the section sprms.

use wordcraft_doc::section::{NotePos, NoteProps, NoteRestart, NumFormat};

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
const DOP_AT: u32 = 0x100;
/// Where `cswNew` follows a Word 97 sized FibRgFcLcb blob (0x9A + 0x5D * 8).
const CSW_NEW_AT: usize = 0x382;

/// A one-paragraph Word 97 file (FibBase.nFib 0x00C1) holding `dop`; `nfib_new` adds a FibRgCswNew
/// whose nFibNew names a later version.
fn doc(dop: &[u8], nfib_new: Option<u16>) -> wordcraft_doc::Document {
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
    if let Some(n) = nfib_new {
        word[CSW_NEW_AT..CSW_NEW_AT + 2].copy_from_slice(&2u16.to_le_bytes());
        word[CSW_NEW_AT + 2..CSW_NEW_AT + 4].copy_from_slice(&n.to_le_bytes());
    }
    read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens")
}

struct Notes {
    fpc: u8,
    rnc_ftn: u16,
    n_ftn: u16,
    rnc_edn: u16,
    n_edn: u16,
    epc: u8,
    nfc_ftn: u16,
    nfc_edn: u16,
}

/// A DOP of `len` bytes with these note options at their DopBase / Dop97 places.
fn dop(len: usize, n: &Notes) -> Vec<u8> {
    let mut d = vec![0u8; len];
    d[0] |= (n.fpc & 3) << 5;
    d[2..4].copy_from_slice(&((n.rnc_ftn & 3) | (n.n_ftn << 2)).to_le_bytes());
    if len >= 56 {
        d[52..54].copy_from_slice(&((n.rnc_edn & 3) | (n.n_edn << 2)).to_le_bytes());
        d[54] |= n.epc & 3;
    }
    if len >= 500 {
        d[492..494].copy_from_slice(&n.nfc_ftn.to_le_bytes());
        d[494..496].copy_from_slice(&n.nfc_edn.to_le_bytes());
    }
    d
}

/// Beneath text, restart each page, start at 3, symbols; endnotes at the end of each section,
/// restarting each section from 4, in Arabic numbers.
const SET: Notes = Notes { fpc: 2, rnc_ftn: 2, n_ftn: 3, rnc_edn: 1, n_edn: 4, epc: 0, nfc_ftn: 9, nfc_edn: 0 };
/// What Word writes for its defaults: bottom of page, continuous from 1, endnotes at the end of
/// the document in i, ii, iii.
const DEFAULTS: Notes = Notes { fpc: 1, rnc_ftn: 0, n_ftn: 1, rnc_edn: 0, n_edn: 1, epc: 3, nfc_ftn: 0, nfc_edn: 2 };

fn check_set(d: &wordcraft_doc::Document, what: &str) {
    let s = &d.settings;
    assert_eq!(
        s.footnote_pr,
        NoteProps { pos: Some(NotePos::BeneathText), num_fmt: None, num_start: Some(3), num_restart: Some(NoteRestart::EachPage) },
        "{what}: footnotes"
    );
    assert_eq!(s.footnote_format, NumFormat::Chicago, "{what}");
    assert_eq!(
        s.endnote_pr,
        NoteProps { pos: Some(NotePos::SectEnd), num_fmt: None, num_start: Some(4), num_restart: Some(NoteRestart::EachSect) },
        "{what}: endnotes"
    );
    assert_eq!(s.endnote_format, NumFormat::Decimal, "{what}");
}

#[test]
fn word97_note_options_are_read() {
    check_set(&doc(&dop(500, &SET), None), "Word 97");
}

#[test]
fn word2000_note_options_are_read() {
    check_set(&doc(&dop(544, &SET), Some(0x00D9)), "Word 2000");
}

#[test]
fn later_versions_ignore_the_dop_note_options() {
    for v in [0x0101u16, 0x010C, 0x0112] {
        let d = doc(&dop(544, &SET), Some(v));
        assert_eq!(d.settings.footnote_pr, NoteProps::default(), "nFib 0x{v:04X}");
        assert_eq!(d.settings.endnote_pr, NoteProps::default(), "nFib 0x{v:04X}");
        assert_eq!(d.settings.footnote_format, NumFormat::Decimal, "nFib 0x{v:04X}");
        assert_eq!(d.settings.endnote_format, NumFormat::LowerRoman, "nFib 0x{v:04X}");
    }
}

#[test]
fn word_defaults_leave_the_settings_unset() {
    let d = doc(&dop(500, &DEFAULTS), None);
    assert_eq!(d.settings.footnote_pr, NoteProps::default());
    assert_eq!(d.settings.endnote_pr, NoteProps::default());
    assert_eq!(d.settings.footnote_format, NumFormat::Decimal);
    assert_eq!(d.settings.endnote_format, NumFormat::LowerRoman);
}

#[test]
fn a_dop95_has_no_number_formats() {
    // 88 bytes: DopBase + Copts80. Placement and restarts are read; nfcFtnRef/nfcEdnRef are not there.
    let d = doc(&dop(88, &SET), None);
    assert_eq!(d.settings.footnote_pr.pos, Some(NotePos::BeneathText));
    assert_eq!(d.settings.endnote_pr.num_start, Some(4));
    assert_eq!(d.settings.footnote_format, NumFormat::Decimal, "Word's default stays");
    assert_eq!(d.settings.endnote_format, NumFormat::LowerRoman, "Word's default stays");
}

#[test]
fn endnote_formats_follow_msonfc() {
    for (nfc, want) in [(0u16, NumFormat::Decimal), (1, NumFormat::UpperRoman), (3, NumFormat::UpperLetter), (9, NumFormat::Chicago)] {
        let d = doc(&dop(500, &Notes { nfc_edn: nfc, ..DEFAULTS }), None);
        assert_eq!(d.settings.endnote_format, want, "nfcEdnRef {nfc}");
    }
}
