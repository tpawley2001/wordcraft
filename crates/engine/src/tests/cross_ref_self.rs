//! Update Field (F9) on a REF that sits inside the bookmark it names: Word shows
//! `Error! Not a valid bookmark self-reference.` whatever the field's switches. A PAGEREF inside
//! its own bookmark still shows its page, and a REF outside the bookmark shows the bookmark's
//! text with the inner field's error in it. A REF, PAGEREF or NOTEREF that names no bookmark shows
//! `Error! No bookmark name given.` (all measured in Microsoft 365 Word; ECMA-376 §17.16.5.45).

use serde_json::json;
use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};

use crate::Session;

const SELF_ERROR: &str = "Error! Not a valid bookmark self-reference.";
const NO_NAME: &str = "Error! No bookmark name given.";

fn field(instr: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: "stale".into(), locked, dirty: false, code: false, form: String::new() }
}

/// Puts `obj` at offset `off` of the body's paragraph number `para`.
fn put(doc: &mut Document, para: usize, off: usize, obj: InlineObject) {
    let path = doc.para_paths(StoryRef::Body).remove(para);
    doc.para_mut(StoryRef::Body, &path).unwrap().insert_object(off, obj, &CharProps::default()).unwrap();
}

/// The length of the body's paragraph number `para`.
fn len(doc: &Document, para: usize) -> usize {
    let path = doc.para_paths(StoryRef::Body).remove(para);
    doc.para(StoryRef::Body, &path).unwrap().len()
}

/// Bookmarks the body's paragraphs `first..=last`, whole, as `name`.
fn mark(doc: &mut Document, first: usize, last: usize, name: &str) {
    let end = len(doc, last);
    put(doc, last, end, InlineObject::BookmarkEnd { name: name.into() });
    put(doc, first, 0, InlineObject::BookmarkStart { name: name.into() });
}

fn update(s: &mut Session) {
    s.run("references.updateFields", &json!({})).unwrap_or_else(|e| panic!("references.updateFields: {e:?}"));
}

/// The body's field results, in document order.
fn results(s: &Session) -> Vec<String> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// Five paragraphs: `[inside {REF Self} tail]`, `[q {REF Q \h \* MERGEFORMAT} r]`,
/// `[n {REF N \p} m]`, `[x {PAGEREF P} y]` (each bookmarked whole under the name its field
/// uses) and `Outside: {REF Self \h}`.
fn probe(locked: bool) -> Document {
    let mut doc = Document::from_text("inside  tail\nq  r\nn  m\nx  y\nOutside: ");
    put(&mut doc, 0, 7, field("REF Self", locked));
    put(&mut doc, 1, 2, field(r"REF Q \h \* MERGEFORMAT", locked));
    put(&mut doc, 2, 2, field(r"REF N \p", locked));
    put(&mut doc, 3, 2, field("PAGEREF P", locked));
    let end = len(&doc, 4);
    put(&mut doc, 4, end, field(r"REF Self \h", locked));
    for (para, name) in ["Self", "Q", "N", "P"].into_iter().enumerate() {
        mark(&mut doc, para, para, name);
    }
    doc
}

#[test]
fn a_ref_inside_its_own_bookmark_shows_the_self_reference_error() {
    let mut s = Session::new(probe(false));
    update(&mut s);
    let got = results(&s);
    assert_eq!(got.len(), 5, "{got:?}");
    assert_eq!(got[0], SELF_ERROR, "REF Self");
    assert_eq!(got[1], SELF_ERROR, r"REF Q \h \* MERGEFORMAT");
    assert_eq!(got[2], SELF_ERROR, r"REF N \p");
    // A second update changes nothing.
    update(&mut s);
    assert_eq!(results(&s), got);
}

#[test]
fn a_pageref_inside_its_own_bookmark_still_shows_its_page() {
    let mut s = Session::new(probe(false));
    update(&mut s);
    assert_eq!(results(&s)[3], "1");
}

#[test]
fn a_ref_outside_the_bookmark_shows_its_text_with_the_inner_error() {
    let mut s = Session::new(probe(false));
    update(&mut s);
    assert_eq!(results(&s)[4], format!("inside {SELF_ERROR} tail"));
}

#[test]
fn locked_fields_keep_their_results() {
    let mut s = Session::new(probe(true));
    update(&mut s);
    assert_eq!(results(&s), ["stale"; 5]);
}

#[test]
fn undo_puts_the_old_results_back() {
    let mut s = Session::new(probe(false));
    update(&mut s);
    assert_eq!(results(&s)[0], SELF_ERROR);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(results(&s), ["stale"; 5]);
}

/// A REF to a bookmark it is not inside updates as before.
#[test]
fn a_ref_to_another_bookmark_is_not_a_self_reference() {
    let mut doc = Document::from_text("Target text\nSee ");
    put(&mut doc, 1, 4, field(r"REF Mark \h", false));
    mark(&mut doc, 0, 0, "Mark");
    let mut s = Session::new(doc);
    update(&mut s);
    assert_eq!(results(&s), ["Target text"]);
}

/// The bookmark runs over three paragraphs and the REF is in the middle one.
#[test]
fn a_ref_in_a_later_paragraph_of_its_bookmark_is_a_self_reference_too() {
    let mut doc = Document::from_text("start\nmid  dle\nend");
    put(&mut doc, 1, 4, field("REF Span", false));
    mark(&mut doc, 0, 2, "Span");
    let mut s = Session::new(doc);
    update(&mut s);
    assert_eq!(results(&s), [SELF_ERROR]);
}

/// Word shows the paragraph's number for `\n` inside the own bookmark, not the error.
#[test]
fn a_paragraph_number_ref_inside_its_bookmark_is_not_an_error() {
    let mut doc = Document::from_text("uv  w");
    put(&mut doc, 0, 3, field(r"REF U \n", false));
    mark(&mut doc, 0, 0, "U");
    let mut s = Session::new(doc);
    update(&mut s);
    assert_ne!(results(&s), [SELF_ERROR]);
}

/// A cross-reference field that names no bookmark, with and without switches.
#[test]
fn a_field_with_no_bookmark_name_says_so() {
    let codes = ["REF", r"REF \h", "PAGEREF", r"PAGEREF \h", "NOTEREF", r"REF \* MERGEFORMAT"];
    let mut doc = Document::from_text("Fields: ");
    for code in codes {
        let end = len(&doc, 0);
        put(&mut doc, 0, end, field(code, false));
    }
    let mut s = Session::new(doc);
    update(&mut s);
    assert_eq!(results(&s), [NO_NAME; 6]);
}
