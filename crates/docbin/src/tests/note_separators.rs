//! The six note separator stories at the start of the header document ([MS-DOC] §2.3.4,
//! §2.8.25 Plcfhdd): footnote separator, continuation separator and continuation notice, then the
//! same three for endnotes. A story holding only its own line (0x03 for the separator, 0x04 for
//! the continuation separator) is the default; anything else is the document's own.

use super::{StructSpec, plc, read, struct_doc, structured_spec};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{Blocks, Document};

/// A document whose six separator stories are `seps` (empty strings for empty stories), followed
/// by one section's six empty header and footer stories.
fn doc_with(seps: [&'static str; 6]) -> Vec<u8> {
    let text = "Body\r";
    let ccp = text.chars().count() as u32;
    let mut cps = vec![0u32];
    let mut at = 0u32;
    for s in seps {
        at += s.chars().count() as u32;
        cps.push(at);
    }
    cps.extend([at; 6]);
    let hdd = plc(&cps, &[]);
    let stories: Vec<&'static str> = seps.into_iter().filter(|s| !s.is_empty()).collect();
    let spec = StructSpec { text, header_stories: stories, extra_pairs: vec![(11, hdd)], ..structured_spec(text, ccp) };
    struct_doc(&spec)
}

fn open(seps: [&'static str; 6]) -> Document {
    read(&doc_with(seps)).expect("opens")
}

/// Each paragraph's text with its note separator lines shown as `[sep]` / `[cont]`.
fn shown(blocks: &Option<Blocks>) -> Option<Vec<String>> {
    let blocks = blocks.as_ref()?;
    Some(
        blocks
            .iter()
            .filter_map(|b| b.as_para())
            .map(|p| {
                let mut objs = p.objects.iter();
                p.text
                    .chars()
                    .map(|c| match c {
                        '\u{FFFC}' => match objs.next().and_then(InlineObject::as_note_separator) {
                            Some(false) => "[sep]".to_string(),
                            Some(true) => "[cont]".to_string(),
                            None => "[obj]".to_string(),
                        },
                        c => c.to_string(),
                    })
                    .collect()
            })
            .collect(),
    )
}

const CONT_SEP: &str = "(Continued from previous page)  \u{3}\r";
const NOTICE: &str = "(continued\u{2026}.)\r";

#[test]
fn c1_a_separator_holding_only_its_line_is_the_default() {
    let doc = open(["\u{3}\r", "", "", "\u{3}\r", "", ""]);
    assert_eq!(doc.footnote_separators.separator, None);
    assert_eq!(doc.endnote_separators.separator, None);
    // Also with the guard mark after the story's paragraph.
    let doc = open(["\u{3}\r\r", "", "", "", "", ""]);
    assert_eq!(doc.footnote_separators.separator, None);
}

#[test]
fn c2_the_continuation_separator_keeps_its_text_and_line() {
    let doc = open(["\u{3}\r", CONT_SEP, NOTICE, "", "", ""]);
    assert_eq!(shown(&doc.footnote_separators.continuation_separator), Some(vec!["(Continued from previous page)  [sep]".to_string()]));
    // The rest of the document is untouched.
    let body: Vec<String> = doc.body.iter().filter_map(|b| b.as_para().map(|p| p.text.clone())).collect();
    assert_eq!(body, ["Body"]);
    assert!(doc.last_section.headers.default.is_none() && doc.last_section.footers.default.is_none());
}

#[test]
fn c3_the_continuation_notice_is_its_text() {
    let doc = open(["\u{3}\r", CONT_SEP, NOTICE, "", "", ""]);
    assert_eq!(shown(&doc.footnote_separators.continuation_notice), Some(vec!["(continued\u{2026}.)".to_string()]));
    assert!(doc.endnote_separators.is_empty());
}

#[test]
fn c4_empty_stories() {
    // Length 0: the defaults.
    let doc = open(["", "", "", "", "", ""]);
    assert!(doc.footnote_separators.is_empty() && doc.endnote_separators.is_empty());
    // Only an empty paragraph mark: no notice; a separator without its line (as DOCX reads an
    // empty separator paragraph).
    let doc = open(["\r", "\r", "\r", "", "", ""]);
    assert_eq!(doc.footnote_separators.continuation_notice, None);
    assert_eq!(shown(&doc.footnote_separators.separator), Some(vec![String::new()]));
    assert_eq!(shown(&doc.footnote_separators.continuation_separator), Some(vec![String::new()]));
}

#[test]
fn c5_a_continuation_separator_holding_only_the_long_line_is_the_default() {
    let doc = open(["", "\u{4}\r", "", "", "\u{4}\r", ""]);
    assert!(doc.footnote_separators.is_empty() && doc.endnote_separators.is_empty());
    // The short line in the continuation separator is the document's own.
    let doc = open(["", "\u{3}\r", "", "", "", ""]);
    assert_eq!(shown(&doc.footnote_separators.continuation_separator), Some(vec!["[sep]".to_string()]));
}

#[test]
fn c6_endnote_stories_fill_the_endnote_separators() {
    let doc = open(["", "", "", "\u{3}\r", CONT_SEP, NOTICE]);
    assert!(doc.footnote_separators.is_empty());
    assert_eq!(doc.endnote_separators.separator, None);
    assert_eq!(shown(&doc.endnote_separators.continuation_separator), Some(vec!["(Continued from previous page)  [sep]".to_string()]));
    assert_eq!(shown(&doc.endnote_separators.continuation_notice), Some(vec!["(continued\u{2026}.)".to_string()]));
}

#[test]
fn c7_hostile_story_bounds_and_paragraph_counts() {
    let text = "Body\r";
    let ccp = text.chars().count() as u32;
    // Stories reaching past the header document, backwards, and at u32::MAX.
    let hdd = plc(&[0, 5, 1_000_000, 3, u32::MAX, u32::MAX, 2, 2, 2, 2, 2, 2, 2], &[]);
    let spec = StructSpec { text, header_stories: vec!["Note\r"], extra_pairs: vec![(11, hdd)], ..structured_spec(text, ccp) };
    let doc = read(&struct_doc(&spec)).expect("opens");
    assert_eq!(shown(&doc.footnote_separators.separator), Some(vec!["Note".to_string()]));
    // More paragraphs than allowed: the read fails as it does for a header story; no panic.
    let many: &'static str = Box::leak("\r".repeat(64).into_boxed_str());
    let bytes = doc_with(["", "", many, "", "", ""]);
    assert!(super::super::read_limited(&bytes, None, 16).is_err());
    assert!(super::super::read_limited(&bytes, None, 10_000).is_ok());
}
