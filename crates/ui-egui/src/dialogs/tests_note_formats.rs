//! Footnote and Endnote dialog: Word's symbol number format (*, †, ‡, §, …; OOXML `chicago`) is in
//! the Number format list, and the Start at box shows the start value in the chosen format.

use serde_json::json;
use wordcraft_doc::section::NumFormat;
use wordcraft_engine::Session;

use crate::dialogs::Dialog;
use crate::dialogs_notes::{FORMATS, start_label};
use crate::{Services, WordApp};

#[test]
fn the_number_format_list_ends_with_words_symbols() {
    let list: &[(&str, &str)] = &FORMATS;
    assert_eq!(list.len(), 6);
    let names: Vec<&str> = list.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["decimal", "lowerLetter", "upperLetter", "lowerRoman", "upperRoman", "chicago"]);
    assert_eq!(list.last(), Some(&("chicago", "*, \u{2020}, \u{2021}, \u{a7}, \u{2026}")));
}

#[test]
fn start_at_shows_the_value_in_the_chosen_format() {
    let symbols = [(1, "*"), (2, "\u{2020}"), (3, "\u{2021}"), (4, "\u{a7}"), (5, "**"), (6, "\u{2020}\u{2020}"), (8, "\u{a7}\u{a7}"), (9, "***")];
    for (n, want) in symbols {
        assert_eq!(start_label("chicago", n), want, "chicago {n}");
    }
    assert_eq!(start_label("decimal", 12), "12");
    assert_eq!(start_label("lowerRoman", 4), "iv");
    assert_eq!(start_label("upperRoman", 9), "IX");
    assert_eq!(start_label("lowerLetter", 3), "c");
    assert_eq!(start_label("upperLetter", 27), "AA");
}

#[test]
fn an_unknown_format_shows_the_plain_number() {
    assert_eq!(start_label("", 7), "7");
    assert_eq!(start_label("nonsense", 7), "7");
}

#[test]
fn choosing_symbols_in_the_dialog_numbers_the_documents_footnotes_with_them() {
    let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
    assert_eq!(a.run("references.noteOptions", json!({})).unwrap(), json!({"pending": "noteOptions"}));
    let Some(Dialog::NoteOptions { form }) = &mut a.dialog else { panic!("dialog") };
    form.fmt[0] = "chicago".into();
    let params = form.params();
    assert_eq!(params["numFmt"], "chicago");
    a.dialog = None;
    a.run("references.noteOptions", params).unwrap();
    // The document's note format lives in `Settings::footnote_format` (saved as settings.xml's w:footnotePr/w:numFmt).
    assert_eq!(a.session.doc.settings.footnote_format, NumFormat::Chicago);
    // The dialog opens again on the symbols; endnotes keep their own format.
    a.run("references.noteOptions", json!({})).unwrap();
    let Some(Dialog::NoteOptions { form }) = &a.dialog else { panic!("dialog") };
    assert_eq!((form.fmt[0].as_str(), form.fmt[1].as_str()), ("chicago", "lowerRoman"));
}
