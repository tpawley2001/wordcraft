//! Toggle operands ([MS-DOC] §2.9.326 ToggleOperand: 0x80 = the style's value, 0x81 = the
//! opposite of the style's value) resolve against the style of the paragraph the character is
//! in, as Word does. The paragraph's style is only known at its mark, so a reader that resolves
//! characters before it reaches the mark must not use no style, or the previous paragraph's.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::{Document, Paragraph};

const BOLD: u16 = 0x0835; // sprmCFBold
const ITALIC: u16 = 0x0836; // sprmCFItalic

/// "Normal" (istd 0, no formatting) and "Body Bold" (istd 1, paragraph style, bold + italic).
fn doc(text: &'static str, chpx_runs: Vec<(u32, Vec<u8>)>, papx_entries: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body Bold", Vec::new(), grpprl(&[(BOLD, &[0x01]), (ITALIC, &[0x01])]))],
        fonts: vec!["Times New Roman"],
        chpx_runs,
        papx_entries,
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn paras(d: &Document) -> Vec<Paragraph> {
    d.body.iter().filter_map(|b| b.as_para().cloned()).collect()
}

fn style_value(d: &Document, p: &Paragraph, get: fn(&wordcraft_doc::CharProps) -> Option<bool>) -> bool {
    p.props.style.as_deref().and_then(|id| d.styles.get(id)).and_then(|s| get(&s.chr)).unwrap_or(false)
}

/// The effective bold of the paragraph's first character: direct value, else the paragraph style's.
fn bold(d: &Document, p: &Paragraph) -> bool {
    p.props_of_char(0).bold.unwrap_or_else(|| style_value(d, p, |c| c.bold))
}

fn italic(d: &Document, p: &Paragraph) -> bool {
    p.props_of_char(0).italic.unwrap_or_else(|| style_value(d, p, |c| c.italic))
}

/// One paragraph "Body" in "Body Bold" whose run carries `run`.
fn one(run: Vec<u8>) -> (Document, Paragraph) {
    let d = doc("Body\r", vec![(0, run)], vec![(0, 1, Vec::new())]);
    let p = paras(&d).into_iter().next().expect("paragraph");
    assert_eq!(p.text, "Body");
    assert!(p.props.style.is_some(), "the paragraph keeps its style");
    (d, p)
}

#[test]
fn invert_in_a_bold_style_paragraph_is_regular() {
    let (d, p) = one(grpprl(&[(BOLD, &[0x81])]));
    assert!(!bold(&d, &p), "0x81 in a bold paragraph style is not bold: {:?}", p.props_of_char(0));
}

#[test]
fn invert_italic_in_an_italic_style_paragraph_is_upright() {
    let (d, p) = one(grpprl(&[(ITALIC, &[0x81])]));
    assert!(!italic(&d, &p), "{:?}", p.props_of_char(0));
    assert!(bold(&d, &p), "bold still comes from the style");
}

#[test]
fn keep_takes_the_paragraph_styles_value() {
    let (d, p) = one(grpprl(&[(BOLD, &[0x80])]));
    assert!(bold(&d, &p), "{:?}", p.props_of_char(0));
}

#[test]
fn absolute_operands_are_unchanged() {
    let (d, p) = one(grpprl(&[(BOLD, &[0x00])]));
    assert!(!bold(&d, &p));
    let (d, p) = one(grpprl(&[(BOLD, &[0x01])]));
    assert!(bold(&d, &p));
}

#[test]
fn invert_in_a_normal_paragraph_is_bold() {
    let d = doc("Body\r", vec![(0, grpprl(&[(BOLD, &[0x81])]))], vec![(0, 0, Vec::new())]);
    let p = paras(&d).into_iter().next().expect("paragraph");
    assert!(bold(&d, &p));
}

#[test]
fn the_previous_paragraphs_style_does_not_leak() {
    // "first\r" in Body Bold, "second\r" in Normal with bold 0x81: Normal isn't bold, so bold.
    let d = doc("first\rsecond\r", vec![(0, Vec::new()), (6, grpprl(&[(BOLD, &[0x81])]))], vec![(0, 1, Vec::new()), (6, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["first", "second"]);
    assert!(bold(&d, &ps[0]), "first: bold from its style");
    assert!(bold(&d, &ps[1]), "second: 0x81 against Normal is bold: {:?}", ps[1].props_of_char(0));
}

#[test]
fn a_later_paragraph_uses_its_own_style() {
    // "first\r" in Normal, "second\r" in Body Bold with bold 0x81: not bold.
    let d = doc("first\rsecond\r", vec![(0, Vec::new()), (6, grpprl(&[(BOLD, &[0x81])]))], vec![(0, 0, Vec::new()), (6, 1, Vec::new())]);
    let ps = paras(&d);
    assert!(!bold(&d, &ps[0]));
    assert!(!bold(&d, &ps[1]), "second: 0x81 against Body Bold is regular: {:?}", ps[1].props_of_char(0));
}

#[test]
fn the_paragraph_mark_resolves_against_its_paragraph_style() {
    let (d, p) = one(grpprl(&[(BOLD, &[0x81])]));
    let mark = p.mark.bold.unwrap_or_else(|| style_value(&d, &p, |c| c.bold));
    assert!(!mark, "mark: {:?}", p.mark);
}
