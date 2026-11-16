//! A turned text box: a click lands on the character drawn under it, and the box being edited
//! is drawn straight (#6056).

use super::*;
use wordcraft_geom::Spin;

fn turned(rot: f32, flip_v: bool) -> wordcraft_doc::para::Float {
    wordcraft_doc::para::Float { wrap: wordcraft_doc::para::Wrap::Square, x: 20.0, y: 20.0, rot, flip_v, ..Default::default() }
}

/// Where the text of box `id` at offset `off` (a quarter into the character) is drawn, given the
/// text's turn about the frame's centre, and the upright point it comes from.
fn drawn_point(l: &DocLayout, id: u32, off: usize, text_spin: Spin) -> ((f32, f32), (f32, f32)) {
    let story = StoryRef::Part(id);
    let at = |o: usize| l.caret(&Pos { story, path: Path::top(0), off: o }).unwrap();
    let (a, b) = (at(off), at(off + 1));
    let up = (a.x + (b.x - a.x) * 0.25, a.top + a.height / 2.0);
    let r = box_rect(l, id);
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    (text_spin.apply(cx, cy, up.0, up.1), up)
}

#[test]
fn a_click_on_a_turned_word_lands_on_the_character_drawn_there() {
    crate::test_fonts();
    // The text turns with the box (upside down when flipped across, never mirrored), so the
    // point drawn turned maps back to the character it shows.
    for (rot, flip_v) in [(30.0, false), (90.0, false), (180.0, false), (0.0, true), (300.0, false)] {
        let mut d = Document::from_text("Body text.");
        let id = text_box(&mut d, 0, "Turned words", 140.0, 50.0, turned(rot, flip_v));
        let l = lay(&d);
        let text_spin = Spin::new(rot + if flip_v { 180.0 } else { 0.0 }, false, false);
        for off in [0usize, 3, 7, 10] {
            let ((x, y), _) = drawn_point(&l, id, off, text_spin);
            let hit = l.hit(0, x, y, StoryRef::Part(id)).expect("a hit in the box");
            assert_eq!((hit.story, hit.off), (StoryRef::Part(id), off), "rot {rot} flip_v {flip_v}: click at ({x}, {y})");
        }
    }
}

#[test]
fn an_upright_box_hits_as_before() {
    crate::test_fonts();
    let mut d = Document::from_text("Body text.");
    let id = text_box(&mut d, 0, "Upright words", 140.0, 50.0, turned(0.0, false));
    let l = lay(&d);
    for off in [0usize, 4, 9] {
        let (_, (x, y)) = drawn_point(&l, id, off, Spin::default());
        assert_eq!(l.hit(0, x, y, StoryRef::Part(id)).map(|p| p.off), Some(off));
    }
}

/// Whether `items` draw `text` inside a `Draw::Rotated`, and whether they draw it at all.
fn find_text(items: &[display::Draw], text: &str, in_turn: bool, turned: &mut bool, found: &mut bool) {
    for it in items {
        match it {
            display::Draw::Glyphs { text: t, .. } if t.contains(text) => {
                *found = true;
                *turned |= in_turn;
            }
            display::Draw::Rotated { items, .. } => find_text(items, text, true, turned, found),
            display::Draw::Clip { items, .. } => find_text(items, text, in_turn, turned, found),
            _ => {}
        }
    }
}

#[test]
fn a_box_being_edited_is_drawn_straight() {
    crate::test_fonts();
    let mut d = Document::from_text("Body text that goes on.");
    let first = text_box(&mut d, 0, "Firstbox", 140.0, 50.0, turned(30.0, false));
    let second = text_box(
        &mut d,
        5,
        "Secondbox",
        140.0,
        50.0,
        wordcraft_doc::para::Float { wrap: wordcraft_doc::para::Wrap::Square, x: 260.0, y: 200.0, rot: 45.0, ..Default::default() },
    );
    let l = lay(&d);
    let shown = |straight: Option<u32>| {
        let opts = display::DisplayOptions { straight_text_box: straight, ..Default::default() };
        let items = display::page_display(&d, &l.pages[0], &opts);
        let state = |text: &str| {
            let (mut turned, mut found) = (false, false);
            find_text(&items, text, false, &mut turned, &mut found);
            assert!(found, "{text} is drawn");
            turned
        };
        (state("Firstbox"), state("Secondbox"))
    };
    assert_eq!(shown(None), (true, true), "not editing: both turned");
    assert_eq!(shown(Some(first)), (false, true), "editing the first: it is straight, the other still turned");
    assert_eq!(shown(Some(second)), (true, false));
    // A box's own shape is drawn straight too: no turned drawing is left for it.
    let opts = display::DisplayOptions { straight_text_box: Some(first), ..Default::default() };
    let items = display::page_display(&d, &l.pages[0], &opts);
    let turns = items.iter().filter(|x| matches!(x, display::Draw::Rotated { spin, .. } if (spin.deg - 30.0).abs() < 0.01)).count();
    assert_eq!(turns, 0, "nothing of the edited 30° box is drawn turned");
}

#[test]
fn a_straight_box_hits_its_upright_text() {
    crate::test_fonts();
    // While a turned box is edited it is drawn straight, so a click goes by its upright text.
    let mut d = Document::from_text("Body text.");
    let id = text_box(&mut d, 0, "Turned words", 140.0, 50.0, turned(30.0, false));
    let l = lay(&d);
    for off in [0usize, 3, 7, 10] {
        let (_, (x, y)) = drawn_point(&l, id, off, Spin::default());
        assert_eq!(l.hit_with(0, x, y, StoryRef::Part(id), Some(id)).map(|p| p.off), Some(off));
        // hit() is hit_with(.., None): the turned drawing.
        let ((tx, ty), _) = drawn_point(&l, id, off, Spin::new(30.0, false, false));
        assert_eq!(l.hit(0, tx, ty, StoryRef::Part(id)), l.hit_with(0, tx, ty, StoryRef::Part(id), None));
    }
}
