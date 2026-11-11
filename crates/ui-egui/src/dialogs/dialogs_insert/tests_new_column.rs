//! Columns dialog › Start new column: enabled only with Apply to: This point forward, where OK
//! inserts a New column section break instead of a Continuous one.
use super::*;
use wordcraft_doc::section::SectionStart;

fn app() -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("one\ntwo")), Default::default())
}

#[test]
fn start_new_column_is_enabled_only_for_this_point_forward() {
    let a = app();
    let mut f = ColumnsForm::read(&a);
    assert!(!f.new_column, "off on open");
    for (apply, on) in [("document", false), ("section", false), ("selection", false), ("forward", true)] {
        f.apply = apply.into();
        assert_eq!(f.new_column_enabled(), on, "{apply}");
    }
}

#[test]
fn params_send_new_column_only_when_enabled_and_ticked() {
    let a = app();
    let mut f = ColumnsForm::read(&a);
    f.preset("two");
    f.new_column = true;
    f.apply = "document".into();
    assert!(f.params().get("newColumn").is_none(), "greyed out: not sent");
    f.apply = "forward".into();
    assert_eq!(f.params()["newColumn"], json!(true));
    f.new_column = false;
    assert!(f.params().get("newColumn").is_none());
    // Unequal widths send it too.
    f.new_column = true;
    f.preset("left");
    assert!(!f.equal || f.count == 2);
    assert_eq!(f.params()["newColumn"], json!(true));
}

#[test]
fn ok_with_start_new_column_makes_a_next_column_section() {
    let mut a = app();
    a.run("caret.docEnd", json!({})).unwrap();
    let mut f = ColumnsForm::read(&a);
    f.preset("two");
    f.apply = "forward".into();
    f.new_column = true;
    a.run("layout.columns", f.params()).unwrap();
    let secs = a.session.doc.sections();
    assert_eq!(secs.len(), 2);
    assert_eq!(a.session.doc.last_section.start, SectionStart::NextColumn);
    assert_eq!(a.session.doc.last_section.columns.count, 2);
}
