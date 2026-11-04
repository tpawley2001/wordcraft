//! EQ fields beyond `\o` (ECMA-376 Part 1 §17.16.5.20): arrays (`\a`), fractions (`\f`),
//! scripts (`\s`) and brackets (`\b`) are laid out as Word draws them, not as their flattened
//! text. Word's PDF of `EQ \a \al (Milk,Eggs,Detergent,Veggies)` (11 pt Calibri) stacks the four
//! items at one left edge, 13.56 pt apart.

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

/// (x, y) of the first drawn `c`.
fn at(cs: &[(String, f32, f32, f32)], c: &str) -> (f32, f32) {
    cs.iter().find(|x| x.0 == c).map(|x| (x.2, x.3)).unwrap_or_else(|| panic!("no {c:?} drawn: {cs:?}"))
}

const LIST: &str = "(Milk,Eggs,Detergent,Veggies)";

#[test]
fn an_array_stacks_its_items_one_per_row() {
    let cs = chars(&doc_with("", &format!(" EQ \\a \\al {LIST} "), ""));
    let rows: Vec<(f32, f32)> = ["M", "E", "D", "V"].iter().map(|c| at(&cs, c)).collect();
    let size = cs[0].1;
    for r in &rows[1..] {
        assert!((r.0 - rows[0].0).abs() < 0.01, "\\al: one left edge: {rows:?}");
    }
    let pitch = rows[1].1 - rows[0].1;
    assert!(pitch > 1.1 * size && pitch < 1.35 * size, "row pitch about one line (Word 13.56 pt at 11 pt): {pitch} at {size}");
    for w in rows.windows(2) {
        assert!((w[1].1 - w[0].1 - pitch).abs() < 0.05, "even rows: {rows:?}");
    }
}

#[test]
fn array_alignment_switches_line_items_up_left_centre_or_right() {
    // The offset of "Milk" from "Detergent" (the widest item): 0 left, half the width gap centred,
    // all of it right.
    let gap = |sw: &str| {
        let cs = chars(&doc_with("", &format!("EQ \\a {sw} {LIST}"), ""));
        at(&cs, "M").0 - at(&cs, "D").0
    };
    let (l, c, r) = (gap("\\al"), gap("\\ac"), gap("\\ar"));
    assert!(l.abs() < 0.01, "{l}");
    assert!(r > 1.0, "right-aligned: the short item starts further right: {r}");
    assert!((r - 2.0 * c).abs() < 0.5, "centred = half of right: {c} vs {r}");
}

#[test]
fn array_columns_and_vertical_spacing() {
    // \co2: items fill rows of two.
    let cs = chars(&doc_with("", "EQ \\a \\al \\co2 (A,B,C,D)", ""));
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|x| at(&cs, x));
    assert!((a.1 - b.1).abs() < 0.01 && (c.1 - d.1).abs() < 0.01, "rows of two: {a:?} {b:?} {c:?} {d:?}");
    assert!(c.1 > a.1 && b.0 > a.0 && d.0 > c.0, "{a:?} {b:?} {c:?} {d:?}");
    assert!((a.0 - c.0).abs() < 0.01 && (b.0 - d.0).abs() < 0.01, "columns line up: {a:?} {b:?} {c:?} {d:?}");
    // \vs6 adds 6 pt between rows.
    let pitch = |instr: &str| {
        let cs = chars(&doc_with("", instr, ""));
        at(&cs, "E").1 - at(&cs, "M").1
    };
    let extra = pitch(&format!("EQ \\a \\al \\vs6 {LIST}")) - pitch(&format!("EQ \\a \\al {LIST}"));
    assert!((extra - 6.0).abs() < 0.1, "\\vs6: {extra}");
}

#[test]
fn a_fraction_puts_the_numerator_over_the_denominator() {
    let cs = chars(&doc_with("x", "EQ \\f(1,2)", "y"));
    let (x, one, two, y) = (at(&cs, "x"), at(&cs, "1"), at(&cs, "2"), at(&cs, "y"));
    assert!(one.1 < x.1 && two.1 > one.1, "numerator above the denominator: x {x:?} 1 {one:?} 2 {two:?}");
    assert!((one.0 - two.0).abs() < 0.1, "both centred over one another: {one:?} {two:?}");
    assert!(one.0 > x.0 && y.0 > one.0, "the fraction sits between x and y: {x:?} {one:?} {y:?}");
    assert!((x.1 - y.1).abs() < 0.01, "the text around keeps its baseline");
}

#[test]
fn script_switches_raise_and_lower_by_points() {
    let cs = chars(&doc_with("x", "EQ \\s\\up4(2)", "y"));
    let (x, two) = (at(&cs, "x"), at(&cs, "2"));
    assert!((x.1 - two.1 - 4.0).abs() < 0.05, "\\up4 raises 4 pt: {x:?} {two:?}");
    let cs = chars(&doc_with("x", "EQ \\s\\do3(i)", "y"));
    let (x, i) = (at(&cs, "x"), at(&cs, "i"));
    assert!((i.1 - x.1 - 3.0).abs() < 0.05, "\\do3 lowers 3 pt: {x:?} {i:?}");
}

#[test]
fn brackets_enclose_their_argument() {
    let cs = chars(&doc_with("", "EQ \\b(a)", ""));
    let (l, a, r) = (at(&cs, "("), at(&cs, "a"), at(&cs, ")"));
    assert!(l.0 < a.0 && a.0 < r.0, "( a ): {cs:?}");
    // \bc sets both sides, the right one mirrored.
    let cs = chars(&doc_with("", "EQ \\b \\bc\\{ (a)", ""));
    let (l, a, r) = (at(&cs, "{"), at(&cs, "a"), at(&cs, "}"));
    assert!(l.0 < a.0 && a.0 < r.0, "{{ a }}: {cs:?}");
}

#[test]
fn guard_overstrike_still_draws_its_arguments_together() {
    let cs = chars(&doc_with("x", "eq \\o\\al(=,/)", ""));
    assert!((at(&cs, "=").0 - at(&cs, "/").0).abs() < 0.01, "{cs:?}");
}
