//! A body paragraph's own spacing (`\sb`, `\sa`, `\sl`/`\slmult`), indents (`\li`, `\ri`, `\fi`) and
//! keep options (`\keepn`, `\keep`, `\noline`), hidden text (`\v`) and raised or lowered text
//! (`\upN`, `\dnN`), RTF 1.9.1: read into the paragraph's and the run's own properties and written
//! back, as Word does.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::Block;
use wordcraft_doc::props::{CharProps, LineSpacing, ParaProps, VertAlign};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn para(d: &Document, i: usize) -> ParaProps {
    d.body[i].as_para().unwrap().props.clone()
}

fn para_mut(d: &mut Document, i: usize) -> &mut wordcraft_doc::Paragraph {
    match std::sync::Arc::make_mut(&mut d.body[i]) {
        Block::Para(p) => p,
        _ => panic!("block {i} is not a paragraph"),
    }
}

fn text(d: &Document, i: usize) -> String {
    d.body[i].as_para().unwrap().text.clone()
}

/// The properties of the run holding `needle` in paragraph 0.
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    for r in &p.runs {
        let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

/// The control words of the body paragraph whose text ends in `marker` (from its `\pard` to the
/// marker), so assertions don't see the stylesheet.
fn para_words(rtf: &str, marker: &str) -> String {
    let end = rtf.find(marker).unwrap_or_else(|| panic!("{marker:?} not in {rtf}"));
    let start = rtf[..end].rfind(r"\pard").expect("a \\pard before the text");
    rtf[start..end].to_string()
}

/// `true` if `words` holds control word `w` exactly (not as the prefix of a longer word or number).
fn has(words: &str, w: &str) -> bool {
    words.match_indices(w).any(|(i, _)| {
        let next = words[i + w.len()..].chars().next();
        !next.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

fn round_trip(body: &str) -> (String, Document) {
    let rtf = export(&open(body));
    let back = import(rtf.as_bytes()).unwrap();
    (rtf, back)
}

// ---- P03-098: spacing and indents ----

#[test]
fn spacing_and_indents_are_written() {
    let (rtf, back) = round_trip(r"\pard\sb240\sa120\li720\ri360\fi-360 Alpha\par\pard\sl360\slmult1 Bravo\par\pard\sl-280\slmult0 Charlie\par\pard\sl280\slmult0 Delta\par");
    let a = para_words(&rtf, "Alpha");
    for w in [r"\sb240", r"\sa120", r"\li720", r"\ri360", r"\fi-360"] {
        assert!(has(&a, w), "{w} missing in {a}");
    }
    assert!(has(&para_words(&rtf, "Bravo"), r"\sl360") && has(&para_words(&rtf, "Bravo"), r"\slmult1"), "{rtf}");
    assert!(has(&para_words(&rtf, "Charlie"), r"\sl-280") && has(&para_words(&rtf, "Charlie"), r"\slmult0"), "{rtf}");
    assert!(has(&para_words(&rtf, "Delta"), r"\sl280") && has(&para_words(&rtf, "Delta"), r"\slmult0"), "{rtf}");

    let p = para(&back, 0);
    assert_eq!((p.space_before, p.space_after), (Some(12.0), Some(6.0)));
    assert_eq!((p.indent_left, p.indent_right, p.indent_first), (Some(36.0), Some(18.0), Some(-18.0)));
    assert_eq!(para(&back, 1).line_spacing, Some(LineSpacing::Multiple(1.5)));
    assert_eq!(para(&back, 2).line_spacing, Some(LineSpacing::Exactly(14.0)));
    assert_eq!(para(&back, 3).line_spacing, Some(LineSpacing::AtLeast(14.0)));
}

#[test]
fn spacing_set_in_the_model_is_written() {
    let mut d = open(r"\pard Echo\par");
    let p = para_mut(&mut d, 0);
    p.props.space_before = Some(12.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.5));
    p.props.indent_left = Some(36.0);
    let w = para_words(&export(&d), "Echo");
    for x in [r"\sb240", r"\sl360", r"\slmult1", r"\li720"] {
        assert!(has(&w, x), "{x} missing in {w}");
    }
}

#[test]
fn a_paragraph_without_its_own_spacing_writes_none() {
    let rtf = export(&open(r"\pard Foxtrot\par"));
    let w = para_words(&rtf, "Foxtrot");
    for x in [r"\sb", r"\sa", r"\sl", r"\li", r"\ri", r"\fi"] {
        assert!(!w.split('\\').any(|t| t.starts_with(&x[1..]) && t[x.len() - 1..].starts_with(|c: char| c == '-' || c.is_ascii_digit())), "{x} written in {w}");
    }
}

// ---- P03-099: keep with next, keep lines together, suppress line numbers ----

#[test]
fn keep_options_are_read_and_written() {
    let (rtf, back) = round_trip(r"\pard\keepn Golf\par\pard\keep Hotel\par\pard\noline India\par\pard Juliet\par");
    let d = open(r"\pard\keepn Golf\par\pard\keep Hotel\par\pard\noline India\par\pard Juliet\par");
    for doc in [&d, &back] {
        assert_eq!(para(doc, 0).keep_next, Some(true));
        assert_eq!(para(doc, 1).keep_lines, Some(true));
        assert_eq!(para(doc, 2).suppress_line_numbers, Some(true));
        // `\pard` resets them.
        let j = para(doc, 3);
        assert_ne!(j.keep_next, Some(true));
        assert_ne!(j.keep_lines, Some(true));
        assert_ne!(j.suppress_line_numbers, Some(true));
    }
    assert!(has(&para_words(&rtf, "Golf"), r"\keepn"), "{rtf}");
    assert!(has(&para_words(&rtf, "Hotel"), r"\keep"), "{rtf}");
    assert!(has(&para_words(&rtf, "India"), r"\noline"), "{rtf}");
}

// ---- P03-100: hidden text ----

#[test]
fn hidden_text_is_kept_hidden_on_open() {
    let d = open(r"\pard x{\v hid}y\par");
    assert_eq!(text(&d, 0), "xhidy");
    assert_eq!(run(&d, "hid").hidden, Some(true));
    assert_ne!(run(&d, "x").hidden, Some(true));
}

#[test]
fn hidden_text_is_written_back() {
    let (rtf, back) = round_trip(r"\pard x{\v hid}y\par");
    assert!(rtf.contains(r"\v"), "{rtf}");
    assert_eq!(text(&back, 0), "xhidy");
    assert_eq!(run(&back, "hid").hidden, Some(true));
    assert_ne!(run(&back, "y").hidden, Some(true));
}

#[test]
fn hidden_set_in_the_model_is_written() {
    let mut d = open(r"\pard Kilo\par");
    let p = para_mut(&mut d, 0);
    for r in &mut p.runs {
        r.props.hidden = Some(true);
    }
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(text(&back, 0), "Kilo");
    assert_eq!(run(&back, "Kilo").hidden, Some(true));
}

// ---- P03-101: raised and lowered text ----

#[test]
fn up_and_dn_are_a_position_not_super_or_subscript() {
    let d = open(r"\pard x{\up6 u}{\dn4 d}y\par");
    let u = run(&d, "u");
    assert_eq!(u.position, Some(3.0));
    assert_ne!(u.vert_align, Some(VertAlign::Superscript));
    let dn = run(&d, "d");
    assert_eq!(dn.position, Some(-2.0));
    assert_ne!(dn.vert_align, Some(VertAlign::Subscript));
    assert!(run(&d, "y").position.is_none_or(|p| p == 0.0));
}

#[test]
fn position_is_written_as_up_and_dn() {
    let (rtf, back) = round_trip(r"\pard x{\up6 u}{\dn4 d}y\par");
    assert!(has(&rtf, r"\up6"), "{rtf}");
    assert!(has(&rtf, r"\dn4"), "{rtf}");
    assert_eq!(run(&back, "u").position, Some(3.0));
    assert_eq!(run(&back, "d").position, Some(-2.0));
}

#[test]
fn super_and_sub_stay_super_and_sub() {
    let d = open(r"\pard x{\super s}{\sub b}y\par");
    assert_eq!(run(&d, "s").vert_align, Some(VertAlign::Superscript));
    assert_eq!(run(&d, "b").vert_align, Some(VertAlign::Subscript));
}
