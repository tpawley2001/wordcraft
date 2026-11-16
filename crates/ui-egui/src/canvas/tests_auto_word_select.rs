//! File › Options › Advanced › Editing options: "When selecting, automatically select entire word" and
//! "Use smart paragraph selection" (both on by default, as in Word).

use super::*;
use egui::{Event, PointerButton};

const TEXT: &str = "Alpha beta gamma delta\rSecond para\rThird para";

fn harness(options: serde_json::Value) -> egui_kittest::Harness<'static, WordApp> {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
    a.run("text.insert", json!({"text": TEXT})).unwrap();
    a.run("view.commentsPane", json!({"value": false})).unwrap();
    if !options.as_object().is_some_and(|o| o.is_empty()) {
        a.run("file.options", options).unwrap();
    }
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    for _ in 0..6 {
        h.step();
    }
    h
}

fn screen(h: &mut egui_kittest::Harness<'_, WordApp>, pos: Pos) -> Pos2 {
    let c = h.state_mut().session.layout().caret(&pos).expect("caret");
    page_to_screen(h.state_mut(), c.page, c.x + 1.0, c.top + c.height / 2.0).expect("on screen")
}

/// A plain single-click drag from `from` to `to`; returns the ordered selection.
fn drag(h: &mut egui_kittest::Harness<'_, WordApp>, from: Pos, to: Pos) -> (Pos, Pos) {
    let (a, b) = (screen(h, from), screen(h, to));
    let button = |at: Pos2, pressed: bool| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    h.event(Event::PointerMoved(a));
    h.event(button(a, true));
    h.step();
    for k in 1..=6 {
        h.event(Event::PointerMoved(a + (b - a) * (k as f32 / 6.0)));
        h.step();
    }
    h.event(button(b, false));
    for _ in 0..3 {
        h.step();
    }
    h.state().session.sel.ordered()
}

#[test]
fn drag_into_the_next_word_selects_whole_words() {
    let mut h = harness(json!({}));
    assert_eq!(drag(&mut h, Pos::body(0, 8), Pos::body(0, 13)), (Pos::body(0, 6), Pos::body(0, 17)));
}

#[test]
fn drag_inside_one_word_stays_character_precise() {
    let mut h = harness(json!({}));
    assert_eq!(drag(&mut h, Pos::body(0, 7), Pos::body(0, 9)), (Pos::body(0, 7), Pos::body(0, 9)));
}

#[test]
fn backward_drag_into_the_previous_word_selects_whole_words() {
    let mut h = harness(json!({}));
    let (s, e) = drag(&mut h, Pos::body(0, 13), Pos::body(0, 8));
    assert_eq!(s, Pos::body(0, 6));
    assert!(e >= Pos::body(0, 16), "end {e:?}");
}

#[test]
fn auto_word_selection_off_keeps_characters() {
    let mut h = harness(json!({"autoWordSelection": false}));
    assert_eq!(drag(&mut h, Pos::body(0, 8), Pos::body(0, 13)), (Pos::body(0, 8), Pos::body(0, 13)));
}

#[test]
fn dragging_over_a_whole_paragraph_takes_its_mark() {
    let mut h = harness(json!({}));
    assert_eq!(drag(&mut h, Pos::body(1, 0), Pos::body(1, 11)), (Pos::body(1, 0), Pos::body(2, 0)));
}

#[test]
fn smart_paragraph_selection_off_stops_at_the_pointer() {
    let mut h = harness(json!({"autoWordSelection": false, "smartParagraphSelection": false}));
    assert_eq!(drag(&mut h, Pos::body(1, 0), Pos::body(1, 11)), (Pos::body(1, 0), Pos::body(1, 11)));
}

#[test]
fn options_report_and_keep_both_switches() {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
    let r = a.run("file.options", json!({"autoWordSelection": true})).unwrap();
    assert_eq!(r["autoWordSelection"], json!(true));
    assert_eq!(r["smartParagraphSelection"], json!(true));
    let r = a.run("file.options", json!({"autoWordSelection": false, "smartParagraphSelection": false})).unwrap();
    assert_eq!(r["autoWordSelection"], json!(false));
    assert_eq!(r["smartParagraphSelection"], json!(false));
}
