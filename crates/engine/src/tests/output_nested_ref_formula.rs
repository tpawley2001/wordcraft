//! Output (PDF export, printing, page images) recomputes an unlocked `=` formula in the body that
//! holds a nested REF field: the nested REF shows the bookmark's current text and the formula is
//! worked out from it, as Update Field would. Measured with Microsoft 365 Word's PDF export of a
//! probe document whose fields all stored the stale result "OLD", with bookmark `x` holding "5":
//! `= { REF x } +1` printed 6, `= x + 1` (bookmark named directly, no nested field) printed OLD,
//! and `= 1 + 1` stored "3" printed 3. The body also holds SEQ fields, so output refreshes fields
//! (the same condition as the IF fields in `output_if_fields.rs`).
//!
//! On import a nested field is flattened into the outer code as its stored text, and the outer
//! field's `code_paras.nested` records where that text starts and the nested field's code.

use wordcraft_doc::para::{CodeParas, NestedCode, code_hash};
use wordcraft_doc::{CharProps, Document, InlineObject, Paragraph, StoryRef, para_block};

use crate::cmd::references::output_fields;

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// `= OLD +1`: an `=` field whose code held `{ REF x }`, read with the nested REF's stored text "OLD".
fn formula_with_nested_ref(locked: bool) -> InlineObject {
    let instr = "= OLD +1";
    let code = CodeParas { code: code_hash(instr), nested: vec![NestedCode { at: 2, instr: "REF x".into() }], ..Default::default() };
    InlineObject::Field {
        instr: instr.into(),
        result: "OLD".into(),
        locked,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: Some(Box::new(code)),
    }
}

/// Paragraph 1: "x = " then the bookmark `x` around "5". Paragraph 2: `SEQ fig`, the case's field,
/// `SEQ fig` (the last field keeps its stored result, so the case is never the body's last field).
fn doc_with(case: InlineObject) -> Document {
    let props = CharProps::default();
    let mut p1 = Paragraph::with_text("x = ", props.clone());
    let off = p1.len();
    p1.insert_object(off, InlineObject::BookmarkStart { name: "x".into() }, &props).unwrap();
    let off = p1.len();
    p1.insert_text(off, "5", &props).unwrap();
    let off = p1.len();
    p1.insert_object(off, InlineObject::BookmarkEnd { name: "x".into() }, &props).unwrap();
    let mut p2 = Paragraph::default();
    for o in [field("SEQ fig", "9", false), case, field("SEQ fig", "9", false)] {
        let off = p2.len();
        p2.insert_object(off, o, &props).unwrap();
    }
    let mut d = Document::new();
    d.body = vec![para_block(p1), para_block(p2)];
    d
}

/// The case field's result (paragraph 2, second field).
fn case_result(d: &Document) -> String {
    let path = d.para_paths(StoryRef::Body).remove(1);
    let fields: Vec<String> = d
        .para(StoryRef::Body, &path)
        .unwrap()
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect();
    fields[1].clone()
}

fn at_output(d: &Document) -> String {
    case_result(&output_fields(d).unwrap_or_else(|| d.clone()))
}

#[test]
fn a_formula_holding_a_nested_ref_is_recomputed_at_output() {
    let d = doc_with(formula_with_nested_ref(false));
    assert_eq!(at_output(&d), "6");
    assert_eq!(case_result(&d), "OLD", "output works on a copy: the document keeps its stored result");
}

#[test]
fn a_locked_formula_keeps_its_stored_result() {
    assert_eq!(at_output(&doc_with(formula_with_nested_ref(true))), "OLD");
}

#[test]
fn formulas_without_a_nested_ref_keep_their_stored_result() {
    // Word: a bookmark named directly in the formula, with no nested field, prints the stored result.
    assert_eq!(at_output(&doc_with(field("= x + 1", "OLD", false))), "OLD");
    assert_eq!(at_output(&doc_with(field("= 1 + 1", "3", false))), "3");
}
