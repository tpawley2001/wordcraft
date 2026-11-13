//! A section break deleted with Track Changes (the paragraph mark carrying it is a tracked
//! deletion), laid out with deletions hidden (PDF export, print, no markup): Word lays the
//! paragraph out joined to the next one, the break is not applied, and its section's content
//! takes the following section's page size, margins, columns and headers. Expected values are
//! from Microsoft 365 Word's PDFs of small documents (US Letter, Times New Roman 12 pt, 72 pt top,
//! right and bottom margins): see each test. Sections A and B carry their own break; C is the
//! body's own (left margin 72 pt).
use crate::*;
use wordcraft_doc::props::{Align, LineSpacing};
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{PartKind, Pos, para_block};

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

/// A next-page section with left margin `left`.
fn sect(left: f32) -> SectionProps {
    SectionProps { margin_left: left, start: SectionStart::NextPage, ..SectionProps::default() }
}

/// `p` carrying section break `s`.
fn ends(mut p: Paragraph, s: SectionProps) -> Paragraph {
    p.section = Some(Box::new(s));
    p
}

fn doc(paras: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    d.body = paras.into_iter().map(para_block).collect();
    d.last_section = sect(72.0);
    d
}

/// Sections A (left 144, break deleted or not), B (left 36) and C, two paragraphs each in A and B.
fn three_sections(deleted: bool) -> Document {
    let a_last = ends(para("A last"), sect(144.0));
    let a_last = if deleted { gone(a_last) } else { a_last };
    doc(vec![para("A one"), a_last, para("B one"), ends(para("B last"), sect(36.0)), para("C one")])
}

fn final_layout(d: &Document) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { hide_deleted: true, ..Default::default() })
}

/// The text lines drawn on page `page`, top to bottom: the line's text without white space and
/// the x of its first glyph and its baseline.
fn lines(d: &Document, l: &DocLayout, page: usize) -> Vec<(String, f32, f32)> {
    let items = display::page_display(d, &l.pages[page], &display::DisplayOptions { markup: false, ..Default::default() });
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

/// Each line's text and the x of its first glyph, rounded to a point.
fn text_x(d: &Document, l: &DocLayout, page: usize) -> Vec<(String, i32)> {
    lines(d, l, page).into_iter().map(|(t, x, _)| (t, x.round() as i32)).collect()
}

fn owned(v: &[(&str, i32)]) -> Vec<(String, i32)> {
    v.iter().map(|(t, x)| ((*t).to_string(), *x)).collect()
}

// Word, row 1: 2 pages. Page 1: "A one", "A lastB one", "B last" at x = 36; page 2: "C one" at 72.
#[test]
fn a_deleted_section_break_takes_the_next_sections_margins() {
    let d = three_sections(true);
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(text_x(&d, &l, 0), owned(&[("Aone", 36), ("AlastBone", 36), ("Blast", 36)]));
    assert_eq!(text_x(&d, &l, 1), owned(&[("Cone", 72)]));
}

// Word, row 2 (control, no deletion): 3 pages at x = 144, 36 and 72.
#[test]
fn a_kept_section_break_keeps_its_section() {
    let d = three_sections(false);
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 3);
    assert_eq!(text_x(&d, &l, 0), owned(&[("Aone", 144), ("Alast", 144)]));
    assert_eq!(text_x(&d, &l, 1), owned(&[("Bone", 36), ("Blast", 36)]));
    assert_eq!(text_x(&d, &l, 2), owned(&[("Cone", 72)]));
}

// Word, row 3: A landscape, its break deleted; both pages are portrait 612 x 792.
#[test]
fn a_deleted_break_of_a_landscape_section_prints_portrait() {
    let a = SectionProps { page_w: 792.0, page_h: 612.0, landscape: true, ..sect(144.0) };
    let d = doc(vec![para("A one"), gone(ends(para("A last"), a)), para("B one"), ends(para("B last"), sect(36.0)), para("C one")]);
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 2);
    for p in &l.pages {
        assert_eq!((p.w, p.h), (612.0, 792.0));
    }
}

// Word, row 4: A in two columns, its break deleted, B one column (continuous): page 1 is one
// column from x = 36, every line starting there.
#[test]
fn a_deleted_break_of_a_two_column_section_lays_out_in_one_column() {
    let mut a = sect(144.0);
    a.columns.count = 2;
    let long = "Alpha words run on and on to fill the column with several lines of text here. ".repeat(4);
    let b = SectionProps { start: SectionStart::Continuous, ..sect(36.0) };
    let d = doc(vec![para(&long), gone(ends(para("A last"), a)), para("B one"), ends(para("B last"), b), para("C one")]);
    let l = final_layout(&d);
    let ls = lines(&d, &l, 0);
    assert!(ls.len() > 4, "{ls:?}");
    for (t, x, _) in &ls {
        assert!((x - 36.0).abs() < 0.5, "{t} at {x}");
    }
    assert!(ls.iter().any(|(t, ..)| t.starts_with("AlastBone")), "{ls:?}");
    assert!(l.pages[0].body.w <= 504.5, "body {:?}", l.pages[0].body);
    let c = l.pages.iter().position(|p| p.items.iter().any(|it| matches!(it, Placed::Lines { path, .. } if path.0 == [4])));
    assert_eq!(c, Some(1));
}

// Word, row 5: page 1 shows B's header ("HDR-B"), page 2 C's; A's header is printed nowhere.
#[test]
fn a_deleted_break_shows_the_next_sections_header() {
    let mut d = doc(Vec::new());
    let mut hdr = |t: &str| d.add_part(PartKind::Header, vec![para_block(para(t))]);
    let (ha, hb, hc) = (hdr("HDR-A"), hdr("HDR-B"), hdr("HDR-C"));
    let mut a = sect(144.0);
    a.headers.default = Some(ha);
    let mut b = sect(36.0);
    b.headers.default = Some(hb);
    d.last_section.headers.default = Some(hc);
    d.body =
        [para("A one"), gone(ends(para("A last"), a)), para("B one"), ends(para("B last"), b), para("C one")].into_iter().map(para_block).collect();
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 2);
    let shows = |p: &Page| {
        p.header_story.and_then(|id| d.story(StoryRef::Part(id))).and_then(|b| b.first()).and_then(|b| b.as_para()).map(|p| p.text.clone())
    };
    assert_eq!(shows(&l.pages[0]).as_deref(), Some("HDR-B"));
    assert_eq!(shows(&l.pages[1]).as_deref(), Some("HDR-C"));
    assert!(l.pages.iter().all(|p| p.header_story != Some(ha) && !p.header.is_empty()));
    // Each page points at its section in the document itself: B (1) and C (2).
    assert_eq!(l.pages.iter().map(|p| p.section).collect::<Vec<_>>(), [1, 2]);
}

// Word, row 6: both breaks deleted: 1 page, everything at C's x = 72.
#[test]
fn a_chain_of_deleted_breaks_merges_every_section() {
    let d = doc(vec![para("A one"), gone(ends(para("A last"), sect(144.0))), para("B one"), gone(ends(para("B last"), sect(36.0))), para("C one")]);
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 1);
    assert_eq!(text_x(&d, &l, 0), owned(&[("Aone", 72), ("AlastBone", 72), ("BlastCone", 72)]));
    assert_eq!(l.pages[0].section, 2);
}

// Word, row 7: A's break on an empty paragraph, B's on one wholly deleted, both marks deleted:
// 1 page, three lines one pitch apart (the break paragraphs take no room).
#[test]
fn deleted_breaks_on_empty_paragraphs_take_no_room() {
    let mut whole = Paragraph::with_text("Gone gone", CharProps { del: Some(0), ..chr() });
    whole.props.space_before = Some(0.0);
    whole.props.space_after = Some(0.0);
    whole.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    let d = doc(vec![para("A one"), gone(ends(para(""), sect(144.0))), para("B one"), gone(ends(whole, sect(36.0))), para("C one")]);
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 1);
    let ls = lines(&d, &l, 0);
    assert_eq!(ls.iter().map(|l| l.0.as_str()).collect::<Vec<_>>(), ["Aone", "Bone", "Cone"]);
    let (first, second) = (ls[1].2 - ls[0].2, ls[2].2 - ls[1].2);
    assert!((first - second).abs() < 0.05, "pitch {first} then {second}");
    assert!(ls.iter().all(|l| (l.1 - 72.0).abs() < 0.5), "{ls:?}");
}

// Word, row 8: "A last" centred, "B one" right-aligned, B's break continuous: the joined line is
// right-aligned, ending at the right margin (540 pt).
#[test]
fn the_joined_line_takes_the_alignment_of_the_chains_last_paragraph() {
    let mut a_last = para("A last");
    a_last.props.align = Some(Align::Center);
    let mut b_one = para("B one");
    b_one.props.align = Some(Align::Right);
    let b = SectionProps { start: SectionStart::Continuous, ..sect(36.0) };
    let d = doc(vec![para("A one"), gone(ends(a_last, sect(144.0))), b_one, ends(para("B last"), b), para("C one")]);
    let l = final_layout(&d);
    let end = l.caret(&Pos::body(2, 5)).unwrap();
    assert_eq!(end.page, 0);
    assert!((end.x - 540.0).abs() < 1.0, "the joined line ends at {}", end.x);
    let ls = text_x(&d, &l, 0);
    assert_eq!(ls.first(), Some(&("Aone".to_string(), 36)));
    assert_eq!(ls.last(), Some(&("Blast".to_string(), 36)));
    assert_eq!(l.pages.len(), 2);
}

// With deletions shown, nothing joins: row 1 lays out as 3 pages with A's margin.
#[test]
fn with_deletions_shown_the_deleted_break_stays() {
    crate::test_fonts();
    let d = three_sections(true);
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    assert_eq!(l.pages.len(), 3);
    assert_eq!(text_x(&d, &l, 0).first(), Some(&("Aone".to_string(), 144)));
    assert_eq!(l.pages.iter().map(|p| p.section).collect::<Vec<_>>(), [0, 1, 2]);
}

// A deleted mark carrying a break followed by a table doesn't join: the break stays.
#[test]
fn a_deleted_break_before_a_table_stays() {
    let mut d = doc(vec![para("A one"), gone(ends(para("A last"), sect(144.0)))]);
    d.body.push(std::sync::Arc::new(Block::Table(wordcraft_doc::Table::new(1, 1, 100.0))));
    d.body.push(para_block(para("C one")));
    let l = final_layout(&d);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(text_x(&d, &l, 0), owned(&[("Aone", 144), ("Alast", 144)]));
    assert_eq!(l.pages[1].section, 1);
}

// After the merge the caret and a click in "A last" address the real paragraph.
#[test]
fn the_caret_and_hits_in_the_merged_paragraph_address_the_real_one() {
    let d = three_sections(true);
    let l = final_layout(&d);
    for off in 0..=6 {
        assert!(l.caret(&Pos::body(1, off)).is_some(), "offset {off}");
    }
    let c = l.caret(&Pos::body(1, 2)).unwrap();
    assert!((c.x - 36.0).abs() > 1.0 && c.x < 100.0, "{c:?}");
    let hit = l.hit(c.page, c.x + 0.1, c.top + c.height / 2.0, StoryRef::Body).unwrap();
    assert_eq!(hit.path, Path::top(1));
}
