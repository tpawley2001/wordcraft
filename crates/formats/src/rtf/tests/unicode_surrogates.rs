//! `\uN` with N a UTF-16 surrogate (RTF 1.9.1, "Unicode RTF"): a high surrogate pairs only with a low
//! surrogate in the very next `\u` (its `\ucN` fallback characters in between are skipped, as for
//! any `\u`). Any other high or low surrogate stands alone and reads as U+FFFD, as Word shows it.

use super::*;
use wordcraft_doc::StoryRef;

const R: char = '\u{FFFD}';

fn body_text(body: &str) -> String {
    let rtf = format!("{{\\rtf1\\ansi\\ansicpg1252{{\\fonttbl\\f0\\fswiss Helvetica;}}\\pard\\f0\\fs22 {body}\\par\n}}");
    import(rtf.as_bytes()).unwrap().plain_text(StoryRef::Body)
}

#[test]
fn a_surrogate_pair_reads_as_one_character() {
    assert_eq!(body_text("a\\uc1\\u-10179?\\u-8704?b").trim_end(), "a\u{1F600}b");
    assert_eq!(body_text("a\\uc0\\u55357\\u56832 b").trim_end(), "a\u{1F600}b");
}

#[test]
fn a_high_surrogate_followed_by_text_is_a_replacement_character() {
    // As tika's testRTFInvalidUnicode.rtf: Word shows "Unpaired hi \u{FFFD} here".
    assert_eq!(body_text("\\uc0 Unpaired hi \\u-10240  here").trim_end(), format!("Unpaired hi {R} here"));
}

#[test]
fn a_held_high_surrogate_does_not_pair_with_a_later_low_one() {
    // Word: "Unpaired hi \u{FFFD} here Unpaired lo \u{FFFD} here"; never U+10000.
    let got = body_text("\\uc0 Unpaired hi \\u-10240  here Unpaired lo \\u-9216  here");
    assert_eq!(got.trim_end(), format!("Unpaired hi {R} here Unpaired lo {R} here"));
    assert!(!got.contains('\u{10000}'), "{got:?}");
}

#[test]
fn a_low_then_high_surrogate_is_two_replacement_characters() {
    // Word: "Mismatched pair \u{FFFD}\u{FFFD} here".
    assert_eq!(body_text("\\uc0 Mismatched pair \\u-9216\\u-10240  here").trim_end(), format!("Mismatched pair {R}{R} here"));
}

#[test]
fn a_high_surrogate_before_another_pair_is_its_own_replacement_character() {
    assert_eq!(body_text("\\uc1 x\\u-10240?\\u-10179?\\u-8704?y").trim_end(), format!("x{R}\u{1F600}y"));
}

#[test]
fn a_high_surrogate_at_the_end_of_a_paragraph_is_kept_as_a_replacement_character() {
    let rtf = "{\\rtf1\\ansi{\\fonttbl\\f0 Arial;}\\pard\\f0\\uc1 end\\u-10240?\\par\\pard next\\u-9216?\\par\n}";
    let d = import(rtf.as_bytes()).unwrap();
    let text = d.plain_text(StoryRef::Body);
    assert!(text.contains(&format!("end{R}")), "{text:?}");
    assert!(text.contains(&format!("next{R}")), "{text:?}");
    assert!(!text.contains('\u{10000}'), "{text:?}");
}

// PR #6105 review. Expected values checked in Word 16 (M365) on 2026-11-09 with these exact bodies.

const EMOJI: char = '\u{1F600}';

fn doc(body: &str) -> wordcraft_doc::Document {
    let rtf = format!("{{\\rtf1\\ansi\\ansicpg1252{{\\fonttbl\\f0\\fswiss Helvetica;}}\\pard\\f0\\fs22\\uc0 {body}\n}}");
    import(rtf.as_bytes()).unwrap()
}

fn part_texts(d: &wordcraft_doc::Document, kind: wordcraft_doc::PartKind) -> Vec<String> {
    d.parts.iter().filter(|(_, p)| p.kind == kind).map(|(id, _)| d.plain_text(StoryRef::Part(*id)).trim_end().to_string()).collect()
}

/// The first body paragraph's runs as (text, bold).
fn bold_runs(d: &wordcraft_doc::Document) -> Vec<(String, bool)> {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    let mut at = 0;
    let mut out: Vec<(String, bool)> = Vec::new();
    for r in &p.runs {
        let text = p.text.get(at..at + r.len).unwrap_or_default().to_string();
        at += r.len;
        let bold = r.props.bold == Some(true);
        match out.last_mut() {
            Some((t, b)) if *b == bold => t.push_str(&text),
            _ => out.push((text, bold)),
        }
    }
    out
}

#[test]
fn a_plain_group_end_or_a_formatting_control_does_not_break_a_pair() {
    // Word: "A😀B" for both.
    assert_eq!(doc("A{\\b\\u-10179}\\u-8704 B\\par").plain_text(StoryRef::Body).trim_end(), format!("A{EMOJI}B"));
    assert_eq!(doc("A\\u-10179\\b\\u-8704\\b0 B\\par").plain_text(StoryRef::Body).trim_end(), format!("A{EMOJI}B"));
}

#[test]
fn a_high_surrogate_left_in_a_footnote_stays_there() {
    // Word: the footnote reads U+FFFD, the body's lone low surrogate is its own U+FFFD; never U+1F600.
    let d = doc("A{\\footnote\\u-10179}\\u-8704 B\\par");
    assert_eq!(part_texts(&d, wordcraft_doc::PartKind::Footnote), [R.to_string()]);
    let body = d.plain_text(StoryRef::Body);
    assert!(!body.contains(EMOJI), "{body:?}");
    assert_eq!(body.matches(R).count(), 1, "{body:?}");
}

#[test]
fn a_high_surrogate_left_in_a_header_stays_there() {
    let d = doc("{\\header\\u-10179}\\u-8704 B\\par");
    assert_eq!(part_texts(&d, wordcraft_doc::PartKind::Header), [R.to_string()]);
    assert_eq!(d.plain_text(StoryRef::Body).trim_end(), format!("{R}B"));
}

#[test]
fn the_replacement_character_keeps_the_formatting_the_surrogate_was_read_in() {
    // Word draws this U+FFFD in the regular face, then "BOLD" bold.
    let d = doc("A\\u-10179\\b BOLD\\b0\\par");
    assert_eq!(bold_runs(&d), [(format!("A{R}"), false), ("BOLD".to_string(), true)]);
    let d = doc("{\\b A\\u-10179} C\\par");
    assert_eq!(bold_runs(&d), [(format!("A{R}"), true), (" C".to_string(), false)]);
}

#[test]
fn a_high_surrogate_just_before_a_row_end_adds_no_cell() {
    // Word: a one-cell row "A" (the surrogate sits on the row's end mark), then "\u{FFFD}C".
    let d = doc("\\trowd\\cellx2000 A\\cell\\u-10179\\row\\pard\\u-8704 C\\par");
    let t = d.body.iter().find_map(|b| if let wordcraft_doc::Block::Table(t) = &**b { Some(t) } else { None }).expect("a table");
    assert_eq!(t.rows.iter().map(|r| r.cells.len()).collect::<Vec<_>>(), [1]);
    let body = d.plain_text(StoryRef::Body);
    assert!(!body.contains(EMOJI), "{body:?}");
    assert!(body.trim_end().ends_with(&format!("{R}C")), "{body:?}");
}

#[test]
fn a_pair_split_by_a_cell_end_is_two_replacement_characters() {
    // Word: cell 1 "A\u{FFFD}", cell 2 "\u{FFFD}B".
    let d = doc("\\trowd\\cellx2000\\cellx4000 A\\u-10179\\cell\\u-8704 B\\cell\\row\\pard C\\par");
    let t = d.body.iter().find_map(|b| if let wordcraft_doc::Block::Table(t) = &**b { Some(t) } else { None }).expect("a table");
    let cells: Vec<String> = t
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<String>())
        .collect();
    assert_eq!(cells, [format!("A{R}"), format!("{R}B")]);
}

#[test]
fn a_high_surrogate_at_the_end_of_the_document_is_a_replacement_character() {
    let d = import(b"{\\rtf1\\ansi\\uc0 A\\u-10240}").unwrap();
    assert_eq!(d.plain_text(StoryRef::Body).trim_end(), format!("A{R}"));
}

#[test]
fn an_unpaired_high_surrogate_with_a_fallback_character() {
    assert_eq!(body_text("\\uc1 A\\u-10179?C").trim_end(), format!("A{R}C"));
}
