//! Fit Text (`w:fitText`, ECMA-376 §17.3.2.14): a run, or the runs next to each other that share its
//! id, are laid out exactly the set width, as Microsoft 365 Word does (measured from Word's PDF,
//! Times New Roman and Arial 12 and 24 pt). Growing: the extra room is split evenly after every
//! character but the last (a single character gets all of it after itself); spaces count as
//! characters. Shrinking: every character, the last included, is squeezed by width / natural width.
//! Word, `abc` fitted to 1440 twips at 12 pt TNR: a 0, b 32.88, c 66.60, next text 72.00.
//! Expectations are computed from this layout's own natural advances (the same text without Fit
//! Text), so they hold whatever the test fonts' metrics.

use crate::*;
use wordcraft_doc::props::FitText;
use wordcraft_doc::{CharProps, Pos, Run};

const FONT: &str = "Times New Roman";

fn chr(size: f32, fit: Option<FitText>) -> CharProps {
    CharProps { font: Some(FONT.into()), size: Some(size), fit_text: fit, ..Default::default() }
}

fn fit(width: f32, id: Option<u32>) -> Option<FitText> {
    Some(FitText { width, id })
}

/// Caret x at every byte offset of a one-paragraph document built from `parts` (ASCII text only).
fn xs(parts: &[(&str, f32, Option<FitText>)]) -> Vec<f32> {
    crate::test_fonts();
    let mut p = wordcraft_doc::Paragraph::new();
    for (t, size, f) in parts {
        p.text.push_str(t);
        p.runs.push(Run { len: t.len(), props: chr(*size, *f) });
    }
    p.mark = chr(12.0, None);
    let len = p.text.len();
    let mut d = Document::new();
    d.body = vec![wordcraft_doc::para_block(p)];
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    (0..=len).map(|off| l.caret(&Pos::body(0, off)).map(|c| c.x).unwrap_or(f32::NAN)).collect()
}

/// Word's positions: natural advances, changed inside each `(start, end, width)` region.
fn expected(nat: &[f32], regions: &[(usize, usize, f32)]) -> Vec<f32> {
    let mut out = vec![nat[0]];
    for i in 0..nat.len() - 1 {
        let mut adv = nat[i + 1] - nat[i];
        if let Some(&(s, e, w)) = regions.iter().find(|(s, e, _)| (*s..*e).contains(&i)) {
            let n = e - s;
            let natural = nat[e] - nat[s];
            if natural <= w {
                if n == 1 {
                    adv += w - natural;
                } else if i != e - 1 {
                    adv += (w - natural) / (n - 1) as f32;
                }
            } else {
                adv *= w / natural;
            }
        }
        out.push(out[i] + adv);
    }
    out
}

/// Lays `parts` out with and without Fit Text and checks every caret position against Word's rule.
fn check(parts: &[(&str, f32, Option<FitText>)], regions: &[(usize, usize, f32)]) {
    let plain: Vec<(&str, f32, Option<FitText>)> = parts.iter().map(|(t, s, _)| (*t, *s, None)).collect();
    let nat = xs(&plain);
    let got = xs(parts);
    let want = expected(&nat, regions);
    for (off, (g, w)) in got.iter().zip(&want).enumerate() {
        assert!((g - w).abs() < 0.05, "offset {off}: x {g:.2}, Word's rule gives {w:.2}\n got {got:?}\nwant {want:?}");
    }
    for &(s, e, w) in regions {
        assert!((got[e] - got[s] - w).abs() < 0.05, "region {s}..{e} is {:.2} wide, set to {w}", got[e] - got[s]);
    }
}

#[test]
fn fit_text_grows_a_run_to_its_width_with_the_room_after_every_char_but_the_last() {
    // F1: "abc" fitted to 1440 twips (72 pt).
    check(&[("F1 ", 12.0, None), ("abc", 12.0, fit(72.0, Some(1))), ("|end", 12.0, None)], &[(3, 6, 72.0)]);
}

#[test]
fn fit_text_counts_a_space_as_a_character() {
    // F3: "ab cd" fitted to 2160 twips (108 pt): Word a 0, b 25.79, c 75.93, d 101.76, next 108.
    check(&[("F3 ", 12.0, None), ("ab cd", 12.0, fit(108.0, Some(3))), ("|end", 12.0, None)], &[(3, 8, 108.0)]);
}

#[test]
fn runs_sharing_a_fit_text_id_are_one_region() {
    // F4: "ab" + "cd", both id 4, val 1440 each: ONE region 72 pt wide (not 144).
    check(&[("F4 ", 12.0, None), ("ab", 12.0, fit(72.0, Some(4))), ("cd", 12.0, fit(72.0, Some(4))), ("|end", 12.0, None)], &[(3, 7, 72.0)]);
}

#[test]
fn runs_with_different_fit_text_ids_are_separate_regions() {
    // F7: "ab" id 7 + "cd" id 8, 72 pt each: Word b 65.88, c 72.00, d 137.91, next 144.03.
    check(
        &[("F7 ", 12.0, None), ("ab", 12.0, fit(72.0, Some(7))), ("cd", 12.0, fit(72.0, Some(8))), ("|end", 12.0, None)],
        &[(3, 5, 72.0), (5, 7, 72.0)],
    );
}

#[test]
fn a_single_fitted_character_takes_all_the_room_after_it() {
    // F5: "a" fitted to 720 twips (36 pt).
    check(&[("F5 ", 12.0, None), ("a", 12.0, fit(36.0, Some(5))), ("|end", 12.0, None)], &[(3, 4, 36.0)]);
}

#[test]
fn fit_text_without_an_id_and_at_24_pt() {
    // F6 (no id) and F8 (24 pt: Word a 0, b 30.00, c 61.32, next 72.00).
    check(&[("F6 ", 12.0, None), ("abc", 12.0, fit(72.0, None)), ("|end", 12.0, None)], &[(3, 6, 72.0)]);
    check(&[("F8 ", 12.0, None), ("abc", 24.0, fit(72.0, Some(9))), ("|end", 12.0, None)], &[(3, 6, 72.0)]);
}

#[test]
fn fit_text_narrower_than_the_text_squeezes_every_character() {
    // F2: 16 letters fitted to 720 twips (36 pt): Word squeezes every advance, the last included.
    check(&[("F2 ", 12.0, None), ("abcdefghijklmnop", 12.0, fit(36.0, Some(2))), ("|end", 12.0, None)], &[(3, 19, 36.0)]);
}

#[test]
fn text_without_fit_text_or_with_a_zero_width_is_unchanged() {
    check(&[("plain ", 12.0, None), ("abc", 12.0, None), ("|end", 12.0, None)], &[]);
    check(&[("zero ", 12.0, None), ("abc", 12.0, fit(0.0, Some(1))), ("|end", 12.0, None)], &[]);
}
