//! Drop caps as Word stores them in RTF (RTF 1.9.1, "Drop Caps" under "Positioned Objects and
//! Frames"): a one-letter frame paragraph (`\dropcapli`, `\dropcapt`, with its position words)
//! followed by the body paragraph. Import merges the two into the model's single drop cap
//! paragraph, as the DOCX reader does for `w:framePr w:dropCap`, and export writes them as two
//! paragraphs again with the words Word needs to show the cap.

use super::*;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::{Block, Document};

const FONTS: &str = r"{\fonttbl{\f0 Times New Roman;}{\f1 Arial;}}";
/// The words Word writes on a three-line dropped cap's paragraph in 12 pt text.
const DROPPED: &str = r"\pard\plain\f0\fs24\dropcapli3\dropcapt1\pvpara\phmrg\posxl\absh-828\dxfrtext0\sl-828\slmult0 ";
const BODY: &str = r"\pard\plain\f0\fs24\qj\sl360\slmult1 rop cap text.\par";
const SECOND: &str = r"\pard\plain\f0\fs24 Second paragraph.\par";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{FONTS}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

fn texts(d: &Document) -> Vec<String> {
    paras(d).iter().map(|p| p.text.clone()).filter(|t| !t.is_empty()).collect()
}

/// The `\pard`-separated pieces of exported RTF that hold paragraph text.
fn chunks(rtf: &str) -> Vec<&str> {
    rtf.split("\\pard").collect()
}

#[test]
fn a_dropped_cap_merges_into_the_paragraph_it_starts() {
    let d = open(&format!(r"{DROPPED}{{\fs110 D}}\par{BODY}{SECOND}"));
    let alone = open(BODY);
    let (p, body) = (paras(&d), paras(&alone));
    assert_eq!(texts(&d), ["Drop cap text.", "Second paragraph."]);
    let x = &p[0].props;
    assert_eq!((x.drop_cap, x.drop_cap_margin, x.drop_cap_distance), (Some(3), None, None));
    assert_eq!(x.frame, None, "a drop cap is not a positioned frame");
    // The merged paragraph is formatted like the body paragraph, not like the cap's frame.
    assert!(body[0].props.align.is_some() && body[0].props.line_spacing.is_some());
    assert_eq!(x.align, body[0].props.align);
    assert_eq!(x.line_spacing, body[0].props.line_spacing);
    assert_eq!(x.indent_first, body[0].props.indent_first);
    assert_eq!(p[1].props.drop_cap, None);
}

#[test]
fn the_page_anchor_makes_a_margin_cap_whatever_dropcapt_says() {
    let cap = |words: &str| open(&format!(r"\pard\plain\f0\fs24{words} {{\fs72 M}}\par\pard\plain\f0\fs24 argin text.\par"));
    let d = cap(r"\dropcapli2\dropcapt2\pvpara\phpg\absh-552\dxfrtext144\sl-552\slmult0");
    assert_eq!(texts(&d), ["Margin text."]);
    let x = &paras(&d)[0].props;
    assert_eq!((x.drop_cap, x.drop_cap_margin, x.drop_cap_distance), (Some(2), Some(true), Some(7.2)));
    // Anchored to the margin, Word drops the letter into the text even with `\dropcapt2`.
    let d = cap(r"\dropcapli2\dropcapt2\pvpara\phmrg\posxl\absh-552");
    assert_eq!(texts(&d), ["Margin text."]);
    let x = &paras(&d)[0].props;
    assert_eq!((x.drop_cap, x.drop_cap_margin), (Some(2), None));
    // And anchored to the page, it hangs in the margin even with `\dropcapt1`.
    let d = cap(r"\dropcapli2\dropcapt1\pvpara\phpg\absh-552");
    let x = &paras(&d)[0].props;
    assert_eq!((x.drop_cap, x.drop_cap_margin), (Some(2), Some(true)));
}

#[test]
fn a_cap_in_its_own_font_keeps_it_as_the_drop_cap_font() {
    let d = open(&format!(r"{DROPPED}{{\f1\fs110 D}}\par{BODY}"));
    let p = paras(&d);
    assert_eq!(p[0].text, "Drop cap text.");
    assert_eq!(p[0].props.drop_cap_font.as_deref(), Some("Arial"));
    assert!(p[0].runs.iter().all(|r| r.props.font.as_deref() != Some("Arial")), "the letter itself takes the text's font");
}

#[test]
fn paragraphs_that_are_not_a_cap_and_its_body_stay_apart() {
    // No position word: Word shows an ordinary one-letter paragraph.
    let d = open(r"\pard\plain\f0\fs24\dropcapli3\dropcapt1 D\par\pard\plain\f0\fs24 rop.\par");
    assert_eq!(texts(&d), ["D", "rop."]);
    // The cap paragraph is the last one.
    let d = open(&format!(r"\pard\plain\f0\fs24 Body.\par{DROPPED}D\par"));
    assert_eq!(texts(&d), ["Body.", "D"]);
    // Two cap paragraphs in a row: only the second has a body.
    let d = open(&format!(r"{DROPPED}A\par{DROPPED}B\par\pard\plain\f0\fs24 ody.\par"));
    assert_eq!(texts(&d), ["A", "Body."]);
    assert_eq!(paras(&d).iter().filter(|p| p.text == "Body.").map(|p| p.props.drop_cap).collect::<Vec<_>>(), [Some(3)]);
    // More than 16 bytes is not a cap.
    let d = open(&format!(r"{DROPPED}ABCDEFGHIJKLMNOPQ\par\pard\plain\f0\fs24 rest.\par"));
    assert_eq!(texts(&d), ["ABCDEFGHIJKLMNOPQ", "rest."]);
}

#[test]
fn export_writes_the_cap_as_its_own_frame_paragraph() {
    let d = open(&format!(r"{DROPPED}{{\fs110 D}}\par{BODY}{SECOND}"));
    let rtf = export(&d);
    assert_eq!(rtf.matches("\\dropcapli3").count(), 1, "{rtf}");
    let all = chunks(&rtf);
    let at = all.iter().position(|c| c.contains("\\dropcapli3")).expect("cap paragraph");
    let cap = all[at];
    for word in ["\\dropcapt1", "\\pvpara", "\\phmrg", "\\posxl", "\\absh-828", "\\sl-828", "\\slmult0"] {
        assert!(cap.contains(word), "cap paragraph lacks {word}: {cap}");
    }
    assert!(!cap.contains("\\phpg") && !cap.contains("rop cap"), "{cap}");
    let body = all[at + 1..].iter().find(|c| c.contains("rop cap text.")).expect("body paragraph after the cap");
    assert!(!body.contains("\\dropcap") && !body.contains("\\pvpara") && !body.contains("\\absh"), "{body}");
}

#[test]
fn export_of_a_margin_cap_and_its_round_trip() {
    let mut d = open(&format!(r"\pard\plain\f0\fs24 Drop cap text.\par{SECOND}"));
    let first = d.body.iter().position(|b| b.as_para().is_some_and(|p| p.text == "Drop cap text.")).expect("first paragraph");
    if let Some(Block::Para(p)) = d.body.get_mut(first).map(std::sync::Arc::make_mut) {
        (p.props.drop_cap, p.props.drop_cap_margin, p.props.drop_cap_distance) = (Some(4), Some(true), Some(6.0));
    }
    let rtf = export(&d);
    let all = chunks(&rtf);
    let cap = all.iter().find(|c| c.contains("\\dropcapli4")).expect("cap paragraph");
    // Four lines of 12 pt text: 4 x 276 twips.
    for word in ["\\dropcapt2", "\\pvpara", "\\phpg", "\\absh-1104", "\\sl-1104", "\\slmult0", "\\dxfrtext120"] {
        assert!(cap.contains(word), "cap paragraph lacks {word}: {cap}");
    }
    assert!(!cap.contains("\\phmrg") && !cap.contains("rop cap"), "{cap}");
    let back = open_bytes(rtf.as_bytes());
    assert_eq!(texts(&back), ["Drop cap text.", "Second paragraph."]);
    let x = &paras(&back)[0].props;
    assert_eq!((x.drop_cap, x.drop_cap_margin, x.drop_cap_distance), (Some(4), Some(true), Some(6.0)));
    assert_eq!(paras(&back)[1].props.drop_cap, None);
}

#[test]
fn a_dropped_cap_round_trips_with_the_bodys_formatting() {
    let d = open(&format!(r"{DROPPED}{{\fs110 D}}\par{BODY}{SECOND}"));
    let back = open_bytes(export(&d).as_bytes());
    assert_eq!(texts(&back), texts(&d));
    let (a, b) = (paras(&d), paras(&back));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!((x.props.drop_cap, x.props.drop_cap_margin, x.props.drop_cap_distance), (y.props.drop_cap, y.props.drop_cap_margin, y.props.drop_cap_distance));
    }
    // The body's own alignment and line spacing come back on the merged paragraph.
    assert!(a[0].props.align.is_some());
    assert_eq!((a[0].props.align, a[0].props.line_spacing), (b[0].props.align, b[0].props.line_spacing));
}

fn open_bytes(bytes: &[u8]) -> Document {
    import(bytes).unwrap()
}
