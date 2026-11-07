//! Floating tables in `.doc` ([MS-DOC] §2.6.2 sprmPPc, sprmPDxaAbs, sprmPDyaAbs, sprmPDxaFromText,
//! sprmPDyaFromText; §2.6.4 table sprms): when the paragraphs of a table carry frame sprms, Word
//! positions the whole table there, as a `.docx` `w:tblpPr` does. The table gets
//! `TableProps.float` and its cell paragraphs no frame of their own. Values are shaped like a real
//! government form: anchored to the page vertically and the margin horizontally, centred, 2521 twips
//! down, 187 twips from the text on the left and right.

use super::{StructSpec, grpprl, read, struct_doc, t_def_table};
use wordcraft_doc::Document;
use wordcraft_doc::para::{Anchor, FloatAlign};

const P_IN_TABLE: u16 = 0x2416;
const P_TTP: u16 = 0x2417;
const P_PC: u16 = 0x261B;
const P_DXA_ABS: u16 = 0x8418;
const P_DYA_ABS: u16 = 0x8419;
const P_DXA_FROM_TEXT: u16 = 0x842F;

/// pcVert = 1 (page) in bits 4-5, pcHorz = 1 (margin) in bits 6-7.
const PC_PAGE_V_MARGIN_H: u8 = 0x50;
/// XAS for "centred".
const XAS_CENTER: i16 = -4;

fn frame_sprms() -> Vec<(u16, Vec<u8>)> {
    vec![
        (P_PC, vec![PC_PAGE_V_MARGIN_H]),
        (P_DXA_ABS, XAS_CENTER.to_le_bytes().to_vec()),
        (P_DYA_ABS, 2521i16.to_le_bytes().to_vec()),
        (P_DXA_FROM_TEXT, 187i16.to_le_bytes().to_vec()),
    ]
}

fn papx(extra: &[(u16, &[u8])], framed: bool) -> Vec<u8> {
    let frame = frame_sprms();
    let mut prls: Vec<(u16, &[u8])> = extra.to_vec();
    if framed {
        prls.extend(frame.iter().map(|(op, v)| (*op, v.as_slice())));
    }
    grpprl(&prls)
}

/// A 2x2 table ("a" "b" / "c" "d") then "tail"; `framed` puts the frame sprms on every paragraph of
/// the table (cell paragraphs and row ends), as Word writes a positioned table.
fn doc(framed: bool) -> Document {
    let text = "a\u{7}b\u{7}c\u{7}d\u{7}tail\r";
    let cell = papx(&[(P_IN_TABLE, &[1])], framed);
    let ttp = [papx(&[(P_IN_TABLE, &[1]), (P_TTP, &[1])], framed), t_def_table(&[0, 2880, 5760])].concat();
    let spec = StructSpec {
        text,
        header_stories: vec![],
        sections: vec![(text.chars().count() as u32, Vec::new())],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![
            (0, 0, cell.clone()), // "a"
            (2, 0, ttp.clone()),  // "b", ends the row (TTP)
            (4, 0, cell),         // "c"
            (6, 0, ttp),          // "d", ends the row (TTP)
            (8, 0, Vec::new()),   // "tail" paragraph
        ],
        plf_lst: Vec::new(),
        plf_lfo: Vec::new(),
        note_stories: vec![],
        endnote_stories: vec![],
        extra_pairs: Vec::new(),
        data_stream: Vec::new(),
    };
    read(&struct_doc(&spec)).expect("opens")
}

#[test]
fn frame_sprms_on_a_tables_paragraphs_float_the_table() {
    let doc = doc(true);
    let tbl = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(tbl.rows.len(), 2);
    assert_eq!(tbl.rows[0].cells.len(), 2);
    let f = tbl.props.float.as_ref().expect("the table floats");
    assert_eq!(f.v_rel, Anchor::Page);
    assert_eq!(f.h_rel, Anchor::Margin);
    assert_eq!(f.h_align, Some(FloatAlign::Center));
    assert_eq!(f.v_align, None);
    assert!((f.y - 126.05).abs() < 0.01, "y = 2521 twips = 126.05 pt, got {}", f.y);
    let d = f.dist_from_text();
    assert!((d[0] - 9.35).abs() < 0.01 && (d[2] - 9.35).abs() < 0.01, "left/right 187 twips = 9.35 pt, got {d:?}");
}

#[test]
fn cell_paragraphs_of_a_floating_table_have_no_frame() {
    let doc = doc(true);
    let tbl = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    for row in &tbl.rows {
        for cell in &row.cells {
            for p in cell.blocks.iter().filter_map(|b| b.as_para()) {
                assert!(p.props.frame.is_none(), "cell paragraph {:?} keeps a frame", p.text);
            }
        }
    }
    let tail = doc.body.last().and_then(|b| b.as_para()).expect("tail");
    assert_eq!(tail.text, "tail");
    assert!(tail.props.frame.is_none());
}

#[test]
fn a_table_without_frame_sprms_stays_inline() {
    let doc = doc(false);
    let tbl = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert!(tbl.props.float.is_none());
}
