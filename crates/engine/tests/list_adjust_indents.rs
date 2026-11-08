//! Adjust List Indents (`list.adjustIndents`): Word's right-click dialog changes the caret's
//! level of the caret's list (number position, text indent, what follows the number, the tab
//! stop after it) for every item of that list at that level.

use serde_json::{Value, json};
use wordcraft_doc::numbering::LevelSuffix;
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{Document, Path, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Three numbered paragraphs of one list; the second is at level 2 (index 1).
fn fixture() -> (Session, u32) {
    let mut s = Session::new(Document::from_text("One\nTwo\nThree\nPlain\n"));
    s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(2, 0) };
    run(&mut s, "para.numbering", json!({}));
    let num = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap().props.numbering.unwrap().num;
    assert_ne!(num, 0);
    s.doc.para_mut(StoryRef::Body, &Path::top(1)).unwrap().props.numbering = Some(NumRef { num, level: 1 });
    s.sel = Selection::caret(Pos::body(0, 0));
    (s, num)
}

#[test]
fn changes_the_caret_level_only() {
    let (mut s, num) = fixture();
    let level2_before = s.doc.numbering.level(num, 1).unwrap().clone();
    let r = run(&mut s, "list.adjustIndents", json!({"numberPosition": 18.0, "textIndent": 54.0, "follow": "space", "tabAt": null}));
    assert_eq!(r["num"], json!(num));
    assert_eq!(r["level"], json!(0));
    let l = s.doc.numbering.level(num, 0).unwrap();
    assert_eq!((l.indent, l.hanging, l.suffix, l.tab), (54.0, 36.0, LevelSuffix::Space, None));
    assert_eq!(s.doc.numbering.level(num, 1).unwrap(), &level2_before);
}

#[test]
fn number_right_of_text_gives_negative_hanging_and_tab_stop() {
    let (mut s, num) = fixture();
    run(&mut s, "list.adjustIndents", json!({"numberPosition": 36.0, "textIndent": 18.0, "follow": "tab", "tabAt": 72.0}));
    let l = s.doc.numbering.level(num, 0).unwrap();
    assert_eq!((l.indent, l.hanging, l.suffix, l.tab), (18.0, -18.0, LevelSuffix::Tab, Some(72.0)));
}

#[test]
fn omitted_fields_keep_the_level_values() {
    let (mut s, num) = fixture();
    let before = s.doc.numbering.level(num, 0).unwrap().clone();
    run(&mut s, "list.adjustIndents", json!({"textIndent": before.indent + 9.0}));
    let l = s.doc.numbering.level(num, 0).unwrap();
    // Number position (indent - hanging) stays where it was; only the text moves.
    assert_eq!(l.indent, before.indent + 9.0);
    assert_eq!(l.indent - l.hanging, before.indent - before.hanging);
    assert_eq!((l.suffix, l.tab, &l.text, l.format), (before.suffix, before.tab, &before.text, before.format));
}

#[test]
fn clears_direct_indents_on_that_level_only() {
    let (mut s, num) = fixture();
    for i in 0..3 {
        let p = s.doc.para_mut(StoryRef::Body, &Path::top(i)).unwrap();
        p.props.indent_left = Some(100.0);
        p.props.indent_first = Some(-10.0);
    }
    run(&mut s, "list.adjustIndents", json!({"numberPosition": 0.0, "textIndent": 36.0}));
    for (i, cleared) in [(0, true), (1, false), (2, true)] {
        let p = &s.doc.para(StoryRef::Body, &Path::top(i)).unwrap().props;
        assert_eq!(p.numbering.unwrap().num, num);
        if cleared {
            assert_eq!((p.indent_left, p.indent_first), (None, None), "paragraph {i}");
        } else {
            assert_eq!((p.indent_left, p.indent_first), (Some(100.0), Some(-10.0)), "paragraph {i}");
        }
    }
}

#[test]
fn one_undo_restores_everything() {
    let (mut s, _) = fixture();
    s.doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap().props.indent_left = Some(100.0);
    let before = s.doc.clone();
    run(&mut s, "list.adjustIndents", json!({"numberPosition": 9.0, "textIndent": 27.0, "follow": "nothing"}));
    assert_ne!(*s.doc.numbering, *before.numbering);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(*s.doc.numbering, *before.numbering);
    assert_eq!(s.doc.para(StoryRef::Body, &Path::top(0)).unwrap().props, before.para(StoryRef::Body, &Path::top(0)).unwrap().props);
}

#[test]
fn caret_outside_a_list_fails() {
    let (mut s, num) = fixture();
    let before = s.doc.numbering.level(num, 0).unwrap().clone();
    s.sel = Selection::caret(Pos::body(3, 0));
    assert!(s.run("list.adjustIndents", &json!({"textIndent": 72.0})).is_err());
    assert_eq!(s.doc.numbering.level(num, 0).unwrap(), &before);
}
