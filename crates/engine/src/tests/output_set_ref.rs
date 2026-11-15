//! A REF to a name a SET field gives a value shows that value, at output (PDF, print) and after F9.
//! Expected results read from Microsoft 365 Word's PDF of a probe document whose fields Word had
//! updated: `SET total 42` / `REF total` prints 42, `SET greet "Hello World"` / `REF greet` prints
//! Hello World, `SET n {= 6*7}` / `REF n` prints 42. Our reader keeps the bookmark Word puts around
//! the SET's result as an empty bookmark just before the SET field.

use serde_json::json;
use wordcraft_doc::{CharProps, Document, InlineObject, Paragraph, Pos, StoryRef, para_block};

use crate::cmd::references::output_fields;
use crate::{Selection, Session};

fn field(instr: &str, result: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// Paragraph 1: the empty bookmark `name` then `set` (stored result `set_result`); paragraph 2:
/// "R = " and `REF <ref_name>` with a stale result.
fn doc(name: &str, set: &str, set_result: &str, ref_name: &str) -> Document {
    let mut p1 = Paragraph::with_text("S = ", CharProps::default());
    for o in [InlineObject::BookmarkStart { name: name.into() }, InlineObject::BookmarkEnd { name: name.into() }, field(set, set_result)] {
        let off = p1.len();
        p1.insert_object(off, o, &CharProps::default()).unwrap();
    }
    let mut p2 = Paragraph::with_text("R = ", CharProps::default());
    let off = p2.len();
    p2.insert_object(off, field(&format!("REF {ref_name}"), "OLD"), &CharProps::default()).unwrap();
    let mut d = Document::new();
    d.body = vec![para_block(p1), para_block(p2)];
    d
}

/// The REF's result in paragraph 2.
fn ref_result(d: &Document) -> String {
    let path = d.para_paths(StoryRef::Body).remove(1);
    d.para(StoryRef::Body, &path)
        .unwrap()
        .objects
        .iter()
        .find_map(|o| match o {
            InlineObject::Field { instr, result, .. } if instr.starts_with("REF") => Some(result.clone()),
            _ => None,
        })
        .unwrap()
}

fn at_output(d: &Document) -> String {
    ref_result(&output_fields(d).unwrap_or_else(|| d.clone()))
}

fn after_f9(d: &Document) -> String {
    let mut s = Session::new(d.clone());
    let path = s.doc.para_paths(StoryRef::Body).remove(1);
    s.sel = Selection::caret(Pos::new(StoryRef::Body, path, 0));
    s.run("references.updateFields", &json!({})).unwrap();
    ref_result(&s.doc)
}

const CASES: &[(&str, &str, &str, &str, &str)] = &[
    // (bookmark, SET code, SET stored result, REF target, Word's result)
    ("total", "SET total 42", "42", "total", "42"),
    ("greet", "SET greet \"Hello World\"", "Hello World", "greet", "Hello World"),
    // A nested formula: the stored result is the value.
    ("n", "SET n 42", "42", "n", "42"),
    // Names are case-insensitive.
    ("Total", "SET Total 42", "42", "TOTAL", "42"),
];

#[test]
fn ref_to_a_set_name_shows_its_value_at_output() {
    for &(name, set, stored, target, want) in CASES {
        assert_eq!(at_output(&doc(name, set, stored, target)), want, "{set} / REF {target}");
    }
}

#[test]
fn ref_to_a_set_name_shows_its_value_after_f9() {
    for &(name, set, stored, target, want) in CASES {
        assert_eq!(after_f9(&doc(name, set, stored, target)), want, "{set} / REF {target}");
    }
}

#[test]
fn a_non_empty_bookmark_keeps_its_own_text() {
    // "abc" bookmarked; the SET names something else.
    let mut p1 = Paragraph::with_text("abc", CharProps::default());
    p1.insert_object(0, InlineObject::BookmarkStart { name: "b".into() }, &CharProps::default()).unwrap();
    let end = p1.len();
    p1.insert_object(end, InlineObject::BookmarkEnd { name: "b".into() }, &CharProps::default()).unwrap();
    let end = p1.len();
    p1.insert_object(end, field("SET other 1", "1"), &CharProps::default()).unwrap();
    let mut p2 = Paragraph::with_text("R = ", CharProps::default());
    let off = p2.len();
    p2.insert_object(off, field("REF b", "OLD"), &CharProps::default()).unwrap();
    let mut d = Document::new();
    d.body = vec![para_block(p1), para_block(p2)];
    assert_eq!(at_output(&d), "abc");
    assert_eq!(after_f9(&d), "abc");
}
