//! A document opened from RTF with `\wraptrsp` (`Settings::wrap_trail_spaces`): when a line wraps
//! after a word followed by a run of two or more spaces, the first space stays at the end of the
//! line and the others start the next line, drawn there before its first word at their full
//! width, and taking room on it (the RTF space squeeze counts them). With one space, or without
//! the option, nothing wraps but the words.
//!
//! Measured in Word (Microsoft 365, files opened from RTF, compatibility mode 11): page text from
//! 144.0 to 527.75 pt, Courier New 12 pt condensed by 0.15 pt, exact 27.3 pt line spacing,
//! default tab stops, two tabs before the text. Word's first two lines are the same in every
//! case: line 1 ends with `the`, line 2 is `very important ... confused about.` (squeezed).

use super::*;
use wordcraft_doc::props::{Align, LineSpacing};
use wordcraft_doc::{CharProps, Paragraph};

const LEFT: f32 = 144.0;
const RIGHT: f32 = 527.75;

const HEAD: &str = "\t\tDR. MYERS:  I thought we would talk around the very important issue that I remain a bit confused about.";
const TAIL: &str = "I would like to ask a question to the people who are experts in this.";

fn courier(condensed: bool) -> CharProps {
    CharProps { font: Some("Courier New".into()), size: Some(12.0), spacing: condensed.then_some(-0.15), ..Default::default() }
}

struct Laid {
    /// Each line's text (the tabs and the trailing spaces left out).
    lines: Vec<String>,
    /// Where each line's first non-space character starts.
    first_x: Vec<f32>,
    /// Where each line's first cluster starts.
    start_x: Vec<f32>,
    /// One space's advance (one character's).
    space: f32,
}

fn lay_probe(spaces: usize, align: Align, wraptrsp: bool, condensed: bool) -> Laid {
    crate::test_fonts();
    let text = format!("{HEAD}{}{TAIL}", " ".repeat(spaces));
    let mut p = Paragraph::with_text(&text, courier(condensed));
    p.props.align = Some(align);
    p.props.space_after = Some(0.0);
    p.props.space_before = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Exactly(27.3));
    let mut d = Document::new();
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 612.0 - RIGHT;
    d.settings.rtf_justification = true;
    d.settings.wrap_trail_spaces = wraptrsp;
    d.body = vec![wordcraft_doc::para_block(p)];
    let l = lay(&d);
    let (para, x) =
        l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some((para.clone(), *x)) } else { None }).unwrap();
    let mut lines = Vec::new();
    let mut first_x = Vec::new();
    let mut start_x = Vec::new();
    for (li, line) in para.lines.iter().enumerate() {
        let s = &text[line.start..line.stop];
        let lead = s.len() - s.trim_start().len();
        lines.push(s.trim().to_string());
        first_x.push(para.x_of(li, line.start + lead).unwrap() + x);
        start_x.push(para.x_of(li, line.start).unwrap() + x);
    }
    // A fixed-pitch font: a space is as wide as a letter (squeezing and stretching take only spaces).
    let at = text.find("DR").unwrap();
    let space = para.x_of(0, at + 1).unwrap() - para.x_of(0, at).unwrap();
    Laid { lines, first_x, start_x, space }
}

const LINE2: &str = "very important issue that I remain a bit confused about.";

fn assert_head(l: &Laid, case: &str) {
    assert!(l.lines.len() >= 4, "{case}: {:?}", l.lines);
    assert!(l.lines[0].ends_with("around the"), "{case}: line 1 is {:?}", l.lines[0]);
    assert_eq!(l.lines[1], LINE2, "{case}: line 2");
}

/// Line 3 starts with `wrapped` spaces before `I`; `experts` begins line 4.
fn assert_wrapped(l: &Laid, wrapped: usize, case: &str) {
    assert_head(l, case);
    assert_eq!(l.lines[2], "I would like to ask a question to the people who are", "{case}: line 3");
    assert!(l.lines[3].starts_with("experts"), "{case}: line 4 is {:?}", l.lines[3]);
    assert!((l.start_x[2] - LEFT).abs() < 0.3, "{case}: line 3 starts at {}", l.start_x[2]);
    let want = LEFT + wrapped as f32 * l.space;
    assert!((l.first_x[2] - want).abs() < 0.3, "{case}: I at {}, want {want}", l.first_x[2]);
    // Word drew a wrapped space 6.96 pt wide (7.05 pt rounded to printer units).
    assert!((l.space - 7.05).abs() < 0.1, "{case}: space {}", l.space);
}

/// Nothing wraps but words: line 3 starts with `I` at the left edge, line 4 is `in this.`.
fn assert_unwrapped(l: &Laid, case: &str) {
    assert_head(l, case);
    assert_eq!(l.lines[2], "I would like to ask a question to the people who are experts", "{case}: line 3");
    assert_eq!(l.lines[3], "in this.", "{case}: line 4");
    assert!((l.first_x[2] - LEFT).abs() < 0.3, "{case}: I at {}", l.first_x[2]);
}

#[test]
fn g_justified_two_spaces_the_second_starts_the_next_line() {
    assert_wrapped(&lay_probe(2, Align::Justify, true, true), 1, "g");
}

#[test]
fn j_justified_three_spaces_two_start_the_next_line() {
    assert_wrapped(&lay_probe(3, Align::Justify, true, true), 2, "j");
}

#[test]
fn i_justified_one_space_wraps_nothing() {
    assert_unwrapped(&lay_probe(1, Align::Justify, true, true), "i");
}

#[test]
fn h_without_the_option_two_spaces_wrap_nothing() {
    assert_unwrapped(&lay_probe(2, Align::Justify, false, true), "h");
}

#[test]
fn k_left_aligned_breaks_as_justified() {
    let k = lay_probe(2, Align::Left, true, true);
    assert_wrapped(&k, 1, "k");
    let g = lay_probe(2, Align::Justify, true, true);
    assert_eq!(k.lines, g.lines, "k and g break alike");
}

/// Plain Courier New 12 pt (not condensed) breaks as before this rule, with the option and
/// without: no line there ends in a word kept by squeezing it into the line and followed by two
/// spaces. With the option `the` overruns too far to be squeezed onto line 1, as in Word.
#[test]
fn plain_courier_keeps_its_breaks_with_and_without_the_option() {
    #[rustfmt::skip]
    let on = [
        "DR. MYERS:  I thought we would talk around",
        "the very important issue that I remain a bit confused",
        "about.  I would like to ask a question to the people",
        "who are experts in this.",
    ];
    #[rustfmt::skip]
    let off = [
        "DR. MYERS:  I thought we would talk around the",
        LINE2,
        "I would like to ask a question to the people who are",
        "experts in this.",
    ];
    for align in [Align::Justify, Align::Left] {
        for (wraptrsp, want) in [(true, on), (false, off)] {
            let l = lay_probe(2, align, wraptrsp, false);
            assert_eq!(l.lines, want, "{align:?}, option {wraptrsp}");
            for (li, x) in l.first_x.iter().enumerate().skip(1) {
                assert!((x - LEFT).abs() < 0.01, "{align:?}, option {wraptrsp}: line {} starts at {x}", li + 1);
            }
        }
    }
}
