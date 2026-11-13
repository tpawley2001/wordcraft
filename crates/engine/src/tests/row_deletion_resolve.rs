//! Accepting and rejecting a table row that is itself a tracked deletion (`RowProps::del`,
//! `w:trPr/w:del`): rejecting keeps the row (shown again when deletions are hidden), accepting
//! removes it, and a table whose rows were all deleted goes with its last row.
use crate::Session;
use serde_json::json;
use wordcraft_doc::{Block, Document, Paragraph, Path, Pos, Revision, RevisionKind, StoryRef, Table, para_block};

/// "BEFORE", a 2 x 2 table whose rows marked in `deleted` are tracked deletions, "AFTER".
fn session(deleted: [bool; 2]) -> Session {
    let mut d = Document::new();
    d.revisions.push(Revision { kind: RevisionKind::Delete, author: "Ana".into(), ..Default::default() });
    let mut t = Table::new(2, 2, 300.0);
    for (ri, (row, del)) in t.rows.iter_mut().zip(deleted).enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.blocks = vec![para_block(Paragraph::with_text(&format!("R{ri}C{ci}"), Default::default()))];
        }
        row.props.del = del.then_some(0);
    }
    d.body = vec![para_block(Paragraph::with_text("BEFORE", Default::default())), std::sync::Arc::new(Block::Table(t)), para_block(Paragraph::with_text("AFTER", Default::default()))];
    Session::new(d)
}

fn table(s: &Session) -> Option<&Table> {
    match s.doc.body.get(1).map(|b| &**b) {
        Some(Block::Table(t)) => Some(t),
        _ => None,
    }
}

/// Whether row `r`'s first cell is laid out with deletions hidden.
fn row_shown(s: &mut Session, r: u32) -> bool {
    s.view.show_markup = false;
    let l = s.layout();
    l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, r, 0, 0]), off: 0 }).is_some()
}

#[test]
fn reject_all_keeps_a_deleted_row_and_shows_it() {
    let mut s = session([false, true]);
    s.run("review.rejectAll", &json!({})).unwrap();
    let t = table(&s).unwrap();
    assert_eq!(t.rows.len(), 2);
    assert!(t.rows.iter().all(|r| r.props.del.is_none()));
    assert!(row_shown(&mut s, 1), "the row is shown again with deletions hidden");
}

#[test]
fn accept_all_removes_a_deleted_row() {
    let mut s = session([false, true]);
    s.run("review.acceptAll", &json!({})).unwrap();
    let t = table(&s).unwrap();
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].cells[0].blocks.len(), 1);
    assert!(t.rows[0].props.del.is_none());
    assert_eq!(s.doc.para(StoryRef::Body, &Path(vec![1, 0, 0, 0])).map(|p| p.plain_text()), Some("R0C0".to_string()));
}

#[test]
fn accept_all_removes_a_table_of_deleted_rows() {
    let mut s = session([true, true]);
    s.run("review.acceptAll", &json!({})).unwrap();
    assert!(s.doc.body.iter().all(|b| matches!(&**b, Block::Para(_))), "no table is left");
    let texts: Vec<String> = s.doc.body.iter().filter_map(|b| if let Block::Para(p) = &**b { Some(p.plain_text()) } else { None }).collect();
    assert_eq!(texts, ["BEFORE", "AFTER"]);
}

#[test]
fn one_row_deletion_is_accepted_or_rejected_at_the_caret() {
    let at = |r: u32, c: u32| crate::Selection::caret(Pos { story: StoryRef::Body, path: Path(vec![1, r, c, 0]), off: 1 });
    let mut s = session([false, true]);
    s.sel = at(1, 0);
    s.run("review.reject", &json!({})).unwrap();
    let t = table(&s).unwrap();
    assert_eq!((t.rows.len(), t.rows[1].props.del), (2, None));

    let mut s = session([false, true]);
    s.sel = at(1, 0);
    s.run("review.accept", &json!({})).unwrap();
    assert_eq!(table(&s).unwrap().rows.len(), 1);

    // Accepting the last row's deletion removes the table too.
    let mut s = session([true, true]);
    s.sel = at(0, 1);
    s.run("review.accept", &json!({})).unwrap();
    assert_eq!(table(&s).unwrap().rows.len(), 1);
    s.sel = at(0, 1);
    s.run("review.accept", &json!({})).unwrap();
    assert!(table(&s).is_none(), "no table is left");
    assert!(s.doc.body.iter().all(|b| matches!(&**b, Block::Para(_))));
}

#[test]
fn undo_restores_the_row_deletion() {
    for cmd in ["review.acceptAll", "review.rejectAll"] {
        let mut s = session([false, true]);
        s.run(cmd, &json!({})).unwrap();
        s.run("edit.undo", &json!({})).unwrap();
        let t = table(&s).unwrap();
        assert_eq!((t.rows.len(), t.rows[1].props.del), (2, Some(0)), "{cmd}");
        assert!(!row_shown(&mut s, 1), "{cmd}: the row is left out again");
    }
}
