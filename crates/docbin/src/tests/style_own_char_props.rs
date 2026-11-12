//! A .doc style's character UPX holds only the style's own settings ([MS-DOC] §2.9.260): what it
//! inherits comes from its base style. The reader keeps it that way, so a style based on another
//! follows a change to its base instead of carrying a copy of the base's font, size and bold, as
//! in Word. Runs still resolve their toggle operands against the style's resolved formatting.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::props::Underline;
use wordcraft_doc::{CharProps, Document, Paragraph, Style};

const BOLD: u16 = 0x0835; // sprmCFBold
const ITALIC: u16 = 0x0836; // sprmCFItalic
const KUL: u16 = 0x2A3E; // sprmCKul
const ICO: u16 = 0x2A42; // sprmCIco
const HPS: u16 = 0x4A43; // sprmCHps, half-points
const FTC0: u16 = 0x4A4F; // sprmCRgFtc0, font index

fn u16le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

/// Normal (bold, 12 pt, Times New Roman, single underline, red), six paragraph styles under it
/// and two character styles; one paragraph in each paragraph style: "a" Plain, "b" Same,
/// "c" Reset, "d" Grand (based on Plain), "e" Flip, "f" Keep, "g" Normal. `first_run` is the
/// direct formatting of the first paragraph's text.
fn doc_with(first_run: Vec<u8>) -> Document {
    let normal = grpprl(&[(BOLD, &[1]), (HPS, &u16le(24)), (FTC0, &u16le(0)), (KUL, &[1]), (ICO, &[6])]);
    let spec = RichSpec {
        text: "a\rb\rc\rd\re\rf\rg\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), normal),
            (1, 0, "Plain", Vec::new(), grpprl(&[(ITALIC, &[1])])),
            (1, 0, "Same", Vec::new(), grpprl(&[(HPS, &u16le(24))])),
            (1, 0, "Reset", Vec::new(), grpprl(&[(BOLD, &[0]), (HPS, &u16le(20)), (KUL, &[0])])),
            (1, 1, "Grand", Vec::new(), grpprl(&[(FTC0, &u16le(1))])),
            (1, 0, "Flip", Vec::new(), grpprl(&[(BOLD, &[0x81])])),
            (1, 0, "Keep", Vec::new(), grpprl(&[(BOLD, &[0x80])])),
            (2, 0x0FFF, "Em", Vec::new(), grpprl(&[(ITALIC, &[1])])),
            (2, 7, "EmBig", Vec::new(), grpprl(&[(HPS, &u16le(28))])),
        ],
        fonts: vec!["Times New Roman", "Arial"],
        chpx_runs: vec![(0, first_run), (1, Vec::new())],
        papx_entries: vec![(1, 1, Vec::new()), (3, 2, Vec::new()), (5, 3, Vec::new()), (7, 4, Vec::new()), (9, 5, Vec::new()), (11, 6, Vec::new()), (13, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn doc() -> Document {
    doc_with(Vec::new())
}

/// The `n`th body paragraph.
fn para(d: &Document, n: usize) -> &Paragraph {
    d.body.iter().filter_map(|b| b.as_para()).nth(n).unwrap_or_else(|| panic!("no paragraph {n}"))
}

/// The style of the `n`th body paragraph.
fn style_of(d: &Document, n: usize) -> &Style {
    let id = para(d, n).props.style.clone().unwrap_or_else(|| panic!("paragraph {n} has no style"));
    d.styles.get(&id).unwrap_or_else(|| panic!("no style with id {id:?}"))
}

/// The style named `name`.
fn named<'a>(d: &'a Document, name: &str) -> &'a Style {
    d.styles.styles.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no style named {name:?}"))
}

fn close(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.01
}

fn some_close(got: Option<f32>, want: f32) -> bool {
    got.is_some_and(|g| close(g, want))
}

#[test]
fn the_fixture_styles_are_the_expected_ones() {
    let d = doc();
    let names: Vec<&str> = (0..7).map(|n| style_of(&d, n).name.as_str()).collect();
    assert_eq!(names, ["Plain", "Same", "Reset", "Grand", "Flip", "Keep", "Normal"]);
    let normal_id = style_of(&d, 6).id.as_str();
    for n in [0, 1, 2, 4, 5] {
        assert_eq!(style_of(&d, n).based_on.as_deref(), Some(normal_id), "paragraph {n}");
    }
    assert_eq!(style_of(&d, 3).based_on.as_deref(), Some(style_of(&d, 0).id.as_str()));
    assert_eq!(named(&d, "EmBig").based_on.as_deref(), Some(named(&d, "Em").id.as_str()));
}

#[test]
fn normal_keeps_all_of_its_own_character_formatting() {
    let d = doc();
    let normal = &style_of(&d, 6).chr;
    assert_eq!(normal.bold, Some(true));
    assert!(some_close(normal.size, 12.0), "size: {:?}", normal.size);
    assert_eq!(normal.font.as_deref(), Some("Times New Roman"));
    assert_eq!(normal.underline, Some(Underline::Single));
    assert!(normal.color.is_some(), "red");
}

#[test]
fn a_derived_style_does_not_copy_its_bases_character_formatting() {
    let d = doc();
    let plain = &style_of(&d, 0).chr;
    assert_eq!(plain.italic, Some(true), "its own italic stays");
    assert_eq!(plain.bold, None, "bold is Normal's, not Plain's own");
    assert_eq!(plain.size, None, "size is Normal's, not Plain's own");
    assert_eq!(plain.font, None, "font is Normal's, not Plain's own");
    assert_eq!(plain.underline, None, "underline is Normal's, not Plain's own");
    assert_eq!(plain.color, None, "colour is Normal's, not Plain's own");
}

#[test]
fn a_paragraph_in_the_derived_style_still_resolves_to_the_inherited_values() {
    let d = doc();
    let normal = d.styles.resolve_char(para(&d, 6).props.style.as_deref(), &CharProps::default());
    let r = d.styles.resolve_char(para(&d, 0).props.style.as_deref(), &CharProps::default());
    assert!(r.bold, "bold comes through Normal");
    assert!(r.italic);
    assert!(close(r.size, 12.0), "size: {}", r.size);
    assert_eq!(r.font, "Times New Roman");
    assert_eq!(r.underline, Underline::Single);
    assert_eq!(r.color, normal.color, "colour comes through Normal");
}

#[test]
fn a_size_the_style_sets_itself_stays_even_when_it_equals_the_bases() {
    let d = doc();
    let same = &style_of(&d, 1).chr;
    assert!(some_close(same.size, 12.0), "size: {:?}", same.size);
    assert_eq!(same.bold, None);
    assert_eq!(same.font, None);
    assert_eq!(same.underline, None);
}

#[test]
fn a_style_that_switches_bold_and_underline_off_keeps_those_values() {
    let d = doc();
    let reset = &style_of(&d, 2).chr;
    assert_eq!(reset.bold, Some(false));
    assert!(some_close(reset.size, 10.0), "size: {:?}", reset.size);
    assert_eq!(reset.underline, Some(Underline::None));
    assert_eq!(reset.font, None);
    assert_eq!(reset.color, None);
    let r = d.styles.resolve_char(para(&d, 2).props.style.as_deref(), &CharProps::default());
    assert!(!r.bold);
    assert!(close(r.size, 10.0), "size: {}", r.size);
    assert_eq!(r.underline, Underline::None);
    assert_eq!(r.font, "Times New Roman");
}

#[test]
fn a_style_two_levels_down_holds_only_its_own_setting() {
    let d = doc();
    let grand = &style_of(&d, 3).chr;
    assert_eq!(grand.font.as_deref(), Some("Arial"));
    assert_eq!(grand.italic, None, "italic is Plain's, not Grand's own");
    assert_eq!(grand.bold, None);
    assert_eq!(grand.size, None);
    let r = d.styles.resolve_char(para(&d, 3).props.style.as_deref(), &CharProps::default());
    assert_eq!(r.font, "Arial");
    assert!(r.italic, "italic comes through Plain");
    assert!(r.bold, "bold comes through Normal");
    assert!(close(r.size, 12.0), "size: {}", r.size);
}

#[test]
fn a_style_toggle_opposite_to_the_base_is_kept_as_its_value() {
    let d = doc();
    assert_eq!(style_of(&d, 4).chr.bold, Some(false), "0x81 under a bold Normal is not bold");
    assert!(!d.styles.resolve_char(para(&d, 4).props.style.as_deref(), &CharProps::default()).bold);
}

#[test]
fn a_style_toggle_same_as_the_base_is_left_to_inheritance() {
    let d = doc();
    assert_eq!(style_of(&d, 5).chr.bold, None, "0x80 is the base's value, not the style's own");
    assert!(d.styles.resolve_char(para(&d, 5).props.style.as_deref(), &CharProps::default()).bold);
}

#[test]
fn a_character_style_based_on_another_holds_only_its_own_setting() {
    let d = doc();
    assert_eq!(named(&d, "Em").chr.italic, Some(true));
    let big = &named(&d, "EmBig").chr;
    assert!(some_close(big.size, 14.0), "size: {:?}", big.size);
    assert_eq!(big.italic, None, "italic is Em's, not EmBig's own");
}

/// The effective bold of the first paragraph's text.
fn first_bold(d: &Document) -> bool {
    let p = para(d, 0);
    assert_eq!(p.text, "a");
    d.styles.resolve_char(p.props.style.as_deref(), p.props_of_char(0)).bold
}

#[test]
fn a_run_toggle_opposite_to_the_style_sees_the_inherited_bold() {
    // Plain's bold comes from Normal: "the opposite of the style's value" is not bold.
    let d = doc_with(grpprl(&[(BOLD, &[0x81])]));
    assert_eq!(para(&d, 0).props_of_char(0).bold, Some(false), "{:?}", para(&d, 0).props_of_char(0));
    assert!(!first_bold(&d));
}

#[test]
fn a_run_toggle_same_as_the_style_stays_bold() {
    let d = doc_with(grpprl(&[(BOLD, &[0x80])]));
    assert!(first_bold(&d), "{:?}", para(&d, 0).props_of_char(0));
}

#[test]
fn paragraphs_carry_no_direct_character_formatting_from_their_style() {
    let d = doc();
    for n in 0..7 {
        let c = para(&d, n).props_of_char(0);
        assert_eq!(c.bold, None, "paragraph {n}");
        assert_eq!(c.size, None, "paragraph {n}");
        assert_eq!(c.font, None, "paragraph {n}");
    }
}
