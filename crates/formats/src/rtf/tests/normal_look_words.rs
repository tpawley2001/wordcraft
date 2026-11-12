//! RTF save: text states what it inherits from Normal and the document defaults, and the built-in
//! styles' entries and lines come from the document's definitions. After `\pard\plain` text has
//! the default font at 12 pt and nothing else (RTF 1.9.1, "Style Sheet"), so a paragraph line
//! states its resolved look once and its runs state what differs, in their own groups.

use super::*;
use std::sync::Arc;
use wordcraft_doc::props::{CharProps, Underline};
use wordcraft_doc::{Block, Document, Rgb, TextColor};

/// A Normal paragraph (with a bold run "upright"), a Heading 1 paragraph and a Heading 3 one.
fn open() -> Document {
    let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}";
    let sheet = r"{\stylesheet{\s0 Normal;}{\s1\sbasedon0 heading 1;}{\s3\sbasedon0 heading 3;}}";
    let body = r"\pard\plain Plain {\b upright} after\par\pard\plain\s1 Head text\par\pard\plain\s3 Third text\par";
    import(format!("{head}{sheet}{body}}}").as_bytes()).unwrap()
}

fn restyle(d: &mut Document, id: &str, f: impl FnOnce(&mut CharProps)) {
    let styles = Arc::make_mut(&mut d.styles);
    let mut st = styles.get(id).cloned().unwrap();
    f(&mut st.chr);
    styles.upsert(st);
}

/// The body line holding `text`, up to the text.
fn line<'a>(rtf: &'a str, text: &str) -> &'a str {
    let l = rtf.lines().find(|l| l.starts_with(r"\pard") && l.contains(text)).unwrap();
    &l[..l.find(text).unwrap()]
}

/// The stylesheet entry named `name`, from its `{` to the name.
fn entry<'a>(rtf: &'a str, name: &str) -> &'a str {
    let sheet = &rtf[rtf.find(r"{\stylesheet").unwrap()..];
    let end = sheet.find(&format!(" {name};}}")).unwrap();
    &sheet[sheet[..end].rfind('{').unwrap()..end]
}

/// The control words of `s`, without backslashes.
fn words(s: &str) -> Vec<&str> {
    s.split('\\').skip(1).map(|w| w.split(|c: char| !c.is_ascii_alphanumeric() && c != '-').next().unwrap_or("")).filter(|w| !w.is_empty()).collect()
}

/// The `\fN` word of font `name`.
fn font_word(rtf: &str, name: &str) -> String {
    let table = &rtf[rtf.find(r"{\fonttbl").unwrap()..rtf.find(r"{\colortbl").unwrap()];
    let at = table.find(&format!(" {name};}}")).unwrap();
    words(&table[table[..at].rfind('{').unwrap()..at]).into_iter().find(|w| w.starts_with('f') && w[1..].parse::<u32>().is_ok()).unwrap().to_string()
}

#[test]
fn a_heading_states_the_font_and_italic_it_inherits_from_normal() {
    let mut d = open();
    restyle(&mut d, "Normal", |c| {
        c.font = Some("Arial".into());
        c.italic = Some(true);
    });
    // Heading 1 sets no font of its own here; Heading 3 turns italic off.
    restyle(&mut d, "Heading1", |c| c.font = None);
    restyle(&mut d, "Heading3", |c| c.italic = Some(false));
    let rtf = export(&d);
    let f = font_word(&rtf, "Arial");
    let head = words(line(&rtf, "Head text"));
    assert!(head.contains(&f.as_str()) && head.contains(&"i"), "{head:?}");
    let third = words(line(&rtf, "Third text"));
    assert!(!third.contains(&"i"), "Heading 3 is upright: {third:?}");
}

#[test]
fn a_run_with_italic_off_in_italic_normal_is_written_with_i0() {
    let mut d = open();
    restyle(&mut d, "Normal", |c| c.italic = Some(true));
    let Block::Para(p) = Arc::make_mut(&mut d.body[0]) else { panic!("a paragraph first") };
    for r in &mut p.runs {
        if r.props.bold == Some(true) {
            r.props.bold = None;
            r.props.italic = Some(false);
        }
    }
    let rtf = export(&d);
    assert!(words(line(&rtf, "Plain")).contains(&"i"), "the line is italic: {rtf}");
    let l = rtf.lines().find(|l| l.contains("upright")).unwrap();
    assert!(l.contains(r"{\i0 upright}"), "{l}");
    // Reopened, the run is upright and the rest italic.
    let back = import(rtf.as_bytes()).unwrap();
    let q = back.body[0].as_para().unwrap();
    let mut at = 0;
    for r in &q.runs {
        let text = q.text.get(at..at + r.len).unwrap_or("");
        at += r.len;
        let italic = back.styles.resolve_char(q.props.style.as_deref(), &r.props).italic;
        assert_eq!(italic, !text.contains("upright"), "{text:?}");
    }
}

#[test]
fn a_built_in_style_the_document_does_not_define_keeps_its_fixed_entry() {
    let mut d = open();
    Arc::make_mut(&mut d.styles).styles.retain(|s| s.id != "Heading3");
    let rtf = export(&d);
    assert_eq!(entry(&rtf, "heading 3"), r"{\s3\sbasedon0\snext0\outlinelevel2\keepn\b\fs28");
}

#[test]
fn normals_colour_bold_lines_and_caps_are_in_its_entry_and_on_its_lines() {
    let mut d = open();
    restyle(&mut d, "Normal", |c| {
        c.color = Some(TextColor::Rgb(Rgb(0xC0, 0x10, 0x20)));
        c.bold = Some(true);
        c.underline = Some(Underline::Single);
        c.strike = Some(true);
        c.small_caps = Some(true);
        c.caps = Some(true);
    });
    let rtf = export(&d);
    let table = &rtf[rtf.find(r"{\colortbl").unwrap()..];
    let table = &table[..table.find('}').unwrap()];
    let n = table.split(';').position(|c| c == r"\red192\green16\blue32").expect("the colour is in the table");
    let cf = format!("cf{n}");
    for (what, got) in [("Normal's entry", words(entry(&rtf, "Normal"))), ("a Normal line", words(line(&rtf, "Plain")))] {
        for w in [cf.as_str(), "b", "ul", "strike", "scaps", "caps"] {
            assert!(got.contains(&w), "{what} has \\{w}: {got:?}");
        }
    }
}

#[test]
fn an_untouched_document_looks_the_same_after_save_and_reopen() {
    let d = open();
    let back = import(export(&d).as_bytes()).unwrap();
    for i in 0..3 {
        let (p, q) = (d.body[i].as_para().unwrap(), back.body[i].as_para().unwrap());
        let want = d.styles.resolve_char(p.props.style.as_deref(), &p.runs[0].props);
        let got = back.styles.resolve_char(q.props.style.as_deref(), &q.runs[0].props);
        assert_eq!((got.font, got.size, got.bold, got.italic, got.color), (want.font, want.size, want.bold, want.italic, want.color), "{}", p.text);
    }
}
