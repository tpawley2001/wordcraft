//! Paste Special: Word's Result texts, Unformatted Unicode Text in the list, and unformatted text
//! taking the formatting at the insertion point, as in Word.

use serde_json::{Value, json};
use wordcraft_doc::{Document, Pos};

use crate::Session;
use crate::cmd::paste::result_text;

const WITH_FORMATTING: &str = "Inserts the contents of the Clipboard as text with font and table formatting.";
const WITHOUT_FORMATTING: &str = "Inserts the contents of the Clipboard as text without any formatting.";

/// "Bold words" with "Bold" in bold.
fn bold_words() -> Session {
    let mut doc = Document::from_text("Bold words");
    doc.format_range(&Pos::body(0, 0), &Pos::body(0, 4), &|c| c.bold = Some(true)).unwrap();
    Session::new(doc)
}

fn para(s: &Session, block: usize) -> String {
    s.doc.para_at(&Pos::body(block, 0)).unwrap().plain_text()
}

fn bold_at(s: &Session, off: usize) -> bool {
    s.doc.para_at(&Pos::body(0, off)).and_then(|p| p.props_of_char(off).bold).unwrap_or(false)
}

fn listed(s: &mut Session, payload: Value) -> Vec<(String, String, String)> {
    let r = s.run("edit.pasteSpecial", &payload).unwrap();
    r["formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let field = |k: &str| f[k].as_str().unwrap_or_default().to_string();
            (field("id"), field("label"), field("result"))
        })
        .collect()
}

#[test]
fn result_text_is_words_wording_for_each_format() {
    assert_eq!(result_text("formatted"), WITH_FORMATTING);
    assert_eq!(result_text("rtf"), WITH_FORMATTING);
    assert_eq!(result_text("html"), "Inserts the contents of the Clipboard as HTML Format.");
    assert_eq!(result_text("picture"), "Inserts the contents of the Clipboard as a picture.");
    assert_eq!(result_text("text"), WITHOUT_FORMATTING);
    assert_eq!(result_text("unicode"), WITHOUT_FORMATTING);
    assert_eq!(result_text("bogus"), "");
}

#[test]
fn plain_text_lists_unformatted_text_then_unformatted_unicode_text() {
    let mut s = Session::new(Document::from_text("x"));
    let got = listed(&mut s, json!({"text": "plain"}));
    let want = [("text", "Unformatted Text", WITHOUT_FORMATTING), ("unicode", "Unformatted Unicode Text", WITHOUT_FORMATTING)];
    let want: Vec<(String, String, String)> = want.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
    assert_eq!(got, want);
}

#[test]
fn rtf_is_labelled_formatted_text_rtf_and_unicode_comes_last() {
    let mut s = Session::new(Document::from_text("x"));
    let got = listed(&mut s, json!({"text": "bold", "rtf": r"{\rtf1\ansi {\b bold}}", "html": "<p><b>bold</b></p>"}));
    let ids: Vec<&str> = got.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(ids, ["rtf", "html", "text", "unicode"]);
    assert_eq!(got[0].1, "Formatted Text (RTF)");
    assert_eq!(got[0].2, WITH_FORMATTING);
    assert_eq!(got[1].1, "HTML Format");
    assert_eq!(got[1].2, "Inserts the contents of the Clipboard as HTML Format.");
}

#[test]
fn unformatted_text_inside_bold_text_is_bold() {
    for (fmt, off) in [("text", 2), ("unicode", 2), ("UNICODE", 2)] {
        let mut s = bold_words();
        s.run("caret.set", &json!({"pos": Pos::body(0, off)})).unwrap();
        let r = s.run("edit.pasteSpecial", &json!({"as": fmt, "text": "xy"})).unwrap();
        assert_eq!(r["pastedAs"], fmt.to_ascii_lowercase(), "{fmt}");
        assert_eq!(para(&s, 0), "Boxyld words", "{fmt}");
        assert!(bold_at(&s, 2) && bold_at(&s, 3), "{fmt}: pasted text takes the bold around it");
        assert!(!bold_at(&s, 8), "{fmt}: the plain text after it stays plain");
    }
}

#[test]
fn unformatted_text_after_plain_text_is_plain() {
    for fmt in ["text", "unicode"] {
        let mut s = bold_words();
        s.run("caret.set", &json!({"pos": Pos::body(0, 10)})).unwrap();
        s.run("edit.pasteSpecial", &json!({"as": fmt, "text": "xy"})).unwrap();
        assert_eq!(para(&s, 0), "Bold wordsxy", "{fmt}");
        assert!(!bold_at(&s, 10) && !bold_at(&s, 11), "{fmt}");
        assert!(bold_at(&s, 0), "{fmt}");
    }
}

#[test]
fn unicode_text_splits_paragraphs_at_line_breaks() {
    let mut s = Session::new(Document::from_text("Plain"));
    s.run("caret.set", &json!({"pos": Pos::body(0, 5)})).unwrap();
    let r = s.run("edit.pasteSpecial", &json!({"as": "unicode", "text": "a\nb"})).unwrap();
    assert_eq!(r["pastedAs"], "unicode");
    assert_eq!(para(&s, 0), "Plaina");
    assert_eq!(para(&s, 1), "b");
}

#[test]
fn unknown_format_error_names_unicode() {
    let mut s = Session::new(Document::from_text("x"));
    let e = s.run("edit.pasteSpecial", &json!({"as": "bogus", "text": "plain"})).unwrap_err();
    assert!(e.to_string().contains("text or unicode"), "{e}");
}
