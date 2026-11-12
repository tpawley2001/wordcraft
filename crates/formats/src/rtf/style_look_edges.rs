//! A paragraph in one of the document's own paragraph styles states the style's formatting on
//! save: line spacing and keeps, a character style's "off" inside it, and nothing changes for
//! the other formats' flow.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::Align;

fn open(stylesheet: &str, body: &str) -> Document {
    let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}{\colortbl;\red255\green0\blue0;}";
    import(format!(r"{head}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}").as_bytes()).unwrap()
}

fn line<'a>(rtf: &'a str, text: &str) -> &'a str {
    rtf.lines().find(|l| l.starts_with(r"\pard") && l.contains(text)).unwrap()
}

#[test]
fn line_spacing_and_keeps_from_the_style_are_stated() {
    let d = open(r"{\s15\sbasedon0\snext15\sl480\slmult1\keep Airy;}", r"\pard\plain\s15 Spread\par");
    let rtf = export(&d);
    let l = line(&rtf, "Spread");
    for word in [r"\sl480\slmult1", r"\keep"] {
        assert_eq!(l.matches(word).count(), 1, "{word} once in {l}");
    }
}

#[test]
fn a_character_style_turning_bold_off_says_so_in_a_bold_paragraph_style() {
    let d = open(r"{\s15\sbasedon0\snext15\b Loud;}{\*\cs20\additive\b0 Calm;}", r"\pard\plain\s15 Loud {\cs20\b0 calm} text\par");
    let rtf = export(&d);
    let l = line(&rtf, "calm");
    let (b0, calm) = (l.find(r"\b0").unwrap(), l.find("calm").unwrap());
    assert!(b0 < calm && l.find(r"\b\").or(l.find(r"\b ")).unwrap() < b0, "{l}");
}

#[test]
fn other_formats_flow_keeps_only_direct_values() {
    let d = open(r"{\s15\sbasedon0\snext15\qc\sb240\b Loud;}", r"\pard\plain\s15 Shouted\par");
    let flow = crate::model::from_doc(&d);
    let Some(crate::model::FBlock::Para(p)) = flow.blocks.first() else { panic!("a paragraph") };
    assert_eq!((p.align, p.spacing.before), (None, None));
    assert_ne!(p.align, Some(Align::Center));
}
