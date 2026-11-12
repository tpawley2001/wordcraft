//! Paragraph shading (`\cbpatN`, `\cfpatN`, `\shadingN`) and the between and box paragraph
//! borders (`\brdrbtw`, `\box`), RTF 1.9.1: read into the paragraph's own properties, reset by
//! `\pard`, and written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{Border, BorderStyle, Rgb};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn open(body: &str) -> Document {
    import(
        format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}}{body}}}").as_bytes(),
    )
    .unwrap()
}

fn shading(d: &Document) -> Vec<Option<Rgb>> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.shading).collect()
}

fn borders(d: &Document, i: usize) -> wordcraft_doc::props::Borders {
    d.body.iter().filter_map(|b| b.as_para()).nth(i).and_then(|p| p.props.borders).unwrap_or_default()
}

fn single(width: f32, color: Option<Rgb>) -> Option<Border> {
    Some(Border { style: BorderStyle::Single, width, color, space: 0.0 })
}

#[test]
fn background_pattern_colour_is_the_paragraph_shading() {
    let d = open(r"\pard\cbpat1 red\par\pard\cbpat0 auto\par\pard plain\par\pard\cbpat99 out of range\par");
    assert_eq!(shading(&d), [Some(RED), None, None, None]);
}

#[test]
fn solid_shading_shows_the_pattern_colour_other_percentages_the_blend() {
    let d = open(r"\pard\cfpat2\cbpat1\shading10000 solid\par\pard\cfpat2\cbpat1\shading2500 quarter\par\pard\cfpat2\shading10000 solid only\par");
    // 25 % blue over red, as Word paints it.
    assert_eq!(shading(&d), [Some(BLUE), Some(Rgb(191, 0, 64)), Some(BLUE)]);
}

#[test]
fn shading_carries_to_the_next_paragraph_until_pard() {
    let d = open(r"\pard\cbpat2 one\par two\par\pard three\par");
    assert_eq!(shading(&d), [Some(BLUE), Some(BLUE), None]);
}

#[test]
fn between_border_is_read() {
    let d = open(r"\pard\brdrbtw\brdrs\brdrw10\brdrcf1 a\par b\par");
    let b = borders(&d, 0);
    assert_eq!(b.between, single(0.5, Some(RED)));
    assert_eq!((b.top, b.bottom), (None, None));
}

#[test]
fn box_sets_all_four_sides() {
    let d = open(r"\pard\box\brdrs\brdrw20\brdrcf2 boxed\par");
    let b = borders(&d, 0);
    let want = single(1.0, Some(BLUE));
    assert_eq!((b.top, b.left, b.bottom, b.right), (want, want, want, want));
    assert_eq!(b.between, None);
}

#[test]
fn shading_and_between_border_round_trip() {
    let d = open(r"\pard\cbpat1\brdrbtw\brdrs\brdrw10\brdrcf2 a\par\pard plain\par");
    let out = export(&d);
    assert!(out.contains(r"\brdrbtw"), "{out}");
    let back = import(out.as_bytes()).unwrap();
    assert_eq!(shading(&back), [Some(RED), None]);
    assert_eq!(borders(&back, 0).between, single(0.5, Some(BLUE)));
}
