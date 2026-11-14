//! Find what codes for objects as Word reads them: `^f` / `^e` / `^2` find footnote and endnote
//! reference marks, `^d` a whole field (only while field codes are shown), `^g` only drawings,
//! `^a` / `^5` nothing; bookmark and comment anchors are not characters to Find, so a word with
//! a bookmark inside it is still found. Measured in Microsoft 365 Word 16.0 (Find.Execute, Use
//! wildcards off), 2026-11-14.

use serde_json::json;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};

use crate::Session;

fn field(instr: &str, result: &str) -> InlineObject {
    InlineObject::Field {
        instr: instr.into(),
        result: result.into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    }
}

/// `one`+footnote mark+` two`+endnote mark+` three four five`+DATE field+` six`+PAGE field+` seven`,
/// a comment around "three" and an empty bookmark between "fo" and "ur".
fn session() -> Session {
    let mut s = Session::new(Document::new());
    s.run("document.setText", &json!({ "text": "one two three four five six seven" })).unwrap();
    let path = s.doc.para_paths(StoryRef::Body).remove(0);
    let p = s.doc.para_mut(StoryRef::Body, &path).unwrap();
    let props = CharProps::default();
    // Highest offset first, so the plain-text offsets stay valid.
    p.insert_object(27, field("PAGE", "1"), &props).unwrap();
    p.insert_object(23, field("DATE \\@ \"yyyy\"", "2026"), &props).unwrap();
    p.insert_object(16, InlineObject::BookmarkEnd { name: "bm".into() }, &props).unwrap();
    p.insert_object(16, InlineObject::BookmarkStart { name: "bm".into() }, &props).unwrap();
    p.insert_object(13, InlineObject::CommentEnd { id: 1 }, &props).unwrap();
    p.insert_object(8, InlineObject::CommentStart { id: 1 }, &props).unwrap();
    p.insert_object(7, InlineObject::NoteRef { kind: NoteKind::Endnote, id: 9001, custom: String::new() }, &props).unwrap();
    p.insert_object(3, InlineObject::NoteRef { kind: NoteKind::Footnote, id: 9002, custom: String::new() }, &props).unwrap();
    s
}

/// The paragraph's text from `a` to `b` (byte offsets), each object shown as `{fn}`, `{en}`,
/// `{field}`, a drawing as `{g}`; bookmark and comment anchors show as nothing.
fn label(s: &Session, a: usize, b: usize) -> String {
    let path = s.doc.para_paths(StoryRef::Body).remove(0);
    let p = s.doc.para(StoryRef::Body, &path).unwrap();
    let mut out = String::new();
    let mut objs = p.objects.iter();
    for (i, c) in p.text.char_indices() {
        let obj = if c == '\u{FFFC}' { objs.next() } else { None };
        if i < a || i >= b {
            continue;
        }
        match obj {
            None => out.push(c),
            Some(InlineObject::NoteRef { kind: NoteKind::Footnote, .. }) => out.push_str("{fn}"),
            Some(InlineObject::NoteRef { .. }) => out.push_str("{en}"),
            Some(InlineObject::Field { .. }) => out.push_str("{field}"),
            Some(o) if o.is_drawing() => out.push_str("{g}"),
            Some(_) => {}
        }
    }
    out
}

/// Every match of `pattern` in the body (normal search, wildcards off), as labelled text.
fn hits(s: &mut Session, pattern: &str) -> Vec<String> {
    s.run("edit.find", &json!({ "text": pattern })).unwrap();
    let found: Vec<(usize, usize)> = s.find.results.iter().map(|(a, b)| (a.off, b.off)).collect();
    found.into_iter().map(|(a, b)| label(s, a, b)).collect()
}

#[test]
fn note_mark_codes_find_their_marks() {
    let mut s = session();
    assert_eq!(hits(&mut s, "^f"), ["{fn}"]);
    assert_eq!(hits(&mut s, "^e"), ["{en}"]);
    assert_eq!(hits(&mut s, "^2"), ["{fn}", "{en}"]);
    assert_eq!(hits(&mut s, "e^f"), ["e{fn}"]);
    assert_eq!(hits(&mut s, "e^2"), ["e{fn}"]);
}

#[test]
fn comment_mark_and_graphic_codes_find_nothing_here() {
    let mut s = session();
    for pattern in ["^a", "^5", "^g"] {
        assert_eq!(hits(&mut s, pattern), Vec::<String>::new(), "{pattern}");
    }
}

#[test]
fn field_code_finds_whole_fields_only_while_codes_are_shown() {
    let mut s = session();
    assert_eq!(hits(&mut s, "^d"), Vec::<String>::new());
    s.run("view.fieldCodes", &json!({ "value": true })).unwrap();
    assert_eq!(hits(&mut s, "^d"), ["{field}", "{field}"]);
}

#[test]
fn bookmarks_and_comment_anchors_are_not_characters() {
    let mut s = session();
    assert_eq!(hits(&mut s, "four"), ["four"]);
    assert_eq!(hits(&mut s, "fo^?"), ["fou"]);
    assert_eq!(hits(&mut s, " three "), [" three "]);
    // A field counts as one character.
    assert_eq!(hits(&mut s, "ve^?"), ["ve{field}", "ven"]);
}

#[test]
fn replace_all_note_marks() {
    let mut s = session();
    s.run("edit.replaceAll", &json!({ "text": "^f", "with": "X" })).unwrap();
    let path = s.doc.para_paths(StoryRef::Body).remove(0);
    let p = s.doc.para(StoryRef::Body, &path).unwrap();
    assert!(p.text.starts_with("oneX two"), "{:?}", p.text);
    assert!(!p.objects.iter().any(|o| matches!(o, InlineObject::NoteRef { kind: NoteKind::Footnote, .. })));
    assert!(p.objects.iter().any(|o| matches!(o, InlineObject::NoteRef { kind: NoteKind::Endnote, .. })));
}
