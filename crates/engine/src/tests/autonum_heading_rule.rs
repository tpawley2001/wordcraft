//! Which headings move the AUTONUM counters (ECMA-376 §17.16.5.4-6). Measured in Microsoft 365
//! Word (PDF export of probe documents whose fields were stored with the result "OLD"): a heading
//! paragraph changes nothing unless it holds an AUTONUM-family field itself, and the body-text
//! fields share one counter that a counted heading field resets.

use serde_json::{Value, json};
use wordcraft_doc::{InlineObject, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (paragraph text, heading level or 0 for Normal, field code or "" for none)
type Row = (&'static str, u8, &'static str);

/// A document of `rows`, each field stored with the stale result "OLD".
fn doc(rows: &[Row]) -> Session {
    let text: Vec<&str> = rows.iter().map(|r| r.0).collect();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text.join("\n")));
    for (t, level, instr) in rows {
        s.run("select.text", &json!({"text": t})).unwrap();
        if *level > 0 {
            run(&mut s, "styles.apply", json!({"style": format!("Heading {level}")}));
        }
        if !instr.is_empty() {
            run(&mut s, "caret.end", json!({}));
            run(&mut s, "insert.field", json!({"instr": instr}));
        }
    }
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &mut s.doc.para_mut(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                *result = "OLD".into();
            }
        }
    }
    s
}

/// Every body field's result after F9 on the whole document, in document order.
fn f9(rows: &[Row]) -> Vec<String> {
    let mut s = doc(rows);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
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

#[test]
fn headings_without_a_field_do_not_restart_body_numbers() {
    let rows: [Row; 10] = [
        ("A1", 0, "AUTONUM"),
        ("H1x", 1, ""),
        ("A2", 0, "AUTONUM"),
        ("A3", 0, "AUTONUM"),
        ("H2x", 2, ""),
        ("A4", 0, "AUTONUM"),
        ("H1y", 1, ""),
        ("A5", 0, "AUTONUM"),
        ("H3x", 3, ""),
        ("A6", 0, "AUTONUM"),
    ];
    assert_eq!(f9(&rows), ["1.", "2.", "3.", "4.", "5.", "6."]);
}

#[test]
fn a_counted_heading_field_restarts_the_body_numbers() {
    let rows: [Row; 9] = [
        ("B1H1", 1, "AUTONUM"),
        ("B2", 0, "AUTONUM"),
        ("B3H2", 2, "AUTONUM"),
        ("B4", 0, "AUTONUM"),
        ("B5", 0, "AUTONUM"),
        ("B6H1", 1, "AUTONUM"),
        ("B7", 0, "AUTONUM"),
        ("B8H3", 3, ""),
        ("B9", 0, "AUTONUM"),
    ];
    assert_eq!(f9(&rows), ["1.", "1.", "1.", "1.", "2.", "2.", "1.", "2."]);
}

#[test]
fn heading_levels_restart_only_at_a_higher_counted_heading() {
    let rows: [Row; 8] = [
        ("C1", 0, "AUTONUM"),
        ("C2", 0, "AUTONUM"),
        ("C3H2", 2, "AUTONUM"),
        ("C4", 0, "AUTONUM"),
        ("C5H1", 1, ""),
        ("C6", 0, "AUTONUM"),
        ("C7H2", 2, "AUTONUM"),
        ("C8", 0, "AUTONUM"),
    ];
    assert_eq!(f9(&rows), ["1.", "2.", "1.", "1.", "2.", "2.", "1."]);
}

#[test]
fn legal_numbers_after_a_counted_heading_keep_its_levels() {
    let rows: [Row; 9] = [
        ("H1", 1, ""),
        ("H2", 2, ""),
        ("H1a", 1, ""),
        ("H3", 3, ""),
        ("R11", 0, "AUTONUM"),
        ("R12", 0, "AUTONUM"),
        ("H1b", 1, ""),
        ("R13", 0, "AUTONUM"),
        ("R14H1", 1, "AUTONUM"),
    ];
    assert_eq!(f9(&rows), ["1.", "2.", "3.", "1."]);
    let mut more = rows.to_vec();
    more.push(("R15", 0, "AUTONUMLGL"));
    assert_eq!(f9(&more), ["1.", "2.", "3.", "1.", "1.1."]);
}
