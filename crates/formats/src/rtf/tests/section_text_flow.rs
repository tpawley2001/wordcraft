//! Section text direction (`\stextflowN`, RTF 1.9.1 section formatting): the RTF form of DOCX
//! `w:sectPr/w:textDirection`. Values as Microsoft Word draws them: 0 and 4 horizontal, 1, 3 and 5
//! top to bottom, 2 bottom to top.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::TextDirection;

fn last(rtf: &str) -> TextDirection {
    import(rtf.as_bytes()).unwrap().last_section.text_direction
}

#[test]
fn each_stextflow_value_reads_as_word_draws_it() {
    let cases = [
        (0, TextDirection::Horizontal),
        (1, TextDirection::Down),
        (2, TextDirection::Up),
        (3, TextDirection::Down),
        (4, TextDirection::Horizontal),
        (5, TextDirection::Down),
        (6, TextDirection::Horizontal),
        (-1, TextDirection::Horizontal),
    ];
    for (n, want) in cases {
        assert_eq!(last(&format!(r"{{\rtf1\ansi\sectd\stextflow{n} a\par}}")), want, "\\stextflow{n}");
    }
}

#[test]
fn no_stextflow_is_horizontal() {
    let d = import(br"{\rtf1\ansi\sectd a\par}").unwrap();
    assert_eq!(d.last_section.text_direction, TextDirection::Horizontal);
    assert!(!d.last_section.text_direction_set);
}

#[test]
fn sectd_resets_it_for_the_next_section() {
    let d = import(br"{\rtf1\ansi\sectd\stextflow1 a\sect\sectd b\par}").unwrap();
    let first = d
        .body
        .iter()
        .find_map(|b| match b.as_ref() {
            wordcraft_doc::Block::Para(p) => p.section.as_deref(),
            _ => None,
        })
        .expect("first section");
    assert_eq!(first.text_direction, TextDirection::Down);
    assert_eq!(d.last_section.text_direction, TextDirection::Horizontal);
}

#[test]
fn export_writes_it_and_it_round_trips() {
    for (dir, word) in [(TextDirection::Down, r"\stextflow1"), (TextDirection::Up, r"\stextflow2")] {
        let mut d = Document::from_text("one");
        d.last_section.text_direction = dir;
        let rtf = export(&d);
        assert!(rtf.contains(word), "{dir:?}: {rtf}");
        assert_eq!(import(rtf.as_bytes()).unwrap().last_section.text_direction, dir);
    }
}

#[test]
fn export_writes_nothing_for_horizontal() {
    let rtf = export(&Document::from_text("one"));
    assert!(!rtf.contains(r"\stextflow"), "{rtf}");
}
