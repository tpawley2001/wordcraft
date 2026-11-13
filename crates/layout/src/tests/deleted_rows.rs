//! Table rows that are themselves tracked deletions (`w:trPr/w:del`, `RowProps::del`) laid out
//! with deletions hidden. Measured in Microsoft 365 Word's PDFs (no markup) of small documents:
//! "BEFORE", a two-column table with single 0.5 pt borders and two rows, "AFTER" (Times New
//! Roman 12 pt). Only the row's own mark hides a row: deleted text and deleted cell marks leave an
//! empty row, a deleted row is gone with its text, and a table of deleted rows takes no room.
use crate::*;
use wordcraft_doc::props::{Border, Borders, LineSpacing};
use wordcraft_doc::{Block, Pos, Table, para_block};

fn chr() -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, chr()) };
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p.mark = chr();
    p
}

/// How a row of the fixture is marked.
#[derive(Clone, Copy)]
enum Mark {
    Plain,
    /// The cells' text is a tracked deletion.
    Text,
    /// The cells' text and paragraph marks.
    TextAndMarks,
    /// The cells' text and marks, and the row itself.
    All,
    /// Only the row itself; its text isn't marked.
    Row,
}

fn cell_para(text: &str, m: Mark) -> Paragraph {
    let mut p = para(text);
    if matches!(m, Mark::Text | Mark::TextAndMarks | Mark::All) {
        p = Paragraph::with_text(text, CharProps { del: Some(0), ..chr() });
        p.props = para("").props;
        p.mark = chr();
    }
    if matches!(m, Mark::TextAndMarks | Mark::All) {
        p.mark = CharProps { del: Some(0), ..chr() };
    }
    p
}

/// "BEFORE", a 2 x 2 table whose rows are marked `rows`, "AFTER".
fn doc(rows: [Mark; 2]) -> Document {
    let mut d = Document::new();
    d.revisions.push(wordcraft_doc::Revision { kind: wordcraft_doc::RevisionKind::Delete, ..Default::default() });
    let mut t = Table::new(2, 2, 300.0);
    t.props.style = None;
    t.props.borders = Some(Borders::all(Border::single(0.5)));
    for (ri, (row, m)) in t.rows.iter_mut().zip(rows).enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.blocks = vec![para_block(cell_para(&format!("R{ri}C{ci}"), m))];
        }
        if matches!(m, Mark::All | Mark::Row) {
            row.props.del = Some(0);
        }
    }
    d.body = vec![para_block(para("BEFORE")), std::sync::Arc::new(Block::Table(t)), para_block(para("AFTER"))];
    d
}

fn lay_out(d: &Document, hide: bool) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { hide_deleted: hide, ..Default::default() })
}

fn top(l: &DocLayout, block: usize) -> f32 {
    l.caret(&Pos::body(block, 0)).unwrap().top
}

/// The top of AFTER with the table's rows marked `rows`.
fn after(rows: [Mark; 2], hide: bool) -> f32 {
    top(&lay_out(&doc(rows), hide), 2)
}

/// Table border rules placed on the first page.
fn rules(l: &DocLayout) -> usize {
    l.pages[0].items.iter().filter(|it| matches!(it, Placed::Rule { .. })).count()
}

/// r0's row pitch: from the top of its first row to the top of its second.
fn row_pitch() -> f32 {
    let d = doc([Mark::Plain; 2]);
    let l = lay_out(&d, true);
    let cell = |r: u32| l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, r, 0, 0]), off: 0 }).unwrap().top;
    cell(1) - cell(0)
}

#[test]
fn deleted_text_and_marks_leave_empty_rows() {
    let r0 = after([Mark::Plain; 2], true);
    for (name, rows) in
        [("r1", [Mark::Plain, Mark::TextAndMarks]), ("r4", [Mark::Plain, Mark::Text]), ("r6", [Mark::TextAndMarks, Mark::TextAndMarks])]
    {
        let at = after(rows, true);
        assert!((at - r0).abs() < 0.5, "{name}: AFTER at {at}, r0 at {r0}");
    }
}

#[test]
fn a_deleted_row_is_left_out() {
    let r0 = after([Mark::Plain; 2], true);
    let pitch = row_pitch();
    assert!(pitch > 10.0, "a row pitch {pitch}");
    for (name, rows) in [("r2", [Mark::Plain, Mark::All]), ("r3", [Mark::Plain, Mark::Row])] {
        let at = after(rows, true);
        assert!((at - (r0 - pitch)).abs() < 0.5, "{name}: AFTER at {at}, r0 at {r0}, a row {pitch}");
    }
}

#[test]
fn a_table_of_deleted_rows_takes_no_room() {
    let d = doc([Mark::All; 2]);
    let l = lay_out(&d, true);
    // AFTER follows BEFORE as if there were no table.
    let mut plain = Document::new();
    plain.body = vec![para_block(para("BEFORE")), para_block(para("AFTER"))];
    let line = top(&lay_out(&plain, true), 1) - top(&lay_out(&plain, true), 0);
    let (before, at) = (top(&l, 0), top(&l, 2));
    assert!((at - (before + line)).abs() < 0.5, "r5: AFTER at {at}, BEFORE at {before}, a line {line}");
    assert_eq!(rules(&l), 0, "no table border is placed");
    // A table shown has its borders.
    assert!(rules(&lay_out(&doc([Mark::Plain; 2]), true)) > 0);
}

#[test]
fn deleted_rows_shown_with_deletions_shown() {
    let r0 = after([Mark::Plain; 2], false);
    for (name, rows) in [("r2", [Mark::Plain, Mark::All]), ("r3", [Mark::Plain, Mark::Row]), ("r5", [Mark::All, Mark::All])] {
        let at = after(rows, false);
        assert!((at - r0).abs() < 0.01, "{name}: AFTER at {at}, r0 at {r0}");
    }
}

#[test]
fn cells_after_a_deleted_row_keep_their_document_rows() {
    // A three-row table whose middle row is left out: the last row's cell is still row 2.
    let mut d = doc([Mark::Plain, Mark::Row]);
    if let Some(Block::Table(t)) = d.body.get_mut(1).map(std::sync::Arc::make_mut) {
        let mut last = t.rows[0].clone();
        last.cells[0].blocks = vec![para_block(para("LAST"))];
        t.rows.push(last);
    }
    let l = lay_out(&d, true);
    let cell = |r: u32| l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, r, 0, 0]), off: 0 }).map(|c| c.top);
    let (first, last) = (cell(0).unwrap(), cell(2).unwrap());
    assert!((last - first - row_pitch()).abs() < 0.5, "row 2 right under row 0: {first} {last}");
}

#[test]
fn a_merge_starting_in_a_deleted_row_starts_at_the_next_row() {
    use wordcraft_doc::props::VMerge;
    let mut d = doc([Mark::Row, Mark::Plain]);
    if let Some(Block::Table(t)) = d.body.get_mut(1).map(std::sync::Arc::make_mut) {
        t.rows[0].cells[0].props.vmerge = VMerge::Restart;
        t.rows[1].cells[0].props.vmerge = VMerge::Continue;
    }
    let l = lay_out(&d, true);
    // The continuing cell is drawn as a cell of its own: its text is laid out.
    assert!(l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, 1, 0, 0]), off: 0 }).is_some());
    let shown = after([Mark::Row, Mark::Plain], true);
    assert!((top(&l, 2) - shown).abs() < 0.5);
}
