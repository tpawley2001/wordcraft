//! A numbered paragraph whose tab list clears the stop at its list level's number tab, as
//! Microsoft 365 Word lays it out (synthetic probes: Letter, 72 pt margins, default tab 36 pt,
//! "1.<tab>Section", the number at 36 pt and the level's tab at 54 pt). A `clear` tab at exactly
//! the level's tab position, from the paragraph style or the paragraph itself, removes that
//! stop: the text goes to the paragraph's next stop (a custom one, else the next default stop).
//! A hanging indent still acts as a stop. Positions are from the left margin.
use crate::*;
use std::sync::Arc;
use wordcraft_doc::numbering::LevelSuffix;
use wordcraft_doc::props::{Align, NumRef, TabAlign, TabLeader, TabStop};
use wordcraft_doc::styles::Style;
use wordcraft_doc::{ListKind, para_block};

fn stop(pos: f32, align: TabAlign) -> TabStop {
    TabStop { pos, align, leader: TabLeader::None }
}

/// Where "Section" starts (from the left margin) in "1.<tab>Section", numbered at level 0 with
/// the level's left indent `indent` and hanging indent `hanging` (negative: a first-line indent)
/// and its tab at 54 pt, in a paragraph style with `style_tabs` and direct tabs `direct_tabs`.
fn text_x(style_tabs: Option<Vec<TabStop>>, direct_tabs: Option<Vec<TabStop>>, indent: f32, hanging: f32) -> f32 {
    crate::test_fonts();
    let mut d = Document::new();
    d.settings.default_tab = 36.0;
    let num = Arc::make_mut(&mut d.numbering).add_list(ListKind::Numbered);
    let numbering = Arc::make_mut(&mut d.numbering);
    let abstract_id = numbering.nums.iter().find(|n| n.id == num).expect("the new list").abstract_id;
    let lvl = numbering.abstracts.iter_mut().find(|a| a.id == abstract_id).and_then(|a| a.levels.first_mut()).expect("its first level");
    lvl.text = "%1.".into();
    lvl.align = Align::Left;
    lvl.indent = indent;
    lvl.hanging = hanging;
    lvl.suffix = LevelSuffix::Tab;
    lvl.tab = Some(54.0);
    let mut style = Style { id: "ParaNum".into(), name: "ParaNum".into(), ..Style::default() };
    style.para.tabs = style_tabs;
    Arc::make_mut(&mut d.styles).upsert(style);
    let mut p = Paragraph::with_text("Section", CharProps::default());
    p.props.style = Some("ParaNum".into());
    p.props.numbering = Some(NumRef { num, level: 0 });
    p.props.tabs = direct_tabs;
    d.body = vec![para_block(p)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let pl = page
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { para, .. } if para.label.is_some() => Some(para),
            _ => None,
        })
        .expect("the numbered paragraph");
    let line = pl.lines.first().expect("a line");
    (line.c0..line.c1)
        .filter(|&k| pl.clusters.get(k).is_some_and(|c| matches!(c.kind, para::ClKind::Text)))
        .filter_map(|k| line.cl_left(k))
        .fold(f32::MAX, f32::min)
}

fn assert_x(got: f32, word: f32, what: &str) {
    // Word's x is on the page; the margin is 72 pt.
    assert!((got - (word - 72.0)).abs() <= 0.1, "{what}: text at {got} from the margin, Word {}", word - 72.0);
}

/// Probe (a), and (d) in compatibility mode 15, which lays out the same: the style's tabs
/// left 36, clear 54, left 72.
#[test]
fn a_style_clearing_the_level_tab_sends_the_text_to_its_next_stop() {
    let tabs = vec![stop(36.0, TabAlign::Left), stop(54.0, TabAlign::Clear), stop(72.0, TabAlign::Left)];
    assert_x(text_x(Some(tabs), None, 0.0, -36.0), 144.0, "probe a");
}

/// Probe (b): the same stops without the clear keep the level's tab.
#[test]
fn without_the_clear_the_level_tab_is_used() {
    let tabs = vec![stop(36.0, TabAlign::Left), stop(72.0, TabAlign::Left)];
    assert_x(text_x(Some(tabs), None, 0.0, -36.0), 126.0, "probe b");
}

/// Probe (c): the paragraph's own clear.
#[test]
fn a_direct_clear_removes_the_level_tab() {
    assert_x(text_x(None, Some(vec![stop(54.0, TabAlign::Clear)]), 0.0, -36.0), 144.0, "probe c");
}

/// Probe (e): only the clear; the text goes to the next default stop.
#[test]
fn a_clear_alone_goes_to_the_next_default_stop() {
    assert_x(text_x(Some(vec![stop(54.0, TabAlign::Clear)]), None, 0.0, -36.0), 144.0, "probe e");
}

/// Probe (f): a hanging indent at 54 pt still acts as a stop.
#[test]
fn a_hanging_indent_still_acts_as_a_stop() {
    assert_x(text_x(Some(vec![stop(54.0, TabAlign::Clear)]), None, 54.0, 18.0), 126.0, "probe f");
}

/// A clear at another position doesn't remove the level's stop.
#[test]
fn a_clear_elsewhere_keeps_the_level_tab() {
    assert_x(text_x(Some(vec![stop(55.0, TabAlign::Clear)]), None, 0.0, -36.0), 126.0, "clear at 55 pt");
}

/// A derived stop at the cleared position (the paragraph sets a left stop where its style
/// cleared one) brings a stop back: the clear no longer applies.
#[test]
fn a_direct_stop_over_the_cleared_position_wins() {
    let style = vec![stop(54.0, TabAlign::Clear), stop(72.0, TabAlign::Left)];
    assert_x(text_x(Some(style), Some(vec![stop(54.0, TabAlign::Left)]), 0.0, -36.0), 126.0, "direct stop at 54 pt");
}
