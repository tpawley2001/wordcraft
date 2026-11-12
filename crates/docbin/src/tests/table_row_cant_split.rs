//! `.doc` table rows: which rows may break across pages.
//!
//! Word 2002 and later write `sprmTFCantSplit90` (0x3403) on rows for Word 97/2000 and keep the
//! real "allow row to break across pages" setting in `sprmTFCantSplit` (0x3466); they also store a
//! second copy of the row's table properties in the Data stream (`sprmPTableProps`, 0x646B, a
//! `PrcData`), and that copy alone decides the row's cantSplit ([MS-DOC] 2.5.12 FibRgCswNew,
//! 2.6.3, 2.9.209 PrcData). Every expectation here was read off Word's own layout of files that
//! differ in these bytes only.

use std::io::Read;

use wordcraft_doc::Document;

use super::{StructSpec, cfb_file, grpprl, read, struct_doc, structured_spec, t_def_table};

/// `sprmTFCantSplit90`: the Word 97/2000 "don't break this row across pages".
const CANT_SPLIT_90: u16 = 0x3403;
/// `sprmTFCantSplit`: the same setting as Word 2002 and later read it.
const CANT_SPLIT: u16 = 0x3466;
/// `sprmPTableProps`: the Data-stream offset of the row's `PrcData`.
const TABLE_PROPS: u16 = 0x646B;

/// The sprms every row-end mark starts with: in a table, row end.
fn ttp() -> Vec<u8> {
    grpprl(&[(0x2416, &[0x01]), (0x2417, &[0x01])])
}

/// One row-end mark: the table sprms of its PAPX and, when given, the sprms of its Data-stream copy.
struct RowEnd {
    papx: Vec<u8>,
    data: Option<Vec<u8>>,
}

/// A 2 x 2 table ("a", "b" / "c", "d") and a tail paragraph. Each row-end PAPX holds the row-end
/// sprms, a `sprmPTableProps` when the row has a Data-stream copy, then `papx`. `version` is
/// written as `nFibNew` (with `cswNew` = 2); `None` leaves the fixture's Word 97 FIB (`cswNew` = 0).
fn table_doc(rows: [RowEnd; 2], version: Option<u16>) -> Document {
    let text = "a\u{7}b\u{7}c\u{7}d\u{7}tail\r";
    let in_tbl = grpprl(&[(0x2416, &[0x01])]);
    // Something before the first PrcData, so no offset is zero.
    let mut data = vec![0u8; 6];
    let mut ends = Vec::new();
    for row in rows {
        let mut papx = ttp();
        if let Some(sprms) = row.data {
            let at = data.len() as u32;
            let prc = [ttp(), sprms].concat();
            data.extend_from_slice(&(prc.len() as u16).to_le_bytes());
            data.extend_from_slice(&prc);
            papx.extend_from_slice(&grpprl(&[(TABLE_PROPS, &at.to_le_bytes())]));
        }
        papx.extend_from_slice(&row.papx);
        ends.push(papx);
    }
    let row2 = ends.pop().expect("two rows");
    let row1 = ends.pop().expect("two rows");
    let spec = StructSpec {
        text,
        papx_entries: vec![(0, 0, in_tbl.clone()), (2, 0, row1), (4, 0, in_tbl), (6, 0, row2), (8, 0, Vec::new())],
        data_stream: data,
        ..structured_spec(text, text.chars().count() as u32)
    };
    let file = struct_doc(&spec);
    let file = match version {
        Some(v) => with_nfib_new(&file, v),
        None => file,
    };
    read(&file).expect("opens")
}

/// The same file with a `FibRgCswNew` of two words: `nFibNew` = `version`, `cQuickSavesNew` = 0.
/// The test FIB ends with `cswNew` at 0x382 and the text starts at 0x600, so the bytes are free.
fn with_nfib_new(file: &[u8], version: u16) -> Vec<u8> {
    const CSW_NEW_AT: usize = 0x9A + 0x5D * 8;
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(file.to_vec())).expect("cfb open");
    let mut streams: Vec<(&str, Vec<u8>)> = Vec::new();
    for name in ["WordDocument", "0Table", "Data"] {
        if comp.is_stream(format!("/{name}")) {
            let mut v = Vec::new();
            comp.open_stream(format!("/{name}")).expect("stream").read_to_end(&mut v).expect("read");
            streams.push((name, v));
        }
    }
    let word = &mut streams[0].1;
    assert_eq!(&word[CSW_NEW_AT..CSW_NEW_AT + 2], &[0, 0], "the fixture FIB ends with cswNew = 0");
    word[CSW_NEW_AT..CSW_NEW_AT + 2].copy_from_slice(&2u16.to_le_bytes());
    word[CSW_NEW_AT + 2..CSW_NEW_AT + 4].copy_from_slice(&version.to_le_bytes());
    word[CSW_NEW_AT + 4..CSW_NEW_AT + 6].copy_from_slice(&[0, 0]);
    let refs: Vec<(&str, &[u8])> = streams.iter().map(|(n, v)| (*n, v.as_slice())).collect();
    cfb_file(&refs)
}

/// A two-cell `sprmTDefTable` (edges 0, 2880, 5760 twips) with default TC80s.
fn plain_def() -> Vec<u8> {
    t_def_table(&[0, 2880, 5760])
}

/// Both rows with the same table sprms in the PAPX and no Data-stream copy.
fn papx_only(sprms: &[u8], version: Option<u16>) -> Document {
    let row = || RowEnd { papx: [sprms, &plain_def()].concat(), data: None };
    table_doc([row(), row()], version)
}

fn cant_split(doc: &Document) -> Vec<bool> {
    let tbl = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(tbl.rows.len(), 2, "two rows");
    tbl.rows.iter().map(|r| r.props.cant_split).collect()
}

// ---- R1: sprmTFCantSplit90 counts only in Word 97/2000 files ----------------------------------

#[test]
fn cant_split_90_keeps_a_row_together_in_a_word_97_file() {
    let doc = papx_only(&grpprl(&[(CANT_SPLIT_90, &[1])]), None);
    assert_eq!(cant_split(&doc), [true, true]);
}

#[test]
fn cant_split_90_keeps_a_row_together_in_a_word_2000_file() {
    // Word, nFibNew patched to 0x00D9: the rows no longer split.
    let doc = papx_only(&grpprl(&[(CANT_SPLIT_90, &[1])]), Some(0x00D9));
    assert_eq!(cant_split(&doc), [true, true]);
    // A FibRgCswNew whose nFibNew is 0 reads the same way.
    let doc = papx_only(&grpprl(&[(CANT_SPLIT_90, &[1])]), Some(0));
    assert_eq!(cant_split(&doc), [true, true]);
}

#[test]
fn cant_split_90_is_ignored_in_word_2002_and_later_files() {
    // Word, nFibNew 0x0101 and 0x010C: the layout is the same with the operand 1 or 0.
    for version in [0x0101u16, 0x010C] {
        let doc = papx_only(&grpprl(&[(CANT_SPLIT_90, &[1])]), Some(version));
        assert_eq!(cant_split(&doc), [false, false], "nFibNew 0x{version:04X}");
    }
}

#[test]
fn cant_split_keeps_a_row_together_in_every_version() {
    for version in [None, Some(0x00D9u16), Some(0x0101), Some(0x010C)] {
        let doc = papx_only(&grpprl(&[(CANT_SPLIT, &[1])]), version);
        assert_eq!(cant_split(&doc), [true, true], "{version:?}");
        // Word writes both on a row that must stay whole.
        let doc = papx_only(&grpprl(&[(CANT_SPLIT_90, &[1]), (CANT_SPLIT, &[1])]), version);
        assert_eq!(cant_split(&doc), [true, true], "{version:?}");
    }
}

#[test]
fn rows_differ_within_one_word_2003_table() {
    let row = |sprms: Vec<u8>| RowEnd { papx: [sprms, plain_def()].concat(), data: None };
    let doc = table_doc([row(grpprl(&[(CANT_SPLIT_90, &[1])])), row(grpprl(&[(CANT_SPLIT_90, &[1]), (CANT_SPLIT, &[1])]))], Some(0x010C));
    assert_eq!(cant_split(&doc), [false, true]);
}

// ---- R2: with sprmPTableProps the Data-stream copy decides cantSplit --------------------------

#[test]
fn table_props_copy_decides_cant_split() {
    // Word (nFibNew 0x0101): sprmTFCantSplit only in the PAPX is not used...
    let row = || RowEnd { papx: [grpprl(&[(CANT_SPLIT, &[1])]), plain_def()].concat(), data: Some(grpprl(&[(CANT_SPLIT_90, &[1])])) };
    let doc = table_doc([row(), row()], Some(0x0101));
    assert_eq!(cant_split(&doc), [false, false], "the PAPX's 0x3466 is ignored beside a sprmPTableProps");
    // ...and only in the Data-stream copy it keeps the row together.
    let row = || RowEnd { papx: [grpprl(&[(CANT_SPLIT_90, &[1])]), plain_def()].concat(), data: Some(grpprl(&[(CANT_SPLIT, &[1])])) };
    let doc = table_doc([row(), row()], Some(0x0101));
    assert_eq!(cant_split(&doc), [true, true]);
    // No cantSplit sprm in the PAPX at all.
    let row = || RowEnd { papx: plain_def(), data: Some(grpprl(&[(CANT_SPLIT, &[1])])) };
    let doc = table_doc([row(), row()], Some(0x010C));
    assert_eq!(cant_split(&doc), [true, true]);
}

#[test]
fn table_props_copy_follows_the_version_rule_for_cant_split_90() {
    let row = || RowEnd { papx: [grpprl(&[(CANT_SPLIT_90, &[1])]), plain_def()].concat(), data: Some(grpprl(&[(CANT_SPLIT_90, &[1])])) };
    // Word, nFibNew 0x00D9: honoured; 0x0101: ignored (the file as Word 2002 wrote it).
    assert_eq!(cant_split(&table_doc([row(), row()], Some(0x00D9))), [true, true]);
    assert_eq!(cant_split(&table_doc([row(), row()], Some(0x0101))), [false, false]);
}

#[test]
fn table_props_copy_keeps_the_rest_of_the_row_from_the_papx() {
    let row = || RowEnd { papx: plain_def(), data: Some(grpprl(&[(CANT_SPLIT, &[1])])) };
    let doc = table_doc([row(), row()], Some(0x0101));
    let tbl = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(tbl.grid, vec![144.0, 144.0]);
    assert_eq!(tbl.rows.len(), 2);
    let text = |r: usize, c: usize| tbl.rows[r].cells[c].blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<String>();
    assert_eq!([text(0, 0), text(0, 1), text(1, 0), text(1, 1)], ["a", "b", "c", "d"]);
    let tail = doc.body.last().and_then(|b| b.as_para()).expect("tail paragraph");
    assert_eq!(tail.text, "tail");
}

#[test]
fn table_props_outside_the_data_stream_falls_back_to_the_papx() {
    let text = "a\u{7}b\u{7}c\u{7}d\u{7}tail\r";
    let in_tbl = grpprl(&[(0x2416, &[0x01])]);
    for (at, data) in [(0x00FF_FF00u32, vec![0u8; 8]), (6, vec![0u8; 7]), (0, vec![0xFF, 0x7F, 0x16]), (0, Vec::new())] {
        let row = || [ttp(), grpprl(&[(TABLE_PROPS, &at.to_le_bytes())]), grpprl(&[(CANT_SPLIT, &[1])]), plain_def()].concat();
        let spec = StructSpec {
            text,
            papx_entries: vec![(0, 0, in_tbl.clone()), (2, 0, row()), (4, 0, in_tbl.clone()), (6, 0, row()), (8, 0, Vec::new())],
            data_stream: data,
            ..structured_spec(text, text.chars().count() as u32)
        };
        let doc = read(&with_nfib_new(&struct_doc(&spec), 0x0101)).expect("opens");
        assert_eq!(cant_split(&doc), [true, true], "offset {at:#x}: the PAPX's own sprms decide");
    }
}
