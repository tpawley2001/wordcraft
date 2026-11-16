//! File › Options › Advanced › Editing: Use Ctrl+Click to follow hyperlink. On (Word's default),
//! a plain click on a link puts the caret in it and Ctrl/⌘+click follows it; off, a plain click
//! follows the link.

use egui::Pos2;
use serde_json::json;
use wordcraft_doc::{Pos, StoryRef};

use super::links::follow_hint;
use crate::WordApp;
use crate::table_resize::tests::{screen, steps};

type H = egui_kittest::Harness<'static, WordApp>;

/// "Alpha beta gamma" with the bookmark Spot on "beta", then "See here" with "here" linked to it.
fn harness(ctrl_click_follows: Option<bool>) -> H {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Alpha beta gamma\nSee here")), Default::default());
    a.run("select.text", json!({"text": "beta"})).unwrap();
    a.run("insert.bookmark", json!({"name": "Spot"})).unwrap();
    a.run("select.text", json!({"text": "here"})).unwrap();
    a.run("insert.link", json!({"url": "#Spot"})).unwrap();
    a.run("caret.docEnd", json!({})).unwrap();
    if let Some(on) = ctrl_click_follows {
        a.run("ui.ctrlClickFollowsLinks", json!({"value": on})).unwrap();
    }
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    steps(&mut h, 6);
    h
}

/// The screen point between the link's "h" and "e" (paragraph 1, offset 5).
fn on_link(h: &mut H) -> Pos2 {
    let c = h.state_mut().session.layout().caret(&Pos { story: StoryRef::Body, path: wordcraft_doc::Path::top(1), off: 5 }).unwrap();
    screen(h, c.x, c.top + c.height / 2.0)
}

fn click(h: &mut H, at: Pos2, modifiers: egui::Modifiers) {
    h.event(egui::Event::PointerMoved(at));
    h.step();
    let press = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers };
    h.event_modifiers(press, modifiers);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers });
    steps(h, 3);
}

/// The caret went to the bookmark Spot (paragraph 0, at "beta").
fn followed(h: &H) -> bool {
    let s = &h.state().session;
    let (start, _) = wordcraft_engine::cmd::insert::bookmark_range(&s.doc, "Spot").unwrap();
    s.sel.is_collapsed() && s.sel.focus == start
}

#[test]
fn the_option_is_on_by_default_and_set_by_its_command() {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
    assert_eq!(a.run("ui.ctrlClickFollowsLinks", json!({})).unwrap(), json!({"ctrlClickFollowsLinks": true}));
    assert_eq!(a.run("ui.ctrlClickFollowsLinks", json!({"value": false})).unwrap(), json!({"ctrlClickFollowsLinks": false}));
    assert!(!a.ui.ctrl_click_follows_links);
    assert!(a.run("ui.ctrlClickFollowsLinks", json!({"value": "no"})).is_err(), "a value that isn't true or false is refused");
    assert!(!a.ui.ctrl_click_follows_links, "a refused value changes nothing");
    assert_eq!(a.run("ui.ctrlClickFollowsLinks", json!({"value": true})).unwrap(), json!({"ctrlClickFollowsLinks": true}));
}

#[test]
fn the_option_is_kept_in_ui_json_and_missing_means_on() {
    let missing: crate::UiState = serde_json::from_value(json!({})).unwrap();
    assert!(missing.ctrl_click_follows_links);
    let off: crate::UiState = serde_json::from_value(json!({"ctrlClickFollowsLinks": false})).unwrap();
    assert!(!off.ctrl_click_follows_links);
    let saved = serde_json::to_value(&off).unwrap();
    assert_eq!(saved["ctrlClickFollowsLinks"], json!(false));
}

#[test]
fn the_hover_tip_says_how_to_follow_the_link() {
    let on = follow_hint(true);
    let key = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    assert_eq!(on, format!("{key}+Click to follow link"));
    assert_eq!(follow_hint(false), "Click to follow link");
}

#[test]
fn with_the_option_on_a_plain_click_puts_the_caret_in_the_link() {
    let mut h = harness(None);
    let at = on_link(&mut h);
    click(&mut h, at, egui::Modifiers::NONE);
    assert!(!followed(&h), "a plain click doesn't follow the link: {:?}", h.state().session.sel);
    assert_eq!(h.state().session.sel.focus.path.last(), 1, "the caret is in the link's paragraph");
}

#[test]
fn with_the_option_on_ctrl_click_follows_the_link() {
    let mut h = harness(Some(true));
    let at = on_link(&mut h);
    click(&mut h, at, egui::Modifiers::COMMAND);
    assert!(followed(&h), "Ctrl/⌘+click goes to the bookmark: {:?}", h.state().session.sel);
}

#[test]
fn with_the_option_off_a_plain_click_follows_the_link() {
    let mut h = harness(Some(false));
    let at = on_link(&mut h);
    click(&mut h, at, egui::Modifiers::NONE);
    assert!(followed(&h), "a plain click goes to the bookmark: {:?}", h.state().session.sel);
}

#[test]
fn with_the_option_off_shift_click_extends_the_selection() {
    let mut h = harness(Some(false));
    h.state_mut().run("caret.docStart", json!({})).unwrap();
    steps(&mut h, 2);
    let at = on_link(&mut h);
    click(&mut h, at, egui::Modifiers::SHIFT);
    let s = &h.state().session;
    assert!(!followed(&h), "Shift+click doesn't follow the link");
    assert!(!s.sel.is_collapsed(), "Shift+click extends the selection: {:?}", s.sel);
    assert_eq!(s.sel.focus.path.last(), 1, "to the link: {:?}", s.sel);
}
