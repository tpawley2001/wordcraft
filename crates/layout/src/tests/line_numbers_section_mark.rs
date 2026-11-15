//! An empty paragraph whose mark only ends a continuous section takes no room, and Word gives it no
//! line number either: measured in Microsoft 365 Word (PDF export) with `S1P1 S1P2 S1P3`, an empty
//! paragraph carrying the first section's `w:sectPr`, then `S2P1 S2P2 S2P3` in a continuous second
//! section, line numbering on in both. Second section restarting `continuous`: 1 2 3, 4 5 6.
//! Restarting `newSection`: 1 2 3, 1 2 3. Ordinary empty paragraphs and a mark paragraph with text
//! are still numbered.

use super::*;
use wordcraft_doc::section::{LineNumberRestart, LineNumbering, SectionStart};

fn numbering(restart: LineNumberRestart) -> Option<LineNumbering> {
    Some(LineNumbering { restart, ..LineNumbering::default() })
}

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

/// `S1P1..S1P3`, the mark paragraph (`mark` text, empty for a bare mark), `S2P1..S2P3`.
fn doc(mark: &str, second: LineNumberRestart) -> Document {
    let mut d = Document::new();
    let mut body: Vec<Paragraph> = (1..=3).map(|i| para(&format!("S1P{i}"))).collect();
    let mut m = para(mark);
    m.section = Some(Box::new(SectionProps { line_numbers: numbering(LineNumberRestart::Continuous), ..SectionProps::default() }));
    body.push(m);
    body.extend((1..=3).map(|i| para(&format!("S2P{i}"))));
    d.body = body.into_iter().map(wordcraft_doc::para_block).collect();
    d.last_section = SectionProps { start: SectionStart::Continuous, line_numbers: numbering(second), ..SectionProps::default() };
    d
}

/// Line numbers on page 1, top to bottom: (y, glyph ids).
fn numbers(l: &DocLayout) -> Vec<(f32, Vec<u32>)> {
    let mut v: Vec<(f32, Vec<u32>)> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story, para, y, .. } if *story == DECOR_STORY => Some((*y, para.glyphs.iter().map(|g| g.gid).collect())),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

/// The glyphs of the numbers 1..=n, from a plain one-section document with n numbered lines.
fn reference(n: usize) -> Vec<Vec<u32>> {
    let mut d = Document::new();
    d.body = (1..=n).map(|i| wordcraft_doc::para_block(para(&format!("L{i}")))).collect();
    d.last_section.line_numbers = numbering(LineNumberRestart::Continuous);
    let nums = numbers(&lay(&d));
    assert_eq!(nums.len(), n, "reference document numbers every line");
    nums.into_iter().map(|(_, g)| g).collect()
}

/// Which numbers (1-based) were drawn, and that no two share a line.
fn drawn(d: &Document) -> Vec<usize> {
    crate::test_fonts();
    let refs = reference(9);
    let nums = numbers(&lay(d));
    for w in nums.windows(2) {
        assert!(w[1].0 - w[0].0 > 1.0, "two line numbers drawn on one line: {:?}", nums.iter().map(|n| n.0).collect::<Vec<_>>());
    }
    nums.iter().map(|(_, g)| refs.iter().position(|r| r == g).map_or(0, |i| i + 1)).collect()
}

#[test]
fn a_bare_continuous_section_mark_is_not_numbered_and_numbering_continues() {
    assert_eq!(drawn(&doc("", LineNumberRestart::Continuous)), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn a_bare_continuous_section_mark_is_not_numbered_when_the_next_section_restarts() {
    assert_eq!(drawn(&doc("", LineNumberRestart::Section)), vec![1, 2, 3, 1, 2, 3]);
}

#[test]
fn a_section_mark_paragraph_with_text_is_still_numbered() {
    assert_eq!(drawn(&doc("MARK", LineNumberRestart::Continuous)), vec![1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn an_ordinary_empty_paragraph_is_still_numbered() {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = ["A", "", "B"].iter().map(|t| wordcraft_doc::para_block(para(t))).collect();
    d.last_section.line_numbers = numbering(LineNumberRestart::Continuous);
    assert_eq!(numbers(&lay(&d)).len(), 3);
}
