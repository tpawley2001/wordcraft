//! A run's character style (`sprmCIstd`, [MS-DOC] §2.6.1, operand = istd of a style with
//! stk = 2) is kept on the run, the way `w:rStyle` is in `.docx` (ECMA-376 §17.3.2.29).

use super::{RichSpec, grpprl, para0, read, rich_doc};

const ISTD: u16 = 0x4A30; // sprmCIstd
const BOLD: u16 = 0x0835; // sprmCFBold
const ITALIC: u16 = 0x0836; // sprmCFItalic

/// "Normal" (istd 0, paragraph) and "Strong Red" (istd 1, character, bold);
/// "plain " is unstyled, "styled" carries `runs[1]`, " tail" is unstyled again.
fn doc(styled_run: Vec<u8>) -> wordcraft_doc::Document {
    let spec = RichSpec {
        text: "plain styled tail\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (2, 0x0FFF, "Strong Red", Vec::new(), grpprl(&[(BOLD, &[0x01])]))],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new()), (6, styled_run), (12, Vec::new())],
        papx_entries: vec![(0, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn char_style_id(doc: &wordcraft_doc::Document) -> String {
    let s = doc.styles.find("Strong Red").expect("character style imported");
    assert_eq!(s.kind, wordcraft_doc::StyleKind::Character);
    s.id.clone()
}

/// The run covering `text` (ASCII, so byte and char offsets agree).
fn run_with<'a>(p: &'a wordcraft_doc::Paragraph, text: &str) -> &'a wordcraft_doc::Run {
    let at = p.text.find(text).unwrap_or_else(|| panic!("no {text:?} in {:?}", p.text));
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at + text.len() <= start + r.len {
            return r;
        }
        start += r.len;
    }
    panic!("no single run covers {text:?}: {:?}", p.runs)
}

#[test]
fn run_keeps_its_character_style() {
    let d = doc(grpprl(&[(ISTD, &1u16.to_le_bytes())]));
    let id = char_style_id(&d);
    let p = para0(&d);
    assert_eq!(p.text, "plain styled tail");
    assert_eq!(run_with(&p, "styled").props.style.as_deref(), Some(id.as_str()));
    assert_eq!(run_with(&p, "plain ").props.style, None);
    assert_eq!(run_with(&p, " tail").props.style, None);
}

#[test]
fn style_formatting_is_not_copied_into_direct_formatting() {
    // The style's bold comes through the style, as for w:rStyle; direct props stay unset.
    let d = doc(grpprl(&[(ISTD, &1u16.to_le_bytes())]));
    let p = para0(&d);
    assert_eq!(run_with(&p, "styled").props.bold, None);
}

#[test]
fn direct_formatting_after_the_style_is_kept() {
    let d = doc(grpprl(&[(ISTD, &1u16.to_le_bytes()), (ITALIC, &[0x01])]));
    let id = char_style_id(&d);
    let p = para0(&d);
    let r = run_with(&p, "styled");
    assert_eq!(r.props.style.as_deref(), Some(id.as_str()));
    assert_eq!(r.props.italic, Some(true));
}

#[test]
fn default_paragraph_font_and_bad_istd_mean_no_style() {
    // istd 10 (sprmCIstd's default, Default Paragraph Font) and an istd past the stylesheet
    // (or naming a paragraph style) leave the run unstyled and still open.
    for istd in [10u16, 0, 999] {
        let d = doc(grpprl(&[(ISTD, &istd.to_le_bytes())]));
        let p = para0(&d);
        assert_eq!(run_with(&p, "styled").props.style, None, "istd {istd}");
    }
}
