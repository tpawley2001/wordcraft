//! Percent shading patterns (`\shadingN`, `\chshdngN`, `\clshdngN` with their pattern and
//! background colours, RTF 1.9.1) on paragraphs, text and table cells: read as the one colour
//! Word paints, the pattern colour over the background at the pattern's coverage. Automatic is
//! black for the pattern and white for the background; exact halves go down.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::Rgb;

const RED: Rgb = Rgb(255, 0, 0);
/// 25 % blue over red.
const QUARTER_BLUE_ON_RED: Rgb = Rgb(191, 0, 64);
const GREY_20: Rgb = Rgb(204, 204, 204);

fn open(body: &str) -> Document {
    import(
        format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}}{body}}}").as_bytes(),
    )
    .unwrap()
}

fn para_shading(d: &Document) -> Vec<Option<Rgb>> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.shading).collect()
}

/// The shading of the run holding `needle` in the first paragraph.
fn run_shading(d: &Document, needle: &str) -> Option<Rgb> {
    let p = d.body[0].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    for r in &p.runs {
        let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
        at += r.len;
        if s.contains(needle) {
            return r.props.shading;
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

fn cell_shading(d: &Document) -> Vec<Option<Rgb>> {
    let t = d.body.iter().find_map(|b| b.as_table()).expect("a table");
    t.rows[0].cells.iter().map(|c| c.props.shading).collect()
}

#[test]
fn paragraph_percent_pattern_blends_pattern_colour_over_background() {
    let d = open(r"\pard\cfpat2\cbpat1\shading2500 a\par\pard\shading2500\cbpat1\cfpat2 b\par");
    assert_eq!(para_shading(&d), [Some(QUARTER_BLUE_ON_RED), Some(QUARTER_BLUE_ON_RED)]);
}

#[test]
fn paragraph_percent_pattern_without_colours_is_grey() {
    let d = open(r"\pard\shading2000 a\par\pard\shading1000 b\par\pard\shading5000 c\par\pard\cfpat0\cbpat0\shading2000 d\par");
    // 10 % is 229.5 and 50 % is 127.5: exact halves go down.
    assert_eq!(para_shading(&d), [Some(GREY_20), Some(Rgb(229, 229, 229)), Some(Rgb(127, 127, 127)), Some(GREY_20)]);
}

#[test]
fn paragraph_solid_pattern_is_the_pattern_colour_black_when_automatic() {
    let d = open(r"\pard\cfpat1\shading10000 a\par\pard\shading10000 b\par\pard\cbpat1\shading10000 c\par\pard\shading20000 d\par");
    assert_eq!(para_shading(&d), [Some(RED), Some(Rgb(0, 0, 0)), Some(Rgb(0, 0, 0)), Some(Rgb(0, 0, 0))]);
}

#[test]
fn paragraph_without_a_percentage_keeps_only_its_background() {
    let d = open(r"\pard\cbpat1 a\par\pard\cbpat1\shading0 b\par\pard\cfpat2 c\par\pard\shading0 d\par\pard\cfpat2\cbpat1 e\par");
    assert_eq!(para_shading(&d), [Some(RED), Some(RED), None, None, Some(RED)]);
}

#[test]
fn paragraph_percent_pattern_ends_at_pard() {
    let d = open(r"\pard\shading2000 a\par b\par\pard c\par");
    assert_eq!(para_shading(&d), [Some(GREY_20), Some(GREY_20), None]);
}

#[test]
fn character_percent_pattern_blends_in_any_order() {
    let d = open(r"\pard{\chshdng5000\chcfpat1 Half}{\chshdng2500\chcfpat2\chcbpat1 Quarter}{\chcbpat1\chcfpat2\chshdng2500 Turned}{\chshdng2000 Grey}plain\par");
    assert_eq!(run_shading(&d, "Half"), Some(Rgb(255, 127, 127)));
    assert_eq!(run_shading(&d, "Quarter"), Some(QUARTER_BLUE_ON_RED));
    assert_eq!(run_shading(&d, "Turned"), Some(QUARTER_BLUE_ON_RED));
    assert_eq!(run_shading(&d, "Grey"), Some(GREY_20));
    assert_eq!(run_shading(&d, "plain"), None);
}

#[test]
fn character_background_alone_is_unchanged() {
    let d = open(r"\pard{\chcbpat1 Pat}{\cb1 Old}{\chshdng0\chcfpat2\chcbpat1 Zero}{\chshdng0\chcfpat0\chcbpat0 None}{\chcfpat2 Ink}plain\par");
    assert_eq!(run_shading(&d, "Pat"), Some(RED));
    assert_eq!(run_shading(&d, "Old"), Some(RED));
    assert_eq!(run_shading(&d, "Zero"), Some(RED));
    assert_eq!(run_shading(&d, "None"), None);
    assert_eq!(run_shading(&d, "Ink"), None);
}

#[test]
fn character_percent_pattern_ends_at_plain_and_at_the_group() {
    let d = open(r"\pard\chshdng2000 Grey\plain Reset{\chshdng10000 Solid}After\par");
    assert_eq!(run_shading(&d, "Grey"), Some(GREY_20));
    assert_eq!(run_shading(&d, "Reset"), None);
    assert_eq!(run_shading(&d, "Solid"), Some(Rgb(0, 0, 0)));
    assert_eq!(run_shading(&d, "After"), None);
}

#[test]
fn cell_percent_pattern_blends_per_cell() {
    let d = open(
        r"\trowd\clshdng2500\clcfpat2\clcbpat1\cellx2000\clshdng3000\cellx4000\clcbpat1\cellx6000\clcbpat1\clcfpat2\clshdng2500\cellx8000\cellx10000\pard\intbl A\cell B\cell C\cell D\cell E\cell\row\pard after\par",
    );
    // 30 % is 178.5: exact halves go down. The last cell has no shading of its own.
    assert_eq!(cell_shading(&d), [Some(QUARTER_BLUE_ON_RED), Some(Rgb(178, 178, 178)), Some(RED), Some(QUARTER_BLUE_ON_RED), None]);
}

#[test]
fn percent_shading_round_trips_as_its_colour() {
    let d = open(
        r"\trowd\clshdng2500\clcfpat2\clcbpat1\cellx2000\clshdng3000\cellx4000\pard\intbl A\cell B\cell\row\pard\shading2000 {\chshdng5000\chcfpat1 Half}\par",
    );
    let out = export(&d);
    for word in [r"\shading", r"\chshdng", r"\clshdng"] {
        assert!(!out.contains(word), "{word} is not written back: {out}");
    }
    let back = import(out.as_bytes()).unwrap();
    assert_eq!(cell_shading(&back), [Some(QUARTER_BLUE_ON_RED), Some(Rgb(178, 178, 178))]);
    let p = back.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains("Half")).expect("the paragraph");
    assert_eq!(p.props.shading, Some(GREY_20));
    assert_eq!(p.runs.iter().find_map(|r| r.props.shading), Some(Rgb(255, 127, 127)));
}
