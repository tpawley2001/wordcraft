//! RTF save states the character formatting text gets from the Normal style, the document
//! defaults and the built-in heading styles: RTF has no inheritance once a paragraph says
//! `\pard\plain`, so the stylesheet entry defines the style and each paragraph line repeats it.

use super::*;
use std::sync::Arc;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

/// A Normal paragraph, a Heading 1 paragraph and one in the document's own style Mine (based on
/// Normal, underlined).
fn open() -> Document {
    let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}";
    let sheet = r"{\stylesheet{\s0 Normal;}{\s1\sbasedon0 heading 1;}{\s15\sbasedon0\ul Mine;}}";
    let body = r"\pard\plain Plain text\par\pard\plain\s1 Head text\par\pard\plain\s15\ul Mine text\par";
    import(format!("{head}{sheet}{body}}}").as_bytes()).unwrap()
}

/// Changes the character formatting of the document's style `id`.
fn restyle(d: &mut Document, id: &str, f: impl FnOnce(&mut CharProps)) {
    let styles = Arc::make_mut(&mut d.styles);
    let mut st = styles.get(id).cloned().unwrap_or_else(|| panic!("style {id}"));
    f(&mut st.chr);
    styles.upsert(st);
}

fn arial_14_italic(c: &mut CharProps) {
    c.font = Some("Arial".into());
    c.size = Some(14.0);
    c.italic = Some(true);
}

/// The body line that holds `text`, up to the text: the words the paragraph states before it.
fn line<'a>(rtf: &'a str, text: &str) -> &'a str {
    let l = rtf.lines().find(|l| l.starts_with(r"\pard") && l.contains(text)).unwrap_or_else(|| panic!("no line with {text}"));
    &l[..l.find(text).unwrap()]
}

/// The stylesheet entry whose name is `name`: from its `{` to the name.
fn entry<'a>(rtf: &'a str, name: &str) -> &'a str {
    let sheet = &rtf[rtf.find(r"{\stylesheet").expect("a stylesheet")..];
    let end = sheet.find(&format!(" {name};}}")).unwrap_or_else(|| panic!("no entry {name}"));
    &sheet[sheet[..end].rfind('{').unwrap()..end]
}

/// The control words of `s`, without their backslashes: `\b\fs40 x` gives `b`, `fs40`.
fn words(s: &str) -> Vec<&str> {
    s.split('\\').skip(1).map(|w| w.split(|c: char| !c.is_ascii_alphanumeric() && c != '-').next().unwrap_or("")).filter(|w| !w.is_empty()).collect()
}

/// The `\fN` word of the font named `name` in the font table.
fn font_word(rtf: &str, name: &str) -> String {
    let table = &rtf[rtf.find(r"{\fonttbl").expect("a font table")..rtf.find(r"{\colortbl").unwrap_or(rtf.len())];
    let at = table.find(&format!(" {name};}}")).unwrap_or_else(|| panic!("{name} is not in the font table: {table}"));
    let item = &table[table[..at].rfind('{').unwrap()..at];
    words(item)
        .into_iter()
        .find(|w| w.starts_with('f') && w[1..].chars().all(|c| c.is_ascii_digit()) && w.len() > 1)
        .expect("a font number")
        .to_string()
}

#[test]
fn normal_styles_own_font_size_and_italic_are_in_its_entry_and_on_its_paragraphs() {
    let mut d = open();
    restyle(&mut d, "Normal", arial_14_italic);
    let rtf = export(&d);
    let f = font_word(&rtf, "Arial");
    let normal = words(entry(&rtf, "Normal"));
    for w in [f.as_str(), "fs28", "i"] {
        assert!(normal.contains(&w), "Normal's entry has \\{w}: {normal:?}");
    }
    let plain = words(line(&rtf, "Plain text"));
    for w in [f.as_str(), "fs28", "i"] {
        assert!(plain.contains(&w), "the Normal paragraph states \\{w}: {plain:?}");
    }
}

#[test]
fn a_paragraph_in_a_style_based_on_normal_states_what_it_inherits() {
    let mut d = open();
    restyle(&mut d, "Normal", arial_14_italic);
    let rtf = export(&d);
    let f = font_word(&rtf, "Arial");
    let mine = words(line(&rtf, "Mine text"));
    for w in [f.as_str(), "fs28", "i", "ul"] {
        assert!(mine.contains(&w), "the Mine paragraph states \\{w}: {mine:?}");
    }
}

#[test]
fn a_document_default_size_other_than_12_pt_is_stated_on_the_paragraph() {
    let mut d = open();
    Arc::make_mut(&mut d.styles).default_chr.size = Some(10.0);
    let p = d.body[0].as_para().unwrap();
    assert_eq!(d.styles.resolve_char(p.props.style.as_deref(), &CharProps::default()).size, 10.0, "the fixture's text is 10 pt");
    let rtf = export(&d);
    let plain = words(line(&rtf, "Plain text"));
    assert!(plain.contains(&"fs20"), "the Normal paragraph states \\fs20: {plain:?}");
    let mine = words(line(&rtf, "Mine text"));
    assert!(mine.contains(&"fs20"), "the Mine paragraph states \\fs20: {mine:?}");
}

#[test]
fn a_redefined_heading_1_is_written_from_the_documents_definition() {
    let mut d = open();
    restyle(&mut d, "Heading1", |c| {
        c.size = Some(13.0);
        c.bold = Some(false);
    });
    let p = d.body[1].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Heading1"));
    let look = d.styles.resolve_char(p.props.style.as_deref(), &CharProps::default());
    assert_eq!((look.size, look.bold), (13.0, false), "the fixture's heading is 13 pt, not bold");
    let rtf = export(&d);
    for (what, got) in [("heading 1's entry", words(entry(&rtf, "heading 1"))), ("the Heading 1 paragraph", words(line(&rtf, "Head text")))] {
        assert!(got.contains(&"fs26"), "{what} has \\fs26: {got:?}");
        assert!(!got.contains(&"fs40") && !got.contains(&"b"), "{what} has no \\fs40 and no \\b: {got:?}");
    }
}

#[test]
fn text_looks_the_same_after_save_and_reopen() {
    let mut d = open();
    restyle(&mut d, "Normal", arial_14_italic);
    let back = import(export(&d).as_bytes()).unwrap();
    for (n, text) in ["Plain text", "Mine text"].into_iter().enumerate() {
        let i = if n == 0 { 0 } else { 2 };
        let (p, q) = (d.body[i].as_para().unwrap(), back.body[i].as_para().unwrap());
        let want = d.styles.resolve_char(p.props.style.as_deref(), &p.runs[0].props);
        let got = back.styles.resolve_char(q.props.style.as_deref(), &q.runs[0].props);
        assert_eq!((got.font.as_str(), got.size, got.italic), (want.font.as_str(), want.size, want.italic), "{text}");
        assert_eq!((want.font.as_str(), want.size, want.italic), ("Arial", 14.0, true), "{text}: the fixture");
    }
}
