//! A `w:position` raise or lowering on a line with multiple line spacing: Word multiplies the plain
//! text's height by the spacing and adds the shift once (Microsoft 365 Word, 12 pt Times New Roman and
//! Arial, measured from Word's PDF; a plain single line P is 13.8 pt there). The expected deltas are
//! written in terms of the plain single line pitch P this layout gives the same font, so they hold
//! whatever the test fonts' metrics: Word's double spacing with a 6 pt raise is (P + 6, 2P) = (19.8, 27.6).

use crate::*;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{CharProps, Run};

fn font(name: &str, position: Option<f32>) -> CharProps {
    CharProps { font: Some(name.into()), size: Some(12.0), position, ..Default::default() }
}

fn para(parts: &[(&str, CharProps)], spacing: LineSpacing) -> std::sync::Arc<wordcraft_doc::Block> {
    let mut p = wordcraft_doc::Paragraph::new();
    for (t, chr) in parts {
        p.text.push_str(t);
        p.runs.push(Run { len: t.len(), props: chr.clone() });
    }
    if let Some((_, chr)) = parts.first() {
        p.mark = chr.clone();
    }
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(spacing);
    wordcraft_doc::para_block(p)
}

/// Baseline deltas (previous plain line -> middle line, middle -> next plain line) for plain single /
/// `"Xx "` + `"shifted"` (raised by `shift` pt, lowered if negative) at `spacing` / plain single.
fn deltas(name: &str, spacing: LineSpacing, shift: Option<f32>) -> (f32, f32) {
    crate::test_fonts();
    let single = LineSpacing::Multiple(1.0);
    let mut d = Document::new();
    d.body = vec![
        para(&[("Plain line", font(name, None))], single),
        para(&[("Xx ", font(name, None)), ("shifted", font(name, shift))], spacing),
        para(&[("Plain line", font(name, None))], single),
    ];
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let b: Vec<f32> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| if let Placed::Lines { y, para, .. } = it { Some((*y, para)) } else { None })
        .map(|(y, pl)| {
            assert_eq!(pl.lines.len(), 1, "one line a paragraph");
            y + pl.lines[0].top + pl.lines[0].baseline
        })
        .collect();
    assert_eq!(b.len(), 3, "{b:?}");
    (b[1] - b[0], b[2] - b[1])
}

/// The plain single line pitch: baseline to baseline of two plain single-spaced lines.
fn pitch(name: &str) -> f32 {
    deltas(name, LineSpacing::Multiple(1.0), None).0
}

/// `want` maps the plain pitch P to the expected (previous -> middle, middle -> next) deltas.
#[track_caller]
fn check(name: &str, spacing: LineSpacing, shift: Option<f32>, want: impl Fn(f32) -> (f32, f32)) {
    let p = pitch(name);
    let want = want(p);
    let got = deltas(name, spacing, shift);
    assert!(
        (got.0 - want.0).abs() <= 0.15 && (got.1 - want.1).abs() <= 0.15,
        "{name} {spacing:?} shift {shift:?}: baseline deltas {got:?}, Word {want:?} (plain pitch {p})"
    );
}

#[test]
fn plain_double_spacing_is_unchanged() {
    for f in ["Times New Roman", "Arial"] {
        check(f, LineSpacing::Multiple(2.0), None, |p| (p, 2.0 * p));
    }
}

#[test]
fn double_spacing_adds_a_raise_once() {
    for f in ["Times New Roman", "Arial"] {
        check(f, LineSpacing::Multiple(2.0), Some(6.0), |p| (p + 6.0, 2.0 * p));
        check(f, LineSpacing::Multiple(2.0), Some(3.0), |p| (p + 3.0, 2.0 * p));
    }
}

#[test]
fn double_spacing_adds_a_lowering_once() {
    for f in ["Times New Roman", "Arial"] {
        check(f, LineSpacing::Multiple(2.0), Some(-6.0), |p| (p, 2.0 * p + 6.0));
    }
}

#[test]
fn one_and_a_half_and_triple_spacing_add_the_shift_once() {
    for f in ["Times New Roman", "Arial"] {
        check(f, LineSpacing::Multiple(1.5), Some(6.0), |p| (p + 6.0, 1.5 * p));
        check(f, LineSpacing::Multiple(1.5), Some(-6.0), |p| (p, 1.5 * p + 6.0));
        check(f, LineSpacing::Multiple(3.0), Some(6.0), |p| (p + 6.0, 3.0 * p));
    }
}

#[test]
fn single_spacing_shift_is_unchanged() {
    for f in ["Times New Roman", "Arial"] {
        check(f, LineSpacing::Multiple(1.0), Some(6.0), |p| (p + 6.0, p));
        check(f, LineSpacing::Multiple(1.0), Some(-6.0), |p| (p, p + 6.0));
    }
}
