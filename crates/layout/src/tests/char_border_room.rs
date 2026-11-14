//! Run borders (`w:bdr`, ECMA-376 17.3.2.4) take their room, as Microsoft 365 Word lays them out
//! (synthetic RTF/DOCX, Word's PDF, 2026-11-14; Times New Roman and Arial at 8, 11 and 24 pt):
//! the bordered text moves right by the border's space plus its line width, the text after it moves
//! by that again, the box's outer edges sit that far outside the glyphs and the plain line, and the
//! line grows by space plus width above and below. Equal borders on adjacent runs make one box.
use crate::display::{Draw, page_display};
use crate::*;
use wordcraft_doc::props::Border;
use wordcraft_doc::{Run, para_block};

const FONT: &str = "Source Serif 4";

fn props(size: f32) -> CharProps {
    CharProps { font: Some(FONT.into()), size: Some(size), ..CharProps::default() }
}

fn border(width: f32, space: f32) -> Border {
    Border { space, ..Border::single(width) }
}

/// Lays out `paras` (each a list of (text, border) runs) and returns the display list of page 1.
fn draws(size: f32, paras: &[Vec<(&str, Option<Border>)>]) -> Vec<Draw> {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = paras
        .iter()
        .map(|runs| {
            let mut p = Paragraph::new();
            for (t, b) in runs {
                p.text.push_str(t);
                p.runs.push(Run { len: t.len(), props: CharProps { border: *b, ..props(size) } });
            }
            para_block(p)
        })
        .collect();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    page_display(&d, page, &Default::default())
}

/// (x, baseline y) of the glyph that shows the first letter of `word`.
fn at(draws: &[Draw], word: &str) -> (f32, f32) {
    draws
        .iter()
        .find_map(|d| match d {
            Draw::Glyphs { glyphs, text, ranges, .. } => {
                let start = text.find(word)?;
                let i = ranges.iter().position(|r| r.start == start)?;
                glyphs.get(i).map(|g| (g.1, g.2))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{word} is drawn"))
}

/// Outer left and right edges of the vertical border rules (centre -+ half the line width).
fn box_sides(draws: &[Draw]) -> Vec<(f32, f32)> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Line { x0, y0, x1, y1, width, .. } if (x0 - x1).abs() < 1e-3 && (y0 - y1).abs() > 1.0 => Some((x0 - width / 2.0, x0 + width / 2.0)),
            _ => None,
        })
        .collect()
}

#[test]
fn bordered_text_and_the_text_after_it_move_out_by_space_plus_width() {
    for size in [11.0f32, 24.0] {
        let plain = draws(size, &[vec![("Alpha ", None), ("Boxed", None), ("Zed", None)]]);
        let b = border(1.5, 2.0);
        let boxed = draws(size, &[vec![("Alpha ", None), ("Boxed", Some(b)), ("Zed", None)]]);
        let (px, _) = at(&plain, "Boxed");
        let (bx, _) = at(&boxed, "Boxed");
        assert!((bx - px - 3.5).abs() < 0.05, "{size} pt: Boxed moved {} pt, want 3.5", bx - px);
        let (pz, _) = at(&plain, "Zed");
        let (bz, _) = at(&boxed, "Zed");
        assert!((bz - pz - 7.0).abs() < 0.05, "{size} pt: the text after moved {} pt, want 7.0", bz - pz);
        let sides = box_sides(&boxed);
        assert_eq!(sides.len(), 2, "{size} pt: one box, two vertical sides: {sides:?}");
        let left = sides.iter().map(|s| s.0).fold(f32::MAX, f32::min);
        let right = sides.iter().map(|s| s.1).fold(f32::MIN, f32::max);
        assert!((left - (bx - 3.5)).abs() < 0.05, "{size} pt: box outer left {left}, want {}", bx - 3.5);
        assert!((right - bz).abs() < 0.05, "{size} pt: box outer right {right} should meet the next text at {bz}");
    }
}

#[test]
fn a_bordered_line_grows_by_space_plus_width_above_and_below() {
    let size = 11.0;
    let b = border(0.5, 0.0);
    let plain = draws(size, &[vec![("Alpha", None)], vec![("Bravo", None)], vec![("Charlie", None)]]);
    let boxed = draws(size, &[vec![("Alpha", None)], vec![("Bravo", Some(b))], vec![("Charlie", None)]]);
    let pitch = |d: &[Draw], a: &str, c: &str| at(d, c).1 - at(d, a).1;
    let p1 = pitch(&plain, "Alpha", "Bravo");
    let p2 = pitch(&plain, "Bravo", "Charlie");
    let b1 = pitch(&boxed, "Alpha", "Bravo");
    let b2 = pitch(&boxed, "Bravo", "Charlie");
    assert!((b1 - p1 - 0.5).abs() < 0.05, "above: {b1} vs plain {p1}, want +0.5");
    assert!((b2 - p2 - 0.5).abs() < 0.05, "below: {b2} vs plain {p2}, want +0.5");
}

#[test]
fn adjacent_runs_with_equal_borders_share_one_box_and_its_room() {
    let size = 11.0;
    let b = border(1.5, 2.0);
    let plain = draws(size, &[vec![("Alpha ", None), ("Box", None), ("ed", None), ("Zed", None)]]);
    let boxed = draws(size, &[vec![("Alpha ", None), ("Box", Some(b)), ("ed", Some(b)), ("Zed", None)]]);
    let (pz, _) = at(&plain, "Zed");
    let (bz, _) = at(&boxed, "Zed");
    assert!((bz - pz - 7.0).abs() < 0.05, "the text after moved {} pt, want 7.0 (room at the group's ends only)", bz - pz);
    assert_eq!(box_sides(&boxed).len(), 2, "one box around both runs");
}

#[test]
fn text_without_a_border_does_not_move() {
    let size = 11.0;
    let a = draws(size, &[vec![("Alpha ", None), ("Boxed", None), ("Zed", None)]]);
    let b = draws(size, &[vec![("Alpha Boxed", None), ("Zed", None)]]);
    let (ax, ay) = at(&a, "Zed");
    let (bx, by) = at(&b, "Zed");
    assert!((ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01, "plain runs: Zed at ({ax}, {ay}) vs ({bx}, {by})");
    assert!(box_sides(&a).is_empty(), "no box without a border");
}
