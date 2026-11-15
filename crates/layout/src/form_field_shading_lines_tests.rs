//! Form Field Shading line by line: each line holding a legacy form field gets its own grey fill,
//! as tall as that line's character shading. A field's shown result is laid out as one unbreakable
//! piece (see `para::paragraph::field`), so it never splits over two lines: on a narrow page a
//! long result moves whole to the next line and its fill goes with it.

use super::*;
use crate::display::{DisplayOptions, Draw, page_display};
use crate::fields::FIELD_SHADING;
use wordcraft_doc::Pos;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::CharProps;

fn text_field(result: &str) -> InlineObject {
    InlineObject::Field {
        instr: "FORMTEXT".into(),
        result: result.into(),
        locked: false,
        dirty: false,
        code: false,
        form: r#"<w:ffData><w:name w:val="Text1"/><w:textInput/></w:ffData>"#.into(),
        code_paras: None,
    }
}

/// The grey fills, as (x0, x1, y0, y1).
fn greys(items: &[Draw]) -> Vec<(f32, f32, f32, f32)> {
    items
        .iter()
        .filter_map(|d| match d {
            Draw::Fill { rect, color, .. } if *color == FIELD_SHADING => Some((rect.x, rect.x + rect.w, rect.y, rect.y + rect.h)),
            _ => None,
        })
        .collect()
}

/// The first glyph of each glyph run showing `needle`, as (x, baseline y).
fn glyphs_of(items: &[Draw], needle: &str) -> Vec<(f32, f32)> {
    items
        .iter()
        .filter_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains(needle) => glyphs.first().map(|g| (g.1, g.2)),
            _ => None,
        })
        .collect()
}

#[test]
fn text_fields_on_two_lines_get_one_fill_per_line() {
    test_fonts();
    // A narrow column: the two long results can't share a line, so each takes its own.
    let mut d = Document::from_text("a b ");
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + 120.0;
    d.insert_object(&Pos::body(0, 4), text_field("Secondresultwide"), &CharProps::default()).unwrap();
    d.insert_object(&Pos::body(0, 2), text_field("Firstresultwide"), &CharProps::default()).unwrap();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let items = page_display(&d, &l.pages[0], &DisplayOptions { form_field_shading: true, ..Default::default() });
    let g = greys(&items);
    assert_eq!(g.len(), 2, "one fill per field, each on its own line: {g:?}");
    let (first, second) = (glyphs_of(&items, "Firstresultwide"), glyphs_of(&items, "Secondresultwide"));
    assert_eq!((first.len(), second.len()), (1, 1), "each result is drawn once");
    let (a, b) = (first[0], second[0]);
    assert!(b.1 > a.1, "the second result is on a later line: {a:?} {b:?}");
    let on = |(x, y): (f32, f32)| g.iter().filter(|(x0, x1, y0, y1)| x >= *x0 - 0.01 && x < *x1 && y > *y0 && y <= *y1).count();
    assert_eq!((on(a), on(b)), (1, 1), "each result sits on exactly one fill: {g:?}");
    let (r0, r1) = (g[0], g[1]);
    assert!(r0.3 <= r1.2 || r1.3 <= r0.2, "the fills' lines don't overlap: {g:?}");
}

#[test]
fn an_empty_text_field_result_draws_no_fill_and_does_not_panic() {
    test_fonts();
    let mut d = Document::from_text("a b");
    d.insert_object(&Pos::body(0, 2), text_field(""), &CharProps::default()).unwrap();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let items = page_display(&d, &l.pages[0], &DisplayOptions { form_field_shading: true, ..Default::default() });
    assert!(greys(&items).is_empty(), "nothing to shade: {:?}", greys(&items));
}

#[test]
fn en_spaces_of_an_unfilled_text_field_are_shaded() {
    test_fonts();
    let mut d = Document::from_text("a b");
    d.insert_object(&Pos::body(0, 2), text_field("\u{2002}\u{2002}\u{2002}\u{2002}\u{2002}"), &CharProps::default()).unwrap();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let items = page_display(&d, &l.pages[0], &DisplayOptions { form_field_shading: true, ..Default::default() });
    let g = greys(&items);
    assert_eq!(g.len(), 1, "{g:?}");
    assert!(g[0].1 - g[0].0 > 1.0, "five en spaces have width: {g:?}");
}
