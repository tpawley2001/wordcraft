//! A run's direction and per-script formatting (RTF 1.9.1 "Associated Character Properties" and
//! "Bidirectional Language Support"): `\rtlch` / `\ltrch`, `\af` / `\afs` / `\ab` / `\ai`, and the
//! `\loch` / `\hich` / `\dbch` font slots, read into the run's own properties (the RTF form of DOCX
//! `w:rtl`, `w:rFonts/@w:cs`, `w:szCs`, `w:bCs`, `w:iCs`, `w:rFonts/@w:eastAsia`) and written back.
//!
//! Measured in Word (Microsoft 365): the words after `\rtlch` are the complex-script formatting,
//! the words after `\ltrch` the other, with or without the `a` prefix and whatever `\fcsN` says; the
//! last of the two is the run's direction; a font word right after `\dbch` names only the East
//! Asian font.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

const FONTS: &str = r"{\fonttbl{\f0 Times New Roman;}{\f1 Arial;}{\f2 Courier New;}{\f3 MS Mincho;}{\f4 SimSun;}}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{FONTS}{body}}}").as_bytes()).unwrap()
}

/// The properties of the run holding `needle` in paragraph 0.
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
    // `Run::len` counts bytes of the paragraph's UTF-8 text.
    let mut at = 0;
    for r in &p.runs {
        let s = p.text.get(at..at + r.len).unwrap_or_default();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

/// `\uN?` escapes for `text`.
fn uni(text: &str) -> String {
    text.chars().map(|c| format!("{}u{}?", '\\', c as u32)).collect()
}

/// (rtl, font, size, bold, italic) and (font_cs, size_cs, bold_cs, italic_cs), unset flags as false.
type Base = (bool, Option<String>, Option<f32>, bool, bool);
type Complex = (Option<String>, Option<f32>, bool, bool);

fn base(c: &CharProps) -> Base {
    (c.rtl == Some(true), c.font.clone(), c.size, c.bold == Some(true), c.italic == Some(true))
}

fn complex(c: &CharProps) -> Complex {
    (c.font_cs.clone(), c.size_cs, c.bold_cs == Some(true), c.italic_cs == Some(true))
}

fn name(s: &str) -> Option<String> {
    Some(s.to_string())
}

const LTR_RUN: &str = r"\pard{\rtlch\fcs1 \af2\afs48\ab\ai \ltrch\fcs0 \f1\fs20 Latin}\par";
const RTL_RUN: &str = r"\pard{\ltrch\fcs0 \af2\afs48\ab \rtlch\fcs1 \f1\fs20\i Latin}\par";
const EA_RUN: &str = r"\pard{\dbch\af4\loch\f1\fs20 Latin}\par";

#[test]
fn left_to_right_run_keeps_its_complex_script_formatting() {
    let c = run(&open(LTR_RUN), "Latin");
    assert_eq!(base(&c), (false, name("Arial"), Some(10.0), false, false));
    assert_eq!(complex(&c), (name("Courier New"), Some(24.0), true, true));
}

#[test]
fn right_to_left_run_takes_the_words_after_rtlch_as_complex_script() {
    let c = run(&open(RTL_RUN), "Latin");
    assert_eq!(base(&c), (true, name("Courier New"), Some(24.0), true, false));
    assert_eq!(complex(&c), (name("Arial"), Some(10.0), false, true));
}

#[test]
fn position_after_the_direction_word_decides_not_the_a_prefix() {
    let d = open(r"\pard{\ltrch \rtlch\ab\f1\fs20 Xb}{\ltrch \af2 \rtlch \b\f1 Xc}{\rtlch \b \ltrch \f1 Xd}\par");
    let b = run(&d, "Xb");
    assert_eq!((b.rtl, b.bold_cs, b.bold == Some(true)), (Some(true), Some(true), false), "\\ab after \\rtlch");
    assert_eq!(b.font_cs, name("Arial"));
    let c = run(&d, "Xc");
    assert_eq!((c.rtl, c.bold_cs, c.bold == Some(true)), (Some(true), Some(true), false), "\\b after \\rtlch");
    assert_eq!(c.font, name("Courier New"), "\\af after \\ltrch");
    let e = run(&d, "Xd");
    assert_eq!((e.rtl == Some(true), e.bold_cs, e.bold == Some(true)), (false, Some(true), false), "\\b after \\rtlch in a left-to-right run");
}

#[test]
fn fcs_makes_no_difference() {
    let d = open(r"\pard{\ltrch\fcs1 \af2\afs48 \rtlch\fcs0 \f1\fs20 Aa}{\ltrch\fcs0 \af2\afs48 \rtlch\fcs1 \f1\fs20 Bb}\par");
    let (a, b) = (run(&d, "Aa"), run(&d, "Bb"));
    assert_eq!(base(&a), (true, name("Courier New"), Some(24.0), false, false));
    assert_eq!(complex(&a), (name("Arial"), Some(10.0), false, false));
    assert_eq!((base(&a), complex(&a)), (base(&b), complex(&b)));
}

#[test]
fn rtlch_alone_makes_a_right_to_left_run() {
    let d = open(&format!(r"\pard{{\rtlch \f1\fs20 Only {}}}after\par", uni("של")));
    let c = run(&d, "Only");
    assert_eq!(c.rtl, Some(true));
    assert_eq!(complex(&c), (name("Arial"), Some(10.0), false, false));
    assert_ne!(c.font, name("Arial"));
    assert_ne!(c.size, Some(10.0));
    assert!(d.body[0].as_para().unwrap().text.contains("של"), "the text itself is unchanged");
    // The group's end restores the direction.
    let after = run(&d, "after");
    assert_ne!(after.rtl, Some(true));
    assert_eq!(complex(&after), (None, None, false, false));
}

#[test]
fn runs_without_direction_words_read_as_before() {
    let d = open(r"\pard{\b\f1\fs20 Plain}{\af2\afs48\ab \f1\fs20 Loose}\par");
    for needle in ["Plain", "Loose"] {
        let c = run(&d, needle);
        assert_eq!((c.rtl == Some(true), c.font.clone(), c.size), (false, name("Arial"), Some(10.0)), "{needle}");
        assert_eq!((c.font_cs.clone(), c.size_cs, c.bold_cs, c.italic_cs, c.font_ea.clone()), (None, None, None, None, None), "{needle}");
    }
    assert_eq!(run(&d, "Plain").bold, Some(true));
    assert_ne!(run(&d, "Loose").bold, Some(true), "\\ab with no direction word is ignored");
}

#[test]
fn a_font_right_after_dbch_is_the_east_asian_font_only() {
    let d = open(
        r"\pard{\dbch\af4\loch\f1\fs20 Da}{\dbch\f4\fs20 Db}{\loch\af2\hich\af2\dbch\f3\fs20 Dc}{\dbch\af4\f1\fs20 Dd}{\dbch\af4 \f2\fs20 De}\par",
    );
    let fonts = |needle: &str| {
        let c = run(&d, needle);
        (c.font, c.font_ea)
    };
    assert_eq!(fonts("Da"), (name("Arial"), name("SimSun")));
    let (font, ea) = fonts("Db");
    assert_eq!(ea, name("SimSun"));
    assert_ne!(font, name("SimSun"), "\\dbch\\f names the East Asian font; Latin text keeps its own");
    assert_eq!(fonts("Dc"), (name("Courier New"), name("MS Mincho")));
    assert_eq!(fonts("Dd"), (name("Arial"), name("SimSun")));
    assert_eq!(fonts("De"), (name("Courier New"), name("SimSun")));
}

#[test]
fn a_plain_font_and_the_high_ansi_slot_set_no_east_asian_font() {
    let d = open(r"\pard{\f4\fs20 Ea}{\hich\af2\loch\f1\fs20 Eb}\par");
    let a = run(&d, "Ea");
    assert_eq!((a.font, a.font_ea), (name("SimSun"), None));
    let b = run(&d, "Eb");
    assert_eq!((b.font, b.font_ea), (name("Arial"), None));
}

#[test]
fn a_font_after_dbch_still_sets_the_code_page() {
    let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}{\f5\fcharset128 MS Gothic;}}\pard{\dbch\f5 \'93\'fa\'96\'7b}\par}";
    let d = import(rtf.as_bytes()).unwrap();
    assert_eq!(d.body[0].as_para().unwrap().text, "日本");
    assert_eq!(run(&d, "日本").font_ea, name("MS Gothic"));
}

#[test]
fn direction_and_script_formatting_survive_export() {
    for (body, word) in [(LTR_RUN, "\\rtlch"), (RTL_RUN, "\\rtlch"), (EA_RUN, "\\dbch")] {
        let d = open(body);
        let out = export(&d);
        assert!(out.contains(word), "{word} in {out}");
        let back = import(out.as_bytes()).unwrap();
        let (c, b) = (run(&d, "Latin"), run(&back, "Latin"));
        assert_eq!((base(&c), complex(&c), c.font_ea.clone()), (base(&b), complex(&b), b.font_ea.clone()), "{body}");
    }
}

#[test]
fn export_puts_the_run_direction_word_last() {
    // Word takes the last of `\rtlch` / `\ltrch` before the text as the run's direction.
    let words = |body: &str| {
        let out = export(&open(body));
        let head = &out[..out.find("Latin").unwrap()];
        (head.rfind("\\rtlch").unwrap(), head.rfind("\\ltrch").unwrap())
    };
    let (rtl, ltr) = words(LTR_RUN);
    assert!(rtl < ltr, "left-to-right run: \\rtlch … \\ltrch …");
    let (rtl, ltr) = words(RTL_RUN);
    assert!(ltr < rtl, "right-to-left run: \\ltrch … \\rtlch …");
}

#[test]
fn plain_text_exports_without_direction_or_slot_words() {
    let out = export(&open(r"\pard{\b\f1\fs20 Plain}\par"));
    for word in ["\\rtlch", "\\ltrch", "\\dbch", "\\loch", "\\hich", "\\fcs"] {
        assert!(!out.contains(word), "{word} in {out}");
    }
}
