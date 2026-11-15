//! Line numbers measured in Microsoft 365 Word (PDF export, Letter, 1-inch margins, three one-line
//! paragraphs, numbering every line, restart continuous):
//! - In a right-to-left section (`w:bidi` in `w:sectPr`) the numbers stand to the right of the text
//!   column, their left edge the auto distance (18 pt) beyond the column's right edge.
//! - A framed paragraph (`w:framePr`) gets no number and is not counted.
//! - An empty paragraph that only carries a section mark gets no number, also before a next-page
//!   section break.

use super::*;
use wordcraft_doc::props::TextFrame;
use wordcraft_doc::section::{LineNumberRestart, LineNumbering, SectionStart};

fn numbering() -> Option<LineNumbering> {
    Some(LineNumbering { restart: LineNumberRestart::Continuous, ..LineNumbering::default() })
}

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

fn doc(paras: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    d.body = paras.into_iter().map(wordcraft_doc::para_block).collect();
    d.last_section.line_numbers = numbering();
    d
}

/// Line numbers on page `page`, top to bottom: (x, y, glyph ids).
fn numbers(l: &DocLayout, page: usize) -> Vec<(f32, f32, Vec<u32>)> {
    let mut v: Vec<(f32, f32, Vec<u32>)> = l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story, para, x, y, .. } if *story == DECOR_STORY => Some((*x, *y, para.glyphs.iter().map(|g| g.gid).collect())),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

/// Which numbers (1-based) page `page` shows, from the glyphs of a plain document numbering 1..=9.
fn shown(l: &DocLayout, page: usize) -> Vec<usize> {
    let refs: Vec<Vec<u32>> = numbers(&lay(&doc((1..=9).map(|i| para(&format!("L{i}"))).collect())), 0).into_iter().map(|n| n.2).collect();
    assert_eq!(refs.len(), 9, "reference document numbers every line");
    numbers(l, page).iter().map(|n| refs.iter().position(|r| *r == n.2).map_or(0, |i| i + 1)).collect()
}

#[test]
fn a_right_to_left_section_has_its_numbers_right_of_the_text() {
    crate::test_fonts();
    let mut d = doc(vec![para("S1P1"), para("S1P2"), para("S1P3")]);
    d.last_section.rtl = true;
    let right = d.last_section.margin_left + d.last_section.gutter + d.last_section.text_width();
    let l = lay(&d);
    let nums = numbers(&l, 0);
    assert_eq!(nums.len(), 3);
    for (x, _, _) in &nums {
        assert!((x - (right + 18.0)).abs() < 0.5, "number at x={x}, expected {} (text column right edge {right} + 18)", right + 18.0);
    }
}

#[test]
fn a_left_to_right_section_keeps_its_numbers_left_of_the_text() {
    crate::test_fonts();
    let mut rtl_para = para("RTLPARA");
    rtl_para.props.bidi = Some(true);
    let d = doc(vec![para("LTRPARA"), rtl_para, para("LTR2")]);
    let left = d.last_section.margin_left + d.last_section.gutter;
    let l = lay(&d);
    let nums = numbers(&l, 0);
    assert_eq!(nums.len(), 3);
    assert!(nums.iter().all(|(x, _, _)| *x < left), "numbers stay left of the text: {nums:?}");
}

#[test]
fn a_framed_paragraph_is_not_numbered_or_counted() {
    crate::test_fonts();
    let mut framed = para("FRAMED");
    framed.props.frame = Some(TextFrame {
        w: Some(144.0),
        h_anchor: "page".into(),
        v_anchor: "text".into(),
        wrap: "around".into(),
        x: 250.0,
        y: 0.05,
        h_space: 9.0,
        ..Default::default()
    });
    let l = lay(&doc(vec![para("BEFORE"), framed, para("AFTER1"), para("AFTER2")]));
    assert_eq!(shown(&l, 0), vec![1, 2, 3]);
}

#[test]
fn an_empty_mark_before_a_next_page_section_break_is_not_numbered() {
    crate::test_fonts();
    let mut mark = para("");
    mark.section = Some(Box::new(SectionProps { line_numbers: numbering(), ..SectionProps::default() }));
    let mut paras: Vec<Paragraph> = (1..=3).map(|i| para(&format!("S1P{i}"))).collect();
    paras.push(mark);
    paras.extend((1..=3).map(|i| para(&format!("S2P{i}"))));
    let mut d = doc(paras);
    d.last_section.start = SectionStart::NextPage;
    let l = lay(&d);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(shown(&l, 0), vec![1, 2, 3]);
    assert_eq!(shown(&l, 1), vec![4, 5, 6]);
}
