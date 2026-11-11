//! A .doc style's paragraph UPX holds only the style's own settings ([MS-DOC] §2.9.260): what it
//! inherits comes from its base style. The reader keeps it that way, so a style with a list and
//! no indent of its own takes the list level's indents instead of a copy of Normal's, as in Word.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::{Align, Document, ParaProps, Style};

const DXA_LEFT: u16 = 0x845E; // sprmPDxaLeft, twips
const DXA_RIGHT: u16 = 0x845D; // sprmPDxaRight, twips
const JC: u16 = 0x2461; // sprmPJc
const KEEP_FOLLOW: u16 = 0x2406; // sprmPFKeepFollow

fn twips(v: i16) -> [u8; 2] {
    v.to_le_bytes()
}

/// Normal (left 1134, right 170, justified) and four styles based on it; one paragraph in each:
/// "a" Listed, "b" Own, "c" Zero, "d" Grand (based on Listed), "e" Normal.
fn doc() -> Document {
    let spec = RichSpec {
        text: "a\rb\rc\rd\re\r",
        styles: vec![
            (1, 0x0FFF, "Normal", grpprl(&[(JC, &[3]), (DXA_RIGHT, &twips(170)), (DXA_LEFT, &twips(1134))]), Vec::new()),
            (1, 0, "Listed", grpprl(&[(KEEP_FOLLOW, &[1])]), Vec::new()),
            (1, 0, "Own", grpprl(&[(DXA_LEFT, &twips(1134))]), Vec::new()),
            (1, 0, "Zero", grpprl(&[(DXA_LEFT, &twips(0)), (JC, &[0])]), Vec::new()),
            (1, 1, "Grand", grpprl(&[(DXA_RIGHT, &twips(300))]), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(1, 1, Vec::new()), (3, 2, Vec::new()), (5, 3, Vec::new()), (7, 4, Vec::new()), (9, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// The properties of the `n`th body paragraph.
fn para(d: &Document, n: usize) -> ParaProps {
    d.body.iter().filter_map(|b| b.as_para()).nth(n).unwrap_or_else(|| panic!("no paragraph {n}")).props.clone()
}

/// The style of the `n`th body paragraph.
fn style_of(d: &Document, n: usize) -> &Style {
    let id = para(d, n).style.unwrap_or_else(|| panic!("paragraph {n} has no style"));
    d.styles.get(&id).unwrap_or_else(|| panic!("no style with id {id:?}"))
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
    let names: Vec<&str> = (0..5).map(|n| style_of(&d, n).name.as_str()).collect();
    assert_eq!(names, ["Listed", "Own", "Zero", "Grand", "Normal"]);
    assert_eq!(style_of(&d, 0).based_on.as_deref(), Some(style_of(&d, 4).id.as_str()));
    assert_eq!(style_of(&d, 3).based_on.as_deref(), Some(style_of(&d, 0).id.as_str()));
}

#[test]
fn normal_keeps_its_own_indents_and_alignment() {
    let d = doc();
    let normal = &style_of(&d, 4).para;
    assert!(some_close(normal.indent_left, 56.7), "left: {:?}", normal.indent_left);
    assert!(some_close(normal.indent_right, 8.5), "right: {:?}", normal.indent_right);
    assert_eq!(normal.align, Some(Align::Justify));
}

#[test]
fn a_derived_style_does_not_copy_its_bases_indents_or_alignment() {
    let d = doc();
    let listed = &style_of(&d, 0).para;
    assert_eq!(listed.indent_left, None, "left indent is Normal's, not Listed's own");
    assert_eq!(listed.indent_right, None, "right indent is Normal's, not Listed's own");
    assert_eq!(listed.align, None, "alignment is Normal's, not Listed's own");
    assert_eq!(listed.keep_next, Some(true), "its own keep with next stays");
}

#[test]
fn a_paragraph_in_the_derived_style_still_resolves_to_the_inherited_values() {
    let d = doc();
    let r = d.styles.resolve_para(&para(&d, 0));
    assert!(close(r.indent_left, 56.7), "left: {}", r.indent_left);
    assert!(close(r.indent_right, 8.5), "right: {}", r.indent_right);
    assert_eq!(r.align, Align::Justify);
    assert!(r.keep_next);
}

#[test]
fn an_indent_the_style_sets_itself_stays_even_when_it_equals_the_bases() {
    let d = doc();
    let own = &style_of(&d, 1).para;
    assert!(some_close(own.indent_left, 56.7), "left: {:?}", own.indent_left);
    assert_eq!(own.indent_right, None);
    assert_eq!(own.align, None);
}

#[test]
fn a_style_that_resets_to_zero_and_left_keeps_those_values() {
    let d = doc();
    let zero = &style_of(&d, 2).para;
    assert!(some_close(zero.indent_left, 0.0), "left: {:?}", zero.indent_left);
    assert_eq!(zero.align, Some(Align::Left));
    assert_eq!(zero.indent_right, None);
    let r = d.styles.resolve_para(&para(&d, 2));
    assert!(close(r.indent_left, 0.0), "left: {}", r.indent_left);
    assert!(close(r.indent_right, 8.5), "right: {}", r.indent_right);
    assert_eq!(r.align, Align::Left);
}

#[test]
fn a_style_two_levels_down_holds_only_its_own_setting() {
    let d = doc();
    let grand = &style_of(&d, 3).para;
    assert!(some_close(grand.indent_right, 15.0), "right: {:?}", grand.indent_right);
    assert_eq!(grand.indent_left, None);
    assert_eq!(grand.align, None);
    assert_eq!(grand.keep_next, None, "keep with next is Listed's, not Grand's own");
    let r = d.styles.resolve_para(&para(&d, 3));
    assert!(close(r.indent_left, 56.7), "left: {}", r.indent_left);
    assert!(close(r.indent_right, 15.0), "right: {}", r.indent_right);
    assert_eq!(r.align, Align::Justify);
    assert!(r.keep_next, "keep with next comes through Listed");
}

#[test]
fn paragraphs_carry_no_direct_indent_from_their_style() {
    let d = doc();
    for n in 0..5 {
        let p = para(&d, n);
        assert_eq!(p.indent_left, None, "paragraph {n}");
        assert_eq!(p.indent_right, None, "paragraph {n}");
    }
}
