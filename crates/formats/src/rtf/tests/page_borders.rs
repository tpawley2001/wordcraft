//! Section page borders: `\pgbrdrt` / `\pgbrdrl` / `\pgbrdrb` / `\pgbrdrr` followed by the usual border
//! words (`\brdrs`, `\brdrwN`, `\brdrcfN`, `\brspN`), RTF 1.9.1 section formatting; the RTF form of DOCX
//! `w:pgBorders`.

use super::*;
use wordcraft_doc::props::{Border, BorderStyle, Borders, Rgb};
use wordcraft_doc::{Block, SectionProps};

fn page_borders(rtf: &str) -> Option<Borders> {
    import(rtf.as_bytes()).unwrap().last_section.page_borders
}

fn border(style: BorderStyle, width: f32, color: Option<Rgb>, space: f32) -> Border {
    Border { style, width, color, space }
}

#[test]
fn top_and_bottom_page_borders_are_read() {
    let b = page_borders(r"{\rtf1\ansi\sectd\pgbrdrt\brdrs\brdrw20\brsp480\pgbrdrb\brdrdb\brdrw10\brsp480 a\par}").expect("page borders");
    assert_eq!(b.top, Some(border(BorderStyle::Single, 1.0, None, 24.0)));
    assert_eq!(b.bottom, Some(border(BorderStyle::Double, 0.5, None, 24.0)));
    assert_eq!((b.left, b.right), (None, None));
}

#[test]
fn a_page_border_colour_comes_from_the_colour_table() {
    let b = page_borders(r"{\rtf1\ansi{\colortbl;\red255\green0\blue0;}\sectd\pgbrdrl\brdrs\brdrw15\brdrcf1 a\par}").expect("page borders");
    assert_eq!(b.left, Some(border(BorderStyle::Single, 0.75, Some(Rgb(255, 0, 0)), 0.0)));
    assert_eq!((b.top, b.bottom, b.right), (None, None, None));
}

#[test]
fn page_border_options_do_not_disturb_the_sides() {
    let b = page_borders(r"{\rtf1\ansi\sectd\pgbrdropt32\pgbrdrhead\pgbrdrfoot\pgbrdrr\brdrth\brdrw30\brsp80 a\par}").expect("page borders");
    assert_eq!(b.right, Some(border(BorderStyle::Thick, 1.5, None, 4.0)));
}

#[test]
fn page_border_words_do_not_border_the_paragraph() {
    let d = import(br"{\rtf1\ansi\sectd\pgbrdrt\brdrs\brdrw20\pgbrdrb\brdrs\brdrw20 a\par}").unwrap();
    let p = d
        .body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => Some(p),
            _ => None,
        })
        .expect("paragraph");
    assert_eq!(p.plain_text(), "a");
    assert!(p.props.borders.is_none(), "{:?}", p.props.borders);
}

#[test]
fn sectd_resets_the_page_borders_for_the_next_section() {
    let d = import(br"{\rtf1\ansi\sectd\pgbrdrt\brdrs\brdrw20 a\sect\sectd b\par}").unwrap();
    let first = d
        .body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref(),
            _ => None,
        })
        .expect("first section");
    let top = first.page_borders.as_ref().and_then(|b| b.top);
    assert_eq!(top, Some(border(BorderStyle::Single, 1.0, None, 0.0)));
    assert!(d.last_section.page_borders.is_none());
}

#[test]
fn export_writes_the_page_borders_and_they_round_trip() {
    let mut d = Document::from_text("one");
    let top = border(BorderStyle::Single, 1.0, Some(Rgb(255, 0, 0)), 24.0);
    d.last_section = SectionProps { page_borders: Some(Borders { top: Some(top), ..Default::default() }), ..Default::default() };
    let rtf = export(&d);
    assert!(rtf.contains("\\pgbrdrt\\brdrs\\brdrw20\\brsp480"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap().last_section.page_borders.expect("page borders back");
    assert_eq!(back.top, Some(top));
    assert_eq!((back.left, back.bottom, back.right), (None, None, None));
}

#[test]
fn export_writes_no_page_borders_when_none_are_set() {
    let rtf = export(&Document::from_text("one"));
    assert!(!rtf.contains("\\pgbrdr"), "{rtf}");
}
