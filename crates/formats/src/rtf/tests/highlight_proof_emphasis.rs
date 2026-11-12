//! Text highlight (`\highlightN`), "Do not check spelling or grammar" (`\noproof`) and emphasis
//! marks (`\accnone`, `\accdot`, `\acccomma`, `\acccircle`, `\accunderdot`), RTF 1.9.1: read into
//! the run's own properties and written back, the RTF form of DOCX `w:highlight`, `w:noProof` and
//! `w:em`. Character shading (`\chcbpatN`, `\cbN`) stays shading (`w:shd`).

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{CharProps, EmphasisMark, Highlight, Rgb};

/// 1 yellow, 2 bright green, 3 a colour that is no highlight colour, 4 one off yellow.
const COLORS: &str =
    r"{\colortbl;\red255\green255\blue0;\red0\green255\blue0;\red200\green10\blue10;\red255\green255\blue1;}";
const ODD: Rgb = Rgb(200, 10, 10);

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{COLORS}{body}}}").as_bytes()).unwrap()
}

/// The properties of the run holding `needle` in paragraph 0.
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    for r in &p.runs {
        let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

/// (highlight, shading) of the run holding `needle`; no highlight is `None` either way it is stored.
fn back(d: &Document, needle: &str) -> (Option<Highlight>, Option<Rgb>) {
    let c = run(d, needle);
    (c.highlight.filter(|h| *h != Highlight::None), c.shading)
}

fn mark(d: &Document, needle: &str) -> Option<EmphasisMark> {
    run(d, needle).emphasis.filter(|m| *m != EmphasisMark::None)
}

fn no_proof(d: &Document, needle: &str) -> bool {
    run(d, needle).no_proof == Some(true)
}

#[test]
fn highlight_reads_as_highlight_not_shading() {
    let d = open(r"\pard{\highlight1 Yel}{\highlight2 Grn}plain\par");
    assert_eq!(back(&d, "Yel"), (Some(Highlight::Yellow), None));
    assert_eq!(back(&d, "Grn"), (Some(Highlight::BrightGreen), None));
    assert_eq!(back(&d, "plain"), (None, None));
}

#[test]
fn character_shading_words_stay_shading() {
    let d = open(r"\pard{\chcbpat1 Pat}{\cb2 Old}plain\par");
    assert_eq!(back(&d, "Pat"), (None, Some(Rgb(255, 255, 0))));
    assert_eq!(back(&d, "Old"), (None, Some(Rgb(0, 255, 0))));
    assert_eq!(back(&d, "plain"), (None, None));
}

#[test]
fn a_highlight_in_no_highlight_colour_stays_shading() {
    let d = open(r"\pard{\highlight3 Odd}{\highlight4 Near}plain\par");
    assert_eq!(back(&d, "Odd"), (None, Some(ODD)));
    assert_eq!(back(&d, "Near"), (None, Some(Rgb(255, 255, 1))));
}

#[test]
fn highlight_and_shading_sit_on_one_run() {
    let d = open(r"\pard{\highlight1\chcbpat3 Both}plain\par");
    assert_eq!(back(&d, "Both"), (Some(Highlight::Yellow), Some(ODD)));
    assert_eq!(back(&d, "plain"), (None, None));
}

#[test]
fn highlight_is_turned_off_by_zero_a_missing_colour_and_plain() {
    let d = open(r"\pard\highlight1 Aaa\highlight0 Bbb{\highlight99 Ccc}\highlight2\plain Ddd\par");
    assert_eq!(back(&d, "Aaa"), (Some(Highlight::Yellow), None));
    assert_eq!(back(&d, "Bbb"), (None, None));
    assert_eq!(back(&d, "Ccc"), (None, None));
    assert_eq!(back(&d, "Ddd"), (None, None));
}

#[test]
fn highlight_is_written_as_highlight() {
    let d = open(r"\pard{\highlight1 Yel}plain\par");
    let out = export(&d);
    assert!(out.contains("\\highlight"), "{out}");
    assert!(!out.contains("\\chcbpat"), "a highlight is not written as shading: {out}");
    let again = import(out.as_bytes()).unwrap();
    assert_eq!(back(&again, "Yel"), (Some(Highlight::Yellow), None));
    assert_eq!(back(&again, "plain"), (None, None));
}

#[test]
fn highlight_over_shading_and_plain_shading_survive_export() {
    let d = open(r"\pard{\highlight2\chcbpat3 Both}{\chcbpat3 Shade}plain\par");
    let out = export(&d);
    assert!(out.contains("\\highlight") && out.contains("\\chcbpat"), "{out}");
    let again = import(out.as_bytes()).unwrap();
    assert_eq!(back(&again, "Both"), (Some(Highlight::BrightGreen), Some(ODD)));
    assert_eq!(back(&again, "Shade"), (None, Some(ODD)));
    assert_eq!(back(&again, "plain"), (None, None));
}

#[test]
fn noproof_is_read_and_turned_off() {
    let d = open(r"\pard{\noproof Skip}{\noproof\noproof0 Check}{\noproof\plain Plain}rest\par");
    assert!(no_proof(&d, "Skip"));
    assert!(!no_proof(&d, "Check"));
    assert!(!no_proof(&d, "Plain"));
    assert!(!no_proof(&d, "rest"));
}

#[test]
fn each_emphasis_mark_is_read() {
    let d = open(
        r"\pard{\accdot Dot}{\acccomma Com}{\acccircle Cir}{\accunderdot Und}{\accdot\accnone Non}{\accdot\acccircle Last}rest\par",
    );
    assert_eq!(mark(&d, "Dot"), Some(EmphasisMark::Dot));
    assert_eq!(mark(&d, "Com"), Some(EmphasisMark::Comma));
    assert_eq!(mark(&d, "Cir"), Some(EmphasisMark::Circle));
    assert_eq!(mark(&d, "Und"), Some(EmphasisMark::UnderDot));
    assert_eq!(mark(&d, "Non"), None);
    assert_eq!(mark(&d, "Last"), Some(EmphasisMark::Circle));
    assert_eq!(mark(&d, "rest"), None);
}

#[test]
fn noproof_and_emphasis_marks_survive_export() {
    let d = open(r"\pard{\noproof Skip}{\accdot Dot}{\acccomma Com}{\acccircle Cir}{\accunderdot Und}rest\par");
    let out = export(&d);
    for w in ["\\noproof", "\\accdot", "\\acccomma", "\\acccircle", "\\accunderdot"] {
        assert!(out.contains(w), "{w} missing: {out}");
    }
    let again = import(out.as_bytes()).unwrap();
    assert!(no_proof(&again, "Skip"));
    assert!(!no_proof(&again, "rest"));
    assert_eq!(mark(&again, "Dot"), Some(EmphasisMark::Dot));
    assert_eq!(mark(&again, "Com"), Some(EmphasisMark::Comma));
    assert_eq!(mark(&again, "Cir"), Some(EmphasisMark::Circle));
    assert_eq!(mark(&again, "Und"), Some(EmphasisMark::UnderDot));
    assert_eq!(mark(&again, "rest"), None);
}

#[test]
fn a_plain_document_writes_none_of_the_words() {
    let out = export(&open(r"\pard plain text\par"));
    for w in ["\\highlight", "\\noproof", "\\accdot", "\\acccomma", "\\acccircle", "\\accunderdot", "\\accnone"] {
        assert!(!out.contains(w), "{w} in {out}");
    }
}
