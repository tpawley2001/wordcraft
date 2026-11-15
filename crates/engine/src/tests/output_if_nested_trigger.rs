//! Output (PDF export, printing, page images) refreshes the body's unlocked IF fields when the body
//! holds a SEQ, REF, DATE, PRINTDATE, SAVEDATE, PAGE, NUMPAGES, SECTION or SECTIONPAGES field
//! (the last body field keeps its stored result). Microsoft 365 Word's PDF export counts such a
//! field also when it is NESTED inside another field's code: in probe documents whose only REF and
//! DATE fields sat inside IF conditions, every IF printed its fresh branch.
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

/// A field whose code is `before` + the nested field's stored text `stored` + `after`.
fn with_nested(before: &str, nested: &str, stored: &str, after: &str, result: &str) -> InlineObject {
    let instr = format!("{before}{stored}{after}");
    let code = CodeParas {
        code: code_hash(&instr),
        nested: vec![NestedCode { at: before.len(), instr: nested.into() }],
        nested_len: vec![stored.len()],
        ..Default::default()
    };
    InlineObject::Field { instr, result: result.into(), locked: false, dirty: false, code: false, form: String::new(), code_paras: Some(Box::new(code)) }
}

/// One paragraph holding `fields` in order.
fn doc_with(fields: Vec<InlineObject>) -> Document {
    let props = CharProps::default();
    let mut p = Paragraph::default();
    for o in fields {
        let off = p.len();
        p.insert_object(off, o, &props).unwrap();
    }
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    d
}

/// The results output shows for the paragraph's fields, in order.
fn shown(d: &Document) -> Vec<String> {
    let out = output_fields(d).unwrap_or_else(|| d.clone());
    let path = out.para_paths(StoryRef::Body).remove(0);
    out.para(StoryRef::Body, &path)
        .unwrap()
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

/// `IF 1 = 1 "A" "B"` with the stale result B: it prints A whenever output refreshes IF fields.
fn plain_if() -> InlineObject {
    field(r#"IF 1 = 1 "A" "B""#, "B")
}

#[test]
fn a_date_nested_in_an_if_triggers_the_refresh() {
    // IF {DATE \@ "yyyy"} > 1000 "NEW" "OLD": its nested DATE is the body's only trigger field.
    let nested = with_nested("IF ", r#"DATE \@ "yyyy""#, "2030", r#" > 1000 "NEW" "OLD""#, "OLD");
    let d = doc_with(vec![plain_if(), nested, field("NUMWORDS", "3")]);
    assert_eq!(shown(&d)[0], "A");
}

#[test]
fn a_ref_nested_in_an_if_triggers_the_refresh() {
    let nested = with_nested("IF ", "REF x", "5", r#" = 5 "Y" "N""#, "Y");
    let d = doc_with(vec![plain_if(), nested, field("NUMWORDS", "3")]);
    assert_eq!(shown(&d)[0], "A");
}

#[test]
fn a_ref_nested_in_a_formula_triggers_the_refresh() {
    let nested = with_nested("= ", "REF x", "5", " + 1", "6");
    let d = doc_with(vec![plain_if(), nested, field("NUMWORDS", "3")]);
    assert_eq!(shown(&d)[0], "A");
}

#[test]
fn control_a_nested_field_that_is_no_trigger_changes_nothing() {
    // A nested MERGEFIELD (and no trigger anywhere): every field keeps its stored result.
    let nested = with_nested("IF ", "MERGEFIELD Amount", "150", r#" >= 100 "big" "small""#, "small");
    let d = doc_with(vec![plain_if(), nested, field("NUMWORDS", "3")]);
    assert_eq!(shown(&d), ["B", "small", "3"]);
}

#[test]
fn control_a_top_level_trigger_still_works() {
    let d = doc_with(vec![plain_if(), field("SEQ fig", "1"), field("NUMWORDS", "3")]);
    assert_eq!(shown(&d)[0], "A");
}
