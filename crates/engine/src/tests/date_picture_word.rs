//! DATE field pictures as Word reads them: d, M and y runs longer than four letters split into
//! pieces of at most four, `yyy` is not a valid item, and a `\*` number format applies to a
//! numeric date result. Expected results measured in Microsoft 365 Word 16.0.20527 (Fields.Add,
//! Update, Field.Result.Text), 2026-11-14; the date itself comes from the clock as the test runs.

use serde_json::json;
use wordcraft_doc::StoryRef;
use wordcraft_doc::para::InlineObject;

use crate::Session;
use crate::cmd::insert::format_date;

/// The result Update Fields gives a field with code `instr`.
fn result(instr: &str) -> String {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("insert.field", &json!({ "instr": instr })).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("references.updateFields", &json!({})).ok();
    let path = s.doc.para_paths(StoryRef::Body).into_iter().next().unwrap();
    s.doc
        .para(StoryRef::Body, &path)
        .unwrap()
        .objects
        .iter()
        .find_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .expect("a field")
}

#[test]
fn letter_runs_longer_than_four_split() {
    assert_eq!(result(r#"DATE \@ "yyyyy""#), format!("{}{}", format_date("yyyy"), format_date("yy")));
    assert_eq!(result(r#"DATE \@ "MMMMM""#), format!("{}{}", format_date("MMMM"), format_date("M")));
    assert_eq!(result(r#"DATE \@ "ddddd""#), format!("{}{}", format_date("dddd"), format_date("d")));
}

#[test]
fn three_ys_are_not_a_picture_item() {
    assert_eq!(result(r#"DATE \@ "yyy""#), "Error! Unknown character in picture string.");
}

#[test]
fn number_formats_apply_to_a_numeric_date() {
    let day: usize = format_date("d").parse().unwrap();
    let month: usize = format_date("M").parse().unwrap();
    let suffix = match (day % 10, day) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    assert_eq!(result(r#"DATE \@ "d" \* Ordinal"#), format!("{day}{suffix}"));
    let ord = [
        "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "eleventh", "twelfth", "thirteenth",
        "fourteenth", "fifteenth", "sixteenth", "seventeenth", "eighteenth", "nineteenth", "twentieth", "twenty-first", "twenty-second",
        "twenty-third", "twenty-fourth", "twenty-fifth", "twenty-sixth", "twenty-seventh", "twenty-eighth", "twenty-ninth", "thirtieth",
        "thirty-first",
    ];
    assert_eq!(result(r#"DATE \@ "d" \* OrdText"#), ord[day - 1]);
    let roman = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii"];
    assert_eq!(result(r#"DATE \@ "M" \* roman"#), roman[month - 1]);
    if format_date("yyyy") == "2026" {
        assert_eq!(result(r#"DATE \@ "yyyy" \* CardText"#), "two thousand twenty-six");
    }
}

#[test]
fn pictures_word_already_agrees_with_stay() {
    assert_eq!(result(r#"DATE \@ "MMMMd""#), format!("{}{}", format_date("MMMM"), format_date("d")));
    assert_eq!(result(r#"DATE \@ "d'th'""#), format!("{}th", format_date("d")));
    assert_eq!(result(r#"DATE \@ "dddd" \* Lower"#), format_date("dddd").to_lowercase());
}
