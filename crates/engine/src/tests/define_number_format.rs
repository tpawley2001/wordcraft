//! Define New Number Format (`para.defineNumber`): the number's alignment and font are stored on
//! the new list's first level, and formats the dialog can't make are refused, as in Word.

use serde_json::{Value, json};
use wordcraft_doc::numbering::Level;
use wordcraft_doc::props::{Align, Rgb, TextColor};
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::{Document, Pos};

use crate::{CmdError, Session};

fn session() -> Session {
    let mut s = Session::new(Document::from_text("First\nSecond\nThird"));
    s.run("select.all", &json!({})).unwrap();
    s
}

/// Runs the command and returns the new list's id and its first level.
fn defined(s: &mut Session, params: Value) -> (u32, Level) {
    let r = s.run("para.defineNumber", &params).unwrap();
    let num = r["num"].as_u64().expect("the result names the new list") as u32;
    let level = s.doc.numbering.level(num, 0).expect("the new list has a first level").clone();
    (num, level)
}

fn list_of(s: &Session, para: usize) -> Option<(u32, u8)> {
    let p = s.doc.para_at(&Pos::body(para, 0)).unwrap();
    p.props.numbering.map(|n| (n.num, n.level))
}

#[test]
fn the_plain_command_makes_a_left_aligned_decimal_list_and_names_it() {
    let mut s = session();
    let (num, level) = defined(&mut s, json!({}));
    assert_eq!((level.format, level.text.as_str(), level.align, level.start), (NumFormat::Decimal, "%1.", Align::Left, 1));
    for para in 0..3 {
        assert_eq!(list_of(&s, para), Some((num, 0)), "paragraph {para}");
    }
}

#[test]
fn alignment_is_stored_on_the_level() {
    for (given, want) in [("left", Align::Left), ("center", Align::Center), ("right", Align::Right)] {
        let mut s = session();
        let (_, level) = defined(&mut s, json!({"format": "upperRoman", "text": "Step %1:", "align": given}));
        assert_eq!((level.format, level.text.as_str(), level.align), (NumFormat::UpperRoman, "Step %1:", want), "{given}");
    }
    let mut s = session();
    let (_, level) = defined(&mut s, json!({"format": "lowerLetter", "text": "(%1)", "align": "center"}));
    assert_eq!((level.format, level.text.as_str(), level.align), (NumFormat::LowerLetter, "(%1)", Align::Center));
}

#[test]
fn the_numbers_font_is_stored_on_the_level() {
    let mut s = session();
    let (_, plain) = defined(&mut s, json!({"text": "%1)"}));
    let mut s = session();
    let (_, level) = defined(&mut s, json!({"text": "%1)", "bold": true, "italic": true, "font": "Arial", "color": "FF0000"}));
    assert_eq!(level.text, "%1)");
    assert_eq!(level.chr.bold, Some(true));
    assert_eq!(level.chr.italic, Some(true));
    assert_eq!(level.chr.font.as_deref(), Some("Arial"));
    assert_eq!(level.chr.color, Rgb::parse("FF0000").map(TextColor::Rgb));
    // Nothing else about the number's font changes.
    let mut expected = plain.chr.clone();
    expected.bold = Some(true);
    expected.italic = Some(true);
    expected.font = Some("Arial".into());
    expected.color = Rgb::parse("FF0000").map(TextColor::Rgb);
    assert_eq!(level.chr, expected);
}

#[test]
fn start_still_sets_the_first_number() {
    let mut s = session();
    let (_, level) = defined(&mut s, json!({"format": "decimal", "text": "%1.", "start": 5, "align": "right"}));
    assert_eq!((level.start, level.align), (5, Align::Right));
}

#[test]
fn formats_the_dialog_cannot_make_are_refused_and_add_no_list() {
    let cases = [
        (json!({"format": "bullet", "text": "%1."}), "bullet"),
        (json!({"format": "decimal", "text": "Step"}), "%1"),
        (json!({"format": "decimal", "text": ""}), "%1"),
        (json!({"format": "decimal", "text": "%1.%2."}), "%1"),
        (json!({"format": "upperLetter", "text": "%9 %1"}), "%1"),
        (json!({"format": "decimal", "text": "%1.", "align": "justify"}), "align"),
        (json!({"format": "decimal", "text": "%1.", "align": "Right"}), "align"),
    ];
    for (params, word) in cases {
        let mut s = session();
        let lists = (s.doc.numbering.nums.len(), s.doc.numbering.abstracts.len());
        match s.run("para.defineNumber", &params) {
            Err(CmdError::Params(message)) => assert!(message.contains(word), "{params}: `{message}` should mention `{word}`"),
            other => panic!("{params}: expected a parameter error, got {other:?}"),
        }
        assert_eq!((s.doc.numbering.nums.len(), s.doc.numbering.abstracts.len()), lists, "{params}: no list is added");
        assert_eq!(list_of(&s, 0), None, "{params}: the paragraphs stay unnumbered");
    }
}

#[test]
fn the_none_style_takes_text_without_a_number() {
    let mut s = session();
    let (num, level) = defined(&mut s, json!({"format": "none", "text": "Note:"}));
    assert_eq!((level.format, level.text.as_str()), (NumFormat::None, "Note:"));
    assert_eq!(list_of(&s, 0), Some((num, 0)));
}

#[test]
fn one_undo_takes_the_numbering_off_again() {
    let mut s = session();
    let depth = s.undo_depth();
    let (num, _) = defined(&mut s, json!({"format": "lowerRoman", "text": "%1)", "align": "right", "bold": true}));
    assert_eq!(s.undo_depth(), depth + 1);
    assert_eq!(list_of(&s, 2), Some((num, 0)));
    s.run("edit.undo", &json!({})).unwrap();
    for para in 0..3 {
        assert_eq!(list_of(&s, para), None, "paragraph {para}");
    }
}
