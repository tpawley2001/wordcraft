//! At output (PDF, print) a REF reads bookmarks only: a SET field whose name has no bookmark in the
//! document (Word adds one only when it updates the SET) leaves `REF <name>` with Word's
//! "reference not found" error. Measured with Microsoft 365 Word's PDF of a mail-merge probe whose
//! SET had never been updated: `REF total` printed `Error! Reference source not found.`.

use wordcraft_doc::{CharProps, Document, InlineObject, Paragraph, StoryRef, para_block};

use crate::cmd::references::{REFERENCE_NOT_FOUND, output_fields};

fn field(instr: &str, result: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// Paragraph 1: optionally the empty bookmark `bookmark`, then `set` (stored `set_result`);
/// paragraph 2: "R = " and `REF <target>` stored `OLD`.
fn doc(bookmark: Option<&str>, set: &str, set_result: &str, target: &str) -> Document {
    let mut p1 = Paragraph::with_text("S = ", CharProps::default());
    let mut objs = Vec::new();
    if let Some(b) = bookmark {
        objs.push(InlineObject::BookmarkStart { name: b.into() });
        objs.push(InlineObject::BookmarkEnd { name: b.into() });
    }
    objs.push(field(set, set_result));
    for o in objs {
        let off = p1.len();
        p1.insert_object(off, o, &CharProps::default()).unwrap();
    }
    let mut p2 = Paragraph::with_text("R = ", CharProps::default());
    let off = p2.len();
    p2.insert_object(off, field(&format!("REF {target}"), "OLD"), &CharProps::default()).unwrap();
    let mut d = Document::new();
    d.body = vec![para_block(p1), para_block(p2)];
    d
}

fn ref_at_output(d: &Document) -> String {
    let out = output_fields(d).unwrap_or_else(|| d.clone());
    let path = out.para_paths(StoryRef::Body).remove(1);
    out.para(StoryRef::Body, &path)
        .unwrap()
        .objects
        .iter()
        .find_map(|o| match o {
            InlineObject::Field { instr, result, .. } if instr.starts_with("REF") => Some(result.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn ref_to_a_set_name_without_a_bookmark_is_not_found() {
    for (set, stored) in [("SET total 42", "42"), ("SET total \"OLD\"", "OLD")] {
        assert_eq!(ref_at_output(&doc(None, set, stored, "total")), REFERENCE_NOT_FOUND, "{set}");
    }
}

#[test]
fn ref_to_a_set_name_with_its_bookmark_still_shows_the_value() {
    assert_eq!(ref_at_output(&doc(Some("total"), "SET total 42", "42", "TOTAL")), "42");
}
