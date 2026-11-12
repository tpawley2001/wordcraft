//! A paragraph with tab stops (`\tx`, `\tb`) keeps its other own properties on open: widow/orphan
//! control (`\widctlpar`, `\nowidctlpar`) and the character-unit indents (`\cufiN`, `\culiN`,
//! `\curiN`), whatever the order of the words, and they are written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{TabAlign, TabLeader};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}\widowctrl{body}}}").as_bytes()).unwrap()
}

/// Each paragraph's own widow control.
fn widow(d: &Document) -> Vec<Option<bool>> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.widow_control).collect()
}

/// Paragraph `i`'s own tab stops: (pos pt, align, leader).
fn tabs(d: &Document, i: usize) -> Vec<(f32, TabAlign, TabLeader)> {
    let p = d.body[i].as_para().unwrap();
    p.props.tabs.iter().flatten().map(|t| (t.pos, t.align, t.leader)).collect()
}

/// (left, right, first) in characters, per paragraph.
fn chars(d: &Document) -> Vec<(Option<f32>, Option<f32>, Option<f32>)> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| (p.props.indent_left_chars, p.props.indent_right_chars, p.props.indent_first_chars)).collect()
}

#[test]
fn widow_control_off_survives_a_tab_stop() {
    let d = open(r"\pard\nowidctlpar\tx720 a\tab b\par");
    assert_eq!(widow(&d), [Some(false)]);
    assert_eq!(tabs(&d, 0), [(36.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn widow_control_on_survives_a_right_tab_with_a_leader() {
    let d = open(r"\pard\widctlpar\tqr\tldot\tx7488 a\tab 3\par");
    assert_eq!(widow(&d), [Some(true)]);
    assert_eq!(tabs(&d, 0), [(374.4, TabAlign::Right, TabLeader::Dot)]);
}

#[test]
fn the_order_of_the_words_does_not_matter() {
    let d = open(r"\pard\tx720\nowidctlpar a\par");
    assert_eq!(widow(&d), [Some(false)]);
    assert_eq!(tabs(&d, 0), [(36.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn character_unit_indents_survive_a_tab_stop() {
    let d = open(r"\pard\cufi150\culi200\curi100\tx1440 a\par");
    assert_eq!(chars(&d), [(Some(2.0), Some(1.0), Some(1.5))]);
    assert_eq!(widow(&d), [None]);
    assert_eq!(tabs(&d, 0), [(72.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn widow_control_and_an_indent_with_two_tab_stops() {
    let d = open(r"\pard\nowidctlpar\culi200\tx720\tx1440 a\par");
    assert_eq!(widow(&d), [Some(false)]);
    assert_eq!(chars(&d), [(Some(2.0), None, None)]);
    assert_eq!(tabs(&d, 0), [(36.0, TabAlign::Left, TabLeader::None), (72.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn the_state_lasts_across_par_until_pard() {
    let d = open(r"\pard\nowidctlpar\tx720 a\par b\par\pard c\par");
    assert_eq!(widow(&d), [Some(false), Some(false), None]);
    assert_eq!([tabs(&d, 0).len(), tabs(&d, 1).len(), tabs(&d, 2).len()], [1, 1, 0]);
}

#[test]
fn tab_stops_alone_invent_nothing() {
    let d = open(r"\pard\tx720 a\par");
    assert_eq!(widow(&d), [None]);
    assert_eq!(chars(&d), [(None, None, None)]);
    assert_eq!(tabs(&d, 0), [(36.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn everything_round_trips() {
    let d = open(r"\pard\nowidctlpar\culi200\tx720\tx1440 a\par\pard\widctlpar\cufi150\tqr\tldot\tx7488 b\tab 3\par\pard c\par");
    let rtf = export(&d);
    for word in [r"\nowidctlpar", r"\culi200", r"\tx720", r"\tx1440", r"\widctlpar", r"\cufi150", r"\tx7488"] {
        assert!(rtf.contains(word), "{word} missing in {rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(widow(&back), [Some(false), Some(true), None]);
    assert_eq!(chars(&back), [(Some(2.0), None, None), (None, None, Some(1.5)), (None, None, None)]);
    assert_eq!(tabs(&back, 0), [(36.0, TabAlign::Left, TabLeader::None), (72.0, TabAlign::Left, TabLeader::None)]);
    assert_eq!(tabs(&back, 1), [(374.4, TabAlign::Right, TabLeader::Dot)]);
    assert!(tabs(&back, 2).is_empty());
}

/// The writer is not the cause: a model paragraph holding all three is written with all three.
#[test]
fn the_writer_already_writes_all_three() {
    let mut d = open(r"\pard\tx720\tx1440 a\par\pard\nowidctlpar\culi200 b\par");
    let stops = d.body[0].as_para().unwrap().props.tabs.clone();
    assert_eq!(stops.as_ref().map(Vec::len), Some(2));
    if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(&mut d.body[1]) {
        p.props.tabs = stops;
    }
    let rtf = export(&d);
    let second = rtf.rsplit(r"\pard").next().unwrap_or_default();
    for word in [r"\nowidctlpar", r"\culi200", r"\tx720", r"\tx1440"] {
        assert!(second.contains(word), "{word} missing in the second paragraph: {rtf}");
    }
}
