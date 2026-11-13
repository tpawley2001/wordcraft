//! The extra leading of a page's last line may hang past the top of a footer taller than the
//! bottom margin, as Word lays it out. Line spacing Multiple above 1 (ECMA-376 Part 1
//! §17.3.1.33, `w:lineRule="auto"`) puts its extra leading below the text: a page's last line
//! stays when its text ends at or above the page's bottom, whether that is the bottom margin or
//! the top of a footer that pushed the body up (measured in Word at 1.15 and double spacing). The
//! top border of the footer's first paragraph, with its space, belongs to the footer. Footnotes,
//! balanced columns, and single, Exact and At least spacing keep the whole line clear.
//! Synthetic documents with the test fonts: the expected lines come from the laid-out metrics.

use super::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing};
use wordcraft_doc::section::{Columns, SectionStart};
use wordcraft_doc::{PartKind, Pos, para_block};

const TEXT: &str = "Several words wrap onto many lines of a long paragraph. ";

fn lay(doc: &Document) -> DocLayout {
    layout(doc, &mut LayoutCache::new(), &LayoutOptions::default())
}

fn footer(text: &str, n: usize) -> Vec<Arc<Block>> {
    (1..=n).map(|i| para_block(Paragraph::with_text(&format!("{text} {i}"), Default::default()))).collect()
}

/// One long wrapping paragraph (widow control off) with `spacing` and `before` points above it,
/// under an eight-line default footer (returned with the document).
fn doc(spacing: LineSpacing, before: f32) -> (Document, u32) {
    let mut d = Document::from_text(&TEXT.repeat(90));
    if let Some(Block::Para(p)) = d.body.get_mut(0).map(Arc::make_mut) {
        p.props.line_spacing = Some(spacing);
        p.props.space_before = Some(before);
        p.props.widow_control = Some(false);
    }
    let id = d.add_part(PartKind::Footer, footer("Footer line", 8));
    d.last_section.footers.default = Some(id);
    (d, id)
}

/// The top of `story` on page `page`: its first line or the top edge of the rule drawn above it.
fn footer_top(l: &DocLayout, page: usize, story: u32) -> f32 {
    let p = &l.pages[page];
    let lines = p
        .footer
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Part(s), y, .. } if *s == story => Some(*y),
            _ => None,
        })
        .fold(f32::MAX, f32::min);
    let rules = p.footer.iter().filter_map(|it| match it {
        // A rule is drawn centred on its position: its top is half its width higher.
        Placed::Rule { y0, y1, border, .. } => Some(y0.min(*y1) - border.width / 2.0),
        _ => None,
    });
    rules.fold(lines, f32::min)
}

/// The body's last line on page `page`: (its top, its text end, its box end, the text end of the
/// next line were it placed on the page too).
fn last_line(l: &DocLayout, page: usize) -> (f32, f32, f32, f32) {
    let Some(Placed::Lines { y, para, l0, l1, .. }) =
        l.pages[page].items.iter().rev().find(|i| matches!(i, Placed::Lines { story: StoryRef::Body, .. }))
    else {
        panic!("no body lines on page {page}")
    };
    let (first, last, next) = (&para.lines[*l0], &para.lines[*l1 - 1], &para.lines[*l1]);
    let top = y + last.top - first.top;
    let text_end = top + (last.baseline - last.top) + last.text_desc;
    let next_text_end = top + last.height + (next.baseline - next.top) + next.text_desc;
    (top, text_end, top + last.height, next_text_end)
}

/// Rule 1 on page `page` against `bottom`: the last line's text fits, the next one's would not.
/// Returns whether the last line's leading hangs past `bottom`.
fn text_rule(l: &DocLayout, page: usize, bottom: f32, what: &str) -> bool {
    let (_, text_end, box_end, next_text_end) = last_line(l, page);
    assert!(text_end <= bottom + 0.01, "{what}: page {page}: the last line's text ends at {text_end}, below {bottom}");
    assert!(next_text_end > bottom + 0.01, "{what}: page {page}: the next line's text would end at {next_text_end}, above {bottom}");
    box_end > bottom + 0.01
}

#[test]
fn the_last_lines_leading_hangs_past_a_tall_footer() {
    crate::test_fonts();
    for m in [1.15, 1.5, 2.0] {
        let mut hangs = 0;
        for before in (0..28).step_by(2) {
            let (d, id) = doc(LineSpacing::Multiple(m), before as f32);
            let l = lay(&d);
            assert!(l.pages.len() >= 2, "{m} {before}");
            let t = footer_top(&l, 0, id);
            assert!(t < d.last_section.page_h - d.last_section.margin_bottom - 1.0, "{m} {before}: the footer is taller than the margin: {t}");
            hangs += usize::from(text_rule(&l, 0, t, &format!("{m} before {before}")));
        }
        assert!(hangs > 0, "{m}: some shift should leave the last line's leading past the footer's top");
    }
}

#[test]
fn single_exact_and_at_least_lines_stay_clear_of_a_tall_footer() {
    crate::test_fonts();
    for spacing in [LineSpacing::Multiple(1.0), LineSpacing::Exactly(15.0), LineSpacing::AtLeast(14.0)] {
        for before in (0..28).step_by(4) {
            let (d, id) = doc(spacing, before as f32);
            let l = lay(&d);
            let t = footer_top(&l, 0, id);
            let (_, _, box_end, _) = last_line(&l, 0);
            assert!(box_end <= t + 0.01, "{spacing:?} before {before}: the line box ends at {box_end}, under the footer at {t}");
        }
    }
}

#[test]
fn a_footnote_keeps_the_whole_last_line_clear_of_a_tall_footer() {
    crate::test_fonts();
    for m in [1.15, 1.5, 2.0] {
        for before in (0..28).step_by(2) {
            let (mut d, id) = doc(LineSpacing::Multiple(m), before as f32);
            let note = d.add_part(PartKind::Footnote, footer("A note", 1));
            d.insert_object(
                &Pos::body(0, 4),
                InlineObject::NoteRef { kind: NoteKind::Footnote, id: note, custom: String::new() },
                &Default::default(),
            )
            .unwrap();
            let l = lay(&d);
            let t = footer_top(&l, 0, id);
            let (_, _, box_end, _) = last_line(&l, 0);
            let (note_top, note_bottom) = l.pages[0]
                .items
                .iter()
                .filter_map(|it| match it {
                    Placed::Lines { story: StoryRef::Part(s), y, para, l0, l1, .. } if *s == note => Some((*y, item_bottom(*y, para, *l0, *l1)?)),
                    _ => None,
                })
                .next()
                .expect("the note is on page 1");
            assert!(box_end <= note_top + 0.01, "{m} before {before}: the line box ends at {box_end}, the note starts at {note_top}");
            assert!(note_bottom <= t + 0.01, "{m} before {before}: the note ends at {note_bottom}, under the footer at {t}");
        }
    }
}

#[test]
fn the_footers_top_border_and_its_space_belong_to_the_footer() {
    crate::test_fonts();
    let mut hangs = 0;
    for before in (0..28).step_by(2) {
        let (plain, id) = doc(LineSpacing::Multiple(1.15), before as f32);
        let mut bordered = plain.clone();
        if let Some(Block::Para(p)) = bordered.parts.get_mut(&id).and_then(|part| part.blocks.get_mut(0)).map(Arc::make_mut) {
            p.props.borders =
                Some(Borders { top: Some(Border { style: BorderStyle::Single, width: 0.5, color: None, space: 1.0 }), ..Default::default() });
        }
        let (lp, lb) = (lay(&plain), lay(&bordered));
        let (tp, tb) = (footer_top(&lp, 0, id), footer_top(&lb, 0, id));
        assert!((tp - tb - 1.5).abs() <= 0.05, "before {before}: the border should raise the footer's top 1.5pt: {tp} -> {tb}");
        hangs += usize::from(text_rule(&lb, 0, tb, &format!("bordered, before {before}")));
    }
    assert!(hangs > 0, "some shift should leave the last line's leading past the bordered footer's top");
}

#[test]
fn a_footer_within_the_margin_lets_the_leading_hang_into_the_margin() {
    crate::test_fonts();
    let (mut d, id) = doc(LineSpacing::Multiple(2.0), 0.0);
    d.parts.insert(id, wordcraft_doc::Part { kind: PartKind::Footer, blocks: footer("Footer", 1), body: Default::default() });
    let mut hangs = 0;
    for before in (0..28).step_by(2) {
        if let Some(Block::Para(p)) = d.body.get_mut(0).map(Arc::make_mut) {
            p.props.space_before = Some(before as f32);
        }
        let l = lay(&d);
        let bottom = d.last_section.page_h - d.last_section.margin_bottom;
        assert!(footer_top(&l, 0, id) > bottom, "the footer fits in the margin");
        hangs += usize::from(text_rule(&l, 0, bottom, &format!("margin, before {before}")));
    }
    assert!(hangs > 0, "some shift should leave the last line's leading in the margin");
}

#[test]
fn each_page_follows_its_own_footer() {
    crate::test_fonts();
    for tall_first in [true, false] {
        let mut hangs = [0, 0];
        for before in (0..28).step_by(2) {
            let (mut d, tall) = doc(LineSpacing::Multiple(1.5), before as f32);
            let short = d.add_part(PartKind::Footer, footer("Footer", 1));
            d.last_section.title_page = true;
            let (first, rest) = if tall_first { (tall, short) } else { (short, tall) };
            d.last_section.footers.first = Some(first);
            d.last_section.footers.default = Some(rest);
            let l = lay(&d);
            assert!(l.pages.len() >= 3, "{tall_first} {before}");
            let margin = d.last_section.page_h - d.last_section.margin_bottom;
            for (page, story) in [(0, first), (1, rest)] {
                let bottom = footer_top(&l, page, story).min(margin);
                if story == tall {
                    assert!(bottom < margin - 1.0, "page {page}: the tall footer reaches above the margin");
                }
                hangs[page] += usize::from(text_rule(&l, page, bottom, &format!("tall first {tall_first}, before {before}")));
            }
        }
        // Only page 1's text moves with the space before.
        assert!(hangs[0] > 0, "tall first {tall_first}: {hangs:?}");
    }
}

#[test]
fn balanced_columns_keep_the_whole_last_line() {
    crate::test_fonts();
    for words in [5, 7, 9] {
        let mut d = Document::from_text(&format!("{}\nAfter the columns.", TEXT.repeat(words)));
        if let Some(Block::Para(p)) = d.body.get_mut(0).map(Arc::make_mut) {
            p.props.line_spacing = Some(LineSpacing::Multiple(2.0));
            p.props.widow_control = Some(false);
        }
        let id = d.add_part(PartKind::Footer, footer("Footer line", 8));
        let cols = SectionProps {
            start: SectionStart::Continuous,
            columns: Columns { count: 2, space: 36.0, ..Columns::default() },
            footers: wordcraft_doc::section::HeaderSet { default: Some(id), ..Default::default() },
            ..d.last_section.clone()
        };
        d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().section = Some(Box::new(cols));
        d.last_section.start = SectionStart::Continuous;
        d.last_section.footers.default = Some(id);
        let l = lay(&d);
        let page = &l.pages[0];
        let boxes: Vec<f32> = page
            .items
            .iter()
            .filter_map(|it| match it {
                Placed::Lines { story: StoryRef::Body, path, y, para, l0, l1, .. } if path.0.first() == Some(&0) => item_bottom(*y, para, *l0, *l1),
                _ => None,
            })
            .collect();
        assert_eq!(boxes.len(), 2, "{words}: the paragraph is balanced over two columns on page 1");
        let after = page
            .items
            .iter()
            .find_map(|it| match it {
                Placed::Lines { story: StoryRef::Body, path, y, .. } if path.0.first() == Some(&1) => Some(*y),
                _ => None,
            })
            .expect("the next section follows on page 1");
        for b in &boxes {
            assert!(*b <= after + 0.01, "{words}: a column's last line box ends at {b}, below the balanced bottom at {after}");
        }
        assert!(after < footer_top(&l, 0, id), "{words}: the section ends mid-page");
    }
}
