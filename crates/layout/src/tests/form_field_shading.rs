//! Form Field Shading (`w:doNotShadeFormData` off, ECMA-376 Part 1 §17.15.1.42): on screen, legacy
//! form fields (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN`) are drawn on light grey, as Word shows
//! them. The shading is a screen aid only: print and PDF leave it out.
use crate::display::{DisplayOptions, Draw, page_display};
use crate::fields::FIELD_SHADING;
use crate::*;
use wordcraft_doc::Pos;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::{CharProps, Rgb};

fn form(instr: &str, result: &str, ff: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: ff.into(), code_paras: None }
}

/// "x <text field 'Alice'> y <check box> z <drop-down showing 'Beta'>", optionally with the text
/// field's run shaded red.
fn doc(no_shade: bool, red_result: bool) -> Document {
    let mut d = Document::from_text("x y z ");
    d.settings.do_not_shade_form_data = no_shade;
    let red = CharProps { shading: red_result.then_some(Rgb(0xFF, 0, 0)), ..Default::default() };
    let dd = r#"<w:ffData><w:name w:val="List"/><w:ddList><w:result w:val="1"/><w:listEntry w:val="Red"/><w:listEntry w:val="Beta"/></w:ddList></w:ffData>"#;
    let cb = r#"<w:ffData><w:name w:val="Box"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/></w:checkBox></w:ffData>"#;
    let tx = r#"<w:ffData><w:name w:val="Text1"/><w:textInput/></w:ffData>"#;
    // Insert from the end so the earlier offsets stay put.
    d.insert_object(&Pos::body(0, 6), form("FORMDROPDOWN", "", dd), &CharProps::default()).unwrap();
    d.insert_object(&Pos::body(0, 4), form("FORMCHECKBOX", "", cb), &CharProps::default()).unwrap();
    d.insert_object(&Pos::body(0, 2), form("FORMTEXT", "Alice", tx), &red).unwrap();
    d
}

fn draws(d: &Document, opts: &DisplayOptions) -> Vec<Draw> {
    crate::test_fonts();
    let l = layout(d, &mut LayoutCache::new(), &LayoutOptions::default());
    page_display(d, &l.pages[0], opts)
}

fn screen() -> DisplayOptions {
    DisplayOptions { form_field_shading: true, ..Default::default() }
}

/// The grey fills drawn, as (index in the draw list, x0, x1).
fn greys(items: &[Draw]) -> Vec<(usize, f32, f32)> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, d)| match d {
            Draw::Fill { rect, color, .. } if *color == FIELD_SHADING => Some((i, rect.x, rect.x + rect.w)),
            _ => None,
        })
        .collect()
}

/// The x of the first glyph of each glyph run whose text contains `needle`.
fn glyph_xs(items: &[Draw], needle: &str) -> Vec<f32> {
    items
        .iter()
        .filter_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains(needle) => glyphs.first().map(|g| g.1),
            _ => None,
        })
        .collect()
}

#[test]
fn every_legacy_form_field_is_drawn_on_grey_on_screen() {
    let items = draws(&doc(false, false), &screen());
    let g = greys(&items);
    assert_eq!(g.len(), 3, "one grey fill per form field: {g:?}");
    let inside = |x: f32| g.iter().any(|(_, a, b)| x >= *a - 0.01 && x < *b);
    for word in ["Alice", "Beta"] {
        let xs = glyph_xs(&items, word);
        assert!(!xs.is_empty(), "{word} is drawn");
        assert!(xs.iter().all(|x| inside(*x)), "{word} at {xs:?} sits on grey {g:?}");
    }
    let squares: Vec<f32> = items
        .iter()
        .filter_map(|d| match d {
            Draw::Shape { rect, kind: wordcraft_doc::para::ShapeKind::Rectangle, stroke: Some(_), .. } => Some(rect.x + rect.w / 2.0),
            _ => None,
        })
        .collect();
    assert_eq!(squares.len(), 1, "the check box square");
    assert!(inside(squares[0]), "the check box sits on grey");
    for d in &items {
        if let Draw::Glyphs { glyphs, text, .. } = d
            && !text.contains("Alice")
            && !text.contains("Beta")
            && (text.starts_with('x') || text.starts_with('z'))
            && let Some(g0) = glyphs.first()
        {
            assert!(!inside(g0.1), "plain text {text:?} at {} is not shaded: {g:?}", g0.1);
        }
    }
}

#[test]
fn shading_turned_off_in_the_document_draws_no_grey() {
    assert!(greys(&draws(&doc(true, false), &screen())).is_empty());
}

#[test]
fn print_and_pdf_options_draw_no_grey() {
    let d = doc(false, false);
    assert!(greys(&draws(&d, &DisplayOptions::default())).is_empty(), "the default (export) options leave it off");
    let data_only = DisplayOptions { form_field_shading: true, forms_data_only: true, ..Default::default() };
    assert!(greys(&draws(&d, &data_only)).is_empty(), "Print only the data from a form draws no shading");
}

#[test]
fn run_shading_on_the_result_draws_over_the_form_shading() {
    let items = draws(&doc(false, true), &screen());
    let g = greys(&items);
    assert_eq!(g.len(), 3, "{g:?}");
    let red = items.iter().position(|d| matches!(d, Draw::Fill { color, .. } if *color == Rgb(0xFF, 0, 0)));
    let red = red.expect("the red run shading is drawn");
    let alice_x = glyph_xs(&items, "Alice")[0];
    let under = g.iter().find(|(_, a, b)| alice_x >= *a - 0.01 && alice_x < *b).expect("grey under Alice");
    assert!(under.0 < red, "grey (draw {}) comes before the red shading (draw {red})", under.0);
}

#[test]
fn a_field_shown_as_code_keeps_only_its_code_shading() {
    let d = doc(false, false);
    crate::test_fonts();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions { field_codes: true, ..Default::default() });
    let count = |opts: &DisplayOptions| greys(&page_display(&d, &l.pages[0], opts)).len();
    assert_eq!(count(&screen()), count(&DisplayOptions::default()), "no form shading on top of the code shading");
}
