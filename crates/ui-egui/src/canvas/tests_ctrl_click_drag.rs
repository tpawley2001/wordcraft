//! Use Ctrl+Click to follow hyperlink off: a drag that starts on a link selects text and doesn't
//! follow the link (only a click does).

use serde_json::json;
use wordcraft_doc::{Pos, StoryRef};

use crate::WordApp;
use crate::table_resize::tests::{screen, steps};

#[test]
fn with_the_option_off_a_drag_from_a_link_selects_and_does_not_follow() {
    let mut a =
        WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Alpha beta gamma\nSee here and more")), Default::default());
    a.run("select.text", json!({"text": "beta"})).unwrap();
    a.run("insert.bookmark", json!({"name": "Spot"})).unwrap();
    a.run("select.text", json!({"text": "here"})).unwrap();
    a.run("insert.link", json!({"url": "#Spot"})).unwrap();
    a.run("caret.docEnd", json!({})).unwrap();
    a.run("ui.ctrlClickFollowsLinks", json!({"value": false})).unwrap();
    // Character-exact drag ends: When selecting, automatically select entire word (on by default)
    // would widen the selection to whole words.
    a.run("file.options", json!({"autoWordSelection": false})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    steps(&mut h, 6);
    let at = |h: &mut egui_kittest::Harness<'static, WordApp>, off: usize| {
        let c = h.state_mut().session.layout().caret(&Pos { story: StoryRef::Body, path: wordcraft_doc::Path::top(1), off }).unwrap();
        screen(h, c.x, c.top + c.height / 2.0)
    };
    let (from, to) = (at(&mut h, 5), at(&mut h, 14));
    let none = egui::Modifiers::NONE;
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: none });
    h.step();
    for i in 1..=5 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (i as f32 / 5.0)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: none });
    steps(&mut h, 3);
    let s = &h.state().session;
    assert!(!s.sel.is_collapsed(), "the drag selects text: {:?}", s.sel);
    assert_eq!(s.sel.anchor.path.last(), 1, "from the link, not the bookmark: {:?}", s.sel);
    assert_eq!(s.sel.focus.path.last(), 1, "{:?}", s.sel);
    assert_eq!((s.sel.anchor.off, s.sel.focus.off), (5, 14), "{:?}", s.sel);
}
