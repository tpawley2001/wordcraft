//! References › Cross-reference: Footnote and Endnote reference types (`NOTEREF`, ECMA-376
//! §17.16.5.40), Insert as hyperlink (`\h`) and Above/below (`\p`), as Word inserts them.

use serde_json::{Value, json};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{Document, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e:?}"))
}

fn caret(s: &mut Session, block: usize, off: usize) {
    run(s, "caret.set", json!({"pos": {"block": block, "off": off}}));
}

/// "Alpha" with footnote 1 ("Note one") after it, "See " with endnote i ("Late note") after it,
/// a third paragraph "bm text" bookmarked as `bm`, and the caret at the end of the third paragraph.
fn session() -> Session {
    let mut s = Session::new(Document::from_text("Alpha\nSee \nbm text"));
    caret(&mut s, 0, 5);
    run(&mut s, "references.footnote", json!({"text": "Note one"}));
    caret(&mut s, 1, 4);
    run(&mut s, "references.endnote", json!({"text": "Late note"}));
    run(&mut s, "select.range", json!({"anchor": {"block": 2, "off": 0}, "focus": {"block": 2, "off": 7}}));
    run(&mut s, "insert.bookmark", json!({"name": "bm"}));
    caret(&mut s, 2, 7);
    s
}

/// The body's fields as (instr, result), in document order.
fn fields(s: &Session) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o {
                out.push((instr.clone(), result.clone()));
            }
        }
    }
    out
}

/// The one field `run` inserted.
fn inserted(params: Value) -> (String, String) {
    let mut s = session();
    let before = fields(&s).len();
    run(&mut s, "insert.crossReference", params.clone());
    let f = fields(&s);
    assert_eq!(f.len(), before + 1, "{params}: {f:?}");
    let new: Vec<_> = f.into_iter().filter(|(i, _)| i.contains("REF")).collect();
    assert_eq!(new.len(), 1, "{params}: {new:?}");
    new.into_iter().next().unwrap()
}

/// `instr` with its `_Ref` bookmark name replaced by `_Ref#`.
fn shape(instr: &str) -> String {
    instr.split_whitespace().map(|w| if w.starts_with("_Ref") { "_Ref#" } else { w }).collect::<Vec<_>>().join(" ")
}

#[test]
fn footnotes_and_endnotes_are_listed_with_their_numbers() {
    let mut s = session();
    assert_eq!(run(&mut s, "insert.crossReference", json!({"to": "footnote"})), json!(["1 Note one"]));
    assert_eq!(run(&mut s, "insert.crossReference", json!({"to": "endnote"})), json!(["i Late note"]));
}

#[test]
fn note_references_are_noteref_fields() {
    let cases = [
        (json!({"to": "footnote", "target": "1", "show": "number"}), "NOTEREF _Ref# \\h", "1"),
        (json!({"to": "footnote", "target": "1", "show": "formattedNumber"}), "NOTEREF _Ref# \\f \\h", "1"),
        (json!({"to": "footnote", "target": "1", "show": "page"}), "PAGEREF _Ref# \\h", "1"),
        (json!({"to": "footnote", "target": "1", "show": "aboveBelow"}), "NOTEREF _Ref# \\p \\h", "above"),
        (json!({"to": "footnote", "target": "1", "show": "number", "hyperlink": false}), "NOTEREF _Ref#", "1"),
        (json!({"to": "endnote", "target": "i", "show": "number"}), "NOTEREF _Ref# \\h", "i"),
    ];
    for (params, instr, result) in cases {
        let (i, r) = inserted(params.clone());
        assert_eq!((shape(&i).as_str(), r.as_str()), (instr, result), "{params}");
    }
}

#[test]
fn the_ref_bookmark_marks_the_note_reference() {
    let mut s = session();
    run(&mut s, "insert.crossReference", json!({"to": "footnote", "target": "1", "show": "number"}));
    let (name, pos) = s.doc.bookmarks().into_iter().find(|(n, _)| n.starts_with("_Ref")).expect("a _Ref bookmark");
    assert_eq!((pos.path.0.as_slice(), pos.off), ([0u32].as_slice(), 5), "{name} starts at the footnote mark");
    // A second reference to the same note reuses it.
    run(&mut s, "insert.crossReference", json!({"to": "footnote", "target": "1", "show": "page"}));
    assert_eq!(s.doc.bookmarks().iter().filter(|(n, _)| n.starts_with("_Ref")).count(), 1);
}

#[test]
fn above_below_keeps_its_switch_for_other_types() {
    let (i, r) = inserted(json!({"to": "bookmark", "target": "bm", "show": "aboveBelow"}));
    assert_eq!((i.as_str(), r.as_str()), ("REF bm \\p \\h", "above"));
}

#[test]
fn insert_as_hyperlink_off_leaves_out_h() {
    let (i, r) = inserted(json!({"to": "bookmark", "target": "bm", "hyperlink": false}));
    assert_eq!((i.as_str(), r.as_str()), ("REF bm", "bm text"));
}

#[test]
fn bad_targets_and_shows_are_refused() {
    for params in [json!({"to": "footnote", "target": "9", "show": "number"}), json!({"to": "bookmark", "target": "bm", "show": "formattedNumber"})] {
        let mut s = session();
        let before = fields(&s);
        assert!(s.run("insert.crossReference", &params).is_err(), "{params}");
        assert_eq!(fields(&s), before, "{params}");
    }
}
