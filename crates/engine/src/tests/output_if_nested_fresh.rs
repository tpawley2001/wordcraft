//! Output (PDF export, printing, page images) re-evaluates an unlocked IF field in the body whose
//! condition holds a nested REF or DATE field: the nested field shows its current value before the
//! comparison, as Microsoft 365 Word's PDF export does (measured with probe documents whose stored
//! results were deliberately stale). A nested formula (`=`), DOCPROPERTY or MERGEFIELD keeps its
//! stored text, and so does the comparison that reads it. The body also holds SEQ fields, so output
//! refreshes fields (the same condition as `output_if_fields.rs`).
//!
//! On import a nested field is flattened into the outer code as its stored text; the outer field's
//! `code_paras.nested` records where that text starts and the nested field's code, `nested_len` its
//! byte length.

use wordcraft_doc::para::{CodeParas, NestedCode, code_hash};
use wordcraft_doc::{CharProps, Document, InlineObject, Paragraph, StoryRef, para_block};

use crate::cmd::references::output_fields;

fn field(instr: &str, result: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// An IF field whose code is `before` + the nested field's stored text `stored` + `after`, the
/// nested field's code being `nested`; the IF's own stored result is `result`.
fn if_with_nested(before: &str, nested: &str, stored: &str, after: &str, result: &str, locked: bool) -> InlineObject {
    let instr = format!("{before}{stored}{after}");
    let code = CodeParas {
        code: code_hash(&instr),
        nested: vec![NestedCode { at: before.len(), instr: nested.into() }],
        nested_len: vec![stored.len()],
        ..Default::default()
    };
    InlineObject::Field { instr, result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: Some(Box::new(code)) }
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
    for o in [field("SEQ fig", "9"), case, field("SEQ fig", "9")] {
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
fn a_nested_ref_in_the_condition_reads_the_bookmark_now() {
    // Word: IF {REF x}(OLD) = 5 "yes" "no" with x = 5 printed yes.
    let d = doc_with(if_with_nested("IF ", "REF x", "OLD", r#" = 5 "yes" "no""#, "no", false));
    assert_eq!(at_output(&d), "yes");
    assert_eq!(case_result(&d), "no", "output works on a copy: the document keeps its stored result");
}

#[test]
fn a_nested_ref_compared_with_its_old_text_takes_the_other_branch() {
    // Word: IF {REF Edited}(OLD) = "OLD" "REF_OLD" "REF_NEW" with Edited = NEW printed REF_NEW.
    let d = doc_with(if_with_nested("IF ", "REF x", "OLD", r#" = "OLD" "REF_OLD" "REF_NEW""#, "REF_OLD", false));
    assert_eq!(at_output(&d), "REF_NEW");
}

#[test]
fn a_nested_ref_as_the_right_operand_reads_the_bookmark_now() {
    let d = doc_with(if_with_nested("IF 5 = ", "REF x", "OLD", r#" "yes" "no""#, "no", false));
    assert_eq!(at_output(&d), "yes");
}

#[test]
fn a_nested_date_in_the_condition_shows_the_current_year() {
    // Word: IF {DATE \@ "yyyy"}(2030) > 2026 "EXPIRED" "VALID" printed VALID in 2026. The stored
    // year here is far in the past so the test holds in any year after 2000.
    let d = doc_with(if_with_nested("IF ", r#"DATE \@ "yyyy""#, "1999", r#" > 2000 "after" "before""#, "before", false));
    assert_eq!(at_output(&d), "after");
}

#[test]
fn a_nested_formula_docproperty_or_mergefield_keeps_its_stored_text() {
    // Word: IF {=2+3}(OLD) = 5 yes no printed no; IF {DOCPROPERTY Title}(Nope) = "Probe" with Title
    // = Probe printed the stored TITLE_MISMATCH.
    let formula = doc_with(if_with_nested("IF ", "=2+3", "OLD", r#" = 5 "yes" "no""#, "no", false));
    assert_eq!(at_output(&formula), "no");
    let mut prop = doc_with(if_with_nested("IF ", "DOCPROPERTY Title", "Nope", r#" = "Probe" "TITLE_MATCH" "TITLE_MISMATCH""#, "TITLE_MISMATCH", false));
    prop.core.title = "Probe".into();
    assert_eq!(at_output(&prop), "TITLE_MISMATCH");
    let merge = doc_with(if_with_nested("IF ", "MERGEFIELD Amount", "150", r#" >= 100 "big" "small""#, "big", false));
    assert_eq!(at_output(&merge), "big");
}

#[test]
fn a_locked_if_keeps_its_stored_result() {
    let d = doc_with(if_with_nested("IF ", "REF x", "OLD", r#" = 5 "yes" "no""#, "no", true));
    assert_eq!(at_output(&d), "no");
}
