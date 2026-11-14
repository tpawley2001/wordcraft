//! Compatibility mode 14 and older with `w:characterSpacingControl` other than `doNotCompress`
//! (ECMA-376 §17.15.1.18, `Settings::char_spacing`): Word keeps the word that overruns a line by
//! squeezing the line's single spaces, by the same rule as a document opened from RTF, in a
//! fixed-pitch font. Mode 15 is unchanged by the setting, and a proportional font keeps today's
//! wrapping.
//!
//! Measured in Microsoft 365 Word (16.0.20527) on synthetic .docx files: Courier New 12 pt, mode
//! 11, 40 paragraphs: 481 lines without the setting or with `doNotCompress`, 453 with
//! `compressPunctuation` or `compressPunctuationAndJapaneseKana` (also mode 14; left, right and
//! justified alike). The probes are those of the RTF test (`tests/rtf_squeeze_any_align.rs`):
//! Letter page (or 792 pt wide where noted), margins 72 pt left / 36 pt right, the probe's right
//! indent, single spaces.

use super::*;
use wordcraft_doc::props::Align;
use wordcraft_doc::{CharProps, CharSpacing, Run};

const LEFT: f32 = 72.0;
const TAIL: &str = " cd cd cd cd cd cd cd cd cd end";

struct Probe {
    name: &'static str,
    size: f32,
    page_width: f32,
    align: Align,
    indent_right: f32,
    /// The first line's text up to and including the overrunning word.
    line: &'static str,
    kept: bool,
}

const S20: &str = "P00 ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab wxyz";
const S10: &str = "P00 abcde abcde abcde abcde abcde abcde abcde abcde abcde wxyz";
const S30: &str = "P00 a a a a a a a a a a a a a a a a a a a a a a a a a a a a a wxyz";
const S5: &str = "P06 abcdefghijklm abcdefghijklm abcdefghijklm abcdefghijk wxyz";
const S15: &str = "P00 ab ab ab ab ab ab ab ab ab ab ab ab ab ab wxyz";

const fn probe(name: &'static str, size: f32, page_width: f32, align: Align, indent_right: f32, line: &'static str, kept: bool) -> Probe {
    Probe { name, size, page_width, align, indent_right, line, kept }
}

#[rustfmt::skip]
const PROBES: &[Probe] = &[
    probe("s20-ql-70", 12.0, 612.0, Align::Left, 70.0, S20, true),
    probe("s20-qj-70", 12.0, 612.0, Align::Justify, 70.0, S20, true),
    probe("s20-ql-72", 12.0, 612.0, Align::Left, 72.0, S20, false),
    probe("s20-qj-72", 12.0, 612.0, Align::Justify, 72.0, S20, false),
    probe("s20-ql-90", 12.0, 612.0, Align::Left, 90.0, S20, false),
    probe("s20-qr-90", 12.0, 612.0, Align::Right, 90.0, S20, false),
    probe("s20-qc-90", 12.0, 612.0, Align::Center, 90.0, S20, false),
    probe("s20-qj-90", 12.0, 612.0, Align::Justify, 90.0, S20, false),
    probe("s20-ql-68.4", 12.0, 612.0, Align::Left, 68.4, S20, true),
    probe("s20-qr-68.4", 12.0, 612.0, Align::Right, 68.4, S20, true),
    probe("s10-ql-91", 12.0, 612.0, Align::Left, 91.0, S10, true),
    probe("s10-ql-94", 12.0, 612.0, Align::Left, 94.0, S10, false),
    probe("s30-ql-63", 12.0, 612.0, Align::Left, 63.0, S30, true),
    probe("s30-ql-65", 12.0, 612.0, Align::Left, 65.0, S30, false),
    probe("s5-ql-74", 12.0, 612.0, Align::Left, 74.0, S5, true),
    probe("s5-ql-77", 12.0, 612.0, Align::Left, 77.0, S5, false),
    probe("f18-ql-194", 18.0, 792.0, Align::Left, 194.0, S15, true),
    probe("f18-ql-198", 18.0, 792.0, Align::Left, 198.0, S15, false),
];

fn props(font: &str, size: f32) -> CharProps {
    CharProps { font: Some(font.into()), size: Some(size), ..Default::default() }
}

/// Lays `text` out as one paragraph of a DOCX document in `compat_mode` with `char_spacing`;
/// returns the first line's text (trailing spaces left out) and where its last character ends.
#[allow(clippy::too_many_arguments)]
fn first_line(
    font: &str,
    size: f32,
    page_width: f32,
    align: Align,
    indent_right: f32,
    text: &str,
    compat_mode: u32,
    char_spacing: CharSpacing,
) -> (String, f32) {
    let mut para = wordcraft_doc::Paragraph::with_text(text, props(font, size));
    para.runs = vec![Run { len: text.len(), props: props(font, size) }];
    para.props.align = Some(align);
    para.props.indent_right = Some(indent_right);
    para.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.last_section.page_w = page_width;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 36.0;
    d.settings.rtf_justification = false;
    d.settings.compat_mode = compat_mode;
    d.settings.char_spacing = char_spacing;
    d.body = vec![wordcraft_doc::para_block(para)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let (para, x) =
        l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some((para.clone(), *x)) } else { None }).unwrap();
    let line = &para.lines[0];
    let shown = text[line.start..line.stop].trim_end().to_string();
    let end = para.x_of(0, line.start + shown.len()).unwrap() + x;
    (shown, end)
}

fn probe_line(p: &Probe, compat_mode: u32, char_spacing: CharSpacing) -> (String, f32) {
    first_line("Courier New", p.size, p.page_width, p.align, p.indent_right, &format!("{}{TAIL}", p.line), compat_mode, char_spacing)
}

#[test]
fn compressing_below_mode_15_squeezes_like_rtf() {
    crate::test_fonts();
    for (mode, cs) in
        [(11, CharSpacing::CompressPunctuation), (11, CharSpacing::CompressPunctuationAndJapaneseKana), (14, CharSpacing::CompressPunctuation)]
    {
        for p in PROBES {
            let (shown, end) = probe_line(p, mode, cs);
            let before = &p.line[..p.line.rfind(' ').unwrap()];
            let want = if p.kept { p.line } else { before };
            assert_eq!(shown, want, "{} (mode {mode}, {cs:?}): first line", p.name);
            let edge = p.page_width - 36.0 - p.indent_right;
            assert!(end <= edge + 0.05, "{} (mode {mode}, {cs:?}): the line ends at {end}, past the edge {edge}", p.name);
        }
    }
}

#[test]
fn mode_11_do_not_compress_wraps_the_overrunning_word() {
    crate::test_fonts();
    for p in PROBES.iter().filter(|p| p.kept && matches!(p.align, Align::Left | Align::Justify)) {
        let (shown, _) = probe_line(p, 11, CharSpacing::DoNotCompress);
        assert_eq!(shown, &p.line[..p.line.rfind(' ').unwrap()], "{}: no squeeze without compression", p.name);
    }
}

#[test]
fn mode_15_ignores_the_setting() {
    crate::test_fonts();
    for p in PROBES {
        for cs in [CharSpacing::CompressPunctuation, CharSpacing::CompressPunctuationAndJapaneseKana] {
            assert_eq!(probe_line(p, 15, cs).0, probe_line(p, 15, CharSpacing::DoNotCompress).0, "{} ({cs:?})", p.name);
        }
    }
}

#[test]
fn a_proportional_font_keeps_todays_wrapping() {
    crate::test_fonts();
    let font = "Times New Roman";
    let text = format!("{S30}{TAIL}");
    let line = |align, ri, cs| first_line(font, 12.0, 612.0, align, ri, &text, 11, cs).0;
    let (shown, natural_end) = first_line(font, 12.0, 612.0, Align::Left, 0.0, S30, 11, CharSpacing::CompressPunctuation);
    assert_eq!(shown, S30);
    let natural = natural_end - LEFT;
    for align in [Align::Left, Align::Justify] {
        for ri in [612.0 - 36.0 - LEFT - natural - 0.2, 612.0 - 36.0 - LEFT - natural + 0.125, 612.0 - 36.0 - LEFT - natural + 1.0] {
            assert_eq!(
                line(align, ri, CharSpacing::CompressPunctuation),
                line(align, ri, CharSpacing::DoNotCompress),
                "{align:?}, right indent {ri}"
            );
        }
        let ri = 612.0 - 36.0 - LEFT - natural + 0.125;
        assert_eq!(line(align, ri, CharSpacing::CompressPunctuation), &S30[..S30.rfind(' ').unwrap()], "{align:?}: 0.125 pt overrun wraps");
    }
}
