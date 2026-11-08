//! A field that becomes one inline object (PAGE, SEQ, REF, ...) takes the character
//! formatting of its cached result, as Word displays it ([MS-DOC] §2.9.84 Plcfld; the
//! `.docx` reader does the same for `w:fldChar` fields): the first non-empty result run's
//! properties, or the field begin character's when the result is empty. A hidden SEQ result
//! (`sprmCFVanish`) stays hidden, a 12 pt PAGE result stays 12 pt.

use super::{RichSpec, grpprl, para0, read, rich_doc};

const BOLD: u16 = 0x0835; // sprmCFBold
const HIDDEN: u16 = 0x083C; // sprmCFVanish
const HPS: u16 = 0x4A43; // sprmCHps, half-points

// CPs: x=0 ' '=1 \x13=2 PAGE=3..=6 \x14=7 '1'=8 '2'=9 \x15=10 ' '=11 y=12 \r=13
const TEXT: &str = "x \u{13}PAGE\u{14}12\u{15} y\r";
// CPs: x=0 ' '=1 \x13=2 PAGE=3..=6 \x14=7 \x15=8 ' '=9 y=10 \r=11
const EMPTY_RESULT: &str = "x \u{13}PAGE\u{14}\u{15} y\r";

fn doc(text: &'static str, chpx_runs: Vec<(u32, Vec<u8>)>) -> wordcraft_doc::Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs,
        papx_entries: vec![(0, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// The run holding the paragraph's single field object, and the field's result.
fn field_run(p: &wordcraft_doc::Paragraph) -> (wordcraft_doc::CharProps, String) {
    let Some(wordcraft_doc::para::InlineObject::Field { result, .. }) = p.objects.first() else { panic!("no field object: {:?}", p.objects) };
    let at = p.text.find('\u{FFFC}').expect("object char");
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return (r.props.clone(), result.clone());
        }
        start += r.len;
    }
    panic!("no run covers the object: {:?}", p.runs)
}

/// The run covering byte `at` of the paragraph text.
fn run_at(p: &wordcraft_doc::Paragraph, at: usize) -> wordcraft_doc::CharProps {
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return r.props.clone();
        }
        start += r.len;
    }
    panic!("no run at {at}: {:?}", p.runs)
}

#[test]
fn bold_result_makes_a_bold_field() {
    let d = doc(TEXT, vec![(0, Vec::new()), (8, grpprl(&[(BOLD, &[0x01])])), (10, Vec::new())]);
    let p = para0(&d);
    assert_eq!(p.text, "x \u{FFFC} y");
    let (props, result) = field_run(&p);
    assert_eq!(result, "12");
    assert_eq!(props.bold, Some(true));
    // Text around the field keeps its own (plain) formatting.
    assert_eq!(run_at(&p, 0).bold, None);
    assert_eq!(run_at(&p, p.text.len() - 1).bold, None);
}

#[test]
fn hidden_result_makes_a_hidden_field() {
    // The SEQ-in-a-footer case: begin, code and result all carry sprmCFVanish 0x81 (the
    // opposite of the style's off), as Word 97 writes it; the field must come out hidden.
    let d = doc(TEXT, vec![(0, Vec::new()), (2, grpprl(&[(HIDDEN, &[0x81])])), (11, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.hidden, Some(true));
    assert_eq!(run_at(&p, 0).hidden, None);
}

#[test]
fn result_size_wins_over_the_code_size() {
    // Begin and code at 8 pt, result at 12 pt: the field shows at 12 pt.
    let d =
        doc(TEXT, vec![(0, Vec::new()), (2, grpprl(&[(HPS, &16u16.to_le_bytes())])), (8, grpprl(&[(HPS, &24u16.to_le_bytes())])), (10, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.size, Some(12.0));
}

#[test]
fn first_result_run_wins_when_the_result_is_mixed() {
    // '1' bold, '2' plain: the object takes the first visible run's properties.
    let d = doc(TEXT, vec![(0, Vec::new()), (8, grpprl(&[(BOLD, &[0x01])])), (9, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.bold, Some(true));
}

#[test]
fn empty_result_keeps_the_begin_characters_formatting() {
    let d = doc(EMPTY_RESULT, vec![(0, Vec::new()), (2, grpprl(&[(BOLD, &[0x01])])), (3, Vec::new())]);
    let p = para0(&d);
    let (props, result) = field_run(&p);
    assert_eq!(result, "");
    assert_eq!(props.bold, Some(true));
}

#[test]
fn plain_field_stays_plain() {
    // Guard: no formatting anywhere -> nothing set on the field.
    let d = doc(TEXT, vec![(0, Vec::new())]);
    let p = para0(&d);
    let (props, result) = field_run(&p);
    assert_eq!(result, "12");
    assert_eq!(props.bold, None);
    assert_eq!(props.hidden, None);
    assert_eq!(props.size, None);
}
