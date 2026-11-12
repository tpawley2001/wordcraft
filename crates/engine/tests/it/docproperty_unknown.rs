//! Update Field on a `DOCPROPERTY` field whose name is neither a custom property nor one of Word's
//! built-in properties stores Word's error text, `Error! Unknown document property name.`, whatever
//! `\*` switches the code has. A name Word knows keeps working (or keeps its stored result when
//! WordCraft cannot compute it yet).

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::para::InlineObject;
use wordcraft_engine::Session;

const UNKNOWN: &str = "Error! Unknown document property name.";

/// Word's built-in DocProperty names that WordCraft does not compute: their stored result stays.
const KEPT: [&str; 7] = ["Bytes", "Lines", "NameofApplication", "ODMADocId", "Security", "Template", "TotalEditingTime"];

/// A session whose body holds one field per code, each with the stored result `stored`.
fn session_with(codes: &[String], stored: &str) -> Session {
    let mut s = Session::new(Document::new());
    for code in codes {
        s.run("insert.field", &json!({"instr": code, "result": stored})).unwrap();
        s.run("text.insert", &json!({"text": " "})).unwrap();
    }
    s
}

/// The displayed results of the fields in the body, in order.
fn results(s: &Session) -> Vec<String> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn update(s: &mut Session) -> Vec<String> {
    s.run("references.updateFields", &json!({})).unwrap();
    results(s)
}

fn owned(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|c| (*c).to_string()).collect()
}

#[test]
fn an_unknown_property_name_shows_words_error() {
    let codes = owned(&["DOCPROPERTY Nope", r#"DOCPROPERTY "No Such Property" \* MERGEFORMAT"#, r#"DOCPROPERTY "Nope""#]);
    let mut s = session_with(&codes, "stale");
    assert_eq!(update(&mut s), [UNKNOWN, UNKNOWN, UNKNOWN]);
}

#[test]
fn the_error_is_not_changed_by_a_capitalization_switch() {
    let codes = owned(&[
        r"DOCPROPERTY Nope \* Upper",
        r"DOCPROPERTY Nope \* Lower",
        r"DOCPROPERTY Nope \* Caps",
        r"DOCPROPERTY Nope \* FirstCap",
        r"DOCPROPERTY Nope \* Upper \* MERGEFORMAT",
    ]);
    let mut s = session_with(&codes, "stale");
    assert_eq!(update(&mut s), [UNKNOWN; 5]);
}

#[test]
fn custom_and_built_in_properties_still_show_their_value() {
    let codes = owned(&["DOCPROPERTY ProjectName", r"DOCPROPERTY projectname \* Upper", r"DOCPROPERTY Title \* Upper", "DOCPROPERTY LastSavedBy"]);
    let mut s = session_with(&codes, "stale");
    s.run("file.properties", &json!({"custom": {"ProjectName": "Atlas"}})).unwrap();
    s.doc.core.title = "Report".into();
    s.doc.core.last_modified_by = "Ada".into();
    assert_eq!(update(&mut s), ["Atlas", "ATLAS", "REPORT", "Ada"]);
}

#[test]
fn a_built_in_name_that_is_not_computed_keeps_its_stored_result() {
    let mut codes = Vec::new();
    for name in KEPT {
        codes.push(format!("DOCPROPERTY {name}"));
        codes.push(format!("DOCPROPERTY {}", name.to_ascii_lowercase()));
        codes.push(format!(r#"DOCPROPERTY "{name}" \* MERGEFORMAT"#));
    }
    let mut s = session_with(&codes, "kept");
    let got = update(&mut s);
    assert_eq!(got.len(), codes.len());
    for (code, result) in codes.iter().zip(&got) {
        assert_eq!(result, "kept", "{code}");
    }
}

#[test]
fn a_field_with_no_name_shows_the_no_name_error() {
    let codes = owned(&["DOCPROPERTY", r"DOCPROPERTY \* Upper"]);
    let mut s = session_with(&codes, "kept");
    assert_eq!(update(&mut s), ["Error! No property name supplied."; 2]);
}

#[test]
fn the_error_gives_way_to_the_value_once_the_property_exists() {
    let mut s = session_with(&owned(&["DOCPROPERTY Nope"]), "stale");
    assert_eq!(update(&mut s), [UNKNOWN]);
    s.run("file.properties", &json!({"custom": {"Nope": "now set"}})).unwrap();
    assert_eq!(update(&mut s), ["now set"]);
}
