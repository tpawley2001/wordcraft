//! A run's direction and per-script fonts (`\rtlch`, `\ltrch`, `\af`, `\dbch`…) with text that is
//! not ASCII, off and size words, and the code page a font word in the East Asian slot sets.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

const FONTS: &str = r"{\fonttbl{\f0 Times New Roman;}{\f1 Arial;}{\f2 Courier New;}{\f4 SimSun;}}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{FONTS}{body}}}").as_bytes()).unwrap()
}

/// `\uN?` escapes for the UTF-16 code units `units`.
fn uni(units: &[u32]) -> String {
    units.iter().map(|n| format!("{}u{n}?", '\\')).collect()
}

/// The properties of the run holding `needle` in paragraph 0 (run lengths are bytes).
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
    let mut at = 0;
    for r in &p.runs {
        let s = p.text.get(at..at + r.len).unwrap();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

#[test]
fn a_right_to_left_group_ends_with_its_group() {
    // ᔓ ᔀ: Hebrew shin, lamed.
    let d = open(&format!(r"\pard{{\rtlch \f1\fs20 Only {}}}after\par", uni(&[1513, 1500])));
    assert_eq!(d.body[0].as_para().unwrap().text, "Only \u{5E9}\u{5DC}after");
    let c = run(&d, "\u{5E9}\u{5DC}");
    assert_eq!((c.rtl, c.font_cs.as_deref(), c.size_cs), (Some(true), Some("Arial"), Some(10.0)));
    assert_ne!(c.font.as_deref(), Some("Arial"));
    let after = run(&d, "after");
    assert_eq!((after.rtl, after.font_cs, after.size_cs), (None, None, None));
}

#[test]
fn off_words_and_size_zero_after_rtlch() {
    let d = open(r"\pard{\rtlch\ab\ai\afs30 One}{\rtlch\b0\i0\fs20\afs0 Two}\par");
    let one = run(&d, "One");
    assert_eq!((one.bold_cs, one.italic_cs, one.size_cs), (Some(true), Some(true), Some(15.0)));
    let two = run(&d, "Two");
    assert_eq!((two.bold_cs, two.italic_cs, two.size_cs), (Some(false), Some(false), None));
    let back = import(export(&d).as_bytes()).unwrap();
    let two = run(&back, "Two");
    assert_eq!((two.rtl, two.bold_cs, two.italic_cs), (Some(true), Some(false), Some(false)));
}

#[test]
fn plain_ends_the_direction_and_the_pending_slot() {
    let d = open(r"\pard{\rtlch\dbch\plain \f1\b Plain}\par");
    let c = run(&d, "Plain");
    assert_eq!((c.rtl, c.font.as_deref(), c.bold, c.bold_cs, c.font_ea), (None, Some("Arial"), Some(true), None, None));
}

#[test]
fn pard_keeps_the_direction() {
    let d = open(r"{\rtlch\pard \f1 Kept\par}");
    let c = run(&d, "Kept");
    assert_eq!((c.rtl, c.font_cs.as_deref()), (Some(true), Some("Arial")));
}

#[test]
fn an_unknown_font_number_sets_nothing_and_uses_up_the_slot() {
    let d = open(r"\pard{\dbch\f9 \f1 Unknown}{\rtlch\af9 \ltrch Gone}\par");
    let c = run(&d, "Unknown");
    assert_eq!((c.font.as_deref(), c.font_ea), (Some("Arial"), None));
    assert_eq!(run(&d, "Gone").font_cs, None);
}

#[test]
fn a_right_to_left_monospaced_run_keeps_its_fonts_on_save() {
    let d = open(&format!(r"\pard{{\ltrch\af2\afs22 \rtlch\f1\b {} x}}\par", uni(&[1513])));
    let c = run(&d, "x");
    assert_eq!((c.font.as_deref(), c.size, c.font_cs.as_deref(), c.bold_cs), (Some("Courier New"), Some(11.0), Some("Arial"), Some(true)));
    let back = import(export(&d).as_bytes()).unwrap();
    let b = run(&back, "x");
    assert_eq!((b.rtl, b.font, b.size, b.font_cs, b.bold_cs, b.bold), (c.rtl, c.font, c.size, c.font_cs, c.bold_cs, c.bold));
}

#[test]
fn an_east_asian_font_alone_round_trips() {
    let d = open(&format!(r"\pard{{\dbch\af4 {}}}\par", uni(&[26085, 26412])));
    let c = run(&d, "\u{65E5}");
    assert_eq!((c.font_ea.as_deref(), c.font.as_deref()), (Some("SimSun"), None));
    let out = export(&d);
    assert!(out.contains(r"\dbch\af"), "{out}");
    let b = run(&import(out.as_bytes()).unwrap(), "\u{65E5}");
    assert_eq!((b.font_ea, b.font, b.rtl), (c.font_ea, c.font, c.rtl));
}
