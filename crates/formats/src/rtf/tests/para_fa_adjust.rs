//! Paragraph font alignment (`\faauto` `\fahang` `\facenter` `\faroman` `\favar` `\fafixed`) and
//! Automatically adjust right indent (`\adjustright`), RTF 1.9.1 paragraph formatting: read into the
//! paragraph's and the paragraph style's own properties and written back. Expected values are
//! Microsoft 365 Word's (16.0.20527, 2026-11-14): each RTF opened and saved as .docx. In RTF,
//! a paragraph or style without `\adjustright` has the setting OFF (DOCX: absent = on).

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{ParaProps, TextAlignment};

fn open(head: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{head}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<ParaProps> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.clone()).collect()
}

/// The paragraph's resolved Automatically adjust right indent.
fn adjust(d: &Document, p: &ParaProps) -> bool {
    d.styles.resolve_para(p).adjust_right_indent
}

/// Auto and unset are the same thing to Word (it writes no w:textAlignment for either).
fn fa(p: &ParaProps) -> TextAlignment {
    p.text_alignment.unwrap_or(TextAlignment::Auto)
}

const FA_BODY: &str =
    r"\pard\faauto\adjustright A1\par\pard\fahang B2\par\pard\facenter C3\par\pard\faroman D4\par\pard\favar E5\par\pard\fafixed F6\par\pard G7\par";

const FA_WORD: [(TextAlignment, bool); 7] = [
    (TextAlignment::Auto, true),
    (TextAlignment::Top, false),
    (TextAlignment::Center, false),
    (TextAlignment::Baseline, false),
    (TextAlignment::Bottom, false),
    (TextAlignment::Auto, false),
    (TextAlignment::Auto, false),
];

const STYLES: &str = r"{\stylesheet{\ql\adjustright\fs24 Normal;}{\s1\ql\fs24\sbasedon0 NoAdj;}}";
const STYLE_BODY: &str = r"\pard\plain\adjustright N1\par\pard\plain N2\par\pard\plain\s1 N3\par";

#[test]
fn font_alignment_and_adjust_right_are_read_as_word_reads_them() {
    let d = open("", FA_BODY);
    let p = paras(&d);
    assert_eq!(p.len(), 7);
    for (i, (want_fa, want_adj)) in FA_WORD.iter().enumerate() {
        assert_eq!(fa(&p[i]), *want_fa, "paragraph {i} font alignment");
        assert_eq!(adjust(&d, &p[i]), *want_adj, "paragraph {i} adjust right indent");
    }
    assert_eq!(p[1].text_alignment, Some(TextAlignment::Top));
    assert_eq!(p[6].text_alignment, None, "no word after \\pard leaves it unset");
}

#[test]
fn the_state_lasts_across_par_until_pard() {
    let d = open("", r"\pard\facenter\adjustright one\par two\par\pard three\par");
    let p = paras(&d);
    assert_eq!((fa(&p[0]), adjust(&d, &p[0])), (TextAlignment::Center, true));
    assert_eq!((fa(&p[1]), adjust(&d, &p[1])), (TextAlignment::Center, true), "\\par keeps paragraph state");
    assert_eq!((fa(&p[2]), adjust(&d, &p[2])), (TextAlignment::Auto, false), "\\pard resets it");
}

#[test]
fn style_entries_carry_adjust_right_and_paragraphs_follow_word() {
    let d = open(STYLES, STYLE_BODY);
    let normal = d.styles.get("Normal").expect("Normal");
    assert_eq!(normal.para.adjust_right_indent, Some(true));
    let no_adj = d.styles.get("NoAdj").expect("style NoAdj");
    assert_eq!(no_adj.para.adjust_right_indent, Some(false), "a paragraph style entry without \\adjustright is off");
    let p = paras(&d);
    assert_eq!(p[2].style.as_deref(), Some("NoAdj"));
    let got: Vec<bool> = p.iter().map(|x| adjust(&d, x)).collect();
    assert_eq!(got, [true, false, false]);
}

#[test]
fn style_font_alignment_is_read() {
    let d = open(
        r"{\stylesheet{\adjustright Normal;}{\s2\favar\adjustright\sbasedon0 Low;}}",
        r"\pard\plain\s2\favar\adjustright x\par",
    );
    let s = d.styles.get("Low").expect("style Low");
    assert_eq!(s.para.text_alignment, Some(TextAlignment::Bottom));
    assert_eq!(s.para.adjust_right_indent, Some(true));
}

#[test]
fn every_setting_round_trips() {
    for (head, body) in [("", FA_BODY), (STYLES, STYLE_BODY)] {
        let d = open(head, body);
        let out = export(&d);
        let back = import(out.as_bytes()).unwrap();
        let (a, b) = (paras(&d), paras(&back));
        assert_eq!(a.len(), b.len(), "{out}");
        for (i, (x, y)) in a.iter().zip(&b).enumerate() {
            assert_eq!(fa(x), fa(y), "paragraph {i} font alignment:\n{out}");
            assert_eq!(adjust(&d, x), adjust(&back, y), "paragraph {i} adjust right indent:\n{out}");
        }
    }
    let back = import(export(&open(STYLES, STYLE_BODY)).as_bytes()).unwrap();
    assert_eq!(back.styles.get("NoAdj").expect("NoAdj").para.adjust_right_indent, Some(false));
}

#[test]
fn a_new_document_writes_adjustright_as_word_does() {
    // Nothing set: DOCX semantics (adjust right indent on), so the RTF must say \adjustright.
    let out = export(&Document::from_text("one\ntwo"));
    let back = import(out.as_bytes()).unwrap();
    for p in paras(&back) {
        assert!(adjust(&back, &p), "a new document's paragraphs stay on:\n{out}");
    }
    for w in [r"\fahang", r"\facenter", r"\faroman", r"\favar"] {
        assert!(!out.contains(w), "{w} in:\n{out}");
    }
}
