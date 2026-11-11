//! Header & Footer › Insert Alignment Tab dialog: opens by name and from the button, starts at
//! Left / Margin / None, and OK inserts the positional tab it describes.
use super::*;
use wordcraft_doc::para::{InlineObject, PTabAlign, PTabBase};
use wordcraft_doc::props::TabLeader;
use wordcraft_doc::{Document, Pos};

fn app() -> WordApp {
    let mut a = WordApp::new(wordcraft_engine::Session::new(Document::from_text("AB")), Default::default());
    a.run("caret.set", json!({"pos": {"block": 0, "off": 1}})).unwrap();
    a
}

#[test]
fn opens_by_name_and_from_the_button_with_word_defaults() {
    let mut a = app();
    let d = crate::dialogs::Dialog::open("alignmentTab", &mut a).expect("an Alignment Tab dialog");
    assert_eq!(d.name(), "alignmentTab");
    assert_eq!(AlignmentTabForm::default().params(), json!({"align": "left", "relativeTo": "margin", "leader": "none"}));
    // The button sends no settings: the dialog asks for them, and nothing is inserted yet.
    assert_eq!(a.run("insert.alignmentTab", json!({})).unwrap(), json!({"pending": "alignmentTab"}));
    assert_eq!(a.dialog.as_ref().map(crate::dialogs::Dialog::name), Some("alignmentTab"));
    assert_eq!(a.session.doc.plain_text(wordcraft_doc::StoryRef::Body), "AB");
}

#[test]
fn ok_inserts_the_chosen_positional_tab() {
    let mut a = app();
    let f = AlignmentTabForm { align: "right", relative_to: "indent", leader: "middleDot" };
    alignment_tab_ok(&mut a, &f);
    let pos = Pos::body(0, 1);
    let got = a.session.doc.para_at(&pos).and_then(|p| p.object_at(pos.off)).cloned();
    assert_eq!(got, Some(InlineObject::PositionalTab { align: PTabAlign::Right, relative_to: PTabBase::Indent, leader: TabLeader::MiddleDot }));
}

#[test]
fn every_leader_is_one_the_command_takes() {
    for (value, _) in ALIGNMENT_TAB_LEADERS {
        let mut a = app();
        let f = AlignmentTabForm { leader: value, ..Default::default() };
        alignment_tab_ok(&mut a, &f);
        assert_eq!(a.session.doc.plain_text(wordcraft_doc::StoryRef::Body), "A\tB", "{value}");
    }
}
