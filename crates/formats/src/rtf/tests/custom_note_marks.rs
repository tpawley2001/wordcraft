//! Footnotes and endnotes with a custom reference mark (RTF 1.9.1: a note whose reference has no
//! `\chftn` uses the text written there), the RTF form of DOCX `w:customMarkFollows`.
//! Word (Microsoft 365) reads `Alpha{\super *}{\footnote\pard {\super *} Star note.}` as body "Alpha*"
//! with a "*" note that takes no number.

use super::*;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::{Block, PartKind};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

/// `\ftnbj` states the footnote position (as Word writes it), so `{\footnote}` without `\ftnalt`
/// stays a footnote; without any position word Word opens such notes as endnotes.
fn wrap(body: &str) -> String {
    format!(r"{{\rtf1\ansi\ftnbj{{\fonttbl{{\f0 Times New Roman;}}}}\pard {body}}}")
}

/// The body's first paragraph text and its note references as `(kind, custom)`.
fn body_refs(d: &Document) -> (String, Vec<(NoteKind, String)>) {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("body paragraph");
    let refs = p
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::NoteRef { kind, custom, .. } => Some((*kind, custom.clone())),
            _ => None,
        })
        .collect();
    (p.plain_text().trim().to_string(), refs)
}

/// Each note's text and the `custom` of its own leading mark (None when it has none).
fn notes(d: &Document) -> Vec<(String, Option<String>)> {
    d.parts
        .values()
        .filter(|p| matches!(p.kind, PartKind::Footnote | PartKind::Endnote))
        .map(|p| {
            let para = p.blocks.iter().find_map(|b| if let Block::Para(q) = &**b { Some(q) } else { None }).expect("note paragraph");
            let own = match para.objects.first() {
                Some(InlineObject::NoteRef { custom, .. }) => Some(custom.clone()),
                _ => None,
            };
            (para.plain_text().trim().to_string(), own)
        })
        .collect()
}

#[test]
fn group_before_footnote_is_its_custom_mark() {
    let d = doc(&wrap(r"Alpha{\super *}{\footnote\pard {\super *} Star note.}\par"));
    assert_eq!(body_refs(&d), ("Alpha".to_string(), vec![(NoteKind::Footnote, "*".to_string())]));
    assert_eq!(notes(&d), [("Star note.".to_string(), Some("*".to_string()))]);
}

#[test]
fn custom_mark_on_a_footnote_without_a_position_opens_as_an_endnote_with_it() {
    // No footnote position word: Word opens the note as an endnote; the custom mark is kept.
    let rtf = format!(r"{{\rtf1\ansi{{\fonttbl{{\f0 Times New Roman;}}}}\pard {}}}", r"Alpha{\super *}{\footnote\pard {\super *} Star note.}\par");
    let d = doc(&rtf);
    assert_eq!(body_refs(&d), ("Alpha".to_string(), vec![(NoteKind::Endnote, "*".to_string())]));
    assert_eq!(notes(&d), [("Star note.".to_string(), Some("*".to_string()))]);
    assert!(d.parts.values().any(|p| p.kind == PartKind::Endnote));
}

#[test]
fn chftn_note_stays_numbered() {
    let d = doc(&wrap(r"Beta{\super\chftn}{\footnote\pard {\super\chftn} Auto.}\par"));
    assert_eq!(body_refs(&d), ("Beta".to_string(), vec![(NoteKind::Footnote, String::new())]));
    assert_eq!(notes(&d), [("Auto.".to_string(), Some(String::new()))]);
}

#[test]
fn unicode_custom_mark_on_an_endnote() {
    let d = doc(&wrap(r"D{\super \u8224?}{\footnote\ftnalt\pard {\super \u8224?} Dag.}\par"));
    assert_eq!(body_refs(&d), ("D".to_string(), vec![(NoteKind::Endnote, "\u{2020}".to_string())]));
    assert_eq!(notes(&d), [("Dag.".to_string(), Some("\u{2020}".to_string()))]);
}

#[test]
fn text_longer_than_ten_characters_is_not_a_mark() {
    let d = doc(&wrap(r"Long{\super abcdefghijk}{\footnote\pard x}\par"));
    assert_eq!(body_refs(&d), ("Longabcdefghijk".to_string(), vec![(NoteKind::Footnote, String::new())]));
}

#[test]
fn custom_mark_is_written_and_read_back() {
    let d = doc(&wrap(r"Alpha{\super *}{\footnote\pard {\super *} Star note.}\par"));
    let out = export(&d);
    assert!(out.contains(r"{\super *}{\footnote"), "{out}");
    assert!(!out.contains(r"\chftn"), "{out}");
    let d2 = doc(&out);
    assert_eq!(body_refs(&d2), body_refs(&d));
    assert_eq!(notes(&d2), notes(&d));
}
