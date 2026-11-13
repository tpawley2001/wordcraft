//! "No tab stop at the hanging indent" (`w:noTabHangInd` in `w:compat`; `Copts60.fNoTabForInd` in
//! a Word 97-2003 file): below compatibility mode 15 a hanging first line has no implicit tab
//! stop at the left indent, so a tab there goes to the next custom or default stop. Explicit
//! stops, default stops and the indent of wrapped lines stay; mode 15 ignores the option.
//! Measured in Microsoft 365 Word on synthetic DOCX files (Letter, 72 pt margins, default stops
//! every 36 pt, 12 pt text); x below is from the left margin.

use super::*;
use wordcraft_doc::props::{NumRef, TabAlign, TabLeader, TabStop};
use wordcraft_doc::{ListKind, para_block};

const MARGIN: f32 = 72.0;
/// The label, its tab, a title with a dot leader typed out, a tab and a page number.
const ENTRY: &str = "C.\tPostpartum..........\tp.13";

fn base(flag: bool, mode: u32) -> Document {
    let mut d = Document::new();
    // Set through the settings' serialized form (`noTabHangInd`), as a saved document carries it.
    let mut v = serde_json::to_value(&d.settings).expect("settings serialize");
    v["noTabHangInd"] = serde_json::Value::Bool(flag);
    d.settings = serde_json::from_value(v).expect("settings deserialize");
    d.settings.compat_mode = mode;
    d.settings.default_tab = 36.0;
    Arc::make_mut(&mut d.styles).default_chr.font = Some(WRAP_TEST_FONT.into());
    Arc::make_mut(&mut d.styles).default_chr.size = Some(12.0);
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (MARGIN, MARGIN, MARGIN, MARGIN);
    d
}

fn para(text: &str, left: f32, hanging: f32, tabs: Option<Vec<TabStop>>) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.indent_left = Some(left);
    p.props.indent_first = Some(-hanging);
    p.props.tabs = tabs;
    p
}

/// `text` as one paragraph indented `left` with a `hanging` first line (points).
fn plain(flag: bool, mode: u32, text: &str, left: f32, hanging: f32, tabs: Option<Vec<TabStop>>) -> Document {
    let mut d = base(flag, mode);
    d.body = vec![para_block(para(text, left, hanging, tabs))];
    d
}

/// `text` as a numbered paragraph whose level is indented 72 pt with an 54 pt hanging number.
fn listed(flag: bool, mode: u32, text: &str) -> Document {
    let mut d = base(flag, mode);
    let num = Arc::make_mut(&mut d.numbering).add_list(ListKind::Numbered);
    let numbering = Arc::make_mut(&mut d.numbering);
    let aid = numbering.nums.iter().find(|n| n.id == num).expect("the new list").abstract_id;
    let lvl = numbering.abstracts.iter_mut().find(|a| a.id == aid).and_then(|a| a.levels.first_mut()).expect("its first level");
    (lvl.indent, lvl.hanging) = (72.0, 54.0);
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.numbering = Some(NumRef { num, level: 0 });
    d.body = vec![para_block(p)];
    d
}

/// Where (from the margin) the text after the `n`th tab starts, and its line.
fn after_tab(l: &DocLayout, text: &str, n: usize) -> (f32, usize) {
    let at = text.match_indices('\t').nth(n).expect("the tab").0 + 1;
    let c = l.caret(&Pos::body(0, at)).expect("caret");
    let para = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para) } else { None }).expect("the paragraph");
    let line = para.lines.iter().position(|ln| at >= ln.start && at < ln.stop.max(ln.start + 1)).expect("its line");
    (c.x - MARGIN, line)
}

/// The x (from the margin) of the paragraph's first character and its line count.
fn start_and_lines(l: &DocLayout) -> (f32, usize) {
    let c = l.caret(&Pos::body(0, 0)).expect("caret");
    let para = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para) } else { None }).expect("the paragraph");
    (c.x - MARGIN, para.lines.len())
}

/// The list number's left edge (from the margin).
fn label_x(l: &DocLayout) -> f32 {
    l.pages[0]
        .items
        .iter()
        .find_map(|i| if let Placed::Lines { para, x, .. } = i { para.label.as_ref().map(|lab| x + lab.x) } else { None })
        .expect("a list number")
        - MARGIN
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= 0.5, "{what}: {got}, want {want}");
}

/// Rows 1-3: left 360 pt, hanging 324 pt. Expectations: (title x, page-number x, its line,
/// paragraph lines).
fn wide_hang(flag: bool, mode: u32, want: (f32, f32, usize, usize)) {
    let l = lay(&plain(flag, mode, ENTRY, 360.0, 324.0, None));
    let (x0, lines) = start_and_lines(&l);
    near(x0, 36.0, "the label");
    let (title, tl) = after_tab(&l, ENTRY, 0);
    near(title, want.0, "the title");
    assert_eq!(tl, 0);
    let (page, pl) = after_tab(&l, ENTRY, 1);
    assert_eq!(pl, want.2, "the page number's line");
    if pl == 0 {
        near(page, want.1, "the page number");
    } else {
        // The title ends past 432 pt, so the next default stop is the right edge (468 pt). Word
        // takes such a tab to the next line, where it goes from the indent (360) to 396; we keep
        // a tab whose stop is exactly at the edge on its line, so the number starts the next
        // line at the indent. That edge rule is separate from this option.
        assert!(page > 360.0 - 0.5 && page < want.1 + 0.5, "the page number on the wrapped line: {page}, want 360..{}", want.1);
    }
    assert_eq!(lines, want.3, "lines");
}

#[test]
fn row1_option_off_tabs_to_the_hanging_indent() {
    wide_hang(false, 11, (360.0, 396.0, 1, 2));
}

#[test]
fn row2_option_on_tabs_to_the_next_default_stops() {
    wide_hang(true, 11, (72.0, 180.0, 0, 1));
}

#[test]
fn row3_mode_15_ignores_the_option() {
    wide_hang(true, 15, (360.0, 396.0, 1, 2));
}

fn narrow_hang(flag: bool, title: f32, page: f32) {
    let l = lay(&plain(flag, 11, ENTRY, 108.0, 72.0, None));
    near(start_and_lines(&l).0, 36.0, "the label");
    near(after_tab(&l, ENTRY, 0).0, title, "the title");
    let (x, line) = after_tab(&l, ENTRY, 1);
    near(x, page, "the page number");
    assert_eq!(line, 0);
}

#[test]
fn row4_option_off_narrow_hang() {
    narrow_hang(false, 108.0, 216.0);
}

#[test]
fn row5_option_on_narrow_hang() {
    narrow_hang(true, 72.0, 180.0);
}

#[test]
fn row6_an_explicit_stop_still_wins_over_default_stops() {
    let stop = TabStop { pos: 252.0, align: TabAlign::Left, leader: TabLeader::None };
    let l = lay(&plain(true, 11, ENTRY, 360.0, 324.0, Some(vec![stop])));
    near(start_and_lines(&l).0, 36.0, "the label");
    near(after_tab(&l, ENTRY, 0).0, 252.0, "the title");
    let (x, line) = after_tab(&l, ENTRY, 1);
    near(x, 360.0, "the page number");
    assert_eq!(line, 0);
}

const ITEM: &str = "Postpartum\tp.13";

fn list_row(flag: bool, mode: u32, title: f32, page: f32) {
    let l = lay(&listed(flag, mode, ITEM));
    near(label_x(&l), 18.0, "the number");
    near(start_and_lines(&l).0, title, "the title");
    let (x, line) = after_tab(&l, ITEM, 0);
    near(x, page, "the page number");
    assert_eq!(line, 0);
}

#[test]
fn row7_list_option_off() {
    list_row(false, 11, 72.0, 144.0);
}

#[test]
fn row8_list_option_on_number_tabs_to_the_next_default_stop() {
    list_row(true, 11, 36.0, 108.0);
}

#[test]
fn row9_list_mode_15_ignores_the_option() {
    list_row(true, 15, 72.0, 144.0);
}

#[test]
fn row10_wrapped_lines_keep_the_left_indent() {
    let text = "C.\tPostpartum care and recovery notes for the first weeks after birth, with follow up visits\tp.13";
    let l = lay(&plain(true, 11, text, 108.0, 72.0, None));
    near(after_tab(&l, text, 0).0, 72.0, "the title");
    let para = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para.clone()) } else { None }).expect("para");
    assert!(para.lines.len() >= 2, "the text wraps");
    let second = para.lines[1].start;
    near(l.caret(&Pos::body(0, second)).expect("caret").x - MARGIN, 108.0, "the wrapped line");
    // The page number goes to the first default stop past the text before its tab.
    let (x, line) = after_tab(&l, text, 1);
    assert_eq!(line, para.lines.len() - 1);
    let tab = text.rfind('\t').expect("tab");
    let end = l.caret(&Pos::body(0, tab)).expect("caret").x - MARGIN;
    near(x, ((end / 36.0).floor() + 1.0) * 36.0, "the page number");
}
