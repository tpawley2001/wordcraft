//! `w:doNotUseHTMLParagraphAutoSpacing` (`Settings::sum_paragraph_spacing`) turns automatic
//! paragraph spacing off altogether: `w:beforeAutospacing` / `w:afterAutospacing` are ignored and
//! the paragraph takes its stored space before / after like any other, with none of the HTML rules
//! (no 14 pt, nothing dropped at the top of the document or at a cell's edges, spaces added).
//! Every expected gap was measured in Microsoft 365 Word's PDFs of probe documents in
//! compatibility modes 11, 12 and 15 (the mode changes none of them). Without the setting the
//! automatic rules stay as they are.

use crate::*;
use wordcraft_doc::{Block, Path, StoryRef, Table};

/// One paragraph's stored space before / after (points) and whether each is marked automatic.
#[derive(Clone, Copy)]
struct Sp {
    before: f32,
    after: f32,
    auto_before: bool,
    auto_after: bool,
}

const fn auto(before: f32, after: f32) -> Sp {
    Sp { before, after, auto_before: true, auto_after: true }
}

const fn plain(before: f32, after: f32) -> Sp {
    Sp { before, after, auto_before: false, auto_after: false }
}

/// One paragraph per entry; `no_html` is the compatibility setting.
fn doc(paras: &[Sp], no_html: bool) -> Document {
    let text = (0..paras.len()).map(|i| format!("P{i}")).collect::<Vec<_>>().join("\n");
    let mut d = Document::from_text(&text);
    d.settings.sum_paragraph_spacing = no_html;
    for (i, s) in paras.iter().enumerate() {
        let p = d.para_mut(StoryRef::Body, &Path::top(i)).unwrap();
        p.props.space_before = Some(s.before);
        p.props.space_after = Some(s.after);
        p.props.space_before_auto = s.auto_before.then_some(true);
        p.props.space_after_auto = s.auto_after.then_some(true);
    }
    d
}

/// Top and bottom of each paragraph's lines on the first page: the body's, or those in a cell.
fn lines(d: &Document, cell: bool) -> Vec<(f32, f32)> {
    let l = layout(d, &mut LayoutCache::new(), &LayoutOptions::default());
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { path, y, para, .. } if (path.0.len() == 4) == cell => Some((*y, *y + para.height)),
            _ => None,
        })
        .collect()
}

/// The space between each paragraph and the one above it.
fn gaps(ys: &[(f32, f32)]) -> Vec<f32> {
    ys.windows(2).map(|w| w[1].0 - w[0].1).collect()
}

fn assert_gaps(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!((g - w).abs() < 0.01, "{what}: gap {i} is {g}, Word leaves {w} ({got:?})");
    }
}

/// The body paragraphs of the measured probe, in order.
const BODY: [Sp; 15] = [
    auto(5.0, 5.0),
    plain(0.0, 0.0),
    auto(5.0, 5.0),
    auto(5.0, 5.0),
    plain(0.0, 0.0),
    Sp { before: 10.0, after: 0.0, auto_before: true, auto_after: false },
    auto(0.0, 0.0),
    plain(0.0, 0.0),
    auto(20.0, 20.0),
    plain(0.0, 0.0),
    plain(12.0, 12.0),
    auto(5.0, 5.0),
    plain(0.0, 0.0),
    auto(3.0, 15.0),
    plain(0.0, 0.0),
];

#[test]
fn stored_spacing_applies_between_body_paragraphs_when_html_auto_spacing_is_off() {
    crate::test_fonts();
    let on = lines(&doc(&BODY, true), false);
    // Spec rows 2-13: after + before of the stored values, added.
    assert_gaps(&gaps(&on), &[5.0, 5.0, 10.0, 5.0, 10.0, 0.0, 0.0, 20.0, 20.0, 12.0, 17.0, 5.0, 3.0, 15.0], "setting on");
}

#[test]
fn automatic_spacing_is_unchanged_between_body_paragraphs_without_the_setting() {
    crate::test_fonts();
    let off = lines(&doc(&BODY, false), false);
    // 14 pt wherever a neighbour is automatic, the larger of two spaces, 12 pt for the plain pair.
    assert_gaps(&gaps(&off), &[14.0, 14.0, 14.0, 14.0, 14.0, 14.0, 14.0, 14.0, 14.0, 12.0, 14.0, 14.0, 14.0, 14.0], "setting off");
}

#[test]
fn first_paragraph_keeps_its_stored_space_before_when_html_auto_spacing_is_off() {
    crate::test_fonts();
    // Spec row 1: at the top of the document automatic spacing leaves none; with the setting the
    // stored 5 pt is there.
    let (on, off) = (lines(&doc(&BODY, true), false), lines(&doc(&BODY, false), false));
    assert!((on[0].0 - off[0].0 - 5.0).abs() < 0.01, "on {} off {}", on[0].0, off[0].0);
}

/// A plain paragraph, a one-cell table holding automatic 5 / 5, automatic 5 / 5, plain 0 / 0 and
/// automatic 5 / 5 paragraphs, and a plain paragraph after it.
fn cell_doc(no_html: bool) -> Document {
    let mut d = doc(&[plain(0.0, 0.0), plain(0.0, 0.0)], no_html);
    let inner = doc(&[auto(5.0, 5.0), auto(5.0, 5.0), plain(0.0, 0.0), auto(5.0, 5.0)], no_html);
    let mut t = Table::new(1, 1, 468.0);
    t.rows[0].cells[0].blocks = inner.body.clone();
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(t)).unwrap();
    d
}

#[test]
fn stored_spacing_applies_inside_a_cell_when_html_auto_spacing_is_off() {
    crate::test_fonts();
    let (on, off) = (cell_doc(true), cell_doc(false));
    let (con, coff) = (lines(&on, true), lines(&off, true));
    let (bon, boff) = (lines(&on, false), lines(&off, false));
    assert_eq!((con.len(), coff.len(), bon.len(), boff.len()), (4, 4, 2, 2));
    // Spec rows 15, 16 and plain -> automatic: 10, 5 and 5 pt; without the setting 14 pt each.
    assert_gaps(&gaps(&con), &[10.0, 5.0, 5.0], "cell, setting on");
    assert_gaps(&gaps(&coff), &[14.0, 14.0, 14.0], "cell, setting off");
    // Spec row 14: the cell's first paragraph keeps its 5 pt before (automatic spacing: none).
    let top = |c: &[(f32, f32)], b: &[(f32, f32)]| c[0].0 - b[0].1;
    assert!((top(&con, &bon) - top(&coff, &boff) - 5.0).abs() < 0.01, "on {} off {}", top(&con, &bon), top(&coff, &boff));
    // Spec row 17: the cell's last paragraph keeps its 5 pt after (automatic spacing: none), so
    // the paragraph after the table sits 5 pt lower.
    let foot = |c: &[(f32, f32)], b: &[(f32, f32)]| b[1].0 - c[3].1;
    assert!((foot(&con, &bon) - foot(&coff, &boff) - 5.0).abs() < 0.01, "on {} off {}", foot(&con, &bon), foot(&coff, &boff));
}
