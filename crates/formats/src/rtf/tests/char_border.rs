//! A character border (`\chbrdr` followed by the border words: a line style, `\brdrwN`,
//! `\brspN`, `\brdrcfN`), RTF 1.9.1, the RTF form of DOCX `w:rPr/w:bdr`: read into the run's
//! border and written back; it never changes a paragraph border, `\plain` ends it and `\pard`
//! leaves it alone.

use super::*;
use wordcraft_doc::props::{Border, BorderStyle, Borders, Rgb};
use wordcraft_doc::{Document, Paragraph};

const HEAD: &str = r"{\fonttbl{\f0 Times New Roman;}}{\colortbl;\red255\green0\blue0;}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{HEAD}\pard\plain\f0\fs24 {body}\par}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

/// The character border of the run holding the first character of `needle`, looked for in every
/// body paragraph. Panics when no paragraph has the text.
fn border_at(d: &Document, needle: &str) -> Option<Border> {
    for p in paras(d) {
        let Some(byte) = p.text.find(needle) else { continue };
        let at = p.text[..byte].chars().count();
        return p.run_ranges().find(|(r, _)| r.contains(&at)).and_then(|(_, c)| c.border);
    }
    panic!("no paragraph holds {needle:?}");
}

fn para_borders(d: &Document, i: usize) -> Borders {
    paras(d).get(i).and_then(|p| p.props.borders).unwrap_or_default()
}

fn line(style: BorderStyle, width: f32, color: Option<Rgb>, space: f32) -> Option<Border> {
    Some(Border { style, width, color, space })
}

fn single() -> Option<Border> {
    line(BorderStyle::Single, 0.5, None, 0.0)
}

#[test]
fn a_character_border_is_read_on_its_run_only() {
    let d = open(r"Plain {\chbrdr\brdrs\brdrw10 boxed} after.");
    assert_eq!(paras(&d)[0].text, "Plain boxed after.");
    assert_eq!(border_at(&d, "boxed"), single());
    assert_eq!(border_at(&d, "Plain"), None);
    assert_eq!(border_at(&d, "after"), None);
    assert_eq!(para_borders(&d, 0), Borders::default(), "no paragraph border comes of it");
}

#[test]
fn style_width_colour_and_space_are_read() {
    let d = open(r"{\chbrdr\brdrdb\brdrw30\brdrcf1 red} {\chbrdr\brdrs\brdrw10\brsp20 spaced} {\chbrdr\brdrdot unsized}");
    assert_eq!(border_at(&d, "red"), line(BorderStyle::Double, 1.5, Some(Rgb(255, 0, 0)), 0.0));
    assert_eq!(border_at(&d, "spaced"), line(BorderStyle::Single, 0.5, None, 1.0));
    assert_eq!(border_at(&d, "unsized"), line(BorderStyle::Dotted, 0.5, None, 0.0));
}

#[test]
fn colour_zero_is_automatic_and_no_line_is_no_border() {
    let d = open(r"{\chbrdr\brdrs\brdrw10\brdrcf0 auto} {\chbrdr\brdrnone none} {\chbrdr\brdrs\brdrw10\chbrdr\brdrnone off}");
    assert_eq!(border_at(&d, "auto"), single());
    assert_eq!(border_at(&d, "none"), None);
    assert_eq!(border_at(&d, "off"), None);
}

#[test]
fn plain_ends_it_and_pard_does_not() {
    let d = open(r"\chbrdr\brdrs\brdrw10 boxed \plain\f0\fs24 not.");
    assert_eq!(border_at(&d, "boxed"), single());
    assert_eq!(border_at(&d, "not."), None);

    let d = open(r"\chbrdr\brdrs\brdrw10 One.\par\pard Two.");
    assert_eq!(paras(&d).len(), 2);
    assert_eq!(border_at(&d, "One."), single());
    assert_eq!(border_at(&d, "Two."), single(), "a character border is character formatting");
    assert_eq!(para_borders(&d, 1), Borders::default());
}

#[test]
fn border_words_after_plain_do_not_reach_the_text() {
    let d = open(r"\chbrdr\plain\f0\fs24\brdrs\brdrw10 Text.");
    assert_eq!(border_at(&d, "Text."), None);
}

#[test]
fn paragraph_and_character_borders_stay_apart() {
    let d = open(r"\brdrt\brdrs\brdrw10\chbrdr\brdrdb\brdrw30 Text.");
    assert_eq!(border_at(&d, "Text."), line(BorderStyle::Double, 1.5, None, 0.0));
    let b = para_borders(&d, 0);
    assert_eq!(b.top, single());
    assert_eq!((b.left, b.bottom, b.right, b.between), (None, None, None, None));

    let d = open(r"\chbrdr\brdrs\brdrw10\brdrb\brdrdb\brdrw30\brsp40 Text.");
    assert_eq!(border_at(&d, "Text."), single());
    let b = para_borders(&d, 0);
    assert_eq!(b.bottom, line(BorderStyle::Double, 1.5, None, 2.0));
    assert_eq!((b.top, b.left, b.right, b.between), (None, None, None, None));
}

#[test]
fn a_character_border_round_trips() {
    let d = open(r"Plain {\chbrdr\brdrdb\brdrw30\brsp20\brdrcf1 boxed} after.");
    let out = export(&d);
    assert_eq!(out.matches(r"\chbrdr").count(), 1, "{out}");
    assert!(out.contains(r"\chbrdr\brdrdb\brdrw30\brsp20\brdrcf"), "{out}");
    let back = import(out.as_bytes()).unwrap();
    assert_eq!(border_at(&back, "boxed"), line(BorderStyle::Double, 1.5, Some(Rgb(255, 0, 0)), 1.0));
    assert_eq!(border_at(&back, "Plain"), None);
    assert_eq!(border_at(&back, "after"), None);
    assert_eq!(para_borders(&back, 0), Borders::default());
}

#[test]
fn text_without_a_border_writes_no_character_border() {
    let d = open(r"\brdrt\brdrs\brdrw10 Plain text.");
    assert!(!export(&d).contains(r"\chbrdr"));
}
