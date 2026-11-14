//! F6 and Shift+F6 cycle the document, the task panes, the status bar and the ribbon, as Word
//! does ("F6: Switch between the document, task pane, status bar, and ribbon", Microsoft Support,
//! Keyboard shortcuts in Word).

use super::{KeyRegion, key_region};
use egui::{Key, Modifiers};

type H = egui_kittest::Harness<'static, crate::WordApp>;

fn harness(panes: &[&str]) -> H {
    let mut app = crate::WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Hello")), crate::Services::default());
    for pane in panes {
        match *pane {
            "navigation" => app.run("view.navigationPane", serde_json::json!({"value": true})).unwrap(),
            "clipboard" => app.run("edit.clipboardPane", serde_json::json!({"value": true})).unwrap(),
            other => panic!("no pane {other}"),
        };
    }
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut crate::WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h.state_mut().canvas.want_focus = true;
    h.run_steps(3);
    assert_eq!(region(&h), KeyRegion::Document, "the page has the keyboard to start");
    h
}

fn region(h: &H) -> KeyRegion {
    key_region(h.state(), &h.ctx)
}

fn press(h: &mut H, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run_steps(3);
}

fn f6(h: &mut H, shift: bool) -> KeyRegion {
    press(h, Key::F6, if shift { Modifiers::SHIFT } else { Modifiers::NONE });
    region(h)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn f6_without_panes_goes_status_bar_ribbon_document() {
    let mut h = harness(&[]);
    assert_eq!(f6(&mut h, false), KeyRegion::StatusBar);
    assert_eq!(f6(&mut h, false), KeyRegion::Ribbon);
    assert_eq!(f6(&mut h, false), KeyRegion::Document);
    assert!(h.state().canvas.focused, "back on the page");
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn shift_f6_without_panes_is_the_reverse() {
    let mut h = harness(&[]);
    assert_eq!(f6(&mut h, true), KeyRegion::Ribbon);
    assert_eq!(f6(&mut h, true), KeyRegion::StatusBar);
    assert_eq!(f6(&mut h, true), KeyRegion::Document);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn f6_visits_each_pane_then_status_bar_then_ribbon() {
    let mut h = harness(&["navigation", "clipboard"]);
    assert_eq!(f6(&mut h, false), KeyRegion::Pane);
    assert_eq!(f6(&mut h, false), KeyRegion::Pane);
    assert_eq!(f6(&mut h, false), KeyRegion::StatusBar, "after the last pane comes the status bar");
    assert_eq!(f6(&mut h, false), KeyRegion::Ribbon);
    assert_eq!(f6(&mut h, false), KeyRegion::Document);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn shift_f6_with_a_pane_runs_backwards_through_it() {
    let mut h = harness(&["navigation"]);
    assert_eq!(f6(&mut h, true), KeyRegion::Ribbon);
    assert_eq!(f6(&mut h, true), KeyRegion::StatusBar);
    assert_eq!(f6(&mut h, true), KeyRegion::Pane, "before the status bar comes the last pane");
    assert_eq!(f6(&mut h, true), KeyRegion::Document);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn escape_from_the_status_bar_or_ribbon_returns_to_the_page() {
    for steps in [1, 2] {
        let mut h = harness(&[]);
        for _ in 0..steps {
            f6(&mut h, false);
        }
        assert_ne!(region(&h), KeyRegion::Document, "{steps} F6 presses left the page");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(region(&h), KeyRegion::Document, "Escape after {steps} F6 presses");
        h.input_mut().events.push(egui::Event::Text("X".into()));
        h.run_steps(2);
        assert!(h.state().session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains('X'), "typing reaches the page again");
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn focus_mode_keeps_f6_off_the_hidden_status_bar_and_ribbon() {
    let mut h = harness(&[]);
    h.state_mut().run("view.focus", serde_json::json!({})).unwrap();
    h.run_steps(4);
    h.state_mut().canvas.want_focus = true;
    h.run_steps(3);
    for shift in [false, true] {
        let r = f6(&mut h, shift);
        assert!(r != KeyRegion::StatusBar && r != KeyRegion::Ribbon, "shift {shift}: {r:?}");
    }
}
