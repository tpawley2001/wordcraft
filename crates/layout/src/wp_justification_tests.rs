//! More WordPerfect-style justification (`Settings::wp_justification`) probes: Word's Q3 (long
//! words and a short last one) and Q5 (a left tab after the first word: the tab does not switch
//! the squeeze off), rebuilt at Word's overrun-to-width ratio from the test font's own natural
//! width; and a word wider than the line is not squeezed onto it.

use super::*;
use wordcraft_doc::props::Align;

const LEFT: f32 = 72.0;
const TEXT_W: f32 = 1000.0;
const TAIL: &str = " nnnnnnnnnnnnnn nnnnnnnnnnnnnn";

fn doc(text: &str, align: Align, indent_right: f32, flag: bool) -> Document {
    let chr = wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() };
    let mut para = wordcraft_doc::Paragraph::with_text(text, chr);
    para.props.align = Some(align);
    para.props.indent_right = Some(indent_right);
    para.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.settings.wp_justification = flag;
    d.settings.compat_mode = 11;
    d.settings.default_tab = 36.0;
    d.last_section.page_w = LEFT + TEXT_W + 72.0;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 72.0;
    d.body = vec![wordcraft_doc::para_block(para)];
    d
}

/// The first line's text (trailing spaces left out), where it ends, and the line count.
fn first_line(d: &Document) -> (String, f32, usize) {
    let text = d.body[0].as_para().unwrap().text.clone();
    let l = layout(d, &mut LayoutCache::new(), &LayoutOptions::default());
    let (para, x) =
        l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some((para.clone(), *x)) } else { None }).unwrap();
    let line = &para.lines[0];
    let shown = text[line.start..line.stop].trim_end().to_string();
    let end = para.x_of(0, line.start + shown.len()).unwrap() + x;
    (shown, end, para.lines.len())
}

/// The first line at Word's overrun-to-width ratio: its text, end, and right edge.
fn probe(line: &str, natural_word: f32, overrun: f32) -> (String, f32, f32) {
    let (shown, end, _) = first_line(&doc(line, Align::Left, 0.0, false));
    assert_eq!(shown, line);
    let natural = end - LEFT;
    let indent = TEXT_W - natural / (1.0 + overrun / (natural_word - overrun));
    let (shown, end, _) = first_line(&doc(&format!("{line}{TAIL}"), Align::Justify, indent, true));
    (shown, end, LEFT + TEXT_W - indent)
}

#[test]
fn q3_long_words_and_a_short_last_one_stay_squeezed() {
    crate::test_fonts();
    let w = "nnnnnnnnnnnnnnnn";
    let line = format!("{w} {w} {w} {w} {w} nn");
    let (shown, end, edge) = probe(&line, 507.0, 14.0);
    assert_eq!(shown, line);
    assert!((end - edge).abs() < 0.05, "ends at {end}, edge {edge}");
}

#[test]
fn q5_a_tab_on_the_line_does_not_switch_it_off() {
    crate::test_fonts();
    let line = format!("nnnn\t{}", ["nnnn"; 17].join(" "));
    let (shown, end, edge) = probe(&line, 492.0, 18.5);
    assert_eq!(shown, line, "kept at 18.5 pt over");
    assert!((end - edge).abs() < 0.05, "ends at {end}, edge {edge}");
    let (shown, _, _) = probe(&line, 492.0, 19.0);
    assert_eq!(shown, &line[..line.rfind(' ').unwrap()], "wrapped at 19 pt over");
}

#[test]
fn a_word_wider_than_the_line_is_not_squeezed() {
    crate::test_fonts();
    let word = "n".repeat(60);
    let (_, natural, _) = first_line(&doc(&word, Align::Left, 0.0, false));
    // Half the word's width: it overruns by far more than the squeeze allows.
    let indent = TEXT_W - (natural - LEFT) / 2.0;
    let (_, with, _) = first_line(&doc(&word, Align::Justify, indent, true));
    let (_, without, _) = first_line(&doc(&word, Align::Justify, indent, false));
    assert!((with - without).abs() < 0.01, "{with} vs {without}");
}
