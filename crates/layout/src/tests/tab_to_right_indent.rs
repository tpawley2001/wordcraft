//! A left tab whose next stop is a default stop past the right indent ends at the right indent
//! instead of wrapping; only the tab after it wraps. Word draws form fill-in lines (a label and
//! a row of underlined tabs) this way: each takes one line, not two.

use crate::*;
use wordcraft_doc::Pos;
use wordcraft_doc::props::{Align, TabAlign, TabLeader, TabStop};

/// `text` in one paragraph with an 18 pt right indent: on a Letter page with 72 pt margins the
/// line's right edge is 450 pt from the margin (page x 522); default stops every 36 pt, the
/// last one before the edge at 432 (page x 504).
fn doc(text: &str, align: Option<Align>, tabs: Option<Vec<TabStop>>) -> Document {
    let mut d = Document::from_text(text);
    d.settings.default_tab = 36.0;
    d.format_paragraphs(&Pos::body(0, 0), &Pos::body(0, 0), &|p| {
        p.indent_right = Some(18.0);
        if align.is_some() {
            p.align = align;
        }
        if tabs.is_some() {
            p.tabs = tabs.clone();
        }
    })
    .unwrap();
    d
}

fn lay(d: &Document) -> DocLayout {
    layout(d, &mut LayoutCache::new(), &LayoutOptions::default())
}

/// The paragraph's lines as cluster ranges.
fn lines(l: &DocLayout) -> Vec<(usize, usize)> {
    let para = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para.clone()) } else { None }).expect("no lines");
    para.lines.iter().map(|l| (l.c0, l.c1)).collect()
}

fn tabs(n: usize) -> String {
    format!("ab{}", "\t".repeat(n))
}

#[test]
fn a_tab_with_no_stop_before_the_right_indent_ends_there() {
    let text = tabs(13);
    let d = doc(&text, None, None);
    let l = lay(&d);
    assert_eq!(lines(&l).len(), 1, "13 tabs: 12 to the default stops, the 13th to the right indent: {:?}", lines(&l));
    let end = l.caret(&Pos::body(0, text.len())).unwrap();
    assert!((end.x - 522.0).abs() < 0.5, "the last tab ends at the right indent (page x 522): {}", end.x);
}

#[test]
fn only_the_tab_after_the_right_indent_wraps() {
    let text = tabs(26);
    let d = doc(&text, None, None);
    let l = lay(&d);
    let ls = lines(&l);
    assert_eq!(ls.len(), 2, "26 tabs take two lines, as in Word: {ls:?}");
    let end = l.caret(&Pos::body(0, text.len())).unwrap();
    assert!((end.x - 522.0).abs() < 0.5, "line 2 also ends at the right indent: {}", end.x);
}

#[test]
fn a_centred_line_of_tabs_fills_the_line() {
    let d = doc(&tabs(13), Some(Align::Center), None);
    let l = lay(&d);
    assert_eq!(lines(&l).len(), 1, "{:?}", lines(&l));
    let start = l.caret(&Pos::body(0, 0)).unwrap();
    assert!((start.x - 72.0).abs() < 0.5, "a full line has no centring slack: starts at {}", start.x);
}

#[test]
fn a_default_stop_before_the_right_indent_is_still_used() {
    let text = tabs(12);
    let d = doc(&text, None, None);
    let l = lay(&d);
    assert_eq!(lines(&l).len(), 1);
    let end = l.caret(&Pos::body(0, text.len())).unwrap();
    assert!((end.x - 504.0).abs() < 0.5, "the 12th tab stops at the default stop 432 (page x 504): {}", end.x);
}

#[test]
fn a_custom_stop_past_the_right_indent_still_wraps() {
    let stop = vec![TabStop { pos: 470.0, align: TabAlign::Left, leader: TabLeader::None }];
    let d = doc("ab\tX", None, Some(stop));
    let l = lay(&d);
    assert_eq!(lines(&l).len(), 2, "a custom left stop past the right indent wraps (#1840): {:?}", lines(&l));
}
