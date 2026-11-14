//! Small capitals (`w:smallCaps`, ECMA-376 Part 1 §17.3.2.33): Word draws a lowercase letter as a
//! capital at 0.8 of the run's size rounded to the half point. Measured 2026-11-14 in Microsoft
//! 365 Word for Windows (PDF export, Times New Roman, Arial and Calibri, same result in each):
//! 7 -> 5.5, 8 -> 6.5, 9 -> 7, 10 -> 8, 11 -> 9, 12 -> 9.5, 13 -> 10.5, 14 -> 11, 16 -> 13,
//! 18 -> 14.5, 20 -> 16, 24 -> 19, 36 -> 29 pt. Capitals typed as capitals keep the full size.

use super::{WRAP_TEST_FONT, lay};
use crate::*;
use wordcraft_doc::CharProps;

/// From the first character's start to the last's: the advances of all but the last character.
fn span(text: &str, size: f32, small_caps: bool) -> f32 {
    let mut p = wordcraft_doc::Paragraph::new();
    let props = CharProps { font: Some(WRAP_TEST_FONT.into()), size: Some(size), small_caps: Some(small_caps), ..Default::default() };
    p.runs.push(wordcraft_doc::para::Run { len: text.len(), props: props.clone() });
    p.text.push_str(text);
    p.mark = props;
    let mut d = Document::new();
    d.body = vec![wordcraft_doc::para_block(p)];
    let l = lay(&d);
    let xs = l.pages[0]
        .items
        .iter()
        .find_map(|it| if let Placed::Lines { para, .. } = it { para.lines.first().map(|x| x.xs.clone()) } else { None })
        .unwrap();
    xs.last().unwrap() - xs.first().unwrap()
}

#[test]
fn small_capitals_are_set_at_the_half_point_size() {
    // (run size, Word's small-capital size)
    for (size, small) in [(7.0, 5.5), (9.0, 7.0), (11.0, 9.0), (12.0, 9.5), (13.0, 10.5), (14.0, 11.0), (18.0, 14.5), (24.0, 19.0), (36.0, 29.0)] {
        let got = span("abcdefghij", size, true);
        let want = span("ABCDEFGHIJ", small, false);
        assert!((got - want).abs() < 0.01, "{size}pt small caps span {got}, want {want} (capitals at {small}pt)");
    }
}

#[test]
fn small_capitals_on_the_half_point_already_are_unchanged() {
    for (size, small) in [(10.0, 8.0), (20.0, 16.0)] {
        let got = span("abcdefghij", size, true);
        assert!((got - span("ABCDEFGHIJ", small, false)).abs() < 0.01, "{size}pt");
    }
    // Capitals in a small-caps run keep the run's full size.
    assert!((span("ABCDEFGHIJ", 11.0, true) - span("ABCDEFGHIJ", 11.0, false)).abs() < 0.01);
}
