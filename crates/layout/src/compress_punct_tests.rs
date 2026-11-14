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

// Proportional fonts (P02-271). Microsoft 365 Word 16.0.20527, synthetic .docx files, mode 11 with
// `compressPunctuation`: a space may shrink down to half the font's OS/2 average character width
// (`xAvgCharWidth`) to keep the overrunning word; fixed-pitch fonts (space = average) keep the
// half-a-space rule above. Word measured per space at 12 pt: Times New Roman .567-.575 pt, Arial
// .697-.720 pt, Calibri none. `w:balanceSingleByteDoubleByteWidth` together with
// `w:useWord97LineBreakRules` switches the proportional squeeze off (either alone doesn't); fixed
// pitch still squeezes with both.

const TNR_SPACE_EM: f32 = 512.0 / 2048.0;
const TNR_AVG_EM: f32 = 821.0 / 2048.0;
const ARIAL_SPACE_EM: f32 = 569.0 / 2048.0;
const ARIAL_AVG_EM: f32 = 904.0 / 2048.0;

#[test]
fn compress_space_room_by_font() {
    // (space width, average character width, room per space), all in points.
    let cases = [
        (TNR_SPACE_EM * 12.0, TNR_AVG_EM * 12.0, 0.594_727),
        (TNR_SPACE_EM * 10.0, TNR_AVG_EM * 10.0, 0.495_605),
        (TNR_SPACE_EM * 20.0, TNR_AVG_EM * 20.0, 0.991_211),
        (ARIAL_SPACE_EM * 12.0, ARIAL_AVG_EM * 12.0, 0.685_547),
        // Calibri at 12 pt: space 463/2048 em, average above twice that: no room.
        (463.0 / 2048.0 * 12.0, 1048.0 / 2048.0 * 12.0, 0.0),
        // Courier New at 12 pt: space = average = 1229/2048 em: half a space.
        (1229.0 / 2048.0 * 12.0, 1229.0 / 2048.0 * 12.0, 3.600_586),
    ];
    for (space, avg, want) in cases {
        let got = crate::para::compress_space_room(space, avg);
        assert!((got - want).abs() < 1e-4, "space {space}, average {avg}: room {got}, want {want}");
    }
}

#[test]
fn twin_average_widths() {
    use wordcraft_fonts::twin_avg_char_width_em as twin;
    for face in ["Liberation Serif", "Tinos", "liberation serif"] {
        assert_eq!(twin("Times New Roman", face), Some(TNR_AVG_EM), "Times New Roman drawn in {face}");
    }
    assert_eq!(twin("times new roman", "Tinos"), Some(TNR_AVG_EM));
    for face in ["Liberation Sans", "Arimo"] {
        assert_eq!(twin("Arial", face), Some(ARIAL_AVG_EM), "Arial drawn in {face}");
    }
    // The font itself, fonts that aren't twins, and twins whose own average already matches Word's.
    for (req, face) in [
        ("Times New Roman", "Times New Roman"),
        ("Arial", "Arial"),
        ("Times New Roman", "Source Serif 4"),
        ("Arial", "Liberation Serif"),
        ("Calibri", "Carlito"),
        ("Georgia", "Liberation Serif"),
    ] {
        assert_eq!(twin(req, face), None, "{req} drawn in {face}");
    }
}

#[test]
fn squeezes_proportional_spaces_switches() {
    let s = |mode: u32, cs: CharSpacing, rtf: bool, balance: bool, word97: bool| {
        let mut s = wordcraft_doc::Settings { compat_mode: mode, char_spacing: cs, rtf_justification: rtf, ..Default::default() };
        s.balance_sbcs_dbcs_width = balance;
        s.use_word97_line_break_rules = word97;
        s.squeezes_proportional_spaces()
    };
    assert!(s(11, CharSpacing::CompressPunctuation, false, false, false));
    assert!(s(14, CharSpacing::CompressPunctuationAndJapaneseKana, false, false, false));
    assert!(s(11, CharSpacing::CompressPunctuation, false, true, false), "balance alone still squeezes");
    assert!(s(11, CharSpacing::CompressPunctuation, false, false, true), "Word 97 rules alone still squeeze");
    assert!(!s(11, CharSpacing::CompressPunctuation, false, true, true), "the pair switches it off");
    assert!(!s(15, CharSpacing::CompressPunctuation, false, false, false), "mode 15");
    assert!(!s(11, CharSpacing::DoNotCompress, false, false, false), "no compression");
    assert!(!s(15, CharSpacing::DoNotCompress, true, true, true), "RTF documents carry the pair");
}

/// [`first_line`] in Courier New with both pair flags on.
fn probe_line_with_pair(p: &Probe, compat_mode: u32, char_spacing: CharSpacing) -> String {
    let text = format!("{}{TAIL}", p.line);
    let mut para = wordcraft_doc::Paragraph::with_text(&text, props("Courier New", p.size));
    para.runs = vec![Run { len: text.len(), props: props("Courier New", p.size) }];
    para.props.align = Some(p.align);
    para.props.indent_right = Some(p.indent_right);
    para.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.last_section.page_w = p.page_width;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 36.0;
    d.settings.rtf_justification = false;
    d.settings.compat_mode = compat_mode;
    d.settings.char_spacing = char_spacing;
    d.settings.balance_sbcs_dbcs_width = true;
    d.settings.use_word97_line_break_rules = true;
    d.body = vec![wordcraft_doc::para_block(para)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let para = l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap();
    let line = &para.lines[0];
    text[line.start..line.stop].trim_end().to_string()
}

#[test]
fn the_pair_keeps_fixed_pitch_squeezing() {
    crate::test_fonts();
    for p in PROBES {
        let want = if p.kept { p.line } else { &p.line[..p.line.rfind(' ').unwrap()] };
        assert_eq!(probe_line_with_pair(p, 11, CharSpacing::CompressPunctuation), want, "{} with the pair", p.name);
        assert_eq!(
            probe_line_with_pair(p, 11, CharSpacing::CompressPunctuation),
            probe_line(p, 11, CharSpacing::CompressPunctuation).0,
            "{}",
            p.name
        );
    }
}
