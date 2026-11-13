//! A line of a document opened from RTF (`Settings::rtf_justification`) that holds a tab keeps the
//! word that overruns its right edge by squeezing the single spaces after the line's last tab,
//! when the overrun is less than half their width and they are in a fixed-pitch font. Spaces
//! before the last tab, or between two tabs, never count and keep their width, as do the tab and
//! the text before it.
//!
//! Measured in Microsoft 365 Word (16.0.20527) from synthetic RTF files: Courier New 10 pt (6 pt
//! a character), Letter page, margins 68.4 pt left and right (text 79.2 characters wide).

use super::*;
use wordcraft_doc::props::{Align, TabAlign, TabLeader, TabStop};
use wordcraft_doc::{CharProps, Run};

const MARGIN: f32 = 68.4;
const PAGE_W: f32 = 612.0;
const EDGE: f32 = PAGE_W - MARGIN;
const COURIER: &str = "Courier New";
const TAIL: &str = " cd cd cd cd cd cd cd cd cd end";

/// `words` words of letters then ` wxyz`, `len` characters in all, single spaces between them.
fn words(n: usize, len: usize) -> String {
    let letters = len - 4 - n;
    let mut s = String::new();
    for i in 0..n {
        let w = letters / n + usize::from(i < letters % n);
        if i > 0 {
            s.push(' ');
        }
        s.extend(std::iter::repeat_n('a', w));
    }
    s + " wxyz"
}

struct Para {
    text: String,
    align: Align,
    tabs: Vec<f32>,
    /// Left and first-line indents.
    indent: (f32, f32),
    font: &'static str,
    rtf: bool,
    page_w: f32,
}

impl Para {
    fn new(text: String) -> Self {
        Para { text, align: Align::Left, tabs: vec![36.0], indent: (0.0, 0.0), font: COURIER, rtf: true, page_w: PAGE_W }
    }
}

fn props(font: &str) -> CharProps {
    CharProps { font: Some(font.into()), size: Some(10.0), ..Default::default() }
}

/// Lays the paragraph out; returns it and its x on the page.
fn lay_para(p: &Para) -> (ParaLayout, f32) {
    let mut para = wordcraft_doc::Paragraph::with_text(&p.text, props(p.font));
    para.runs = vec![Run { len: p.text.len(), props: props(p.font) }];
    para.props.align = Some(p.align);
    para.props.indent_left = Some(p.indent.0);
    para.props.indent_first = Some(p.indent.1);
    para.props.space_after = Some(0.0);
    para.props.tabs = Some(p.tabs.iter().map(|&pos| TabStop { pos, align: TabAlign::Left, leader: TabLeader::None }).collect());
    let mut d = Document::new();
    d.last_section.page_w = p.page_w;
    d.last_section.margin_left = MARGIN;
    d.last_section.margin_right = MARGIN;
    d.settings.rtf_justification = p.rtf;
    d.body = vec![wordcraft_doc::para_block(para)];
    let l = lay(&d);
    l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, x, .. } = it { Some(((**para).clone(), *x)) } else { None }).unwrap()
}

/// The line `wxyz` is on, from 1.
fn wxyz_line(p: &Para) -> usize {
    let (pl, _) = lay_para(p);
    let at = p.text.find("wxyz").unwrap();
    pl.lines.iter().position(|l| at >= l.start && at < l.stop).unwrap() + 1
}

/// The text of the paragraph's lines, trailing spaces left out.
fn lines(p: &Para) -> Vec<String> {
    let (pl, _) = lay_para(p);
    pl.lines.iter().map(|l| p.text[l.start..l.stop].trim_end().to_string()).collect()
}

fn tabbed(label: &str, n: usize, len: usize) -> Para {
    Para::new(format!("{label}\t{}{TAIL}", words(n, len)))
}

/// `left`, a tab to the stop at 144 pt, two words and ` wxyz`, `len` characters after the tab.
fn at_144(left: &str, tabs: Vec<f32>, len: usize) -> Para {
    Para { tabs, ..Para::new(format!("{left}\t{}{TAIL}", words(2, len))) }
}

fn hanging(text: &str) -> Para {
    Para { tabs: vec![18.0, 46.8, 75.6], indent: (75.6, -28.8), ..Para::new(text.to_string()) }
}

#[test]
fn the_test_font_is_six_points_a_character() {
    crate::test_fonts();
    let (pl, _) = lay_para(&Para::new("abcd".into()));
    let w = pl.x_of(0, 4).unwrap() - pl.x_of(0, 0).unwrap();
    assert!((w - 24.0).abs() < 0.01, "four characters are {w} pt");
}

#[test]
fn a_tabbed_line_keeps_the_overrunning_word() {
    crate::test_fonts();
    let probes: Vec<(&str, Para, usize)> = vec![
        // 74 characters after the tab, 9 single spaces: 4.8 pt over, half the spaces 27 pt.
        ("A1", tabbed("A1", 9, 74), 1),
        ("B1", tabbed("B1", 9, 77), 1),
        ("C1", tabbed("C1", 9, 78), 2),
        ("C2", tabbed("C2", 12, 78), 1),
        ("D0", tabbed("D0", 1, 74), 2),
        ("D1", tabbed("D1", 2, 74), 1),
        ("E", at_144("D a b c d e f", vec![144.0], 56), 1),
        ("J1", Para { align: Align::Justify, ..tabbed("J1", 9, 77) }, 1),
        ("K1", Para { align: Align::Right, ..tabbed("K1", 9, 74) }, 1),
    ];
    for (name, p, want) in probes {
        assert_eq!(wxyz_line(&p), want, "{name}");
    }
}

#[test]
fn spaces_before_the_last_tab_do_not_count() {
    crate::test_fonts();
    // 10.8 pt over; the two spaces after the tab allow 6 pt (21 with the 5 before it).
    assert_eq!(wxyz_line(&at_144("D a b c d e f", vec![144.0], 57)), 2, "D");
    // The same with 3 spaces between two tabs (15 pt if they counted).
    assert_eq!(wxyz_line(&at_144("I\tab cd ef gh", vec![36.0, 144.0], 57)), 2, "I");
}

#[test]
fn the_squeeze_takes_only_the_spaces_after_the_tab() {
    crate::test_fonts();
    let after = words(9, 77);
    let short = after.trim_end_matches(" wxyz").to_string();
    let p = Para::new(format!("B1\t{after}{TAIL}"));
    let plain = Para::new(format!("B1\t{short}"));
    let (pl, x) = lay_para(&p);
    let (pl0, x0) = lay_para(&plain);
    assert_eq!(pl0.lines.len(), 1);
    let tab = p.text.find('\t').unwrap();
    for off in [0, tab + 1] {
        let (a, b) = (pl.x_of(0, off).unwrap() + x, pl0.x_of(0, off).unwrap() + x0);
        assert!((a - b).abs() < 0.01, "offset {off}: {a} squeezed, {b} plain");
    }
    let from = tab + 1;
    let wxyz = p.text.find("wxyz").unwrap();
    let spaces: Vec<f32> =
        (from..wxyz).filter(|&k| p.text.as_bytes()[k] == b' ').map(|k| pl.x_of(0, k + 1).unwrap() - pl.x_of(0, k).unwrap()).collect();
    assert_eq!(spaces.len(), 9);
    // 22.8 pt over 9 spaces: 3.47 pt each.
    for w in &spaces {
        assert!((w - spaces[0]).abs() < 0.05 && *w < 4.0, "spaces {spaces:?}");
    }
    let end = pl.x_of(0, wxyz + 4).unwrap() + x;
    assert!((end - EDGE).abs() < 0.5, "wxyz ends at {end}");
}

#[test]
fn a_hanging_numbered_paragraph_is_one_line() {
    crate::test_fonts();
    let h1 = hanging("8.\tAny drilling difficulties, and adjustments or corrections to the work.");
    assert_eq!(lines(&h1).len(), 1, "H1: {:?}", lines(&h1));
    let h2 = hanging("10.\tAmount of water disposed of on site and any difficulties related to disposal.");
    let l2 = lines(&h2);
    assert!(l2[0].ends_with(" to"), "H2: {l2:?}");
    let h3 = hanging("9.\tAny drilling difficulties, and adjustments or corrections to the works here.");
    let l3 = lines(&h3);
    assert!(l3[0].ends_with(" works"), "H3: {l3:?}");
}

#[test]
fn a_double_space_after_the_tab_does_not_count() {
    // D1 with its middle space doubled (one letter less, still 74 characters): only the space
    // before wxyz is single, and 4.8 pt is not less than half of it. This follows from the rule
    // that runs of spaces never count; not measured again with a tab.
    crate::test_fonts();
    let d1 = words(2, 74);
    let doubled = format!("{}  {}", &d1[..33], &d1[35..]);
    assert_eq!(d1.len(), 74);
    assert_eq!(doubled.len(), 74);
    assert_eq!(doubled.matches("  ").count(), 1);
    assert_eq!(wxyz_line(&Para::new(format!("D1\t{doubled}{TAIL}"))), 2);
}

#[test]
fn guards() {
    crate::test_fonts();
    assert_eq!(wxyz_line(&Para { rtf: false, ..tabbed("B1", 9, 77) }), 2, "not from RTF");
    // A proportional font: lay out B1 on a wide page to find where wxyz ends, then narrow the
    // page so that it overruns by 1 pt: still far less than half its spaces, but it wraps.
    let font = "Times New Roman";
    let wide = Para { font, page_w: 2000.0, ..tabbed("B1", 9, 77) };
    let (pl, x) = lay_para(&wide);
    let at = wide.text.find("wxyz").unwrap();
    assert_eq!(pl.lines.len(), 1);
    let natural = pl.x_of(0, at + 4).unwrap() + x;
    let narrow = Para { font, page_w: natural - 1.0 + MARGIN, ..tabbed("B1", 9, 77) };
    assert_eq!(wxyz_line(&narrow), 2, "proportional");
    // No tab (F1): 80 characters, 10 single spaces, 4.8 pt over: kept as before.
    let f1 = Para::new(format!("F1 {}{TAIL}", words(9, 77)));
    assert_eq!(f1.text.find("wxyz").unwrap() + 4, 80);
    assert_eq!(wxyz_line(&f1), 1, "F1");
}
