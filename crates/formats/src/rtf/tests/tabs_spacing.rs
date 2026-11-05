//! Tab stops (`\tx`, `\tb` with `\tq*` alignment and `\tl*` leaders) and character spacing
//! (`\expnd`, `\expndtw`), RTF 1.9.1: read into the paragraph's and the run's own properties and
//! written back, as Word does.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{TabAlign, TabLeader};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Courier New;}}}}{body}}}").as_bytes()).unwrap()
}

/// Paragraph `i`'s own tab stops: (pos pt, align, leader).
fn tabs(d: &Document, i: usize) -> Vec<(f32, TabAlign, TabLeader)> {
    let p = d.body[i].as_para().unwrap();
    p.props.tabs.iter().flatten().map(|t| (t.pos, t.align, t.leader)).collect()
}

/// Each run's (text, spacing) in paragraph `i`.
fn spacing(d: &Document, i: usize) -> Vec<(String, Option<f32>)> {
    let p = d.body[i].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    p.runs
        .iter()
        .map(|r| {
            let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
            at += r.len;
            (s, r.props.spacing)
        })
        .collect()
}

#[test]
fn a_right_tab_with_a_dot_leader() {
    // An agenda line: page number right-aligned at 7488 twips after dots.
    let d = open(r"\pard\tqr\tldot\tx7488 Call to Order\tab 3\par");
    assert_eq!(tabs(&d, 0), [(374.4, TabAlign::Right, TabLeader::Dot)]);
}

#[test]
fn alignments_leaders_and_bar_tabs() {
    let d = open(r"\pard\tx720\tqc\tlhyph\tx2880\tqdec\tlul\tx4320\tb5040\tlmdot\tx6480 a\par");
    assert_eq!(
        tabs(&d, 0),
        [
            (36.0, TabAlign::Left, TabLeader::None),
            (144.0, TabAlign::Center, TabLeader::Hyphen),
            (216.0, TabAlign::Decimal, TabLeader::Underscore),
            (252.0, TabAlign::Bar, TabLeader::None),
            (324.0, TabAlign::Left, TabLeader::MiddleDot),
        ]
    );
}

#[test]
fn pard_clears_the_stops() {
    let d = open(r"\pard\tqr\tx7488 one\par two\par\pard three\par");
    assert_eq!(tabs(&d, 0), [(374.4, TabAlign::Right, TabLeader::None)]);
    // Paragraph properties carry on to the next paragraph until `\pard`.
    assert_eq!(tabs(&d, 1), [(374.4, TabAlign::Right, TabLeader::None)]);
    assert!(tabs(&d, 2).is_empty());
}

#[test]
fn condensed_and_expanded_text() {
    // `\expndtw` is twips; `\expnd` quarter points; `\plain` resets.
    let d = open(r"\pard{\expnd0\expndtw-3 tight}{\expnd4 wide}{\expnd4\expndtw30 \plain plain}\par");
    let got = spacing(&d, 0);
    let find = |t: &str| got.iter().find(|(s, _)| s.contains(t)).map(|(_, v)| *v).unwrap_or_else(|| panic!("{got:?}"));
    assert_eq!(find("tight"), Some(-0.15));
    assert_eq!(find("wide"), Some(1.0));
    assert!(matches!(find("plain"), None | Some(0.0)), "{got:?}");
}

#[test]
fn tabs_and_spacing_round_trip() {
    let d = open(r"\pard\tqr\tldot\tx7488\tb5040{\expndtw-3 Call to Order}\tab 3\par");
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(tabs(&back, 0), tabs(&d, 0));
    assert_eq!(tabs(&back, 0).len(), 2);
    assert_eq!(spacing(&back, 0).iter().find(|(s, _)| s.contains("Call")).map(|(_, v)| *v), Some(Some(-0.15)));
}
