//! Section document grid: `\sectspecifyl` (line grid), `\sectspecifycl` (line and character grid) and
//! `\sectlinegridN` (line pitch, twips), RTF 1.9.1 section formatting; the RTF form of DOCX `w:docGrid`.
//! Expected values checked in Word 16 (M365) on 2026-11-10: a pitch or `\sectspecifygen` alone turns no grid on.

use super::*;
use wordcraft_doc::SectionProps;
use wordcraft_doc::section::{DocGrid, DocGridType};

fn grid(words: &str) -> Option<DocGrid> {
    let rtf = format!("{{\\rtf1\\ansi\\sectd{words} a\\par}}");
    import(rtf.as_bytes()).unwrap().last_section.doc_grid
}

fn kind_and_pitch(words: &str) -> Option<(DocGridType, f32)> {
    grid(words).map(|g| (g.kind, g.line_pitch))
}

#[test]
fn the_line_grid_words_are_read() {
    assert_eq!(kind_and_pitch("\\sectspecifyl\\sectlinegrid720"), Some((DocGridType::Lines, 36.0)));
    assert_eq!(kind_and_pitch("\\sectspecifycl\\sectlinegrid360"), Some((DocGridType::LinesAndChars, 18.0)));
    assert_eq!(kind_and_pitch("\\sectspecifyl\\sectspecifygen1\\sectlinegrid720"), Some((DocGridType::Lines, 36.0)));
    assert_eq!(grid("\\sectspecifyl\\sectlinegrid720").map(|g| g.char_space), Some(0));
}

#[test]
fn a_pitch_or_sectspecifygen_alone_turns_no_grid_on() {
    assert_eq!(grid(""), None);
    assert_eq!(grid("\\sectlinegrid720"), None);
    assert_eq!(grid("\\sectspecifygen1\\sectlinegrid720"), None);
}

#[test]
fn a_grid_without_a_pitch_uses_the_default_pitch() {
    assert_eq!(kind_and_pitch("\\sectspecifyl"), Some((DocGridType::Lines, DocGrid::default().line_pitch)));
    assert_eq!(kind_and_pitch("\\sectspecifyl\\sectlinegrid0"), Some((DocGridType::Lines, DocGrid::default().line_pitch)));
}

#[test]
fn sectd_resets_the_grid_for_the_next_section() {
    let rtf = "{\\rtf1\\ansi\\sectd\\sectspecifyl\\sectlinegrid720 a\\par\\sect\\sectd b\\par}";
    let d = import(rtf.as_bytes()).unwrap();
    assert_eq!(d.last_section.doc_grid, None);
}

#[test]
fn export_writes_the_grid_and_it_round_trips() {
    for (kind, word) in [(DocGridType::Lines, "\\sectspecifyl"), (DocGridType::LinesAndChars, "\\sectspecifycl")] {
        let mut d = Document::from_text("one");
        d.last_section = SectionProps { doc_grid: Some(DocGrid { kind, line_pitch: 15.6, char_space: 0 }), ..Default::default() };
        let rtf = export(&d);
        assert!(rtf.contains(&format!("{word}\\sectlinegrid312")), "{rtf}");
        let back = import(rtf.as_bytes()).unwrap();
        assert_eq!(back.last_section.doc_grid.map(|g| (g.kind, g.line_pitch)), Some((kind, 15.6)));
    }
}

#[test]
fn export_writes_no_grid_when_none_is_set() {
    for doc_grid in [None, Some(DocGrid::default())] {
        let mut d = Document::from_text("one");
        d.last_section = SectionProps { doc_grid, ..Default::default() };
        let rtf = export(&d);
        assert!(!rtf.contains("\\sectspecify"), "{rtf}");
        assert!(!rtf.contains("\\sectlinegrid"), "{rtf}");
    }
}
