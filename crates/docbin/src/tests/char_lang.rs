//! `.doc` run languages (sprmCRgLid0_80/1_80, sprmCRgLid0/1, sprmCLidBi: a Windows LCID mapped to
//! a language tag) and complex-script bold, italic and size (sprmCFBoldBi, sprmCFItalicBi,
//! sprmCHpsBi), [MS-DOC] §2.6.1.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

const LID0_80: u16 = 0x4A47;
const LID1_80: u16 = 0x4A48;
const LID0: u16 = 0x486D;
const LID1: u16 = 0x486E;
const LID_BI: u16 = 0x485F;
const BOLD_BI: u16 = 0x085C;
const ITALIC_BI: u16 = 0x085D;
const HPS_BI: u16 = 0x4A61;

/// "Hello" in one run with `run` as its CHPX, in a paragraph of style Body (istd 1) whose CHPX is `style_chpx`.
fn doc(run: Vec<u8>, style_chpx: Vec<u8>) -> Document {
    let spec = RichSpec {
        text: "Hello\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body", Vec::new(), style_chpx)],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, run)],
        papx_entries: vec![(5, 1, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn run_props(d: &Document) -> CharProps {
    d.body.iter().find_map(|b| b.as_para()).expect("paragraph").props_of_char(0).clone()
}

fn with(prls: &[(u16, &[u8])]) -> CharProps {
    run_props(&doc(grpprl(prls), Vec::new()))
}

fn lid(op: u16, lid: u16) -> CharProps {
    with(&[(op, &lid.to_le_bytes())])
}

#[test]
fn the_default_language_is_read() {
    assert_eq!(lid(LID0, 0x0407).lang.as_deref(), Some("de-DE"));
}

#[test]
fn the_word_97_default_language_is_read() {
    assert_eq!(lid(LID0_80, 0x040C).lang.as_deref(), Some("fr-FR"));
}

#[test]
fn the_later_language_sprm_wins() {
    let c = with(&[(LID0_80, &0x0409u16.to_le_bytes()), (LID0, &0x0809u16.to_le_bytes())]);
    assert_eq!(c.lang.as_deref(), Some("en-GB"));
}

#[test]
fn the_east_asian_language_is_read() {
    assert_eq!(lid(LID1, 0x0411).lang_ea.as_deref(), Some("ja-JP"));
    assert_eq!(lid(LID1_80, 0x0804).lang_ea.as_deref(), Some("zh-CN"));
}

#[test]
fn the_complex_script_language_is_read() {
    assert_eq!(lid(LID_BI, 0x0401).lang_bidi.as_deref(), Some("ar-SA"));
    assert_eq!(lid(LID_BI, 0x040D).lang_bidi.as_deref(), Some("he-IL"));
}

#[test]
fn no_language_and_unknown_lids_set_nothing() {
    assert_eq!(lid(LID0, 0x0400).lang, None);
    assert_eq!(lid(LID0, 0x0000).lang, None);
    assert_eq!(lid(LID0, 0x1234).lang, None);
}

#[test]
fn a_paragraph_style_keeps_its_language() {
    let d = doc(Vec::new(), grpprl(&[(LID0, &0x0407u16.to_le_bytes())]));
    let body = d.styles.styles.iter().find(|s| s.name == "Body").expect("Body style");
    assert_eq!(body.chr.lang.as_deref(), Some("de-DE"));
}

#[test]
fn complex_script_bold_and_italic_are_read() {
    assert_eq!(with(&[(BOLD_BI, &[1])]).bold_cs, Some(true));
    assert_eq!(with(&[(ITALIC_BI, &[1])]).italic_cs, Some(true));
}

#[test]
fn complex_script_bold_0x81_over_a_bold_style_is_off() {
    let d = doc(grpprl(&[(BOLD_BI, &[0x81])]), grpprl(&[(BOLD_BI, &[1])]));
    assert_ne!(run_props(&d).bold_cs, Some(true));
}

#[test]
fn complex_script_size_is_read() {
    assert_eq!(with(&[(HPS_BI, &28u16.to_le_bytes())]).size_cs, Some(14.0));
}
