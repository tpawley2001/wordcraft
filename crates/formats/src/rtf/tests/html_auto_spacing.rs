//! `\htmautsp` (RTF 1.9.1 document formatting, "Use HTML paragraph auto spacing"): Word writes it in
//! every RTF it saves; with it the space between two paragraphs is the larger of space after and
//! space before. Without it (older writers) Word adds the two (measured in Word, 2026-11-10). It is
//! the RTF form of DOCX `w:doNotUseHTMLParagraphAutoSpacing` (`Settings::sum_paragraph_spacing`).

use super::*;
use wordcraft_doc::Document;

fn open(header: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{header}\sectd\pard\sa480 a\par\pard\sb480 b\par}}").as_bytes()).unwrap()
}

#[test]
fn without_htmautsp_spacing_is_summed() {
    assert!(open("").settings.sum_paragraph_spacing, "no \\htmautsp: Word adds space after and before");
}

#[test]
fn htmautsp_keeps_the_larger_space() {
    assert!(!open(r"\htmautsp").settings.sum_paragraph_spacing);
    assert!(!open(r"\htmautsp1").settings.sum_paragraph_spacing);
}

#[test]
fn htmautsp0_is_off() {
    assert!(open(r"\htmautsp0").settings.sum_paragraph_spacing);
}

#[test]
fn export_writes_htmautsp_unless_summed() {
    let mut d = open(r"\htmautsp");
    assert!(!d.settings.sum_paragraph_spacing);
    let rtf = export(&d);
    assert!(rtf.contains(r"\htmautsp"), "collapsing documents say so");
    assert!(!import(rtf.as_bytes()).unwrap().settings.sum_paragraph_spacing, "round trip keeps collapsing");

    d.settings.sum_paragraph_spacing = true;
    let rtf = export(&d);
    assert!(!rtf.contains(r"\htmautsp"), "summing documents leave it out");
    assert!(import(rtf.as_bytes()).unwrap().settings.sum_paragraph_spacing, "round trip keeps summing");
}
