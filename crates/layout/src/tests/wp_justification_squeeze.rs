//! WordPerfect-style justification (`w:wpJustification` in `w:compat`; `Copts80.fWPJust` in a
//! Word 97-2003 file): below compatibility mode 15 a justified line keeps the word that overruns
//! its right edge when the overrun is at most 0.0391 of the line's available width, and the line
//! is squeezed to end at the edge. Measured in Microsoft 365 Word on synthetic DOCX files (Times
//! New Roman, compatibility mode 11): each probe gives the first line's natural width and the
//! overrun at which Word still kept (`kept`) or first wrapped (`wrapped`) the line's last word.
//! The test rebuilds each probe from the natural width its own font gives the words, at the same
//! overrun-to-width ratio, so it does not depend on the test font's metrics.

use super::*;
use wordcraft_doc::props::Align;

const LEFT: f32 = 72.0;
/// A page wide enough for every probe's first line.
const PAGE_W: f32 = 72.0 + 1000.0 + 72.0;
const TEXT_W: f32 = 1000.0;
/// The words that follow the probe's first line.
const TAIL: &str = " nnnnnnnnnnnnnn nnnnnnnnnnnnnn";

struct Probe {
    name: &'static str,
    size: f32,
    line: &'static str,
    /// Word's natural width of `line`, in points.
    natural: f32,
    /// The overrun (points) at which Word kept the last word, and the one at which it wrapped it.
    kept: f32,
    wrapped: f32,
}

const N4X18: &str = "nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn";

#[rustfmt::skip]
const PROBES: &[Probe] = &[
    Probe { name: "P1", size: 12.0, line: N4X18, natural: 483.0, kept: 18.0, wrapped: 18.5 },
    Probe { name: "P3", size: 10.0, line: N4X18, natural: 402.5, kept: 15.0, wrapped: 15.5 },
    Probe { name: "Q1", size: 12.0, line: "nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn nnnnnnnnn", natural: 510.0, kept: 19.0, wrapped: 19.5 },
    Probe { name: "Q2", size: 12.0, line: "nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnn nnnnnnnnnnnn", natural: 531.0, kept: 19.0, wrapped: 20.0 },
    Probe { name: "Q4", size: 12.0, line: "nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn nn", natural: 447.0, kept: 16.5, wrapped: 17.0 },
];

fn doc(text: &str, size: f32, align: Align, indent_right: f32, flag: bool, mode: u32) -> Document {
    let chr = wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() };
    let mut para = wordcraft_doc::Paragraph::with_text(text, chr);
    para.props.align = Some(align);
    para.props.indent_right = Some(indent_right);
    para.props.space_after = Some(0.0);
    let mut d = Document::new();
    // The flag is set through the settings' serialized form (`wpJustification`), as a saved
    // document carries it.
    let mut v = serde_json::to_value(&d.settings).expect("settings serialize");
    v["wpJustification"] = serde_json::Value::Bool(flag);
    d.settings = serde_json::from_value(v).expect("settings deserialize");
    d.settings.compat_mode = mode;
    d.last_section.page_w = PAGE_W;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 72.0;
    d.body = vec![wordcraft_doc::para_block(para)];
    d
}

/// The number of lines, the first line's text (trailing spaces left out), where each of its
/// words starts and where its last character ends, in page points.
fn first_line(d: &Document) -> (usize, String, Vec<f32>, f32) {
    let text = d.body[0].as_para().unwrap().text.clone();
    let l = lay(d);
    let (para, x) =
        l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some((para.clone(), *x)) } else { None }).unwrap();
    let line = &para.lines[0];
    let shown = text[line.start..line.stop].trim_end().to_string();
    let mut starts = Vec::new();
    let mut at = line.start;
    for w in shown.split(' ') {
        starts.push(para.x_of(0, at).unwrap() + x);
        at += w.len() + 1;
    }
    let end = para.x_of(0, line.start + shown.len()).unwrap() + x;
    (para.lines.len(), shown, starts, end)
}

/// The natural width of `line` in the test font: left-aligned, no flag, the whole text width.
fn natural(line: &str, size: f32) -> f32 {
    let (_, shown, _, end) = first_line(&doc(line, size, Align::Left, 0.0, false, 11));
    assert_eq!(shown, line, "the probe fits the page at its natural width");
    end - LEFT
}

/// The right indent that leaves the probe's line the available width at which it overruns as
/// Word's line did with `overrun` points over (the same overrun-to-width ratio).
fn indent_for(p: &Probe, overrun: f32) -> f32 {
    let ratio = overrun / (p.natural - overrun);
    TEXT_W - natural(p.line, p.size) / (1.0 + ratio)
}

fn without_last_word(line: &str) -> &str {
    &line[..line.rfind(' ').unwrap()]
}

fn probe_line(p: &Probe, overrun: f32, align: Align, flag: bool, mode: u32) -> (String, Vec<f32>, f32, f32) {
    let indent = indent_for(p, overrun);
    let (_, shown, starts, end) = first_line(&doc(&format!("{}{TAIL}", p.line), p.size, align, indent, flag, mode));
    (shown, starts, end, LEFT + TEXT_W - indent)
}

#[test]
fn a_justified_line_keeps_a_word_that_overruns_by_up_to_0_0391_of_its_width() {
    crate::test_fonts();
    for p in PROBES {
        let (shown, starts, end, edge) = probe_line(p, p.kept, Align::Justify, true, 11);
        assert_eq!(shown, p.line, "{}: the last word stays at {} pt over", p.name, p.kept);
        assert!((end - edge).abs() < 0.05, "{}: the kept line ends at {end}, the edge is {edge}", p.name);
        assert!((starts[0] - LEFT).abs() < 0.01, "{}: the line starts at the margin", p.name);
    }
}

#[test]
fn a_justified_line_wraps_a_word_that_overruns_by_more() {
    crate::test_fonts();
    for p in PROBES {
        let (shown, _, _, _) = probe_line(p, p.wrapped, Align::Justify, true, 11);
        assert_eq!(shown, without_last_word(p.line), "{}: the last word wraps at {} pt over", p.name, p.wrapped);
    }
}

#[test]
fn a_small_overrun_comes_off_the_spaces_alike_and_the_words_keep_their_width() {
    crate::test_fonts();
    let p = &PROBES[0];
    let (_, _, nat_starts, _) = first_line(&doc(p.line, p.size, Align::Left, 0.0, false, 11));
    let pitch = nat_starts[1] - nat_starts[0];
    let (shown, starts, end, edge) = probe_line(p, 6.0, Align::Justify, true, 11);
    assert_eq!(shown, p.line);
    assert!((end - edge).abs() < 0.05, "ends at {end}, edge {edge}");
    let over = natural(p.line, p.size) - (edge - LEFT);
    let want = pitch - over / 17.0;
    for (i, pair) in starts.windows(2).enumerate() {
        let got = pair[1] - pair[0];
        assert!((got - want).abs() < 0.13, "word {i}: pitch {got}, want {want}");
    }
}

#[test]
fn a_large_overrun_shrinks_spaces_by_at_most_a_quarter_and_takes_the_rest_from_the_lines_end() {
    crate::test_fonts();
    let p = &PROBES[0];
    let (_, _, nat_starts, nat_end) = first_line(&doc(p.line, p.size, Align::Left, 0.0, false, 11));
    let pitch = nat_starts[1] - nat_starts[0];
    let word = nat_end - nat_starts[17];
    let space = pitch - word;
    let (shown, starts, end, edge) = probe_line(p, p.kept, Align::Justify, true, 11);
    assert_eq!(shown, p.line);
    assert!((end - edge).abs() < 0.05, "ends at {end}, edge {edge}");
    let pitches: Vec<f32> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    for (i, got) in pitches.iter().enumerate() {
        assert!(*got <= pitch + 0.01, "word {i}: nothing is stretched ({got} > {pitch})");
        // A quarter of the space, and at most 0.12 pt off each of the five characters.
        assert!(*got >= pitch - 0.25 * space - 5.0 * 0.12 - 0.05, "word {i}: pitch {got} of {pitch}");
    }
    assert!(pitches[0] + 0.01 >= pitches[16], "the characters lose width from the line's end: {pitches:?}");
    assert!(pitches[0] >= pitch - 0.25 * space - 0.05, "the line's first space loses a quarter at most: {pitches:?}");
}

#[test]
fn the_paragraphs_last_line_is_squeezed_too() {
    crate::test_fonts();
    let p = &PROBES[0];
    let indent = indent_for(p, p.kept);
    let (lines, shown, _, end) = first_line(&doc(p.line, p.size, Align::Justify, indent, true, 11));
    assert_eq!((lines, shown.as_str()), (1, p.line), "the paragraph stays on one line");
    let edge = LEFT + TEXT_W - indent;
    assert!((end - edge).abs() < 0.05, "ends at {end}, edge {edge}");
}

#[test]
fn a_left_aligned_paragraph_is_never_squeezed() {
    crate::test_fonts();
    for p in PROBES {
        let (shown, _, _, _) = probe_line(p, 0.5, Align::Left, true, 11);
        assert_eq!(shown, without_last_word(p.line), "{}", p.name);
    }
}

#[test]
fn without_the_flag_nothing_changes_below_mode_15() {
    crate::test_fonts();
    for p in PROBES {
        for mode in [11, 12, 14] {
            let (shown, _, _, _) = probe_line(p, 0.5, Align::Justify, false, mode);
            assert_eq!(shown, without_last_word(p.line), "{} in mode {mode}", p.name);
        }
    }
}

#[test]
fn compatibility_mode_15_ignores_the_flag() {
    crate::test_fonts();
    for p in PROBES {
        for overrun in [3.0, 6.0, 9.0, 12.0, p.kept, p.wrapped] {
            let with = probe_line(p, overrun, Align::Justify, true, 15);
            let without = probe_line(p, overrun, Align::Justify, false, 15);
            assert_eq!(with.0, without.0, "{} at {overrun} pt over: first line", p.name);
            assert!((with.2 - without.2).abs() < 0.01, "{} at {overrun} pt over: line end", p.name);
        }
    }
}
