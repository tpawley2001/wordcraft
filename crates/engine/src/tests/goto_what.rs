//! Go To (F5): Word's "Go to what" kinds beyond Page and Bookmark. `edit.goto {"what", "n"?, "by"?,
//! "name"?}`: `n` is the 1-based object number (clamped), `by` steps from the selection, `name` picks a
//! comment's reviewer or a field's type. Body only, like Select Browse Object.

use serde_json::{Value, json};
use wordcraft_doc::para::{InlineObject, NoteKind, OBJ};
use wordcraft_doc::{Pos, StoryRef};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id} {v}: {e}"))
}

fn doc(text: &str) -> Session {
    Session::new(wordcraft_doc::Document::from_text(text))
}

/// The body paragraph index the caret is in.
fn caret_para(s: &Session) -> usize {
    assert_eq!(s.sel.focus.story, StoryRef::Body);
    s.doc.para_paths(StoryRef::Body).iter().position(|p| *p == s.sel.focus.path).expect("caret in a body paragraph")
}

/// The inline object that starts at the caret.
fn obj_at_caret(s: &Session) -> InlineObject {
    let p = s.doc.para_at(&s.sel.focus).expect("caret paragraph");
    let off = s.sel.focus.off;
    assert!(p.text[off..].starts_with(OBJ), "the caret is on an object (off {off} in {:?})", p.text);
    p.objects[p.text[..off].matches(OBJ).count()].clone()
}

/// Caret at the end of top-level paragraph `i`.
fn caret_end_of(s: &mut Session, i: usize) {
    let len = s.doc.para_at(&Pos::body(i, 0)).unwrap().text.len();
    s.sel = Selection::caret(Pos::body(i, len));
}

/// Footnotes at the end of paragraphs 0, 2 and 3; the caret back at the start.
fn footnotes() -> Session {
    let mut s = doc("alpha\nbeta\ngamma\ndelta");
    for i in [0, 2, 3] {
        caret_end_of(&mut s, i);
        run(&mut s, "references.footnote", json!({"text": format!("note {i}")}));
    }
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

fn is_footnote(o: &InlineObject) -> bool {
    matches!(o, InlineObject::NoteRef { kind: NoteKind::Footnote, .. })
}

#[test]
fn footnote_by_number_is_clamped_to_the_document() {
    let mut s = footnotes();
    let body = s.doc.plain_text(StoryRef::Body);
    let r = run(&mut s, "edit.goto", json!({"what": "footnote", "n": 2}));
    assert_eq!(caret_para(&s), 2);
    assert!(is_footnote(&obj_at_caret(&s)));
    assert_eq!((r["found"].clone(), r["index"].clone(), r["count"].clone()), (json!(true), json!(2), json!(3)));
    run(&mut s, "edit.goto", json!({"what": "footnote", "n": 99}));
    assert_eq!(caret_para(&s), 3, "past the last: the last footnote");
    run(&mut s, "edit.goto", json!({"what": "footnote", "n": 0}));
    assert_eq!(caret_para(&s), 0, "0 counts as the first");
    assert!(is_footnote(&obj_at_caret(&s)));
    assert_eq!(s.doc.plain_text(StoryRef::Body), body, "Go To changes nothing");
}

#[test]
fn footnote_plus_and_minus_step_from_the_selection() {
    let mut s = footnotes();
    run(&mut s, "edit.goto", json!({"what": "footnote", "n": 1}));
    run(&mut s, "edit.goto", json!({"what": "footnote", "by": 1}));
    assert_eq!(caret_para(&s), 2, "+1 from footnote 1");
    run(&mut s, "edit.goto", json!({"what": "footnote", "by": -1}));
    assert_eq!(caret_para(&s), 0, "-1 from footnote 2");
    run(&mut s, "edit.goto", json!({"what": "footnote", "by": 5}));
    assert_eq!(caret_para(&s), 3, "+5 with two ahead: the last");
    let at = s.sel.focus.clone();
    let r = run(&mut s, "edit.goto", json!({"what": "footnote", "by": 1}));
    assert_eq!(r["found"], false, "nothing after the last");
    assert_eq!(s.sel.focus, at, "the caret stays");
    s.sel = Selection::caret(Pos::body(1, 0));
    run(&mut s, "edit.goto", json!({"what": "footnote"}));
    assert_eq!(caret_para(&s), 2, "no number = Next");
    s.sel = Selection::caret(Pos::body(1, 0));
    run(&mut s, "edit.goto", json!({"what": "footnote", "by": -1}));
    assert_eq!(caret_para(&s), 0, "Previous from between footnotes 1 and 2");
}

#[test]
fn a_kind_the_document_lacks_is_an_error_and_leaves_the_caret() {
    let mut s = footnotes();
    let at = s.sel.clone();
    for what in ["endnote", "comment", "field", "table", "equation", "object", "heading"] {
        let e = s.run("edit.goto", &json!({"what": what, "n": 1})).unwrap_err().to_string();
        assert!(e.to_lowercase().contains(what), "{what}: {e}");
        assert_eq!(s.sel, at, "{what}");
    }
    assert!(s.run("edit.goto", &json!({"what": "nonsense", "n": 1})).is_err());
    assert!(s.run("edit.goto", &json!({"what": "bookmark"})).is_err(), "bookmark needs a name");
}

#[test]
fn hostile_numbers_never_panic() {
    let mut s = footnotes();
    for v in [json!({"n": u64::MAX}), json!({"by": i64::MIN}), json!({"by": i64::MAX}), json!({"by": 0})] {
        let mut p = v.clone();
        p["what"] = json!("footnote");
        assert!(s.run("edit.goto", &p).is_ok(), "{p}");
    }
}

#[test]
fn comments_by_reviewer() {
    let mut s = doc("alpha\nbeta\ngamma\ndelta");
    for (i, who) in [(0, "Ann"), (1, "Bob"), (3, "Ann")] {
        s.author = who.into();
        s.sel = Selection { anchor: Pos::body(i, 0), focus: Pos::body(i, 4) };
        run(&mut s, "review.newComment", json!({"text": format!("from {who}")}));
    }
    s.sel = Selection::caret(Pos::body(0, 0));
    run(&mut s, "edit.goto", json!({"what": "comment", "name": "ann", "n": 2}));
    assert_eq!(caret_para(&s), 3, "Ann's second comment (reviewer name ignores case)");
    assert!(matches!(obj_at_caret(&s), InlineObject::CommentStart { .. }));
    run(&mut s, "edit.goto", json!({"what": "comment", "name": "", "n": 2}));
    assert_eq!(caret_para(&s), 1, "any reviewer");
    s.sel = Selection::caret(Pos::body(2, 0));
    run(&mut s, "edit.goto", json!({"what": "comment", "name": "Bob", "by": -1}));
    assert_eq!(caret_para(&s), 1);
    let at = s.sel.clone();
    let e = s.run("edit.goto", &json!({"what": "comment", "name": "Zed", "by": 1})).unwrap_err().to_string();
    assert!(e.contains("Zed"), "{e}");
    assert_eq!(s.sel, at);
}

#[test]
fn fields_by_field_name() {
    let mut s = doc("alpha\nbeta\ngamma");
    for (i, instr) in [(0, "PAGE"), (1, "DATE \\@ \"d MMMM yyyy\""), (2, " page  \\* roman")] {
        caret_end_of(&mut s, i);
        run(&mut s, "insert.field", json!({"instr": instr}));
    }
    s.sel = Selection::caret(Pos::body(0, 0));
    run(&mut s, "edit.goto", json!({"what": "field", "name": "Page", "n": 2}));
    assert_eq!(caret_para(&s), 2, "the type is the instruction's first word, any case");
    run(&mut s, "edit.goto", json!({"what": "field", "name": "DATE", "n": 1}));
    assert_eq!(caret_para(&s), 1);
    run(&mut s, "edit.goto", json!({"what": "field", "n": 3}));
    assert_eq!(caret_para(&s), 2, "any field");
    assert!(matches!(obj_at_caret(&s), InlineObject::Field { .. } | InlineObject::FieldStart { .. }));
    assert!(s.run("edit.goto", &json!({"what": "field", "name": "REF", "n": 1})).is_err());
}

#[test]
fn sections_start_with_section_one_at_the_top() {
    let mut s = doc("one\ntwo\nthree");
    caret_end_of(&mut s, 0);
    run(&mut s, "layout.break", json!({"kind": "nextPage"}));
    let n = s.doc.para_paths(StoryRef::Body).len();
    caret_end_of(&mut s, n - 2);
    run(&mut s, "layout.break", json!({"kind": "continuous"}));
    let last = s.doc.para_paths(StoryRef::Body).len() - 1;
    caret_end_of(&mut s, last);
    run(&mut s, "edit.goto", json!({"what": "section", "n": 1}));
    assert_eq!(s.sel.focus, Pos::body(0, 0));
    let section_end = |s: &Session, i: usize| s.doc.para_at(&Pos::body(i, 0)).is_some_and(|p| p.section.is_some());
    run(&mut s, "edit.goto", json!({"what": "section", "n": 2}));
    let two = caret_para(&s);
    assert!(two > 0 && section_end(&s, two - 1) && s.sel.focus.off == 0, "section 2 starts after the first break");
    run(&mut s, "edit.goto", json!({"what": "section", "by": 1}));
    let three = caret_para(&s);
    assert!(three > two && section_end(&s, three - 1), "section 3");
    run(&mut s, "edit.goto", json!({"what": "section", "n": 9}));
    assert_eq!(caret_para(&s), three, "the last section");
}

#[test]
fn heading_table_and_equation() {
    let mut s = doc("intro\nHead\nbody\nlast");
    s.sel = Selection::caret(Pos::body(1, 0));
    run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
    caret_end_of(&mut s, 2);
    run(&mut s, "insert.equation", json!({"linear": "x^2", "edit": false}));
    caret_end_of(&mut s, 3);
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    s.sel = Selection::caret(Pos::body(0, 0));

    run(&mut s, "edit.goto", json!({"what": "heading", "n": 1}));
    assert_eq!((caret_para(&s), s.sel.focus.off), (1, 0));
    run(&mut s, "edit.goto", json!({"what": "equation", "n": 1}));
    assert_eq!(caret_para(&s), 2);
    assert!(matches!(obj_at_caret(&s), InlineObject::Equation { .. }));
    run(&mut s, "edit.goto", json!({"what": "table", "by": 1}));
    assert!(s.sel.focus.path.cell().is_some(), "the caret is in the table's first cell: {:?}", s.sel.focus.path);
}

#[test]
fn page_and_bookmark_through_what() {
    let mut s = doc("one");
    caret_end_of(&mut s, 0);
    run(&mut s, "insert.pageBreak", json!({}));
    run(&mut s, "text.insert", json!({"text": "two"}));
    run(&mut s, "edit.goto", json!({"what": "page", "n": 1}));
    let page = |s: &mut Session| s.layout().caret_on(&s.sel.focus, s.page_hint).unwrap().page + 1;
    assert_eq!(page(&mut s), 1);
    run(&mut s, "edit.goto", json!({"what": "page", "by": 1}));
    assert_eq!(page(&mut s), 2);
    s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(0, 3) };
    run(&mut s, "insert.bookmark", json!({"name": "First"}));
    s.sel = Selection::caret(Pos::body(0, 0));
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "edit.goto", json!({"what": "bookmark", "name": "First"}));
    assert_eq!(caret_para(&s), 0);
}
