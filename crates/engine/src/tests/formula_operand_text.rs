//! A formula field (`= …`, ECMA-376 §17.16.5.1) reads every number in an operand's text and
//! carries the operand's look into its result, as Microsoft Word does. Every row below was
//! measured in Microsoft 365 Word (16.0.20527, en-US) with the text in a bookmark and again with
//! the text in a table cell; the two agree. Word keeps a stored result until the field is
//! updated, so these tests update the fields.

use serde_json::{Value, json};
use wordcraft_doc::{CharProps, InlineObject, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (bookmark name, the text it marks): one paragraph each, at the start of the body. The names
/// are letters only, so none reads as a cell reference or a function.
const OPERANDS: [(&str, &str); 18] = [
    ("Opa", "3 apples 4"),
    ("Opb", "a 2, b 3"),
    ("Opc", "Total: 1,234.50 USD"),
    ("Opd", "5-3"),
    ("Ope", "10 - 4"),
    ("Opf", "-5"),
    ("Opg", "50%"),
    ("Oph", "12abc"),
    ("Opi", "3.4.5"),
    ("Opj", "1.234,50"),
    ("Opk", "1 000"),
    ("Opl", "1e3"),
    ("Opm", "$5"),
    ("Opn", "1,234.50"),
    ("Opo", "(5)"),
    ("Opp", "12"),
    ("Opq", "abc"),
    ("Opr", "1234.5"),
];

/// (field code, what Word shows after Update Field) for operands that hold more than one number
/// or text around their numbers: every number is added up, `-` subtracts, `%` divides by 100.
const VALUE_ROWS: [(&str, &str); 12] = [
    ("= Opa", "7"),
    ("= Opb", "5"),
    ("= Opd", "2"),
    ("= Ope", "6"),
    ("= Opf + 1", "-4"),
    ("= Opg", "0.5"),
    ("= Oph", "12"),
    ("= Opi", "3.9"),
    ("= Opj", "51.234"),
    ("= Opk", "1"),
    ("= Opl", "0"),
    ("= Opq + 1", "1"),
];

/// (field code, what Word shows): the result keeps the operand's currency sign, thousands
/// separator and parentheses; a plain operand gives a plain result.
const LOOK_ROWS: [(&str, &str); 10] = [
    ("= Opc", "1,234.5"),
    ("= Opm", "$5.00"),
    ("= Opm + 1", "$6.00"),
    ("= Opn", "1,234.5"),
    ("= Opn * 2", "2,469"),
    ("= Opo", "(5)"),
    ("= Opo + 1", "(4)"),
    ("= Opp / 7", "1.71"),
    ("= Opp * 1000", "12000"),
    ("= Opr", "1234.5"),
];

/// The operand paragraphs, each wrapped in its bookmark, then one paragraph per row ending in
/// its field with the stale result "OLD".
fn session(rows: &[(&str, &str)]) -> Session {
    let mut text: Vec<String> = OPERANDS.iter().map(|b| b.1.to_string()).collect();
    text.extend((0..rows.len()).map(|i| format!("F{:02}", i + 1)));
    let mut doc = wordcraft_doc::Document::from_text(&text.join("\n"));
    for (i, path) in doc.para_paths(StoryRef::Body).into_iter().enumerate() {
        let p = doc.para_mut(StoryRef::Body, &path).unwrap();
        if let Some((name, _)) = OPERANDS.get(i) {
            p.insert_object(p.len(), InlineObject::BookmarkEnd { name: (*name).into() }, &CharProps::default()).unwrap();
            p.insert_object(0, InlineObject::BookmarkStart { name: (*name).into() }, &CharProps::default()).unwrap();
            continue;
        }
        let field = InlineObject::Field {
            instr: rows[i - OPERANDS.len()].0.to_string(),
            result: "OLD".into(),
            locked: false,
            dirty: false,
            code: false,
            form: String::new(),
code_paras: None,
        };
        p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    }
    Session::new(doc)
}

/// The rows whose field shows something else than Word after Update Field, as
/// "code: got X, Word shows Y".
fn mismatches(rows: &[(&str, &str)]) -> Vec<String> {
    let mut s = session(rows);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let mut got = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o {
                got.push((instr.clone(), result.clone()));
            }
        }
    }
    assert_eq!(got.len(), rows.len(), "one field per row");
    got.iter()
        .zip(rows)
        .filter(|((_, result), (_, want))| result != want)
        .map(|((code, result), (_, want))| format!("{code}: got {result}, Word shows {want}"))
        .collect()
}

#[test]
fn the_operands_are_bookmarked_as_written() {
    let s = session(&VALUE_ROWS);
    for (name, text) in OPERANDS {
        assert_eq!(crate::cmd::references::bookmark_text(&s.doc, name).as_deref(), Some(text), "{name}");
    }
}

#[test]
fn every_number_in_an_operands_text_counts() {
    let wrong = mismatches(&VALUE_ROWS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_result_keeps_the_operands_look() {
    let wrong = mismatches(&LOOK_ROWS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_numeric_picture_still_wins_over_the_operands_look() {
    let rows = [(r#"= Opm + 1 \# "0.0""#, "6.0"), (r#"= Opp * 2 \# "0.00""#, "24.00")];
    let wrong = mismatches(&rows);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
