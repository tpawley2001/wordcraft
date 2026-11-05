//! EQ field integral limits (`\i`, ECMA-376 Part 1 §17.16.5.20): Word draws the lower and upper
//! limits of `\i`, `\i\su` and `\i\pr` at the field's own size, not a script size. Measured in
//! Word's PDF of `EQ \i\su(i=0,n\s(2),\r(i))` in 10 pt Times New Roman: the limits' glyph boxes are
//! as tall as the text's (11.1 / 11.0 pt); the lower limit's baseline is 12.4 pt (1.24 em) below the
//! line's and the upper limit's 12.9 pt (1.29 em) above.

use crate::*;
use wordcraft_doc::Pos;
use wordcraft_doc::para::InlineObject;

/// `before` + the field + `after` in one paragraph.
fn doc_with(before: &str, instr: &str, after: &str) -> Document {
    let mut d = Document::from_text(&format!("{before}{after}"));
    let at = before.chars().count();
    d.insert_object(
        &Pos::body(0, at),
        InlineObject::Field { instr: instr.into(), result: String::new(), locked: false, dirty: false, code: false, form: String::new() },
        &Default::default(),
    )
    .unwrap();
    d
}

/// Every drawn character on page 1: (character, size, x, baseline y).
fn chars(d: &Document) -> Vec<(String, f32, f32, f32)> {
    let l = super::lay(d);
    let mut out = Vec::new();
    for i in display::page_display(d, &l.pages[0], &Default::default()) {
        if let display::Draw::Glyphs { text, size, glyphs, ranges, .. } = &i {
            for (g, r) in glyphs.iter().zip(ranges) {
                out.push((text.get(r.clone()).unwrap_or("").to_string(), *size, g.1, g.2));
            }
        }
    }
    out
}

/// (size, x, baseline y) of the first drawn `c`.
fn glyph(cs: &[(String, f32, f32, f32)], c: &str) -> (f32, f32, f32) {
    cs.iter().find(|x| x.0 == c).map(|x| (x.1, x.2, x.3)).unwrap_or_else(|| panic!("no {c:?} drawn: {cs:?}"))
}

#[test]
fn sum_limits_take_the_fields_size() {
    let cs = chars(&doc_with("", "EQ \\i\\su(i,n,x)", ""));
    let (lo, hi, body) = (glyph(&cs, "i"), glyph(&cs, "n"), glyph(&cs, "x"));
    assert!((lo.0 - body.0).abs() < 0.01, "lower limit at the field's size: {lo:?} vs {body:?}");
    assert!((hi.0 - body.0).abs() < 0.01, "upper limit at the field's size: {hi:?} vs {body:?}");
}

#[test]
fn integral_limits_take_the_fields_size() {
    let cs = chars(&doc_with("", "EQ \\i(a,b,c)", ""));
    let (lo, hi, body) = (glyph(&cs, "a"), glyph(&cs, "b"), glyph(&cs, "c"));
    assert!((lo.0 - body.0).abs() < 0.01, "lower limit at the field's size: {lo:?} vs {body:?}");
    assert!((hi.0 - body.0).abs() < 0.01, "upper limit at the field's size: {hi:?} vs {body:?}");
}

#[test]
fn sum_limits_sit_about_one_and_a_quarter_em_from_the_baseline() {
    let cs = chars(&doc_with("", "EQ \\i\\su(i,n,x)", ""));
    let (lo, hi, body) = (glyph(&cs, "i"), glyph(&cs, "n"), glyph(&cs, "x"));
    let em = body.0;
    let below = (lo.2 - body.2) / em;
    let above = (body.2 - hi.2) / em;
    assert!((1.09..=1.39).contains(&below), "lower limit's baseline 1.24 em below (Word): {below} em");
    assert!((1.14..=1.44).contains(&above), "upper limit's baseline 1.29 em above (Word): {above} em");
    assert!((lo.1 - hi.1).abs() < 0.5, "both limits centred over the symbol: {lo:?} {hi:?}");
}

#[test]
fn guard_inline_limits_are_unchanged() {
    let cs = chars(&doc_with("", "EQ \\i\\in(a,b,c)", ""));
    let (lo, hi, body) = (glyph(&cs, "a"), glyph(&cs, "b"), glyph(&cs, "c"));
    assert!(lo.0 < body.0 && hi.0 < body.0, "\\in keeps its smaller limits (not measured in Word): {lo:?} {hi:?} {body:?}");
}
