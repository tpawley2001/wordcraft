//! A Word 97-2003 file (a DOP shorter than Dop2007's 674 bytes, [MS-DOC] §2.7) splits a page
//! break from the paragraph mark after it: `Settings::split_pg_break_and_para_mark` is on. A
//! Dop2007 or later carries `Copts.fSplitPgBreakAndParaMark` ([MS-DOC] §2.7.1): bit 27 of the
//! u32 at Dop offset 512.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
/// Where the test DOP starts in the Table stream (past the Clx).
const DOP_AT: u32 = 0x100;

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

/// A zeroed DOP of `len` bytes with the Copts flags word (offset 512) set to `flags`.
fn dop(len: usize, flags: u32) -> Vec<u8> {
    let mut d = vec![0u8; len];
    if len >= 516 {
        d[512..516].copy_from_slice(&flags.to_le_bytes());
    }
    d
}

#[test]
fn word_97_2003_files_split_the_break_and_the_mark() {
    // Dop95, Dop97, Dop2000, Dop2002, Dop2003.
    for len in [88, 500, 544, 594, 616] {
        assert!(doc_with_dop(Some(&dop(len, 0))).settings.split_pg_break_and_para_mark, "DOP of {len} bytes");
    }
    // The flags of a real Word 2003 file.
    assert!(doc_with_dop(Some(&dop(616, 0x0003_DC08))).settings.split_pg_break_and_para_mark);
}

#[test]
fn a_dop2007_or_later_says_so_itself() {
    // Dop2007, Dop2010, Dop2013.
    for len in [674, 690, 694] {
        assert!(!doc_with_dop(Some(&dop(len, 0))).settings.split_pg_break_and_para_mark, "DOP of {len} bytes, bit clear");
        assert!(doc_with_dop(Some(&dop(len, 0x0800_0000))).settings.split_pg_break_and_para_mark, "DOP of {len} bytes, bit set");
        // Every other flag set, bit 27 clear.
        assert!(!doc_with_dop(Some(&dop(len, !0x0800_0000))).settings.split_pg_break_and_para_mark, "DOP of {len} bytes, other bits");
    }
}

#[test]
fn no_dop_leaves_the_option_off() {
    assert!(!doc_with_dop(None).settings.split_pg_break_and_para_mark);
}
