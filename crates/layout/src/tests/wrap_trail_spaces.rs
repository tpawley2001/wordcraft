//! `Settings::wrap_trail_spaces` outside the RTF squeeze: a space that starts at or past the
//! line's right edge begins the next line instead of hanging. Measured in Word (Times New Roman
//! 12 pt, a line of text ending 0.65 pt before the right edge, then spaces and a paragraph mark):
//! two or three spaces give the paragraph one more line, one space or none doesn't, the same in
//! justified text, and mid-paragraph the wrapped space starts the next line (its text starts right
//! of the left edge). Without the option the spaces hang as before. The probes here use the
//! fixed-pitch test font (7.2 pt per character at 12 pt) with the same 0.65 pt of room.

use super::*;
use wordcraft_doc::props::Align;
use wordcraft_doc::{CharProps, Paragraph};

const LEFT: f32 = 72.0;
/// The right edge: 0.65 pt past the end of [`FULL`] (65 characters from the left margin).
const EDGE: f32 = 72.0 + 65.0 * 7.2 + 0.65;
const SPACE: f32 = 7.2;
/// 65 characters of text, no trailing space.
const FULL: &str = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmmm";

fn courier() -> CharProps {
    CharProps { font: Some("Courier New".into()), size: Some(12.0), ..Default::default() }
}

struct Laid {
    /// Where each of the first paragraph's lines starts.
    first_x: Vec<f32>,
    /// Top of the paragraph after it.
    next_y: f32,
    /// The first paragraph's first line height.
    line_h: f32,
}

/// Lays out `text` then a paragraph `NEXT`, both with alignment `align`.
fn lay_probe(text: &str, align: Align, wrap: bool) -> Laid {
    crate::test_fonts();
    let para = |t: &str| {
        let mut p = Paragraph::with_text(t, courier());
        p.props.align = Some(align);
        p.props.space_after = Some(0.0);
        p.props.space_before = Some(0.0);
        wordcraft_doc::para_block(p)
    };
    let mut d = Document::new();
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 612.0 - EDGE;
    d.settings.wrap_trail_spaces = wrap;
    d.body = vec![para(text), para("NEXT")];
    let l = lay(&d);
    let mut found = l.pages[0].items.iter().filter_map(|it| match it {
        Placed::Lines { para, x, y, l0, .. } => Some((para.clone(), *x, *y, *l0)),
        _ => None,
    });
    let (first, x, _, _) = found.next().unwrap();
    let (next, _, ny, nl0) = found.find(|(p, ..)| p.text_len == 4).unwrap();
    let first_x = (0..first.lines.len()).map(|li| first.x_of(li, first.lines[li].start).unwrap() + x).collect();
    Laid { first_x, next_y: ny + next.lines[nl0].top, line_h: first.lines[0].height }
}

fn lines(text: &str, align: Align, wrap: bool) -> usize {
    lay_probe(text, align, wrap).first_x.len()
}

#[test]
fn the_probe_line_ends_just_short_of_the_edge() {
    let one = lay_probe(FULL, Align::Left, false);
    assert_eq!(one.first_x.len(), 1);
    assert!((one.first_x[0] - LEFT).abs() < 0.01);
    assert_eq!(FULL.len(), 65);
    assert!(EDGE - (LEFT + 65.0 * SPACE) < SPACE);
}

#[test]
fn b1_two_trailing_spaces_make_a_line_of_their_own() {
    assert_eq!(lines(&format!("{FULL}  "), Align::Left, true), 2);
}

#[test]
fn b2_without_the_option_the_spaces_hang_as_before() {
    let plain = lay_probe(FULL, Align::Left, false);
    let spaced = lay_probe(&format!("{FULL}  "), Align::Left, false);
    assert_eq!(spaced.first_x.len(), 1);
    assert_eq!(spaced.next_y, plain.next_y);
    assert_eq!(spaced.first_x, plain.first_x);
    // Wrapped text without the option: unchanged by it.
    let long = format!("{FULL}  On March 3, the rest of the order and its terms follow here.");
    let off = lay_probe(&long, Align::Left, false);
    assert_eq!(off.first_x.len(), 2);
    assert!((off.first_x[1] - LEFT).abs() < 0.01);
}

#[test]
fn b3_one_space_that_starts_inside_the_line_stays() {
    assert_eq!(lines(&format!("{FULL} "), Align::Left, true), 1);
}

#[test]
fn b4_no_trailing_space_no_extra_line() {
    let on = lay_probe(FULL, Align::Left, true);
    let off = lay_probe(FULL, Align::Left, false);
    assert_eq!(on.first_x.len(), 1);
    assert_eq!(on.next_y, off.next_y);
}

#[test]
fn b5_three_spaces_make_one_extra_line_not_two() {
    let on = lay_probe(&format!("{FULL}   "), Align::Left, true);
    let off = lay_probe(FULL, Align::Left, false);
    assert_eq!(on.first_x.len(), 2);
    // One more line of the paragraph's normal height.
    let line_h = off.line_h;
    assert!((on.next_y - off.next_y - line_h).abs() < 0.01, "{} vs {}", on.next_y, off.next_y);
}

#[test]
fn b6_justified_text_gets_the_extra_line_too() {
    assert_eq!(lines(&format!("{FULL}  "), Align::Justify, true), 2);
    assert_eq!(lines(&format!("{FULL} "), Align::Justify, true), 1);
    assert_eq!(lines(&format!("{FULL}  "), Align::Justify, false), 1);
}

#[test]
fn b7_mid_paragraph_the_wrapped_space_starts_the_next_line() {
    let long = format!("{FULL}  On March 3, the rest of the order and its terms follow here.");
    let on = lay_probe(&long, Align::Left, true);
    let off = lay_probe(&long, Align::Left, false);
    assert_eq!(on.first_x.len(), 2);
    assert_eq!(on.next_y, off.next_y);
    // Line 2 starts with the wrapped space: its text begins at least a space right of the edge.
    let text_x = on.first_x[1];
    assert!((text_x - LEFT).abs() < 0.01, "line 2 starts at its space: {text_x}");
    let laid = {
        crate::test_fonts();
        let mut p = Paragraph::with_text(&long, courier());
        p.props.space_after = Some(0.0);
        let mut d = Document::new();
        d.last_section.margin_left = LEFT;
        d.last_section.margin_right = 612.0 - EDGE;
        d.settings.wrap_trail_spaces = true;
        d.body = vec![wordcraft_doc::para_block(p)];
        let l = lay(&d);
        l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some((para.clone(), *x)) } else { None }).unwrap()
    };
    let (para, x) = laid;
    let on_at = long.find("On").unwrap();
    let on_x = para.x_of(1, on_at).unwrap() + x;
    assert!(on_x - LEFT >= SPACE - 0.5, "line 2's text starts {} pt right of the left edge", on_x - LEFT);
}
