//! A Word 97-2003 file with `Copts80.f2ptExtLeadingOnly` (`Settings::ext_leading_2pt_only`): a line
//! with superscript or subscript text is as tall as the same line with that text not raised or
//! lowered, its full size plus 2 pt. Measured in Microsoft 365 Word on RTF files with `\sprslnsp`
//! (Times New Roman, baseline to baseline): 10 / 11 (at least 12 pt) / 12 pt lines with a
//! superscript "th" are 12.0 / 13.0 / 14.0 pt tall, like plain lines; a 10 pt line holding a 20 pt
//! superscript run is 22.0 (its baseline 19.8 below the plain line before it, the next plain
//! baseline 14.16 below it). A `w:position` raise still adds its amount.

use crate::*;
use wordcraft_doc::props::{LineSpacing, VertAlign};
use wordcraft_doc::{CharProps, Run};

const SINGLE: LineSpacing = LineSpacing::Multiple(1.0);

fn times(size: f32) -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() }
}

/// A paragraph of `parts` (text, its formatting), spaced `spacing`, no space before or after.
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

/// The (top, height, absolute baseline) of each body paragraph's single line on page 1.
fn lines(flag: bool, paras: Vec<std::sync::Arc<wordcraft_doc::Block>>) -> Vec<(f32, f32, f32)> {
    let mut d = Document::new();
    d.settings.ext_leading_2pt_only = flag;
    d.body = paras;
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let out: Vec<(f32, f32, f32)> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| if let Placed::Lines { y, para, .. } = it { Some((*y, para)) } else { None })
        .map(|(y, pl)| {
            assert_eq!(pl.lines.len(), 1, "one line a paragraph");
            let line = &pl.lines[0];
            (y + line.top, line.height, y + line.top + line.baseline)
        })
        .collect();
    assert_eq!(out.len(), d.body.len(), "{out:?}");
    out
}

/// Times New Roman's descent (without line gap) at `size`, as the layout resolves the face here.
fn descent(size: f32) -> f32 {
    crate::test_fonts();
    let face = wordcraft_fonts::word::resolve("Times New Roman", false, false).face;
    let (_, d) = wordcraft_fonts::word::line_metrics_no_leading(&face);
    (d * f64::from(size) / face.upem.max(1.0)) as f32
}

fn script(size: f32, v: VertAlign) -> CharProps {
    CharProps { vert_align: Some(v), ..times(size) }
}

/// The height of the middle line of plain / `mixed` / plain paragraphs.
fn middle_height(flag: bool, size: f32, spacing: LineSpacing, mixed: &[(&str, CharProps)]) -> f32 {
    let plain = [("Plain text line", times(size))];
    let ls = lines(flag, vec![para(&plain, spacing), para(mixed, spacing), para(&plain, spacing)]);
    // The line's own height and its share of the page agree.
    let pitch = ls[2].0 - ls[1].0;
    assert!((pitch - ls[1].1).abs() <= 0.01, "pitch {pitch} vs height {}", ls[1].1);
    ls[1].1
}

fn assert_height(row: u32, got: f32, expected: f32) {
    assert!((got - expected).abs() <= 0.01, "row {row}: line {got} pt tall, Word {expected}");
}

#[test]
fn row1_a_plain_line_is_its_size_plus_2pt() {
    assert_height(1, middle_height(true, 10.0, SINGLE, &[("Plain text", times(10.0))]), 12.0);
}

#[test]
fn row2_a_superscript_adds_nothing() {
    let mixed = [("The 4", times(10.0)), ("th", script(10.0, VertAlign::Superscript)), (" day", times(10.0))];
    assert_height(2, middle_height(true, 10.0, SINGLE, &mixed), 12.0);
}

#[test]
fn row3_a_subscript_adds_nothing() {
    let mixed = [("H", times(10.0)), ("2", script(10.0, VertAlign::Subscript)), ("O water", times(10.0))];
    assert_height(3, middle_height(true, 10.0, SINGLE, &mixed), 12.0);
}

#[test]
fn row4_a_whole_superscript_line_is_its_full_size_plus_2pt() {
    let mixed = [("All raised text", script(10.0, VertAlign::Superscript))];
    assert_height(4, middle_height(true, 10.0, SINGLE, &mixed), 12.0);
}

#[test]
fn row5_at_least_spacing_with_a_superscript() {
    let mixed = [("The 4", times(11.0)), ("th", script(11.0, VertAlign::Superscript)), (" day", times(11.0))];
    assert_height(5, middle_height(true, 11.0, LineSpacing::AtLeast(12.0), &mixed), 13.0);
}

#[test]
fn row6_twelve_point_with_a_superscript() {
    let mixed = [("The 4", times(12.0)), ("th", script(12.0, VertAlign::Superscript)), (" day", times(12.0))];
    assert_height(6, middle_height(true, 12.0, SINGLE, &mixed), 14.0);
}

#[test]
fn row7_a_large_superscript_run_takes_its_full_size_line() {
    let plain = [("Plain text line", times(10.0))];
    let mixed = [("Small ", times(10.0)), ("BIG", script(20.0, VertAlign::Superscript))];
    let ls = lines(true, vec![para(&plain, SINGLE), para(&mixed, SINGLE), para(&plain, SINGLE)]);
    assert_height(7, ls[1].1, 22.0);
    let (d10, d20) = (descent(10.0), descent(20.0));
    // Ascent of the 20 pt line 22 - d20, descent d20 (Word: 17.68 and 4.32).
    let above = ls[1].2 - ls[1].0;
    assert!((above - (22.0 - d20)).abs() <= 0.01, "row 7: baseline {above} below the line top, expected {}", 22.0 - d20);
    let before = ls[1].2 - ls[0].2;
    let after = ls[2].2 - ls[1].2;
    assert!((before - (d10 + 22.0 - d20)).abs() <= 0.01, "row 7: {before} below the plain baseline (Word 19.8)");
    assert!((after - (d20 + 12.0 - d10)).abs() <= 0.01, "row 7: next baseline {after} below (Word 14.16)");
}

#[test]
fn row8_a_position_raise_still_adds_its_amount() {
    let raised = CharProps { position: Some(3.0), ..times(10.0) };
    let mixed = [("Plain ", times(10.0)), ("up", raised), (" text", times(10.0))];
    assert_height(8, middle_height(true, 10.0, SINGLE, &mixed), 15.0);
}

#[test]
fn row9_without_the_option_a_superscript_line_is_a_plain_line() {
    let plain = [("Plain text line", times(10.0))];
    let mixed = [("The 4", times(10.0)), ("th", script(10.0, VertAlign::Superscript)), (" day", times(10.0))];
    let ls = lines(false, vec![para(&plain, SINGLE), para(&mixed, SINGLE), para(&plain, SINGLE)]);
    assert!((ls[1].1 - ls[0].1).abs() <= 0.01, "row 9: {} vs plain {}", ls[1].1, ls[0].1);
}
