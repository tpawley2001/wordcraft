//! A paragraph's outline level apart from a heading style (`\outlinelevelN`), the paragraph
//! border's distance from the text (`\brspN`) and the bar border (`\brdrbar`), RTF 1.9.1: Word
//! keeps the paragraph's own style and formatting and only gives it the outline level; the
//! border space is read for each side and written back; a bar border, which has no model value,
//! changes no other side.

use super::*;
use wordcraft_doc::props::{Border, BorderStyle, Borders};
use wordcraft_doc::{Document, Paragraph};

const STYLES: &str = r"{\stylesheet{\s0 Normal;}{\s1\sbasedon0\snext0\outlinelevel0\b\fs32 heading 1;}{\s15\sbasedon0\snext15 Chapter;}}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{STYLES}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

/// The paragraph's own outline level, with "none" and body text (9) as `None`.
fn level(p: &Paragraph) -> Option<u8> {
    p.props.outline_level.filter(|l| *l < 9)
}

/// The paragraph's style id, with no style and `Normal` as `None`.
fn style(p: &Paragraph) -> Option<&str> {
    p.props.style.as_deref().filter(|s| *s != "Normal")
}

fn any_bold(p: &Paragraph) -> bool {
    p.run_ranges().any(|(_, c)| c.bold == Some(true))
}

fn any_italic(p: &Paragraph) -> bool {
    p.run_ranges().any(|(_, c)| c.italic == Some(true))
}

fn borders(d: &Document, i: usize) -> Borders {
    paras(d).get(i).and_then(|p| p.props.borders).unwrap_or_default()
}

fn single(space: f32) -> Option<Border> {
    Some(Border { style: BorderStyle::Single, width: 0.5, color: None, space })
}

const OUTLINE_BODY: &str = r"\pard\plain\f0\fs24 Plain one.\par\pard\plain\outlinelevel0\f0\fs24 Level one.\par\pard\plain\outlinelevel2\i\f0\fs24 Level three.\par\pard\plain\f0\fs24 Plain two.\par";

#[test]
fn an_outline_level_alone_keeps_the_paragraph_normal_and_unbolded() {
    let d = open(OUTLINE_BODY);
    let p = paras(&d);
    assert_eq!(p.len(), 4);
    assert_eq!(p.iter().map(|p| level(p)).collect::<Vec<_>>(), [None, Some(0), Some(2), None]);
    assert_eq!(p.iter().map(|p| style(p)).collect::<Vec<_>>(), [None, None, None, None], "no Heading style");
    assert!(p.iter().all(|p| !any_bold(p)), "Word shows no bold text in this file");
    assert!(any_italic(p[2]), "the paragraph's own formatting stays");
    assert!(!any_italic(p[1]));
}

#[test]
fn an_outline_level_keeps_the_paragraph_spacing_of_normal() {
    let d = open(OUTLINE_BODY);
    let p = paras(&d);
    let plain = d.styles.resolve_para(&p[0].props);
    let lvl = d.styles.resolve_para(&p[1].props);
    assert_eq!((lvl.space_before, lvl.space_after, lvl.keep_next), (plain.space_before, plain.space_after, plain.keep_next));
}

#[test]
fn an_outline_level_on_a_custom_style_keeps_the_style() {
    let d = open(r"\pard\plain\s15\outlinelevel1\f0\fs24 Chapter text.\par\pard\plain\f0\fs24 After.\par");
    let p = paras(&d);
    assert_eq!(style(p[0]), Some("Chapter"));
    assert_eq!(level(p[0]), Some(1));
    assert!(!any_bold(p[0]));
    assert_eq!((style(p[1]), level(p[1])), (None, None));
}

#[test]
fn a_heading_style_is_still_a_heading() {
    let d = open(r"\pard\plain\s1\outlinelevel0\b\f0\fs32 Title.\par\pard\plain\f0\fs24 After.\par");
    let p = paras(&d);
    assert_eq!(style(p[0]), Some("Heading1"));
    assert_eq!(d.styles.resolve_para(&p[0].props).outline_level, Some(0));
    assert_eq!((style(p[1]), level(p[1])), (None, None));
}

#[test]
fn outline_level_nine_is_body_text() {
    let d = open(r"\pard\plain\outlinelevel9\f0\fs24 Body.\par");
    let p = paras(&d);
    assert_eq!((style(p[0]), level(p[0])), (None, None));
    assert!(!any_bold(p[0]));
}

#[test]
fn an_outline_level_round_trips_without_becoming_a_heading() {
    let d = open(OUTLINE_BODY);
    let out = export(&d);
    assert!(out.contains(r"\outlinelevel0"), "{out}");
    assert!(out.contains(r"\outlinelevel2"), "{out}");
    let back = import(out.as_bytes()).unwrap();
    let p = paras(&back);
    assert_eq!(p.len(), 4);
    assert_eq!(p.iter().map(|p| level(p)).collect::<Vec<_>>(), [None, Some(0), Some(2), None]);
    assert_eq!(p.iter().map(|p| style(p)).collect::<Vec<_>>(), [None, None, None, None], "{out}");
    assert!(p.iter().all(|p| !any_bold(p)), "{out}");
    assert!(any_italic(p[2]));
}

#[test]
fn a_custom_style_with_an_outline_level_round_trips() {
    let d = open(r"\pard\plain\s15\outlinelevel1\f0\fs24 Chapter text.\par\pard\plain\s1\outlinelevel0\b\f0\fs32 Title.\par");
    let back = import(export(&d).as_bytes()).unwrap();
    let p = paras(&back);
    assert_eq!((style(p[0]), level(p[0])), (Some("Chapter"), Some(1)));
    assert_eq!(style(p[1]), Some("Heading1"));
}

#[test]
fn each_side_keeps_its_space_from_the_text() {
    let d = open(
        r"\pard\plain\brdrt\brdrs\brdrw10\brsp20\brdrb\brdrs\brdrw10\brsp80\brdrl\brdrs\brdrw10\brsp60\brdrr\brdrs\brdrw10\brsp40\f0\fs24 Boxed.\par",
    );
    let b = borders(&d, 0);
    assert_eq!((b.top, b.bottom, b.left, b.right), (single(1.0), single(4.0), single(3.0), single(2.0)));
    assert_eq!(b.between, None);
}

#[test]
fn box_and_between_borders_keep_their_space() {
    let d = open(r"\pard\plain\box\brdrs\brdrw10\brsp100\f0\fs24 All four.\par\pard\plain\brdrbtw\brdrs\brdrw10\brsp60\f0\fs24 Between.\par");
    let b = borders(&d, 0);
    assert_eq!((b.top, b.left, b.bottom, b.right), (single(5.0), single(5.0), single(5.0), single(5.0)));
    assert_eq!(borders(&d, 1).between, single(3.0));
}

#[test]
fn border_space_is_clamped_and_reset_by_pard() {
    let d = open(
        r"\pard\plain\brdrt\brdrs\brdrw10\brsp99999\f0\fs24 Far.\par\pard\plain\brdrt\brdrs\brdrw10\brsp-40\f0\fs24 Negative.\par\pard\plain\brdrt\brdrs\brdrw10\f0\fs24 None.\par",
    );
    assert_eq!(borders(&d, 0).top, single(31.0));
    assert_eq!(borders(&d, 1).top, single(0.0));
    assert_eq!(borders(&d, 2).top, single(0.0));
}

#[test]
fn border_space_round_trips() {
    let d = open(r"\pard\plain\brdrt\brdrs\brdrw10\brsp20\brdrb\brdrs\brdrw10\brsp80\f0\fs24 Boxed.\par\pard\plain\f0\fs24 Plain.\par");
    let out = export(&d);
    assert!(out.contains(r"\brsp20") && out.contains(r"\brsp80"), "{out}");
    let back = import(out.as_bytes()).unwrap();
    let b = borders(&back, 0);
    assert_eq!((b.top, b.bottom), (single(1.0), single(4.0)));
    assert_eq!(borders(&back, 1), Borders::default());
}

#[test]
fn a_bar_border_changes_no_other_side() {
    let d = open(r"\pard\plain\brdrt\brdrs\brdrw10\brdrbar\brdrdb\brdrw30\brsp60\f0\fs24 Top and bar.\par");
    let b = borders(&d, 0);
    assert_eq!(b.top, single(0.0));
    assert_eq!((b.left, b.bottom, b.right, b.between), (None, None, None, None));
}

#[test]
fn a_bar_border_alone_leaves_the_paragraph_without_borders() {
    let d = open(r"\pard\plain\brdrbar\brdrs\brdrw20\f0\fs24 Bar only.\par\pard\plain\f0\fs24 After.\par");
    assert_eq!(borders(&d, 0), Borders::default());
    let p = paras(&d);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].text, "Bar only.");
}

#[test]
fn a_page_border_space_is_unchanged() {
    let d = open(r"\sectd\pgbrdrt\brdrs\brdrw10\brsp480\pard\plain\brdrt\brdrs\brdrw10\brsp20\f0\fs24 Both.\par");
    assert_eq!(borders(&d, 0).top, single(1.0));
    let page_top = d.last_section.page_borders.as_ref().and_then(|b| b.top);
    assert_eq!(page_top, single(24.0));
}
