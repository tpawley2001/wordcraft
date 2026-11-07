//! Generated lists and page cross-references write a page as its footer shows it: in the number
//! format of the page's section (`w:pgNumType w:fmt`), as Word does (#3856). The front matter is
//! numbered i, ii; the body restarts at 1.

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;
use wordcraft_doc::para::InlineObject;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

/// Page i: "Front"; page ii: "Preface" (Heading 1); next-page section break; page 1: "Chapter one"
/// (Heading 1). Section 1 lowerRoman from 1, section 2 decimal from 1.
fn two_sections() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("Front\nPreface\nChapter one"));
    for h in ["Preface", "Chapter one"] {
        run(&mut s, "select.text", json!({"text": h}));
        run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
    }
    run(&mut s, "select.text", json!({"text": "Preface"}));
    run(&mut s, "caret.home", json!({}));
    run(&mut s, "layout.break", json!({"kind": "page"}));
    run(&mut s, "select.text", json!({"text": "Chapter one"}));
    run(&mut s, "caret.home", json!({}));
    run(&mut s, "layout.break", json!({"kind": "nextPage"}));
    run(&mut s, "select.text", json!({"text": "Chapter one"}));
    run(&mut s, "layout.pageNumberFormat", json!({"format": "decimal", "start": 1}));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "layout.pageNumberFormat", json!({"format": "lowerRoman", "start": 1}));
    s
}

fn styled(s: &Session, prefix: &str) -> Vec<String> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .filter(|p| p.props.style.as_deref().is_some_and(|st| st.starts_with(prefix) && st != "TOCHeading"))
        .map(|p| p.plain_text())
        .collect()
}

fn field_results(s: &Session, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o
                && instr.trim_start().starts_with(name)
            {
                out.push(result.clone());
            }
        }
    }
    out
}

#[test]
fn toc_page_numbers_follow_the_section_format() {
    let mut s = two_sections();
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.toc", json!({"levels": 1, "hyperlinks": false, "rightAlign": false}));
    assert_eq!(styled(&s, "TOC1"), ["Preface ii", "Chapter one 1"]);
}

#[test]
fn toc_pageref_results_follow_the_section_format() {
    let mut s = two_sections();
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.toc", json!({"levels": 1, "hyperlinks": true, "rightAlign": false}));
    assert_eq!(field_results(&s, "PAGEREF"), ["ii", "1"]);
}

#[test]
fn toc_upper_roman_and_letters() {
    let mut s = two_sections();
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "layout.pageNumberFormat", json!({"format": "upperRoman", "start": 4}));
    run(&mut s, "select.text", json!({"text": "Chapter one"}));
    run(&mut s, "layout.pageNumberFormat", json!({"format": "lowerLetter", "start": 3}));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.toc", json!({"levels": 1, "hyperlinks": false, "rightAlign": false}));
    // Preface is the section's 2nd page: 4 + 1 = 5 = V; Chapter one starts at 3 = c.
    assert_eq!(styled(&s, "TOC1"), ["Preface V", "Chapter one c"]);
}

#[test]
fn index_pages_follow_the_section_format() {
    let mut s = two_sections();
    run(&mut s, "select.text", json!({"text": "Preface"}));
    run(&mut s, "references.markEntry", json!({"entry": "Preface"}));
    run(&mut s, "select.text", json!({"text": "Chapter one"}));
    run(&mut s, "references.markEntry", json!({"entry": "Chapter"}));
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "references.index", json!({"columns": 1, "rightAlign": false}));
    let text = s.doc.plain_text(StoryRef::Body);
    // The index is the last thing in the document: its lines start with the entry.
    let entry = |name: &str| text.lines().rfind(|l| l.starts_with(name)).unwrap_or_default().to_string();
    let preface = entry("Preface");
    assert!(preface.ends_with("ii"), "index line {preface:?} in {text:?}");
    let chapter = entry("Chapter");
    assert!(chapter.ends_with('1') && chapter != "Chapter one", "index line {chapter:?} in {text:?}");
}

#[test]
fn page_cross_reference_follows_the_section_format() {
    let mut s = two_sections();
    run(&mut s, "caret.docEnd", json!({}));
    // Before this change the field was inserted with "1", not even the decimal 2: find out which
    // page it reads, too.
    run(&mut s, "insert.crossReference", json!({"to": "heading", "target": "Preface", "show": "page"}));
    assert_eq!(field_results(&s, "PAGEREF"), ["ii"]);
}

#[test]
fn update_fields_still_agree() {
    // F9 already formats PAGEREF by the section; inserting must give the same as updating.
    let mut s = two_sections();
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.toc", json!({"levels": 1, "hyperlinks": true, "rightAlign": false}));
    let inserted = field_results(&s, "PAGEREF");
    crate::cmd::references::update_fields_in(&mut s, true).unwrap();
    assert_eq!(field_results(&s, "PAGEREF"), inserted);
}
