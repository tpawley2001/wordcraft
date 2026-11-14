//! QUOTE with a date-time picture (ECMA-376 §17.16.5.50, §17.16.4.1): the quoted text is read as a
//! date and/or time and shown with the `\@` picture. Expected results measured in Microsoft 365
//! Word for Windows (en-US), updating the fields.

use serde_json::{Value, json};
use wordcraft_doc::{InlineObject, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Every body field's code and result, in document order.
fn fields(s: &Session) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o {
                out.push((instr.trim().to_string(), result.clone()));
            }
        }
    }
    out
}

/// (field code, Word's result)
const WORD: [(&str, &str); 6] = [
    (r#"QUOTE "14/11/2026 13:05:09" \@ "h:mm am/pm""#, "1:05 PM"),
    (r#"QUOTE "2026-11-14 13:05:09" \@ "h:mm AM/PM""#, "1:05 PM"),
    (r#"QUOTE "00:05" \@ "h:mm AM/PM""#, "12:05 AM"),
    (r#"QUOTE "12:30" \@ "h:mm am/pm""#, "12:30 PM"),
    (r#"QUOTE "00:05" \@ "hh:mm""#, "12:05"),
    (r#"QUOTE "November 14, 2026 9:07 PM" \@ "HH:mm:ss""#, "21:07:00"),
];

#[test]
fn quote_formats_date_and_time_text_with_its_picture() {
    let text: Vec<String> = (0..WORD.len()).map(|i| format!("R{i}")).collect();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text.join("\n")));
    for (i, (instr, _)) in WORD.iter().enumerate() {
        run(&mut s, "select.text", json!({"text": format!("R{i}")}));
        run(&mut s, "caret.end", json!({}));
        run(&mut s, "insert.field", json!({"instr": instr}));
    }
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let want: Vec<(String, String)> = WORD.iter().map(|(i, r)| (i.to_string(), r.to_string())).collect();
    assert_eq!(fields(&s), want);
}

#[test]
fn plain_quote_shows_its_text_on_update() {
    let mut s = Session::new(wordcraft_doc::Document::from_text("R0"));
    run(&mut s, "select.text", json!({"text": "R0"}));
    run(&mut s, "caret.end", json!({}));
    run(&mut s, "insert.field", json!({"instr": r#"QUOTE "13:05""#}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(fields(&s), [(r#"QUOTE "13:05""#.to_string(), "13:05".to_string())]);
}
