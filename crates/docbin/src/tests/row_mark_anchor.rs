//! `.doc` table rows whose row-end mark ([MS-DOC] 2.4.3: the last 0x07 of a row, with
//! `sprmPFTtp` and the row's `sprmTDefTable`) also holds the anchors of floating objects Word
//! stores at the end of the row. The mark still only ends the row: it makes no extra cell, and the
//! objects move to the end of the row's last cell. Every fixture is a synthetic stream built here.

use wordcraft_doc::Document;
use wordcraft_doc::para::{InlineObject, ShapeKind};
use wordcraft_doc::table::Table;

use super::drawing::{art_content, fopt, fsp, sp, spa, spa_flags};
use super::{StructSpec, grpprl, plc, read, struct_doc, structured_spec, t_def_table};

/// Cell edges in twips: a narrow label column and a wide text column.
const EDGES: &[i16] = &[0, 1112, 10713];

fn in_table() -> Vec<u8> {
    grpprl(&[(0x2416, &[0x01])])
}

/// The row-end mark's PAPX: in table, row end (`sprmPFTtp`), the row's cell edges.
fn row_end() -> Vec<u8> {
    [grpprl(&[(0x2416, &[0x01]), (0x2417, &[0x01])]), t_def_table(EDGES)].concat()
}

/// A floating red ellipse (square wrap) with the given shape id.
fn ellipse(spid: u32) -> Vec<u8> {
    sp(&[fsp(3, spid, 0xA00), fopt(&[(0x0181, 0x0000_00FF)], &[])])
}

/// A table of rows `A`/`B` whose row-end marks hold `ends[r]` before the mark, then a tail
/// paragraph. Each U+0008 in a row end anchors one floating ellipse, the n-th n inches wide.
fn doc(ends: &[&str]) -> Document {
    let mut text = String::new();
    let mut papx = Vec::new();
    let mut anchors = Vec::new();
    for end in ends {
        papx.push((text.chars().count() as u32, 0, in_table()));
        text.push_str("A\u{7}B\u{7}");
        papx.push((text.chars().count() as u32, 0, row_end()));
        for c in end.chars() {
            if c == '\u{8}' {
                anchors.push(text.chars().count() as u32);
            }
            text.push(c);
        }
        text.push('\u{7}');
    }
    papx.push((text.chars().count() as u32, 0, Vec::new()));
    text.push_str("tail\r");
    let text: &'static str = Box::leak(text.into_boxed_str());
    let ccp = text.chars().count() as u32;
    let spids: Vec<u32> = (0..anchors.len() as u32).map(|i| 1026 + i).collect();
    let shapes: Vec<Vec<u8>> = spids.iter().map(|s| ellipse(*s)).collect();
    // The n-th anchor's shape is n inches wide, so the order shows in the imported widths.
    let spas: Vec<Vec<u8>> = spids.iter().zip(1i32..).map(|(s, n)| spa(*s, [0, 0, 1440 * n, 720], spa_flags(2, 2, 2, false))).collect();
    let mut extra_pairs = Vec::new();
    if !anchors.is_empty() {
        let cps: Vec<u32> = anchors.iter().copied().chain([ccp - 1]).collect();
        extra_pairs = vec![(40, plc(&cps, &spas.concat())), (50, art_content(&[], &shapes))];
    }
    let spec = StructSpec { text, papx_entries: papx, extra_pairs, ..structured_spec(text, ccp) };
    read(&struct_doc(&spec)).expect("opens")
}

fn table(doc: &Document) -> &Table {
    doc.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// The text of each cell's paragraphs, per row.
fn texts(t: &Table) -> Vec<Vec<Vec<String>>> {
    t.rows.iter().map(|r| r.cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()).collect()).collect()
}

fn object_count(t: &Table) -> usize {
    t.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.blocks).filter_map(|b| b.as_para()).map(|p| p.objects.len()).sum()
}

/// The objects on the last paragraph of a row's second cell.
fn second_cell_objects(t: &Table, row: usize) -> &[InlineObject] {
    let cell = t.rows.get(row).and_then(|r| r.cells.get(1)).expect("a second cell");
    let p = cell.blocks.last().and_then(|b| b.as_para()).expect("ends with a paragraph");
    assert_eq!(p.text.trim_end_matches('\u{FFFC}'), "B", "{p:?}");
    &p.objects
}

fn assert_two_columns(t: &Table) {
    assert_eq!(t.grid.len(), 2, "{:?}", t.grid);
    assert!(t.grid.iter().zip([55.6, 480.05]).all(|(g, w)| (g - w).abs() < 0.01), "{:?}", t.grid);
    for r in &t.rows {
        assert_eq!(r.cells.len(), 2, "{r:?}");
        assert!(r.cells.iter().all(|c| c.props.width.is_some()), "{r:?}");
    }
}

#[test]
fn an_anchor_in_the_row_end_mark_makes_no_extra_cell() {
    let d = doc(&["\u{8}"]);
    let t = table(&d);
    assert_two_columns(t);
    let objs = second_cell_objects(t, 0);
    assert_eq!(objs.len(), 1);
    assert!(objs.iter().all(InlineObject::is_floating), "{objs:?}");
    assert!(matches!(objs.first(), Some(InlineObject::Shape { kind: ShapeKind::Ellipse, .. })), "{objs:?}");
    assert_eq!(object_count(t), 1);
}

#[test]
fn two_anchors_in_the_row_end_mark_move_in_stored_order() {
    let d = doc(&["\u{8}\u{8}"]);
    let t = table(&d);
    assert_two_columns(t);
    let objs = second_cell_objects(t, 0);
    assert_eq!(objs.len(), 2);
    let w: Vec<f32> = objs.iter().filter_map(|o| if let InlineObject::Shape { w, .. } = o { Some(*w) } else { None }).collect();
    assert_eq!(w, [72.0, 144.0]);
}

#[test]
fn an_empty_row_end_mark_is_unchanged() {
    let d = doc(&[""]);
    let t = table(&d);
    assert_two_columns(t);
    assert_eq!(texts(t), [[vec!["A".to_string()], vec!["B".to_string()]]]);
    assert_eq!(object_count(t), 0);
}

#[test]
fn a_row_end_mark_with_a_real_character_still_ends_a_cell() {
    let d = doc(&["x"]);
    let t = table(&d);
    // Today's shape for this malformed row: the mark ends a third cell of its own.
    assert_eq!(texts(t), [[vec!["A".to_string()], vec!["B".to_string()], vec!["x".to_string()]]]);
}

#[test]
fn only_the_last_rows_anchor_moves_into_its_last_cell() {
    let d = doc(&["", "", "\u{8}"]);
    let t = table(&d);
    assert_eq!(t.rows.len(), 3);
    assert_two_columns(t);
    assert!(second_cell_objects(t, 0).is_empty());
    assert!(second_cell_objects(t, 1).is_empty());
    assert_eq!(second_cell_objects(t, 2).len(), 1);
    assert_eq!(object_count(t), 1);
}
