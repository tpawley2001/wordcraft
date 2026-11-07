//! Floating tables in `.doc` from the TAP ([MS-DOC] §2.6.4 sprmTPc, sprmTDxaAbs, sprmTDyaAbs,
//! sprmTDxaFromText, sprmTDyaFromText, sprmTDxaFromTextRight, sprmTDyaFromTextBottom,
//! sprmTFNoAllowOverlap): newer Word writes a positioned table's place as table sprms on the
//! row end, and they override the paragraph frame sprms field by field. A framed paragraph right
//! after a floating table keeps its own frame.

//!
//! These build the paragraph marks straight from their PAPX grpprls and assemble them the way
//! the reader does, so they need no whole-file fixture.

use wordcraft_doc::para::{Anchor, FloatAlign, Paragraph};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Blocks, Document};

use crate::table::{self, ParaOut};
use crate::{fmt, sprm};

const P_IN_TABLE: u16 = 0x2416;
const P_TTP: u16 = 0x2417;
const P_PC: u16 = 0x261B;
const P_DXA_ABS: u16 = 0x8418;
const P_DYA_ABS: u16 = 0x8419;
const P_DXA_WIDTH: u16 = 0x841A;

const T_PC: u16 = 0x360D;
const T_DXA_ABS: u16 = 0x940E;
const T_DYA_ABS: u16 = 0x940F;
const T_DXA_FROM_TEXT: u16 = 0x9410;
const T_DYA_FROM_TEXT: u16 = 0x9411;
const T_DXA_FROM_TEXT_RIGHT: u16 = 0x941E;
const T_DYA_FROM_TEXT_BOTTOM: u16 = 0x941F;
const T_F_NO_ALLOW_OVERLAP: u16 = 0x3465;

/// A paragraph mark: its text, its PAPX sprms and whether the mark is a cell end (0x07).
fn mark(text: &str, prls: &[(u16, &[u8])], cell_mark: bool) -> ParaOut {
    let papx: Vec<u8> = prls.iter().flat_map(|(op, v)| op.to_le_bytes().into_iter().chain(v.iter().copied())).collect();
    let mut para = Paragraph::with_text(text, CharProps::default());
    for prl in sprm::iter(&papx).filter(|p| p.sgc() == 1) {
        fmt::apply_para(&mut para.props, &prl);
    }
    let mut row = table::decode(&papx);
    row.cell_mark = cell_mark;
    ParaOut { para, end_cp: 0, row: Some(Box::new(row)) }
}

/// A one-row, two-cell table ("a" "b") then "tail": `cell` and `ttp` are the extra sprms of the
/// cell paragraph and the row end, `tail` those of the paragraph after the table.
fn doc(cell: &[(u16, &[u8])], ttp: &[(u16, &[u8])], tail: &[(u16, &[u8])]) -> Document {
    let body: Blocks = table::assemble(vec![
        mark("a", &[&[(P_IN_TABLE, &[1u8][..])][..], cell].concat(), true),
        mark("b", &[&[(P_IN_TABLE, &[1u8][..]), (P_TTP, &[1u8][..])][..], ttp].concat(), true),
        mark("tail", tail, false),
    ]);
    let mut doc = Document::default();
    doc.body = body;
    doc
}

fn table(doc: &Document) -> &wordcraft_doc::table::Table {
    doc.body.iter().find_map(|b| b.as_table()).expect("table")
}

#[test]
fn tap_table_sprms_alone_float_the_table() {
    let doc = doc(
        &[],
        &[
            // pcVert = 2 (paragraph), pcHorz = 2 (page).
            (T_PC, &[0xA0]),
            (T_DXA_ABS, &1440i16.to_le_bytes()),
            (T_DYA_ABS, &(-12i16).to_le_bytes()),
            (T_DXA_FROM_TEXT, &100i16.to_le_bytes()),
            (T_DYA_FROM_TEXT, &40i16.to_le_bytes()),
            (T_DXA_FROM_TEXT_RIGHT, &200i16.to_le_bytes()),
            (T_DYA_FROM_TEXT_BOTTOM, &60i16.to_le_bytes()),
            (T_F_NO_ALLOW_OVERLAP, &[1]),
        ],
        &[],
    );
    let f = table(&doc).props.float.expect("the table floats");
    assert_eq!(f.v_rel, Anchor::Paragraph);
    assert_eq!(f.h_rel, Anchor::Page);
    assert_eq!(f.h_align, None);
    assert!((f.x - 72.0).abs() < 0.01, "x = 1440 twips, got {}", f.x);
    assert_eq!(f.v_align, Some(FloatAlign::End));
    let d = f.dist_from_text();
    assert!((d[0] - 5.0).abs() < 0.01 && (d[1] - 2.0).abs() < 0.01, "left/top, got {d:?}");
    assert!((d[2] - 10.0).abs() < 0.01 && (d[3] - 3.0).abs() < 0.01, "right/bottom, got {d:?}");
    assert!(!f.overlap);
}

#[test]
fn tap_table_sprms_override_the_paragraph_frame() {
    let para: &[(u16, &[u8])] = &[(P_PC, &[0x50]), (P_DXA_ABS, &(-4i16).to_le_bytes()), (P_DYA_ABS, &2000i16.to_le_bytes())];
    // The TAP moves it down and aligns it right; the paragraph's anchors stay.
    let tap: &[(u16, &[u8])] = &[(T_DXA_ABS, &(-8i16).to_le_bytes()), (T_DYA_ABS, &3000i16.to_le_bytes())];
    let doc = doc(para, &[para, tap].concat(), &[]);
    let tbl = table(&doc);
    let f = tbl.props.float.expect("the table floats");
    assert_eq!((f.v_rel, f.h_rel), (Anchor::Page, Anchor::Margin));
    assert_eq!(f.h_align, Some(FloatAlign::End));
    assert!((f.y - 150.0).abs() < 0.01, "y = 3000 twips, got {}", f.y);
    assert!(f.overlap, "overlap is allowed by default");
    for p in tbl.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.blocks).filter_map(|b| b.as_para()) {
        assert!(p.props.frame.is_none(), "cell paragraph {:?} keeps a frame", p.text);
    }
}

#[test]
fn distances_or_overlap_alone_leave_the_table_inline() {
    let doc = doc(&[], &[(T_DXA_FROM_TEXT, &100i16.to_le_bytes()), (T_F_NO_ALLOW_OVERLAP, &[1])], &[]);
    assert!(table(&doc).props.float.is_none());
}

#[test]
fn a_framed_paragraph_after_a_floating_table_keeps_its_frame() {
    let pos: &[(u16, &[u8])] = &[(P_PC, &[0x50]), (P_DYA_ABS, &2000i16.to_le_bytes())];
    let tail: &[(u16, &[u8])] = &[(P_PC, &[0x50]), (P_DYA_ABS, &6000i16.to_le_bytes()), (P_DXA_WIDTH, &4000i16.to_le_bytes())];
    let doc = doc(pos, pos, tail);
    assert!(table(&doc).props.float.is_some());
    let p = doc.body.last().and_then(|b| b.as_para()).expect("tail");
    assert_eq!(p.text, "tail");
    let f = p.props.frame.as_ref().expect("the paragraph after the table keeps its frame");
    assert_eq!(f.v_anchor, "page");
    assert!((f.y - 300.0).abs() < 0.01 && f.w.is_some_and(|w| (w - 200.0).abs() < 0.01), "{f:?}");
}
