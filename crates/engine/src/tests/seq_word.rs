//! SEQ fields (ECMA-376 §17.16.5.56) number as Microsoft 365 Word numbers them on Update Field:
//! identifiers ignore case, `\r n` takes the number without its sign or fraction, `\h` shows the
//! result when a `\*` format switch is present, a bookmark argument gives the sequence's number at
//! that bookmark, and a SEQ without an identifier shows Word's error. Expected results measured in
//! Microsoft 365 Word (Fields.Update), one document per test, fields in document order.

use serde_json::{Value, json};
use wordcraft_doc::{CharProps, InlineObject, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Every body field's result, in document order.
fn results(s: &Session) -> Vec<String> {
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

/// One paragraph `Fnn ` per code, each ending in its field. The paragraph at index `bookmark.1`
/// (field included) is wrapped in the bookmark named `bookmark.0`.
fn session(codes: &[&str], bookmark: Option<(&str, usize)>) -> Session {
    let text: Vec<String> = (0..codes.len()).map(|i| format!("F{:02}", i + 1)).collect();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text.join("\n")));
    for (i, code) in codes.iter().enumerate() {
        s.run("select.text", &json!({"text": format!("F{:02}", i + 1)})).unwrap();
        run(&mut s, "caret.end", json!({}));
        run(&mut s, "insert.field", json!({"instr": code}));
    }
    if let Some((name, i)) = bookmark {
        let path = s.doc.para_paths(StoryRef::Body)[i].clone();
        let p = s.doc.para_mut(StoryRef::Body, &path).unwrap();
        p.insert_object(p.len(), InlineObject::BookmarkEnd { name: name.into() }, &CharProps::default()).unwrap();
        p.insert_object(0, InlineObject::BookmarkStart { name: name.into() }, &CharProps::default()).unwrap();
    }
    s
}

fn updated(codes: &[&str], bookmark: Option<(&str, usize)>) -> Vec<String> {
    let mut s = session(codes, bookmark);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    results(&s)
}

#[test]
fn seq_identifiers_ignore_case() {
    let codes = ["SEQ Figure", "SEQ Figure", r"SEQ Figure \c", r"SEQ Figure \n", "SEQ figure", "SEQ Table"];
    assert_eq!(updated(&codes, None), ["1", "2", "2", "3", "4", "1"]);
}

#[test]
fn seq_repeat_before_any_number_is_zero() {
    assert_eq!(updated(&[r"SEQ Figure \c", "SEQ Figure"], None), ["0", "1"]);
}

#[test]
fn seq_reset_takes_the_number_without_sign_or_fraction() {
    let codes = [
        "SEQ Figure",
        r"SEQ Figure \r 10",
        "SEQ Figure",
        r"SEQ Figure \r 0",
        "SEQ Figure",
        r"SEQ Figure \r -3",
        "SEQ Figure",
        r"SEQ Figure \r 2.7",
        "SEQ Figure",
    ];
    assert_eq!(updated(&codes, None), ["1", "10", "11", "0", "1", "3", "4", "2", "3"]);
}

#[test]
fn seq_hidden_shows_a_formatted_result() {
    let codes = ["SEQ Figure", r"SEQ Figure \h", "SEQ Figure", r"SEQ Figure \c \h", r"SEQ Figure \h \* ROMAN"];
    assert_eq!(updated(&codes, None), ["1", "", "3", "", "IV"]);
}

#[test]
fn seq_format_switches() {
    let codes = [
        r"SEQ Figure \r 12 \* ROMAN",
        r"SEQ Figure \* alphabetic",
        r"SEQ Figure \* ALPHABETIC",
        r"SEQ Figure \* roman",
        r"SEQ Figure \* Ordinal",
        r"SEQ Figure \* CardText",
        r"SEQ Figure \* Hex",
        r#"SEQ Figure \# "000""#,
        r"SEQ Figure \* ArabicDash",
    ];
    assert_eq!(updated(&codes, None), ["XII", "m", "N", "xv", "16th", "seventeen", "12", "019", "- 20 -"]);
}

#[test]
fn seq_bookmark_argument_gives_the_number_at_the_bookmark() {
    let codes = ["SEQ Figure", "SEQ Figure", "SEQ Figure", "SEQ Figure bm1", "SEQ Figure nosuch", r"SEQ Figure bm1 \c"];
    let want = ["1", "2", "3", "2", "Error! Bookmark not defined.", "2"];
    assert_eq!(updated(&codes, Some(("bm1", 1))), want);
}

#[test]
fn seq_without_identifier_is_an_error() {
    let codes = ["SEQ", r"SEQ \r 3", "SEQ Fig-1", "SEQ Fig_1", "SEQ 1Fig"];
    let err = "Error! No sequence specified.";
    assert_eq!(updated(&codes, None), [err, err, "1", "1", "1"]);
}
