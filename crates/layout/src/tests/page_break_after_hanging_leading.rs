//! A paragraph that opens with a page break stays at the bottom of a full page: its break line
//! needs no room and the text after the break starts the next page. When the page's last line
//! hangs its line-spacing leading past the bottom margin (Multiple spacing above 1 puts the extra
//! leading below the text, and only the text has to fit), the page is over-full: Word starts the
//! paragraph on the next page, and its break then leaves that page blank. Measured in Word on
//! synthetic documents (Letter, 72 pt margins, one-line paragraphs at exactly 12 pt, a 1.3-spaced
//! line, then `[page break][page break]BODY` or `[page break]BODY`).

use crate::{Document, LayoutCache, LayoutOptions, Placed, StoryRef, layout};
use wordcraft_doc::Paragraph;
use wordcraft_doc::props::LineSpacing;

const BREAK: &str = "\u{000C}";

fn chr() -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(11.0), ..Default::default() }
}

fn para(text: &str, spacing: LineSpacing) -> Paragraph {
    let mut p = Paragraph::with_text(text, chr());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(spacing);
    p
}

/// A one-line paragraph exactly `h` points tall.
fn exact(text: &str, h: f32) -> Paragraph {
    para(text, LineSpacing::Exactly(h))
}

/// The 1.3-spaced paragraph.
fn spaced() -> Paragraph {
    para("Spaced", LineSpacing::Multiple(1.3))
}

fn doc(paras: Vec<Paragraph>, mode: u32) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let s = &mut d.last_section;
    (s.page_w, s.page_h, s.margin_top, s.margin_bottom) = (612.0, 792.0, 72.0, 72.0);
    d.body = paras.into_iter().map(wordcraft_doc::para_block).collect();
    d
}

/// The spaced paragraph's line as laid out: (where its text ends, its height), from its top.
fn spaced_metrics() -> (f32, f32) {
    crate::test_fonts();
    let l = layout(&doc(vec![spaced()], 12), &mut LayoutCache::new(), &LayoutOptions::default());
    let line = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, .. } => para.lines.first().cloned(),
            _ => None,
        })
        .expect("the spaced line");
    ((line.baseline - line.top) + line.text_desc, line.height)
}

/// `n` exact 12 pt filler paragraphs.
fn filler(n: usize) -> Vec<Paragraph> {
    (0..n).map(|i| exact(&format!("Line {i}"), 12.0)).collect()
}

/// The page count, and the page holding the last line of each block.
fn pages(paras: Vec<Paragraph>, mode: u32) -> (usize, Vec<Option<usize>>) {
    crate::test_fonts();
    let blocks = paras.len();
    let l = layout(&doc(paras, mode), &mut LayoutCache::new(), &LayoutOptions::default());
    let mut on = vec![None; blocks];
    for (pi, page) in l.pages.iter().enumerate() {
        for it in &page.items {
            if let Placed::Lines { story: StoryRef::Body, path, para, l1, .. } = it
                && *l1 == para.lines.len()
                && let Some(b) = path.0.first().and_then(|b| on.get_mut(*b as usize))
            {
                *b = Some(pi);
            }
        }
    }
    (l.pages.len(), on)
}

fn br(n: usize) -> Paragraph {
    exact(&format!("{}BODY", BREAK.repeat(n)), 12.0)
}

/// Checks the page count and the page (1-based) that holds BODY, the last block.
fn check(mut paras: Vec<Paragraph>, breaks: usize, mode: u32, want: (usize, usize), what: &str) -> Vec<Option<usize>> {
    paras.push(br(breaks));
    let (n, on) = pages(paras, mode);
    assert_eq!(n, want.0, "{what}: page count");
    assert_eq!(on.last().copied().flatten().map(|p| p + 1), Some(want.1), "{what}: BODY's page");
    on
}

#[test]
fn a_full_page_keeps_the_break() {
    check(filler(54), 2, 12, (3, 3), "row 1");
    check(filler(54), 1, 12, (2, 2), "row 3");
    let mut paras = filler(53);
    paras.push(exact("", 8.7));
    check(paras, 2, 12, (3, 3), "row 5");
}

#[test]
fn a_spaced_last_line_whose_leading_fits_keeps_the_break() {
    let (_, height) = spaced_metrics();
    let mut paras = filler(52);
    paras.push(spaced());
    check(paras, 2, 12, (3, 3), "row 7");
    // Room left under the spaced line, less than a line.
    let gap = 648.0 - 52.0 * 12.0 - height - 3.56;
    assert!(gap > 0.0, "the spaced line is {height} tall");
    let mut paras = filler(52);
    paras.push(exact("", gap));
    paras.push(spaced());
    check(paras, 2, 12, (3, 3), "row 8");
}

#[test]
fn a_spaced_last_line_hanging_its_leading_moves_the_break_on() {
    let (text, height) = spaced_metrics();
    assert!(height - text > 1.0, "the spaced line has leading under its text: {text} of {height}");
    // The spaced line's text ends above the bottom margin, its leading past it.
    let gap = 648.0 - 52.0 * 12.0 - (text + height) / 2.0;
    for (breaks, want) in [(2, (4, 4)), (1, (3, 3))] {
        let mut paras = filler(52);
        paras.push(exact("", gap));
        paras.push(spaced());
        let on = check(paras, breaks, 12, want, &format!("row 9, {breaks} breaks"));
        assert_eq!(on.get(53).copied().flatten(), Some(0), "the spaced line stays on page 1");
    }
}
