//! Page Number Format › Include chapter number (#3999): `w:pgNumType w:chapStyle` / `w:chapSep`
//! (ECMA-376 Part 1 §17.6.12, §17.18.12). Page numbers read `<chapter><separator><page>`, the
//! chapter being the list number of the last numbered heading of the chosen level.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Page 1: "Chapter one" (Heading 1) + "Alpha"; page 2: "Chapter two" (Heading 1) + "Beta".
/// With `numbered`, the headings carry the Multilevel List "headings" preset (1, 2).
fn chapters(numbered: bool) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("Chapter one\nAlpha\nChapter two\nBeta"));
    for h in ["Chapter one", "Chapter two"] {
        run(&mut s, "select.text", json!({"text": h}));
        run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
    }
    if numbered {
        run(&mut s, "select.text", json!({"text": "Chapter one"}));
        run(&mut s, "list.multilevelPreset", json!({"preset": "headings"}));
    }
    run(&mut s, "select.text", json!({"text": "Chapter two"}));
    run(&mut s, "caret.home", json!({}));
    run(&mut s, "layout.break", json!({"kind": "page"}));
    s
}

/// The TOC1 lines (`1\tChapter one 1-1`: numbered headings keep their number before a tab) of a TOC added at the end of the document (so the chapters keep pages 1 and 2).
fn toc(s: &mut Session) -> Vec<String> {
    run(s, "caret.docEnd", json!({}));
    run(s, "references.toc", json!({"levels": 1, "hyperlinks": false, "rightAlign": false}));
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .filter(|p| p.props.style.as_deref() == Some("TOC1"))
        .map(|p| p.plain_text())
        .collect()
}

#[test]
fn the_command_sets_and_clears_chapter_numbers() {
    let mut s = chapters(true);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "layout.pageNumberFormat", json!({"chapterStyle": 2, "chapterSeparator": "emDash"}));
    let sec = run(&mut s, "layout.section", json!({}));
    assert_eq!(sec["chapterStyle"], json!(2), "{sec}");
    assert_eq!(sec["chapterSep"], json!("emDash"), "{sec}");
    // Left out: unchanged.
    run(&mut s, "layout.pageNumberFormat", json!({"format": "decimal"}));
    let sec = run(&mut s, "layout.section", json!({}));
    assert_eq!(sec["chapterStyle"], json!(2), "{sec}");
    // null: off.
    run(&mut s, "layout.pageNumberFormat", json!({"chapterStyle": null}));
    let sec = run(&mut s, "layout.section", json!({}));
    assert!(sec.get("chapterStyle").is_none_or(Value::is_null), "{sec}");
}

#[test]
fn bad_values_are_rejected() {
    let mut s = chapters(true);
    for bad in [json!({"chapterStyle": 0}), json!({"chapterStyle": 10}), json!({"chapterSeparator": "slash"})] {
        assert!(s.run("layout.pageNumberFormat", &bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn toc_pages_carry_the_chapter_number() {
    // (separator param, expected TOC lines); no separator given = hyphen.
    let cases: [(Option<&str>, [&str; 2]); 6] = [
        (None, ["1\tChapter one 1-1", "2\tChapter two 2-2"]),
        (Some("hyphen"), ["1\tChapter one 1-1", "2\tChapter two 2-2"]),
        (Some("period"), ["1\tChapter one 1.1", "2\tChapter two 2.2"]),
        (Some("colon"), ["1\tChapter one 1:1", "2\tChapter two 2:2"]),
        (Some("emDash"), ["1\tChapter one 1\u{2014}1", "2\tChapter two 2\u{2014}2"]),
        (Some("enDash"), ["1\tChapter one 1\u{2013}1", "2\tChapter two 2\u{2013}2"]),
    ];
    for (sep, want) in cases {
        let mut s = chapters(true);
        run(&mut s, "caret.docStart", json!({}));
        let mut v = json!({"chapterStyle": 1});
        if let Some(sep) = sep {
            v["chapterSeparator"] = json!(sep);
        }
        run(&mut s, "layout.pageNumberFormat", v);
        assert_eq!(toc(&mut s), want, "separator {sep:?}");
    }
}

#[test]
fn the_page_part_keeps_the_section_format() {
    let mut s = chapters(true);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "layout.pageNumberFormat", json!({"format": "lowerRoman", "chapterStyle": 1, "chapterSeparator": "period"}));
    assert_eq!(toc(&mut s), ["1\tChapter one 1.i", "2\tChapter two 2.ii"]);
}

#[test]
fn no_numbered_heading_gives_the_plain_page_number() {
    let mut s = chapters(false);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "layout.pageNumberFormat", json!({"chapterStyle": 1}));
    assert_eq!(toc(&mut s), ["Chapter one 1", "Chapter two 2"]);
}

#[test]
fn off_by_default() {
    let mut s = chapters(true);
    assert_eq!(toc(&mut s), ["1\tChapter one 1", "2\tChapter two 2"]);
}
