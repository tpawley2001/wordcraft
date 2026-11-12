//! Update Field on a `DOCVARIABLE` field (ECMA-376 Part 1 §17.16.5.14) shows the document
//! variable it names, or Microsoft Word's error text. The expected results were measured in
//! Microsoft 365 Word (16.0.20527) on a file holding the variables below.

use serde_json::json;
use wordcraft_doc::{DocVar, InlineObject};

use crate::Session;

/// (field code, the result after Update Field).
const ROWS: [(&str, &str); 9] = [
    ("DOCVARIABLE Client", "Acme"),
    ("DOCVARIABLE client", "Acme"),
    (r#"DOCVARIABLE "Two Words""#, "Second value"),
    (r"DOCVARIABLE Client \* Upper", "ACME"),
    ("DOCVARIABLE  Client  MERGEFORMAT", "Acme"),
    ("DOCVARIABLE Two Words", "Error! No document variable supplied."),
    ("DOCVARIABLE Missing", "Error! No document variable supplied."),
    ("DOCVARIABLE Empty", "Error! No document variable supplied."),
    ("DOCVARIABLE", "Error! Document Variable not defined."),
];

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    for (name, value) in [("Client", "Acme"), ("Two Words", "Second value"), ("Empty", "")] {
        s.doc.settings.doc_vars.push(DocVar { name: name.into(), value: value.into() });
    }
    s
}

/// The results of the fields in the first body paragraph, in order.
fn results(s: &Session) -> Vec<String> {
    s.doc
        .body
        .first()
        .and_then(|b| b.as_para())
        .map(|p| {
            p.objects
                .iter()
                .filter_map(|o| match o {
                    InlineObject::Field { result, .. } => Some(result.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn update_field_shows_the_variable_or_words_error_text() {
    for (instr, want) in ROWS {
        let mut s = session();
        s.run("insert.field", &json!({"instr": instr, "result": "stale"})).unwrap();
        assert_eq!(results(&s), ["stale"], "{instr}: the stored result stays until the field is updated");
        s.run("references.updateFields", &json!({})).unwrap();
        assert_eq!(results(&s), [want], "{instr}");
    }
}

#[test]
fn several_fields_update_together_and_follow_a_changed_variable() {
    let mut s = session();
    s.run("insert.field", &json!({"instr": "DOCVARIABLE Client", "result": "old"})).unwrap();
    s.run("text.insert", &json!({"text": " and "})).unwrap();
    s.run("insert.field", &json!({"instr": "DOCVARIABLE Missing", "result": "old"})).unwrap();
    s.run("references.updateFields", &json!({})).unwrap();
    assert_eq!(results(&s), ["Acme", "Error! No document variable supplied."]);

    s.doc.settings.doc_vars[0].value = "Zenith".into();
    s.doc.settings.doc_vars.push(DocVar { name: "missing".into(), value: "found".into() });
    s.run("references.updateFields", &json!({})).unwrap();
    assert_eq!(results(&s), ["Zenith", "found"]);
}
