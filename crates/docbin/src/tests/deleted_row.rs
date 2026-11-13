//! A table row deleted with tracking ([MS-DOC] §2.6.1 sprmCFRMarkDel): the row's end mark (the
//! row-end 0x07 whose paragraph carries the row's table properties) holds the deletion revision
//! in its character properties, as a deleted paragraph mark does. The row reads as a tracked
//! deletion (`RowProps::del`, like `w:trPr/w:del`); deleted cell marks alone don't delete it.

use wordcraft_doc::{Document, RevisionKind};

use super::{RichExtra, RichSpec, dttm, grpprl, read, rich_doc_with, t_def_table};

/// Two rows of two cells, each cell ended by its own mark and each row by an empty row-end
/// mark: "a" 0, mark 1, "b" 2, mark 3, row end 4; "c" 5, mark 6, "d" 7, mark 8, row end 9; "tail" 10.
const TEXT: &str = "a\u{7}b\u{7}\u{7}c\u{7}d\u{7}\u{7}tail\r";
const ROW2_END: u32 = 9;

/// sprmCFRMarkDel on, author 1 ("Bob"), with a date.
fn deleted() -> Vec<u8> {
    grpprl(&[(0x0800, &[0x01]), (0x4863, &[0x01, 0x00]), (0x6864, &dttm(2015, 5, 25, 13, 47))])
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
    let extra = RichExtra { rmark_authors: vec!["Alice", "Bob"], ..Default::default() };
    read(&rich_doc_with(&spec, &extra)).expect("opens")
}

fn rows_del(doc: &Document) -> Vec<Option<(RevisionKind, String, String)>> {
    let table = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(table.rows.len(), 2, "two rows");
    assert!(table.rows.iter().all(|r| r.cells.len() == 2), "two cells a row");
    table
        .rows
        .iter()
        .map(|r| r.props.del.and_then(|i| doc.revisions.get(i as usize)).map(|r| (r.kind, r.author.clone(), r.date.clone())))
        .collect()
}

#[test]
fn a_deleted_row_end_mark_deletes_the_row() {
    let doc = open(vec![(0, Vec::new()), (ROW2_END, deleted()), (ROW2_END + 1, Vec::new())]);
    assert_eq!(rows_del(&doc), vec![None, Some((RevisionKind::Delete, "Bob".to_string(), "2015-05-25T13:47:00Z".to_string()))]);
}

#[test]
fn deleted_cell_marks_alone_leave_the_row() {
    // Row 2's cell text and cell marks deleted, its row-end mark not.
    let doc = open(vec![(0, Vec::new()), (5, deleted()), (ROW2_END, Vec::new())]);
    assert_eq!(rows_del(&doc), vec![None, None]);
    let table = doc.body.iter().find_map(|b| b.as_table()).expect("table");
    let cell = table.rows.get(1).and_then(|r| r.cells.first()).and_then(|c| c.blocks.first()).and_then(|b| b.as_para()).expect("cell paragraph");
    assert!(cell.mark.del.is_some(), "the cell mark itself is deleted");
}

#[test]
fn a_document_without_deletions_has_no_deleted_rows() {
    let doc = open(vec![(0, Vec::new())]);
    assert_eq!(rows_del(&doc), vec![None, None]);
}
