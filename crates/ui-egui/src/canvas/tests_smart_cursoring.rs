//! File › Options › Advanced › Editing options › "Use smart cursoring" (on by default, as in Word): after scrolling
//! the caret out of view, an arrow key acts on the page in view instead of jumping back.

use super::*;
use egui::{Event, Key};

fn harness(options: serde_json::Value) -> egui_kittest::Harness<'static, WordApp> {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
    for _ in 0..11 {
        a.run("text.insert", json!({"text": "Page text"})).unwrap();
        a.run("layout.break", json!({"kind": "page"})).unwrap();
    }
    a.run("text.insert", json!({"text": "Page text"})).unwrap();
    a.run("view.commentsPane", json!({"value": false})).unwrap();
    if !options.as_object().is_some_and(|o| o.is_empty()) {
        a.run("file.options", options).unwrap();
    }
    a.run("caret.docStart", json!({})).unwrap();
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
    assert_eq!(h.state_mut().session.layout().pages.len(), 12);
    // Scroll to page 9 without touching the caret.
    let target = h.state().canvas.page_rects.get(8).map(|r| r.min.y - h.state().canvas.page_rects[0].min.y).expect("page 9");
    let at = h.state().canvas.scroll_offset.y;
    h.state_mut().canvas.scroll_request = Some(vec2(0.0, at + target));
    for _ in 0..4 {
        h.step();
    }
    h
}

/// The caret's page index after pressing `key` once.
fn page_after(h: &mut egui_kittest::Harness<'_, WordApp>, key: Key) -> usize {
    h.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.event(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    let pos = h.state().session.sel.focus.clone();
    h.state_mut().session.layout().caret(&pos).expect("caret").page
}

#[test]
fn down_arrow_after_scrolling_lands_on_the_page_in_view() {
    let mut h = harness(json!({}));
    let page = page_after(&mut h, Key::ArrowDown);
    assert!((8..=9).contains(&page), "caret page {page}");
}

#[test]
fn right_arrow_after_scrolling_lands_on_the_page_in_view() {
    let mut h = harness(json!({}));
    let page = page_after(&mut h, Key::ArrowRight);
    assert!((8..=9).contains(&page), "caret page {page}");
}

#[test]
fn smart_cursoring_off_moves_from_the_old_caret() {
    let mut h = harness(json!({"smartCursoring": false}));
    let page = page_after(&mut h, Key::ArrowDown);
    assert!(page <= 1, "caret page {page}");
}

#[test]
fn options_report_and_keep_smart_cursoring() {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
    let r = a.run("file.options", json!({"smartCursoring": true})).unwrap();
    assert_eq!(r["smartCursoring"], json!(true));
    let r = a.run("file.options", json!({"smartCursoring": false})).unwrap();
    assert_eq!(r["smartCursoring"], json!(false));
}
