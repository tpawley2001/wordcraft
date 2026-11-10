//! Columns dialog › Start new column (Word): This point forward with Start new column puts a
//! New column section break (`w:type nextColumn`, ECMA-376 §17.18.77) at the caret.
use super::*;

fn doc() -> Session {
    Session::new(wordcraft_doc::Document::from_text("one\ntwo\nthree"))
}

fn starts(s: &Session) -> Vec<SectionStart> {
    s.doc.sections().iter().map(|(_, x)| x.start).collect()
}

fn counts(s: &Session) -> Vec<u32> {
    s.doc.sections().iter().map(|(_, x)| x.columns.count).collect()
}

#[test]
fn forward_with_new_column_inserts_a_next_column_break() {
    let mut s = doc();
    s.run("caret.docEnd", &json!({})).unwrap();
    let undo = s.undo_depth();
    s.run("layout.columns", &json!({"count": 2, "apply": "forward", "newColumn": true})).unwrap();
    assert_eq!(counts(&s), vec![1, 2]);
    assert_eq!(s.doc.last_section.start, SectionStart::NextColumn);
    assert_eq!(s.undo_depth(), undo + 1, "one undo step");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(counts(&s), vec![1]);
}

#[test]
fn forward_without_new_column_stays_continuous() {
    for p in [json!({"count": 2, "apply": "forward"}), json!({"count": 2, "apply": "forward", "newColumn": false})] {
        let mut s = doc();
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("layout.columns", &p).unwrap();
        assert_eq!(s.doc.last_section.start, SectionStart::Continuous, "{p}");
    }
}

#[test]
fn new_column_is_ignored_unless_forward() {
    for apply in ["document", "section"] {
        let mut s = doc();
        s.run("layout.columns", &json!({"count": 3, "apply": apply, "newColumn": true})).unwrap();
        assert_eq!(counts(&s), vec![3], "{apply}: no break inserted");
        assert_eq!(starts(&s), vec![SectionStart::NextPage], "{apply}: start unchanged");
    }
}

#[test]
fn break_kind_next_column() {
    let mut s = doc();
    s.run("caret.docEnd", &json!({})).unwrap();
    s.run("layout.break", &json!({"kind": "nextColumn"})).unwrap();
    assert_eq!(s.doc.sections().len(), 2);
    assert_eq!(s.doc.last_section.start, SectionStart::NextColumn);
    // Still refused for nonsense kinds.
    assert!(s.run("layout.break", &json!({"kind": "newColumnPlease"})).is_err());
}
