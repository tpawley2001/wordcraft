//! File › Print › Print Selection (`range: "selectedText"`): only the selected content prints,
//! laid out afresh from the top of the first page with the page setup of the section it starts in,
//! and without headers and footers, as in Word. The open document is not touched.
//! (`range: "selection"`, Selected pages, still prints the whole pages the selection touches.)

use serde_json::{Value, json};
use wordcraft_doc::section::HeaderSet;
use wordcraft_doc::{Pos, StoryRef};

use crate::{CmdError, Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn three() -> Session {
    Session::new(wordcraft_doc::Document::from_text("Alpha one\nBravo two\nCharlie three"))
}

/// The print document and its page count for Print Selection, or panic.
fn printed(s: &Session) -> (wordcraft_doc::Document, usize) {
    let (doc, lay) = s
        .print_selection(&json!({"range": "selectedText"}))
        .unwrap_or_else(|e| panic!("print_selection: {e}"))
        .expect("Print Selection gives a document to print");
    (doc, lay.pages.len())
}

fn body_text(d: &wordcraft_doc::Document) -> String {
    d.plain_text(StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
}

#[test]
fn a_selected_paragraph_prints_alone_on_one_page() {
    let mut s = three();
    run(&mut s, "select.text", json!({"text": "Bravo two"}));
    let (doc, pages) = printed(&s);
    assert_eq!(body_text(&doc), "Bravo two");
    assert_eq!(pages, 1);
}

#[test]
fn part_of_a_paragraph_prints_just_that_text() {
    let mut s = three();
    // "two" in "Bravo two".
    s.sel = Selection { anchor: Pos::body(1, 6), focus: Pos::body(1, 9) };
    let (doc, _) = printed(&s);
    assert_eq!(body_text(&doc), "two");
}

#[test]
fn nothing_selected_is_an_error() {
    let mut s = three();
    s.sel = Selection::caret(Pos::body(1, 3));
    match s.print_selection(&json!({"range": "selectedText"})) {
        Err(CmdError::Params(_)) => {}
        other => panic!("a caret alone has no selection to print: {:?}", other.map(|o| o.is_some())),
    }
}

#[test]
fn other_ranges_print_the_document() {
    let mut s = three();
    run(&mut s, "select.text", json!({"text": "Bravo two"}));
    for v in [json!({}), json!({"range": "all"}), json!({"range": "selection"}), json!({"range": "current"})] {
        let r = s.print_selection(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert!(r.is_none(), "{v} is not Print Selection");
    }
}

#[test]
fn headers_and_footers_do_not_print_with_a_selection() {
    let mut s = three();
    run(&mut s, "insert.header", json!({"text": "HEAD"}));
    run(&mut s, "insert.closeHeader", json!({}));
    run(&mut s, "insert.footer", json!({"text": "FOOT"}));
    run(&mut s, "insert.closeHeader", json!({}));
    assert!(s.doc.last_section.headers.default.is_some() && s.doc.last_section.footers.default.is_some(), "setup: header and footer");
    s.sel = Selection::caret(Pos::body(0, 0));
    run(&mut s, "select.text", json!({"text": "Bravo two"}));
    let (doc, _) = printed(&s);
    for (_, sec) in doc.sections() {
        assert_eq!(sec.headers, HeaderSet::default(), "no header prints");
        assert_eq!(sec.footers, HeaderSet::default(), "no footer prints");
    }
    assert_eq!(doc.last_section.headers, HeaderSet::default());
    assert_eq!(doc.last_section.footers, HeaderSet::default());
    assert!(s.doc.last_section.headers.default.is_some(), "the open document keeps its header");
}

#[test]
fn the_selection_keeps_its_sections_page_setup() {
    let mut s = three();
    {
        let sec = &mut s.doc.last_section;
        (sec.page_w, sec.page_h) = (500.0, 700.0);
        (sec.margin_top, sec.margin_bottom, sec.margin_left, sec.margin_right) = (30.0, 40.0, 50.0, 60.0);
    }
    run(&mut s, "select.text", json!({"text": "Charlie three"}));
    let (doc, _) = printed(&s);
    let sec = &doc.last_section;
    assert_eq!((sec.page_w, sec.page_h), (500.0, 700.0));
    assert_eq!((sec.margin_top, sec.margin_bottom, sec.margin_left, sec.margin_right), (30.0, 40.0, 50.0, 60.0));
}

fn long_doc() -> Session {
    let text: Vec<String> = (0..200).map(|i| format!("Word{i}")).collect();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text.join("\n")));
    // Paragraphs 150..=154, well past page 1.
    let end = "Word154".len();
    s.sel = Selection { anchor: Pos::body(150, 0), focus: Pos::body(154, end) };
    s
}

#[test]
fn a_selection_deep_in_a_long_document_prints_on_one_page() {
    let s = long_doc();
    let lay = s.print_layout(&json!({}));
    assert!(lay.pages.len() > 2, "setup: the source runs to several pages");
    let (doc, pages) = printed(&s);
    assert_eq!(body_text(&doc), "Word150\nWord151\nWord152\nWord153\nWord154");
    assert_eq!(pages, 1);
    let (_, plan) = s.print_pdf(&json!({"range": "selectedText"})).unwrap();
    assert_eq!((plan.pages.clone(), plan.chosen, plan.copies), (vec![0], 1, 1));
    let (_, plan) = s.print_pdf(&json!({"range": "selectedText", "copies": 2})).unwrap();
    assert_eq!((plan.pages.clone(), plan.chosen, plan.copies), (vec![0, 0], 1, 2));
}

#[test]
fn selected_pages_still_print_whole_pages() {
    let s = long_doc();
    let (_, plan) = s.print_pdf(&json!({"range": "selection"})).unwrap();
    assert!(plan.pages.iter().all(|p| *p > 0), "Selected pages prints the source's own pages: {:?}", plan.pages);
}

#[test]
fn printing_a_selection_leaves_the_document_alone() {
    let mut s = three();
    run(&mut s, "select.text", json!({"text": "Bravo two"}));
    let (text, sel, depth, dirty) = (s.doc.plain_text(StoryRef::Body), s.sel.clone(), s.undo_depth(), s.dirty);
    let _ = printed(&s);
    s.print_pdf(&json!({"range": "selectedText"})).unwrap();
    assert_eq!(s.doc.plain_text(StoryRef::Body), text);
    assert_eq!(s.sel, sel);
    assert_eq!((s.undo_depth(), s.dirty), (depth, dirty));
}
