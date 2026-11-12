//! A style-sheet entry's formatting words that body text already reads (RTF 1.9.1, "Style Sheet":
//! an entry's `<formatting>` is `<brdrdef> | <parfmt> | <shading> | <chrfmt>`): all caps, small
//! caps, hidden, superscript and subscript, highlight, underline colour, character shading,
//! paragraph shading and paragraph borders reach the document's style when the file is opened.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{Border, BorderStyle, Borders, CharProps, Highlight, ParaProps, Rgb, Underline, VertAlign};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
/// Not one of the 16 highlighter colours.
const ODD: Rgb = Rgb(1, 2, 3);

/// Colour table: 1 red, 2 blue, 3 yellow, 4 [`ODD`].
fn open(stylesheet: &str, body: &str) -> Document {
    let colors = r"{\colortbl;\red255\green0\blue0;\red0\green0\blue255;\red255\green255\blue0;\red1\green2\blue3;}";
    let rtf = format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{colors}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}");
    import(rtf.as_bytes()).unwrap()
}

/// The character formatting of a character style (`XChar`) and of a paragraph style (`XPara`)
/// whose entries hold `words`.
fn chr_of(words: &str) -> [CharProps; 2] {
    let sheet = format!(r"{{\*\cs10 \additive {words} XChar;}}{{\s15 {words} XPara;}}");
    let d = open(&sheet, r"\pard\plain\f0\fs24 Text.\par");
    ["XChar", "XPara"].map(|id| d.styles.get(id).unwrap_or_else(|| panic!("no style {id}")).chr.clone())
}

/// The paragraph formatting of a paragraph style (`XPara`) whose entry holds `words`.
fn para_of(words: &str) -> ParaProps {
    let d = open(&format!(r"{{\s15 {words} XPara;}}"), r"\pard\plain\f0\fs24 Text.\par");
    d.styles.get("XPara").expect("style XPara").para.clone()
}

fn line(style: BorderStyle, width: f32, color: Option<Rgb>, space: f32) -> Option<Border> {
    Some(Border { style, width, color, space })
}

#[test]
fn caps_small_caps_and_hidden_reach_the_style() {
    for c in chr_of(r"\caps") {
        assert_eq!((c.caps, c.small_caps, c.hidden), (Some(true), None, None), r"\caps");
    }
    for c in chr_of(r"\scaps") {
        assert_eq!((c.caps, c.small_caps, c.hidden), (None, Some(true), None), r"\scaps");
    }
    for c in chr_of(r"\v") {
        assert_eq!((c.caps, c.small_caps, c.hidden), (None, None, Some(true)), r"\v");
    }
}

#[test]
fn a_stated_off_is_kept_so_it_undoes_the_base_style() {
    for c in chr_of(r"\caps0\scaps0\v0") {
        assert_eq!((c.caps, c.small_caps, c.hidden), (Some(false), Some(false), Some(false)));
    }
}

#[test]
fn superscript_and_subscript_reach_the_style() {
    for (words, want) in [
        (r"\super", Some(VertAlign::Superscript)),
        (r"\sub", Some(VertAlign::Subscript)),
        (r"\nosupersub", Some(VertAlign::Baseline)),
        (r"\super0", Some(VertAlign::Baseline)),
        (r"\sub0", Some(VertAlign::Baseline)),
        (r"\super\sub", Some(VertAlign::Subscript)),
        (r"\sub\super", Some(VertAlign::Superscript)),
        (r"\b", None),
    ] {
        for c in chr_of(words) {
            assert_eq!(c.vert_align, want, "{words}");
        }
    }
}

#[test]
fn a_highlighter_colour_is_the_style_highlight() {
    for c in chr_of(r"\highlight3") {
        assert_eq!((c.highlight, c.shading), (Some(Highlight::Yellow), None));
    }
}

#[test]
fn a_highlight_in_another_colour_is_character_shading_as_on_a_run() {
    for c in chr_of(r"\highlight4") {
        assert_eq!((c.highlight, c.shading), (None, Some(ODD)));
    }
}

#[test]
fn highlight_zero_is_no_highlight() {
    for c in chr_of(r"\highlight0") {
        assert_eq!(c.highlight.unwrap_or_default(), Highlight::None);
        assert_eq!(c.shading, None);
    }
}

#[test]
fn underline_colour_reaches_the_style() {
    for c in chr_of(r"\ul\ulc1") {
        assert_eq!((c.underline, c.underline_color), (Some(Underline::Single), Some(RED)));
    }
    // Entry 0 is the automatic colour; an index past the table is no colour either.
    for words in [r"\ul\ulc0", r"\ul\ulc99", r"\ul"] {
        for c in chr_of(words) {
            assert_eq!((c.underline, c.underline_color), (Some(Underline::Single), None), "{words}");
        }
    }
}

#[test]
fn character_shading_reaches_the_style() {
    for words in [r"\chcbpat1", r"\cb1"] {
        for c in chr_of(words) {
            assert_eq!((c.shading, c.highlight), (Some(RED), None), "{words}");
        }
    }
    for c in chr_of(r"\chcbpat0") {
        assert_eq!(c.shading, None, "the automatic colour is no shading");
    }
}

#[test]
fn a_character_shading_pattern_is_its_blend_in_any_order() {
    // 25 % blue over red: (0*2500 + 255*7500 + 4999) / 10000 = 191; (255*2500 + 4999) / 10000 = 64.
    for words in [r"\chshdng2500\chcfpat2\chcbpat1", r"\chcbpat1\chcfpat2\chshdng2500"] {
        for c in chr_of(words) {
            assert_eq!(c.shading, Some(Rgb(191, 0, 64)), "{words}");
        }
    }
}

#[test]
fn paragraph_shading_reaches_the_paragraph_style() {
    assert_eq!(para_of(r"\cbpat1").shading, Some(RED));
    assert_eq!(para_of(r"\cfpat2\cbpat1\shading2500").shading, Some(Rgb(191, 0, 64)));
    assert_eq!(para_of(r"\shading2500\cbpat1\cfpat2").shading, Some(Rgb(191, 0, 64)));
    assert_eq!(para_of(r"\cbpat0").shading, None);
    assert_eq!(para_of(r"\cbpat99").shading, None);
    assert_eq!(para_of(r"\qc").shading, None);
}

#[test]
fn a_top_border_reaches_the_paragraph_style() {
    let b = para_of(r"\brdrt\brdrs\brdrw10\brdrcf1").borders.expect("borders");
    assert_eq!(b, Borders { top: line(BorderStyle::Single, 0.5, Some(RED), 0.0), ..Default::default() });
}

#[test]
fn a_bottom_border_keeps_its_style_width_and_space() {
    let b = para_of(r"\brdrb\brdrdb\brdrw30\brsp20").borders.expect("borders");
    assert_eq!(b, Borders { bottom: line(BorderStyle::Double, 1.5, None, 1.0), ..Default::default() });
}

#[test]
fn box_sets_the_four_sides_of_the_paragraph_style() {
    let b = para_of(r"\box\brdrs\brdrw20\brdrcf2").borders.expect("borders");
    let side = line(BorderStyle::Single, 1.0, Some(BLUE), 0.0);
    assert_eq!(b, Borders { top: side, left: side, bottom: side, right: side, ..Default::default() });
}

#[test]
fn between_left_and_right_borders_reach_the_paragraph_style() {
    let b = para_of(r"\brdrbtw\brdrs\brdrw10").borders.expect("borders");
    assert_eq!(b, Borders { between: line(BorderStyle::Single, 0.5, None, 0.0), ..Default::default() });
    let b = para_of(r"\brdrl\brdrs\brdrw10\brdrr\brdrdot\brdrw15").borders.expect("borders");
    let want = Borders {
        left: line(BorderStyle::Single, 0.5, None, 0.0),
        right: line(BorderStyle::Dotted, 0.75, None, 0.0),
        ..Default::default()
    };
    assert_eq!(b, want);
}

#[test]
fn a_character_border_and_a_paragraph_border_in_one_entry_are_both_kept() {
    let d = open(r"{\s15 \chbrdr\brdrs\brdrw10\brdrt\brdrdb\brdrw30 XPara;}", r"\pard\plain\f0\fs24 Text.\par");
    let s = d.styles.get("XPara").expect("style XPara");
    assert_eq!(s.chr.border, line(BorderStyle::Single, 0.5, None, 0.0));
    let b = s.para.borders.expect("paragraph borders");
    assert_eq!(b, Borders { top: line(BorderStyle::Double, 1.5, None, 0.0), ..Default::default() });
}

#[test]
fn a_character_border_alone_is_not_a_paragraph_border() {
    assert_eq!(para_of(r"\chbrdr\brdrdb\brdrw30").borders, None);
}

#[test]
fn a_character_style_takes_no_paragraph_shading_or_borders() {
    let d = open(r"{\*\cs10 \additive \brdrt\brdrs\brdrw10\cbpat1\b XChar;}", r"\pard\plain\f0\fs24 Text.\par");
    let s = d.styles.get("XChar").expect("style XChar");
    assert_eq!((s.para.shading, s.para.borders), (None, None));
    assert_eq!(s.chr.bold, Some(true));
    assert_eq!((s.chr.shading, s.chr.border), (None, None));
}

const CHAIN: &str = r"{\s20 \scaps\cbpat1 XBase;}{\s21\sbasedon20 \b XDerived;}";

#[test]
fn a_derived_style_inherits_through_the_model_not_by_copy() {
    let d = open(CHAIN, r"\pard\plain\s21\b\scaps\cbpat1\f0\fs24 Text.\par");
    let derived = d.styles.get("XDerived").expect("style XDerived");
    assert_eq!(derived.based_on.as_deref(), Some("XBase"));
    assert_eq!((derived.chr.bold, derived.chr.small_caps, derived.para.shading), (Some(true), None, None));
    let para = d.styles.resolve_para(&ParaProps { style: Some("XDerived".into()), ..Default::default() });
    assert_eq!(para.shading, Some(RED));
    let run = d.styles.resolve_char(Some("XDerived"), &CharProps::default());
    assert!(run.small_caps && run.bold);
}

#[test]
fn a_paragraph_in_the_style_keeps_the_style() {
    let d = open(CHAIN, r"\pard\plain\s20\scaps\cbpat1\f0\fs24 Text.\par");
    let p = d.body[0].as_para().expect("paragraph");
    assert_eq!(p.props.style.as_deref(), Some("XBase"));
    assert_eq!(d.styles.resolve_para(&p.props).shading, Some(RED));
    assert!(p.run_ranges().all(|(_, c)| d.styles.resolve_char(p.props.style.as_deref(), c).small_caps));
}
