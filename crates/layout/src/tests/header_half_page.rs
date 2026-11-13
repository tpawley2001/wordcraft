//! A header is at most half the page tall: the line or table row whose bottom lies more than half
//! the page height below the top of the header's content is not printed, nor is anything after
//! it, and the body starts right below what is kept (or at the top margin when that ends above
//! it). Measured in Microsoft 365 Word with RTF probes on Letter, Letter landscape and A4 pages.
//!
//! Every header line here is exactly `PITCH` tall (exact line spacing, nothing before or after),
//! so the counts follow from the page height. Synthetic documents built in code.

use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps, TableProps};
use wordcraft_doc::section::SectionProps;
use wordcraft_doc::table::{Cell, Row, Table};
use wordcraft_doc::{PartKind, para_block};

const PITCH: f32 = 13.8;
const EPS: f32 = 0.05;

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn line(text: &str) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Exactly(PITCH)), ..Default::default() };
    para_block(p)
}

/// `n` one-line paragraphs H1..Hn.
fn paras(n: usize) -> Vec<Arc<Block>> {
    (1..=n).map(|i| line(&format!("H{i}"))).collect()
}

fn cell(blocks: Vec<Arc<Block>>) -> Cell {
    let mut c = Cell::empty();
    c.props.width = Some(468.0);
    c.blocks = blocks;
    c
}

/// A one-column borderless table, one row per entry of `rows` (the paragraphs in it).
fn table(rows: Vec<Vec<Arc<Block>>>) -> Arc<Block> {
    let rows = rows.into_iter().map(|b| Row { cells: vec![cell(b)], ..Default::default() }).collect();
    let t = Table { props: TableProps { cell_margins: Some([0.0; 4]), ..Default::default() }, grid: vec![468.0], rows, controls: Default::default() };
    Arc::new(Block::Table(t))
}

/// A document with a default header holding `header` on page setup `sect`.
fn doc(header: Vec<Arc<Block>>, mut sect: SectionProps) -> Document {
    let mut d = Document::from_text("Body text");
    sect.headers.default = Some(d.add_part(PartKind::Header, header));
    d.last_section = sect;
    d
}

/// The first page's header lines, in order (top-level paragraphs and paragraphs in cells).
fn header_lines(l: &DocLayout) -> Vec<(f32, f32)> {
    let page = l.pages.first().expect("a page");
    let mut v = Vec::new();
    for it in &page.header {
        if let Placed::Lines { story: StoryRef::Part(_), para, l0, l1, y, .. } = it {
            let first = para.lines.get(*l0).expect("first line");
            for ln in para.lines.get(*l0..*l1).expect("lines") {
                v.push((y + ln.top - first.top, ln.height));
            }
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn body_top(l: &DocLayout) -> f32 {
    l.pages.first().expect("a page").body.y
}

/// The number of whole lines that fit in half of `page_h`.
fn fit(page_h: f32) -> usize {
    ((page_h / 2.0 + 0.01) / PITCH).floor() as usize
}

/// Checks that exactly `n` header lines are printed and that the body starts at the bottom of the
/// last one (`margin_top` at least).
fn check(l: &DocLayout, sect: &SectionProps, n: usize) {
    let lines = header_lines(l);
    assert_eq!(lines.len(), n, "printed header lines: {lines:?}");
    let last = lines.last().expect("a header line");
    let bottom = last.0 + last.1;
    assert!((bottom - (sect.header + n as f32 * PITCH)).abs() < EPS, "header bottom {bottom}");
    assert!(bottom - sect.header <= sect.page_h / 2.0 + EPS, "header reaches {bottom}");
    let want = bottom.max(sect.margin_top);
    assert!((body_top(l) - want).abs() < EPS, "body top {} want {want}", body_top(l));
}

/// Row 1: 60 one-line paragraphs on a Letter page: H1..H28 print, the body starts below H28.
#[test]
fn letter_header_of_60_lines_stops_at_half_the_page() {
    let sect = SectionProps::default();
    let l = lay(&doc(paras(60), sect.clone()));
    assert_eq!(fit(sect.page_h), 28);
    check(&l, &sect, 28);
}

/// Row 2: a table of 25 one-line rows fits (345pt): it prints whole, as before.
#[test]
fn table_within_half_the_page_prints_whole() {
    let sect = SectionProps::default();
    let l = lay(&doc(vec![table((1..=25).map(|i| vec![line(&format!("H{i}"))]).collect())], sect.clone()));
    check(&l, &sect, 25);
}

/// Row 3: tables of 35, 45 and 55 one-line rows print rows H1..H28.
#[test]
fn table_rows_past_half_the_page_are_dropped() {
    for rows in [35, 45, 55] {
        let sect = SectionProps::default();
        let l = lay(&doc(vec![table((1..=rows).map(|i| vec![line(&format!("H{i}"))]).collect())], sect.clone()));
        check(&l, &sect, 28);
        let cells = l.pages.first().unwrap().header.iter().filter(|it| matches!(it, Placed::Cell { .. })).count();
        assert_eq!(cells, 28, "{rows} rows: cells drawn");
    }
}

/// Rows 4, 5 and 8: the header distance and the margins don't move the limit.
#[test]
fn limit_ignores_header_distance_and_margins() {
    let base = SectionProps::default();
    let cases = [
        SectionProps { header: 18.0, ..base.clone() },
        SectionProps { header: 72.0, ..base.clone() },
        SectionProps { margin_bottom: 144.0, ..base.clone() },
        SectionProps { margin_bottom: 216.0, ..base.clone() },
        SectionProps { margin_top: 144.0, ..base.clone() },
    ];
    for sect in cases {
        let l = lay(&doc(paras(60), sect.clone()));
        check(&l, &sect, 28);
    }
}

/// Row 6: a landscape Letter page (612pt tall) prints H1..H22.
#[test]
fn landscape_letter_limit_is_half_its_height() {
    let base = SectionProps::default();
    let sect = SectionProps { page_w: 792.0, page_h: 612.0, landscape: true, ..base };
    let l = lay(&doc(paras(60), sect.clone()));
    assert_eq!(fit(sect.page_h), 22);
    check(&l, &sect, 22);
}

/// Row 7: an A4 page prints H1..H30.
#[test]
fn a4_limit_is_half_its_height() {
    let base = SectionProps::default();
    let sect = SectionProps { page_w: 595.3, page_h: 841.9, ..base };
    let l = lay(&doc(paras(60), sect.clone()));
    assert_eq!(fit(sect.page_h), 30);
    check(&l, &sect, 30);
}

/// Row 9: two short rows, then one row of 40 paragraphs (552pt): that row is dropped whole and
/// the body starts at the top margin, the kept rows ending above it.
#[test]
fn a_row_crossing_the_limit_is_dropped_whole() {
    let sect = SectionProps::default();
    let l = lay(&doc(vec![table(vec![vec![line("R1")], vec![line("R2")], paras(40)])], sect.clone()));
    let lines = header_lines(&l);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!((body_top(&l) - sect.margin_top).abs() < EPS, "body top {}", body_top(&l));
}

/// A header within the limit lays out exactly as before: every line, the body below it.
#[test]
fn header_within_the_limit_is_unchanged() {
    let sect = SectionProps::default();
    let l = lay(&doc(paras(20), sect.clone()));
    check(&l, &sect, 20);
    // The same header on a page twice as tall (no limit in reach) is placed identically.
    let tall = SectionProps { page_h: 1584.0, ..sect.clone() };
    let t = lay(&doc(paras(20), tall));
    assert_eq!(header_lines(&l), header_lines(&t));
    assert_eq!(body_top(&l), body_top(&t));
}

/// The lines left out aren't drawn, and the body on the page gets the room.
#[test]
fn dropped_lines_are_not_drawn() {
    let sect = SectionProps::default();
    let d = doc(paras(60), sect);
    let l = lay(&d);
    let page = l.pages.first().unwrap();
    let id = page.header_story.unwrap();
    let mut shown = Vec::new();
    for it in &page.header {
        if let Placed::Lines { story: StoryRef::Part(p), path, l0, l1, .. } = it
            && *p == id
            && l1 > l0
        {
            shown.push(path.0.first().copied().unwrap());
        }
    }
    assert_eq!(shown, (0..28).collect::<Vec<u32>>());
    assert_eq!(l.pages.len(), 1);
}

/// The header's first line always prints, even when it alone is taller than half the page, so a
/// header is never left out altogether; the lines after it are dropped.
#[test]
fn the_first_line_always_prints() {
    let sect = SectionProps::default();
    let mut tall = Paragraph::with_text("Tall", CharProps::default());
    tall.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Exactly(450.0)), ..Default::default() };
    let mut blocks = vec![para_block(tall)];
    blocks.extend(paras(3));
    let l = lay(&doc(blocks, sect.clone()));
    let lines = header_lines(&l);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!((body_top(&l) - (sect.header + 450.0)).abs() < EPS, "body top {}", body_top(&l));
}
