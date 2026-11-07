//! AUTONUM, AUTONUMLGL and AUTONUMOUT fields (ECMA-376 §17.16.5.4-6) number paragraphs as Word does
//! on F9 and in output: one set of level counters shared by the three, the level from the heading
//! style or the last heading + 1. Expected results read from Microsoft 365 Word's PDF of a probe
//! document whose fields were stored with the result "OLD".

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

/// (paragraph text, style or "" for Normal, field code or "" for none, Word's result)
const PROBE: [(&str, &str, &str, &str); 16] = [
    ("Fields", "Heading 2", "", ""),
    ("A01", "", "AUTONUM", "1."),
    ("A02", "", "AUTONUM", "2."),
    ("A03", "", r"AUTONUM \s :", "3:"),
    ("A04", "", "AUTONUM", "4."),
    ("A05", "Heading 1", "AUTONUMLGL", "1."),
    ("A06", "Heading 2", "AUTONUMLGL", "1.1."),
    ("A07", "Heading 2", "AUTONUMLGL", "1.2."),
    ("A08", "Heading 1", "AUTONUMLGL", "2."),
    ("A09", "Heading 2", r"AUTONUMLGL \e", "2.1"),
    ("A10", "", "AUTONUMLGL", "2.1.1."),
    ("A11", "Heading 1", "AUTONUMOUT", "III."),
    ("A12", "Heading 2", "AUTONUMOUT", "A."),
    ("A13", "Heading 3", "AUTONUMOUT", "1."),
    ("A14", "Heading 2", "AUTONUMOUT", "B."),
    ("A15", "", "AUTONUM", "1."),
];

/// The probe, preceded by `before` (paragraph text, field code) Normal paragraphs; each field
/// stored with the stale result "OLD", the rows starting with one of `locked` locked.
fn probe(before: &[(&str, &str)], locked: &[&str]) -> Session {
    let mut rows: Vec<(&str, &str, &str)> = before.iter().map(|(t, f)| (*t, "", *f)).collect();
    rows.extend(PROBE.iter().map(|r| (r.0, r.1, r.2)));
    let text: Vec<&str> = rows.iter().map(|r| r.0).collect();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text.join("\n")));
    for (t, style, instr) in &rows {
        s.run("select.text", &json!({"text": t})).unwrap();
        if !style.is_empty() {
            run(&mut s, "styles.apply", json!({"style": style}));
        }
        if !instr.is_empty() {
            run(&mut s, "caret.end", json!({}));
            run(&mut s, "insert.field", json!({"instr": instr}));
        }
    }
    for path in s.doc.para_paths(StoryRef::Body) {
        let p = s.doc.para_mut(StoryRef::Body, &path).unwrap();
        let is_locked = locked.iter().any(|l| p.text.starts_with(l));
        for o in &mut p.objects {
            if let InlineObject::Field { result, locked, .. } = o {
                *result = "OLD".into();
                *locked = is_locked;
            }
        }
    }
    s
}

fn want() -> Vec<(String, String)> {
    PROBE.iter().filter(|r| !r.2.is_empty()).map(|r| (r.2.to_string(), r.3.to_string())).collect()
}

#[test]
fn f9_numbers_autonum_fields_as_word_does() {
    let mut s = probe(&[], &[]);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(fields(&s), want());
}

#[test]
fn listnum_fields_before_do_not_move_the_autonum_counters() {
    let mut s = probe(&[("L1", "LISTNUM"), ("L2", r"LISTNUM \l 3")], &[]);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let got = fields(&s);
    assert_eq!(got[..2], [("LISTNUM".to_string(), "1)".to_string()), (r"LISTNUM \l 3".to_string(), "i)".to_string())]);
    assert_eq!(got[2..], want());
}

#[test]
fn a_locked_autonum_keeps_its_result() {
    let mut s = probe(&[], &["A02"]);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    // Whether Word still counts a locked field was not measured: only its own result is checked.
    let got: Vec<String> = fields(&s).into_iter().map(|f| f.1).take(2).collect();
    assert_eq!(got, ["1.", "OLD"]);
}
