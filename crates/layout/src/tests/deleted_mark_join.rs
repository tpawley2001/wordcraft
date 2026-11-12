//! A paragraph whose mark is a tracked deletion, laid out with deletions hidden (PDF export,
//! print, no markup): Word lays it out as part of the next paragraph, which gives the joined
//! paragraph its alignment, indents, spacing and numbering. Expected values are from Microsoft
//! 365 Word's PDFs of seven small documents (Times New Roman 12 pt, 72 pt margins): see each
//! test. With deletions shown the paragraphs stay separate.
use crate::*;
use std::sync::Arc;
use wordcraft_doc::props::{Align, LineSpacing, NumRef};
use wordcraft_doc::{ListKind, Pos, para_block};

fn chr() -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, chr()) };
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

/// `p` with its paragraph mark a tracked deletion.
fn gone(mut p: Paragraph) -> Paragraph {
    p.mark.del = Some(0);
    p
}

fn numbered(mut p: Paragraph, num: u32) -> Paragraph {
    p.props.numbering = Some(NumRef { num, level: 0 });
    p
}

fn doc(paras: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    d.body = paras.into_iter().map(para_block).collect();
    d
}

/// The layout with deletions hidden, as PDF export and print use it.
fn final_layout(d: &Document) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { hide_deleted: true, ..Default::default() })
}

/// The text lines drawn on the first page, top to bottom: the line's text without white space
/// (a list label included), the x of its first glyph and its baseline.
fn lines(d: &Document, l: &DocLayout) -> Vec<(String, f32, f32)> {
    let items = display::page_display(d, &l.pages[0], &display::DisplayOptions { markup: false, ..Default::default() });
    let mut runs: Vec<(f32, f32, String)> = items
        .iter()
        .filter_map(|i| match i {
            display::Draw::Glyphs { glyphs, text, .. } => glyphs.first().map(|g| (g.2, g.1, text.clone())),
            _ => None,
        })
        .collect();
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut out: Vec<(String, f32, f32)> = Vec::new();
    for (base, x, text) in runs {
        let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if t.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if (last.2 - base).abs() < 0.5 => last.0.push_str(&t),
            _ => out.push((t, x, base)),
        }
    }
    out
}

fn texts(d: &Document, l: &DocLayout) -> Vec<String> {
    lines(d, l).into_iter().map(|l| l.0).collect()
}

/// The list labels on the first page, top to bottom.
fn labels(l: &DocLayout) -> Vec<String> {
    let mut v: Vec<(f32, String)> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, y, l0: 0, .. } => para.label.as_ref().map(|l| (*y, l.text.trim().to_owned())),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().map(|(_, t)| t).collect()
}

// Word: one line "Alpha alphaBeta beta" starting at x = 144 (the second paragraph's 72 pt indent),
// not centred; three lines in all.
#[test]
fn the_joined_paragraph_takes_the_next_paragraphs_indent_not_its_own_centring() {
    let mut a = para("Alpha alpha");
    a.props.align = Some(Align::Center);
    let mut b = para("Beta beta");
    b.props.indent_left = Some(72.0);
    let d = doc(vec![para("Top"), gone(a), b, para("End")]);
    let l = final_layout(&d);
    let ls = lines(&d, &l);
    assert_eq!(texts(&d, &l), ["Top", "AlphaalphaBetabeta", "End"]);
    assert!((ls[1].1 - 144.0).abs() < 0.5, "joined line starts at {}", ls[1].1);
}

// Word: one line "AlphaBeta", right-aligned (it ends at the right margin, x = 540), and "End" one
// normal line pitch below it: the first paragraph's 24 pt space after is not applied.
#[test]
fn the_joined_paragraph_takes_the_next_paragraphs_alignment_and_spacing() {
    let mut a = para("Alpha");
    a.props.space_after = Some(24.0);
    let mut b = para("Beta");
    b.props.align = Some(Align::Right);
    let d = doc(vec![para("Top"), gone(a), b, para("End")]);
    let l = final_layout(&d);
    let ls = lines(&d, &l);
    assert_eq!(texts(&d, &l), ["Top", "AlphaBeta", "End"]);
    // "AlphaBeta" in 12 pt Times New Roman is about 51 pt wide: right-aligned it starts near 489.
    assert!(ls[1].1 > 480.0 && ls[1].1 < 495.0, "joined line starts at {}", ls[1].1);
    let (above, below) = (ls[1].2 - ls[0].2, ls[2].2 - ls[1].2);
    assert!((above - below).abs() < 0.05, "pitch above {above}, below {below}");
}

// Word: "OneTwo" at the left margin with no number (the second paragraph is not in the list),
// then "1. Three" and "2. Four": the joined-away list item takes no number.
#[test]
fn a_list_item_joined_to_a_plain_paragraph_has_no_number_and_takes_none() {
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(ListKind::Numbered);
    let body = vec![para("Intro"), gone(numbered(para("One"), num)), para("Two"), numbered(para("Three"), num), numbered(para("Four"), num)];
    d.body = body.into_iter().map(para_block).collect();
    let l = final_layout(&d);
    let ls = lines(&d, &l);
    assert_eq!(labels(&l), ["1.", "2."]);
    assert_eq!(ls.len(), 4, "{ls:?}");
    assert_eq!(ls[1].0, "OneTwo");
    assert!((ls[1].1 - 72.0).abs() < 0.5, "joined line starts at {}", ls[1].1);
    assert!(ls[2].0.ends_with("Three") && ls[3].0.ends_with("Four"), "{ls:?}");
}

// Word: "1. OneTwo", "2. Three", "3. Four": the joined paragraph is numbered once (as the second
// list item), and the empty list item whose mark is deleted takes no number.
#[test]
fn list_items_joined_together_are_numbered_once() {
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(ListKind::Numbered);
    let body = vec![
        para("Intro"),
        gone(numbered(para("One"), num)),
        numbered(para("Two"), num),
        numbered(para("Three"), num),
        gone(numbered(para(""), num)),
        numbered(para("Four"), num),
    ];
    d.body = body.into_iter().map(para_block).collect();
    let l = final_layout(&d);
    let ls = lines(&d, &l);
    assert_eq!(labels(&l), ["1.", "2.", "3."]);
    assert_eq!(ls.len(), 4, "{ls:?}");
    assert!(ls[1].0.ends_with("OneTwo"), "{ls:?}");
    assert!(ls[3].0.ends_with("Four"), "{ls:?}");
}

// Word: one line "ABC"; three lines in all.
#[test]
fn a_chain_of_deleted_marks_joins_every_paragraph_in_it() {
    let d = doc(vec![para("Top"), gone(para("A")), gone(para("B")), para("C"), para("End")]);
    let l = final_layout(&d);
    assert_eq!(texts(&d, &l), ["Top", "ABC", "End"]);
}

// Word: X and Y on adjacent lines, for an empty paragraph and for one whose whole text is a
// tracked deletion (both already so before this change).
#[test]
fn an_empty_or_wholly_deleted_paragraph_with_a_deleted_mark_takes_no_line() {
    let mut whole = Paragraph::with_text("Gone gone", CharProps { del: Some(0), ..chr() });
    whole.props.space_before = Some(0.0);
    whole.props.space_after = Some(0.0);
    whole.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    for middle in [gone(para("")), gone(whole)] {
        let d = doc(vec![para("X"), middle, para("Y"), para("Z")]);
        let l = final_layout(&d);
        let ls = lines(&d, &l);
        assert_eq!(texts(&d, &l), ["X", "Y", "Z"]);
        let (first, second) = (ls[1].2 - ls[0].2, ls[2].2 - ls[1].2);
        assert!((first - second).abs() < 0.05, "pitch {first} then {second}");
    }
}

// With deletions shown (the default layout) the paragraphs stay separate.
#[test]
fn with_deletions_shown_the_paragraphs_stay_separate() {
    crate::test_fonts();
    let d = doc(vec![para("Top"), gone(para("A")), gone(para("B")), para("C"), para("End")]);
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    assert_eq!(texts(&d, &l), ["Top", "A", "B", "C", "End"]);
}

// The join is layout only: every offset of every source paragraph still has a caret, the three
// joined paragraphs' starts sit on one line from left to right, and a click lands in the body.
#[test]
fn every_source_paragraph_keeps_its_carets_on_the_joined_line() {
    let d = doc(vec![para("Top"), gone(para("A")), gone(para("B")), para("C"), para("End")]);
    let l = final_layout(&d);
    for (i, len) in [(0, 3), (1, 1), (2, 1), (3, 1), (4, 3)] {
        for off in 0..=len {
            assert!(l.caret(&Pos::body(i, off)).is_some(), "paragraph {i} offset {off}");
        }
    }
    let at = |i| l.caret(&Pos::body(i, 0)).unwrap();
    let (a, b, c) = (at(1), at(2), at(3));
    assert!((a.top - b.top).abs() < 0.01 && (b.top - c.top).abs() < 0.01, "{a:?} {b:?} {c:?}");
    assert!(a.x < b.x && b.x < c.x, "{a:?} {b:?} {c:?}");
    let hit = l.hit(c.page, c.x + 0.1, c.top + c.height / 2.0, StoryRef::Body);
    assert!(hit.is_some());
}

// Hostile input: 10,000 deleted marks in a row. The chain is capped, so no joined paragraph is
// unbounded, and the layout stays small (unjoined it is over 200 pages of one-letter lines).
#[test]
fn ten_thousand_deleted_marks_in_a_row_lay_out_in_bounded_paragraphs() {
    let mut paras: Vec<Paragraph> = (0..10_000).map(|_| gone(para("a"))).collect();
    paras.push(para("z"));
    let d = doc(paras);
    let l = final_layout(&d);
    assert!(l.pages.len() < 20, "{} pages", l.pages.len());
    let longest = l
        .pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|it| if let Placed::Lines { para, .. } = it { Some(para.lines.last().map_or(0, |l| l.c1)) } else { None })
        .max()
        .unwrap_or(0);
    assert!(longest <= 4096, "a joined paragraph of {longest} characters");
}
