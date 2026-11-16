use super::*;

/// A page break at the end of a paragraph moves the caret to a new paragraph, so the caret sits
/// on the page the break starts; in the middle of one it stays in the paragraph, and in a table
/// cell (where layout draws no page break) no paragraph is added. One undo takes it all back.
#[test]
fn page_break_at_paragraph_end_starts_a_new_paragraph() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Hello"}));
    run(&mut s, "text.pageBreak", json!({}));
    assert_eq!(text(&s), "Hello\u{c}\n");
    assert_eq!(s.sel.focus.path.0, vec![1], "the caret is in the new paragraph");
    assert!(s.undo());
    assert_eq!(text(&s), "Hello");

    let mut s = Session::new(wordcraft_doc::Document::from_text("Hello"));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "caret.right", json!({}));
    run(&mut s, "insert.pageBreak", json!({}));
    assert_eq!(text(&s), "H\u{c}ello", "mid-paragraph: no new paragraph");

    let mut s = Session::new(wordcraft_doc::Document::from_text("Hello world"));
    run(&mut s, "select.text", json!({"text": "world"}));
    run(&mut s, "insert.pageBreak", json!({}));
    assert_eq!(text(&s), "Hello \u{c}\n", "a selection at the end is replaced, then the paragraph ends");

    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "text.pageBreak", json!({}));
    assert_eq!(s.sel.focus.path.0.last(), Some(&0), "in a table cell the caret stays in the cell's paragraph");
}
