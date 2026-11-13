//! A table row whose end-of-row mark is hidden text ([MS-DOC] sprmCFVanish 0x083C in the row
//! mark's character properties) reads as `RowProps::hidden`, as `w:trPr/w:hidden` does. Hidden
//! cell marks alone don't set it: they stay with their paragraphs.

use wordcraft_doc::Document;

use super::{RichSpec, grpprl, read, rich_doc, t_def_table};

/// Two rows of two cells, each cell ended by its own mark and each row by an empty row-end
/// mark: "a" 0, mark 1, "b" 2, mark 3, row end 4; "c" 5, mark 6, "d" 7, mark 8, row end 9; "tail" 10.
const TEXT: &str = "a\u{7}b\u{7}\u{7}c\u{7}d\u{7}\u{7}tail\r";
const ROW2_END: u32 = 9;

fn vanish() -> Vec<u8> {
    grpprl(&[(0x083C, &[0x01])])
}

fn open(chpx_runs: Vec<(u32, Vec<u8>)>) -> Document {
    let in_tbl = grpprl(&[(0x2416, &[0x01])]);
    let ttp: Vec<u8> = [grpprl(&[(0x2416, &[0x01]), (0x2417, &[0x01])]), t_def_table(&[0, 2880, 5760])].concat();
    let spec = RichSpec {
        text: TEXT,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs,
        papx_entries: vec![
            (0, 0, in_tbl.clone()),
            (2, 0, in_tbl.clone()),
            (4, 0, ttp.clone()),
            (5, 0, in_tbl.clone()),
            (7, 0, in_tbl),
            (ROW2_END, 0, ttp),
            (ROW2_END + 1, 0, Vec::new()),
        ],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn rows_hidden(doc: &Document) -> Vec<bool> {
    let table = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(table.rows.len(), 2, "two rows");
    assert!(table.rows.iter().all(|r| r.cells.len() == 2), "two cells a row");
    table.rows.iter().map(|r| r.props.hidden).collect()
}

#[test]
fn a_hidden_row_end_mark_hides_the_row() {
    // Row 2's text, cell marks and row-end mark hidden.
    let doc = open(vec![(0, Vec::new()), (5, vanish()), (ROW2_END + 1, Vec::new())]);
    assert_eq!(rows_hidden(&doc), vec![false, true]);
}

#[test]
fn hidden_cell_marks_alone_leave_the_row() {
    // Row 2's text and cell marks hidden, its row-end mark not.
    let doc = open(vec![(0, Vec::new()), (5, vanish()), (ROW2_END, Vec::new())]);
    assert_eq!(rows_hidden(&doc), vec![false, false]);
    let table = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    let cell = table.rows.get(1).and_then(|r| r.cells.first()).and_then(|c| c.blocks.first()).and_then(|b| b.as_para()).expect("cell paragraph");
    assert_eq!(cell.mark.hidden, Some(true), "the cell mark itself is hidden");
}
