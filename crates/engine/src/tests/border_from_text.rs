//! `para.borders` with `sides`: each side's optional `space` is the border's distance from the
//! text in points (Word's Borders and Shading › Options… › From text, 0 to 31 pt; `w:space` of
//! ECMA-376 §17.3.1.24 `w:pBdr`). A side without `space` gets Word's default: 1 pt above, below
//! and between, 4 pt left and right.

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_doc::props::Borders;

use crate::Session;

/// The caret paragraph's own borders after `para.borders` with these `sides`.
fn borders_after(sides: Value) -> Borders {
    let mut s = Session::new(Document::new());
    s.run("text.insert", &json!({"text": "Boxed"})).unwrap();
    s.run("para.borders", &json!({"sides": sides})).unwrap();
    s.doc.para_at(&s.sel.focus).and_then(|p| p.props.borders).expect("the paragraph has borders")
}

fn space(b: Option<wordcraft_doc::props::Border>) -> Option<f32> {
    b.map(|x| x.space)
}

#[test]
fn sides_without_space_get_words_default_distances() {
    let b = borders_after(json!({"top": {}, "bottom": {}, "left": {}, "right": {}, "between": {}}));
    assert_eq!(
        [space(b.top), space(b.bottom), space(b.left), space(b.right), space(b.between)],
        [Some(1.0), Some(1.0), Some(4.0), Some(4.0), Some(1.0)]
    );
}

#[test]
fn each_side_takes_its_own_distance_from_text() {
    let b = borders_after(json!({"top": {"space": 0}, "bottom": {"space": 6}, "left": {"space": 12.5}, "right": {"space": 31}}));
    assert_eq!([space(b.top), space(b.bottom), space(b.left), space(b.right)], [Some(0.0), Some(6.0), Some(12.5), Some(31.0)]);
    assert!(b.between.is_none());
}

#[test]
fn distance_from_text_is_held_to_words_range() {
    let b = borders_after(json!({"top": {"space": 40}, "left": {"space": -3}}));
    assert_eq!([space(b.top), space(b.left)], [Some(31.0), Some(0.0)]);
}

#[test]
fn a_distance_that_is_not_a_number_counts_as_left_out() {
    let b = borders_after(json!({"top": {"space": "wide"}, "left": {"space": null}}));
    assert_eq!([space(b.top), space(b.left)], [Some(1.0), Some(4.0)]);
}

#[test]
fn style_width_and_colour_still_apply_beside_space() {
    let b = borders_after(json!({"bottom": {"style": "double", "width": 1.5, "color": "C00000", "space": 9}}));
    let x = b.bottom.expect("bottom border");
    assert_eq!(
        (x.style, x.width, x.color, x.space),
        (wordcraft_doc::props::BorderStyle::Double, 1.5, Some(wordcraft_doc::props::Rgb(0xC0, 0, 0)), 9.0)
    );
}
