//! Word keeps an inline picture and the word right after it (no space between) on one line: the
//! picture is a character of that word. Synthetic shapes only; no external documents.
use super::*;

/// The paragraph's lines, one char per cluster: `O` inline object, `T` text, `_` space; markers
/// skipped. `objects` are (offset, width, height), inserted from the end so offsets stay those of
/// `text`.
fn lines(text: &str, objects: &[(usize, f32, f32)], column: Option<f32>) -> Vec<String> {
    crate::test_fonts();
    let mut d = Document::from_text(text);
    if let Some(w) = column {
        d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + w;
    }
    for &(at, w, h) in objects.iter().rev() {
        d.insert_object(&Pos::body(0, at), picture(w, h, Default::default()), &Default::default()).unwrap();
    }
    let l = lay(&d);
    let Some(para) = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para.clone()) } else { None }) else {
        panic!("no lines")
    };
    para.lines
        .iter()
        .map(|line| {
            para.clusters
                .get(line.c0..line.c1)
                .unwrap_or_default()
                .iter()
                .filter_map(|c| match c.kind {
                    para::ClKind::Object(_) => Some('O'),
                    para::ClKind::Text => Some('T'),
                    para::ClKind::Space => Some('_'),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

#[test]
fn a_long_word_right_after_a_picture_starts_on_the_pictures_line() {
    // picture.doc (Apache POI): a 225 pt picture, then a 1,000-letter word. Word fills the rest of
    // the picture's line with the word's first letters and goes on mid-word on the next line.
    let got = lines(&"qwertyuiop".repeat(60), &[(0, 200.0, 50.0)], None);
    assert!(got.len() > 2, "the word is meant to run over several lines: {got:?}");
    assert!(got[0].starts_with('O') && got[0].contains('T'), "the word starts beside the picture: {got:?}");
    assert!(got[1].starts_with('T'), "and goes on mid-word: {got:?}");
}

#[test]
fn a_picture_moves_down_with_the_word_it_touches() {
    // A 150 pt picture, a space, a 40 pt picture glued to "Glued", in a 200 pt column: the second
    // picture fits on line 1 but its word doesn't, so both go to line 2.
    let got = lines(" Glued", &[(0, 150.0, 20.0), (1, 40.0, 20.0)], Some(200.0));
    assert_eq!(got.iter().map(|l| l.trim_end_matches('_')).collect::<Vec<_>>(), ["O", "OTTTTT"], "{got:?}");
}

#[test]
fn a_space_after_a_picture_is_still_a_place_to_wrap() {
    let got = lines("  Glued", &[(0, 150.0, 20.0), (1, 40.0, 20.0)], Some(200.0));
    assert_eq!(got.iter().map(|l| l.trim_end_matches('_')).collect::<Vec<_>>(), ["O_O", "TTTTT"], "{got:?}");
}

#[test]
fn touching_pictures_move_down_together_with_their_word() {
    // PR #6100 review. Word 16 (checked 2026-11-09): two touching pictures right before a word go to the
    // next line together with it, even when the first would fit on this line. Here a 150 pt picture, a
    // space, then two touching 20 pt pictures glued to "Glued" in a 200 pt column: the first 20 pt picture
    // fits on line 1 (150 + space + 20), the rest doesn't.
    let got = lines(" Glued", &[(0, 150.0, 20.0), (1, 20.0, 20.0), (1, 20.0, 20.0)], Some(200.0));
    assert_eq!(got, ["O_", "OOTTTTT"], "{got:?}");
}

#[test]
fn touching_pictures_split_at_a_space() {
    // With a space between the two small pictures, the first stays on line 1.
    let got = lines("  Glued", &[(0, 150.0, 20.0), (1, 20.0, 20.0), (2, 20.0, 20.0)], Some(200.0));
    assert_eq!(got, ["O_O_", "OTTTTT"], "{got:?}");
}
