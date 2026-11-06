//! A .doc paragraph names its style by istd ([MS-DOC] §2.4.6.5), not by name, so two styles whose
//! names differ only in letter case ("Body Text" and "Body text") stay two styles, each with its
//! own formatting, as in Word.

use super::{RichSpec, grpprl, read, rich_doc};

const HPS: u16 = 0x4A43; // sprmCHps, half-points

fn size(hps: u16) -> Vec<u8> {
    grpprl(&[(HPS, &hps.to_le_bytes())])
}

/// The style ids of the document's first two paragraphs.
fn para_styles(doc: &wordcraft_doc::Document) -> Vec<String> {
    doc.body.iter().filter_map(|b| b.as_para()).take(2).map(|p| p.props.style.clone().expect("paragraph has a style")).collect()
}

fn style<'a>(doc: &'a wordcraft_doc::Document, id: &str) -> &'a wordcraft_doc::Style {
    doc.styles.get(id).unwrap_or_else(|| panic!("no style with id {id:?}"))
}

/// Normal (istd 0); "Body Text" (istd 1, 7 pt); "Body text" (istd 2, based on istd 1, 10 pt).
/// "one" is in istd 1, "two" in istd 2.
fn two_cases() -> wordcraft_doc::Document {
    let spec = RichSpec {
        text: "one\rtwo\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "Body Text", Vec::new(), size(14)),
            (1, 1, "Body text", Vec::new(), size(20)),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(3, 1, Vec::new()), (7, 2, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

#[test]
fn styles_differing_only_in_case_get_their_own_ids() {
    let d = two_cases();
    let ids = para_styles(&d);
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1], "both paragraphs point at one style: {ids:?}");
}

#[test]
fn each_paragraph_keeps_its_own_style_formatting() {
    let d = two_cases();
    let ids = para_styles(&d);
    let built_in = style(&d, &ids[0]);
    let user = style(&d, &ids[1]);
    assert_eq!(built_in.chr.size, Some(7.0));
    assert_eq!(user.chr.size, Some(10.0));
    assert_eq!(built_in.name, "Body Text");
    assert_eq!(user.name, "Body text");
}

#[test]
fn the_lower_case_style_is_based_on_the_other() {
    let d = two_cases();
    let ids = para_styles(&d);
    assert_eq!(style(&d, &ids[1]).based_on.as_deref(), Some(ids[0].as_str()));
}

/// Guard: one "Body Text" style alone keeps working (passes before the change).
#[test]
fn a_single_style_is_unchanged() {
    let spec = RichSpec {
        text: "one\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body Text", Vec::new(), size(14))],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(3, 1, Vec::new())],
    };
    let d = read(&rich_doc(&spec)).expect("opens");
    let ids = para_styles(&d);
    assert_eq!(ids.len(), 1);
    assert_eq!(style(&d, &ids[0]).chr.size, Some(7.0));
}
