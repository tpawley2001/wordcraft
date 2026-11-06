//! EQ field radical index (`\r(n,x)`, ECMA-376 Part 1 §17.16.5.20): Word draws the index at the
//! field's own size, not a script size. Measured in Word's PDF of a corpus file with `EQ \r(-3,…)`
//! in body text: the index's glyph box is 11.1 pt tall, as the body text's, and its baseline is
//! about 0.67 em above the radicand's.

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
fn radical_index_takes_the_fields_size() {
    let cs = chars(&doc_with("", "EQ \\r(3,x)", ""));
    let (index, base) = (glyph(&cs, "3"), glyph(&cs, "x"));
    assert!((index.0 - base.0).abs() < 0.01, "index at the field's size: {index:?} vs {base:?}");
}

#[test]
fn radical_index_sits_two_thirds_of_an_em_above_the_baseline() {
    let cs = chars(&doc_with("", "EQ \\r(3,x)", ""));
    let (index, base) = (glyph(&cs, "3"), glyph(&cs, "x"));
    let above = (base.2 - index.2) / base.0;
    assert!((0.57..=0.77).contains(&above), "index baseline 0.67 em above the radicand's (Word): {above} em");
}

#[test]
fn radical_index_stays_left_of_the_radicand() {
    let cs = chars(&doc_with("", "EQ \\r(3,x)", ""));
    let (index, base) = (glyph(&cs, "3"), glyph(&cs, "x"));
    assert!(index.1 < base.1, "index over the sign's left arm, before the radicand: {index:?} {base:?}");
}

#[test]
fn guard_radical_without_index_keeps_the_fields_size() {
    let plain = chars(&doc_with("", "EQ \\r(x)", ""));
    let body = chars(&Document::from_text("x"));
    assert!((glyph(&plain, "x").0 - glyph(&body, "x").0).abs() < 0.01, "{plain:?}");
}
