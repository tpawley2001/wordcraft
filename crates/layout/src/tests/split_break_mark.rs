//! `Settings::split_pg_break_and_para_mark` (`w:splitPgBreakAndParaMark`, on for Word 97-2003
//! files): below compatibility mode 15 the paragraph mark after a page break is an empty line at
//! the top of the new page. Measured in Microsoft 365 Word's PDFs of synthetic documents (Times
//! New Roman 12 pt, 13.80 pt lines): the paragraph after a "page break, paragraph mark" starts
//! 13.80 pt below the top of the new page (also when the break ends a paragraph of text); with a
//! 20 pt mark, 6 pt before and 12 pt after it starts 41.04 pt down (6 + 23.04 + 12); its own
//! 12 pt space before adds to the line (25.80); a break followed by text in the same paragraph
//! starts the page as before; without the option, or in mode 15, the mark stays with the break.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::Paragraph;

const BREAK: &str = "\u{000C}";

fn chr() -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, chr());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p
}

/// A 12 pt Times New Roman line.
fn line() -> f32 {
    crate::test_fonts();
    let face = wordcraft_fonts::word::resolve("Times New Roman", false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    ((a + d) / face.upem * 12.0) as f32
}

/// Lays `middle` out between a first paragraph and a last one: the page count, and how far
/// below the top of its page's text area the last paragraph starts.
fn last_offset(split: bool, mode: u32, middle: Vec<Paragraph>) -> (usize, f32) {
    let mut d = Document::new();
    d.settings.split_pg_break_and_para_mark = split;
    d.settings.compat_mode = mode;
    let mut paras = vec![para("First")];
    paras.extend(middle);
    paras.push(para("Last"));
    d.body = paras.into_iter().map(wordcraft_doc::para_block).collect();
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let tops = |page: &crate::Page| -> Vec<f32> {
        page.items.iter().filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect()
    };
    let top = tops(&l.pages[0])[0];
    let last = *tops(l.pages.last().unwrap()).last().unwrap();
    (l.pages.len(), last - top)
}

fn assert_offset(split: bool, mode: u32, middle: Vec<Paragraph>, pages: usize, expected: f32) {
    let (n, offset) = last_offset(split, mode, middle);
    assert_eq!(n, pages, "split {split} mode {mode}");
    assert!((offset - expected).abs() <= 0.05, "split {split} mode {mode}: {offset} vs {expected}");
}

#[test]
fn the_mark_after_a_break_only_paragraph_starts_the_new_page() {
    for mode in [11, 12, 14] {
        assert_offset(true, mode, vec![para(BREAK)], 2, line());
    }
}

#[test]
fn the_mark_after_text_and_a_break_starts_the_new_page() {
    assert_offset(true, 11, vec![para(&format!("Second{BREAK}"))], 2, line());
}

#[test]
fn text_after_the_break_starts_the_page_itself() {
    // The option changes nothing here: "Last" follows the paragraph's line of text on page 2.
    let (pages, without) = last_offset(false, 11, vec![para(&format!("{BREAK}Second"))]);
    assert_eq!(pages, 2);
    assert!(without >= line() - 0.05, "{without}");
    assert_offset(true, 11, vec![para(&format!("{BREAK}Second"))], 2, without);
}

#[test]
fn the_marks_paragraph_spacing_goes_with_it() {
    let mut p = para(BREAK);
    p.props.space_before = Some(6.0);
    p.props.space_after = Some(12.0);
    assert_offset(true, 11, vec![p.clone()], 2, 6.0 + line() + 12.0);
    assert_offset(false, 11, vec![p], 2, 0.0);
}

#[test]
fn the_next_paragraphs_space_before_follows_the_mark() {
    let mut d = Document::new();
    d.settings.split_pg_break_and_para_mark = true;
    d.settings.compat_mode = 11;
    let mut last = para("Last");
    last.props.space_before = Some(12.0);
    d.body = vec![para("First"), para(BREAK), last].into_iter().map(wordcraft_doc::para_block).collect();
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    assert_eq!(l.pages.len(), 2);
    let tops = |page: &crate::Page| -> Vec<f32> {
        page.items.iter().filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect()
    };
    let offset = *tops(&l.pages[1]).last().unwrap() - tops(&l.pages[0])[0];
    assert!((offset - (line() + 12.0)).abs() <= 0.05, "{offset}");
}

#[test]
fn two_break_only_paragraphs_make_three_pages() {
    assert_offset(true, 11, vec![para(BREAK), para(BREAK)], 3, line());
}

#[test]
fn mode_15_keeps_the_mark_with_the_break() {
    assert_offset(true, 15, vec![para(BREAK)], 2, 0.0);
}

#[test]
fn without_the_option_the_mark_stays_with_the_break() {
    for mode in [11, 14, 15] {
        assert_offset(false, mode, vec![para(BREAK)], 2, 0.0);
    }
    assert_offset(false, 11, vec![para(&format!("Second{BREAK}"))], 2, 0.0);
}
