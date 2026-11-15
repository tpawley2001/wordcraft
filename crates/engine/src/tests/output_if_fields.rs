//! Output (PDF export, printing, page images) shows unlocked IF fields in the body with their fresh
//! result when the body also holds a field output refreshes (SEQ, REF, DATE, PAGE ...), except the
//! body's last field. Expected results read from Microsoft 365 Word's PDF export of probe documents
//! whose IF fields all stored a stale result; `=` formulas keep their stored result.

use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};

use crate::cmd::references::output_fields;

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// A document whose first body paragraph holds `fields`, in order.
fn doc_with(fields: Vec<InlineObject>) -> Document {
    let mut doc = Document::new();
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let para = doc.para_mut(StoryRef::Body, &path).unwrap();
    for f in fields.into_iter().rev() {
        para.insert_object(0, f, &CharProps::default()).unwrap();
    }
    doc
}

/// The results of the body's fields, in order.
fn results(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        for o in &doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// What output shows for each field code (stored result "OLD"), between a `SEQ fig` field first
/// and one last (as in Word's probes: the SEQ makes output refresh, and the body's last field
/// keeps its stored result).
fn shown(codes: &[&str]) -> Vec<String> {
    let mut fields = vec![field("SEQ fig", "9", false)];
    fields.extend(codes.iter().map(|c| field(c, "OLD", false)));
    fields.push(field("SEQ fig", "9", false));
    let doc = doc_with(fields);
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    let mut r = results(&out);
    assert_eq!(r.len(), codes.len() + 2);
    r.pop();
    r.remove(0);
    r
}

/// The four IF fields of Word's probes after a first field `first` (stored `stored`), as output shows them.
fn probe(first: &str, stored: &str) -> Vec<String> {
    let mut fields = vec![field(first, stored, false)];
    for n in 2..=5 {
        fields.push(field(&format!(r#"IF {n} = {n} "A{n}" "B{n}""#), &format!("B{n}"), false));
    }
    let doc = doc_with(fields);
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    results(&out).split_off(1)
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn if_fields_refresh_only_with_a_refreshed_field_and_never_as_the_last_field() {
    for first in ["SEQ fig", r#"DATE \@ "yyyy""#, "SECTION", "PAGE", "NUMPAGES", "SECTIONPAGES"] {
        assert_eq!(probe(first, "1"), strs(&["A2", "A3", "A4", "B5"]), "{first}");
    }
    for first in ["AUTHOR", "= 1 + 1", "NUMWORDS", "FILENAME", "DOCVARIABLE x", "SYMBOL 169", "LISTNUM", "AUTONUM"] {
        assert_eq!(probe(first, "1"), strs(&["B2", "B3", "B4", "B5"]), "{first}");
    }
    // No other field at all: every IF keeps its stored result.
    let fields: Vec<InlineObject> = (2..=5).map(|n| field(&format!(r#"IF {n} = {n} "A{n}" "B{n}""#), &format!("B{n}"), false)).collect();
    let doc = doc_with(fields);
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    assert_eq!(results(&out), strs(&["B2", "B3", "B4", "B5"]));
}

const ERR: &str = "Error! Unknown op code for conditional.";

#[test]
fn if_fields_show_their_fresh_result_at_output() {
    let cases: &[(&str, &str)] = &[
        (r#"IF 5 > 3 "yes" "no""#, "yes"),
        ("IF 5 > 3 yes no", "yes"),
        ("IF 5 = 4 yes", ""),
        (r#"IF 5 = 5 "multi word true" "multi word false""#, "multi word true"),
        ("IF 5 <> 4 yes no", "yes"),
        ("IF 5 >= 5 yes no", "yes"),
        ("IF 5 <= 4 yes no", "no"),
        ("IF 5 > 3 yes no extra", "yes"),
        ("IF 5 = 5 a b c", "a"),
        (r#"IF "" = "" yes no"#, "yes"),
        (r#"IF "a b" = "a b" yes no"#, "yes"),
        (r#"IF 5 = "" yes no"#, "no"),
        (r#"IF 5 > 3 "yes" "no" \* Upper"#, "YES"),
        (r#"IF 1 = 2 "WRONG" "RIGHT""#, "RIGHT"),
        (r#"IF "b" <> "a" "NE_TRUE" "NE_FALSE""#, "NE_TRUE"),
    ];
    let codes: Vec<&str> = cases.iter().map(|c| c.0).collect();
    let got = shown(&codes);
    for ((code, want), got) in cases.iter().zip(&got) {
        assert_eq!(got, want, "{code}");
    }
    assert_eq!(got.len(), cases.len());
}

#[test]
fn if_comparisons_follow_words_number_and_text_rules() {
    let cases: &[(&str, &str)] = &[
        ("IF 10 > 9 yes no", "yes"),
        (r#"IF "10" > "9" yes no"#, "no"),
        ("IF abc < abd yes no", "yes"),
        (r#"IF 5 > "abc" yes no"#, "no"),
        ("IF 5.0 = 5 yes no", "yes"),
        ("IF -1 < 0 yes no", "yes"),
        ("IF 1,000 > 999 yes no", "yes"),
        (r#"IF "abc" = "ABC" yes no"#, "no"),
        (r#"IF "1" = "1" "QUOTED_MATCH" "QUOTED_MISS""#, "QUOTED_MATCH"),
    ];
    let codes: Vec<&str> = cases.iter().map(|c| c.0).collect();
    for ((code, want), got) in cases.iter().zip(&shown(&codes)) {
        assert_eq!(got, want, "{code}");
    }
}

#[test]
fn if_wildcards_only_on_the_right_and_only_for_equality() {
    let cases: &[(&str, &str)] = &[
        (r#"IF "abc" = "a*" yes no"#, "yes"),
        (r#"IF "abc" = "a?c" yes no"#, "yes"),
        (r#"IF "abc" <> "a*" yes no"#, "no"),
        (r#"IF "abc" < "a*" yes no"#, "no"),
        (r#"IF "a?c" = "abc" yes no"#, "no"),
    ];
    let codes: Vec<&str> = cases.iter().map(|c| c.0).collect();
    for ((code, want), got) in cases.iter().zip(&shown(&codes)) {
        assert_eq!(got, want, "{code}");
    }
}

#[test]
fn if_without_spaces_around_the_operator_is_words_error() {
    assert_eq!(shown(&["IF 5=5 yes no", "IF 5 ! 5 yes no"]), vec![ERR.to_string(), ERR.to_string()]);
}

#[test]
fn locked_if_and_formulas_keep_their_stored_result() {
    let doc = doc_with(vec![
        field("SEQ fig", "9", false),
        field("IF 5 > 3 yes no", "OLD", true),
        field("= 1 + 1", "3", false),
        field("SEQ fig", "9", false),
    ]);
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    assert_eq!(results(&out)[1..3], strs(&["OLD", "3"]));
}

#[test]
fn output_leaves_the_document_itself_alone() {
    let doc = doc_with(vec![field("SEQ fig", "1", false), field("IF 5 > 3 yes no", "OLD", false), field("SEQ fig", "2", false)]);
    let out = output_fields(&doc).expect("the IF shows a fresh result, so output is a copy");
    assert_eq!(results(&out)[1], "yes");
    assert_eq!(results(&doc)[1], "OLD");
}
