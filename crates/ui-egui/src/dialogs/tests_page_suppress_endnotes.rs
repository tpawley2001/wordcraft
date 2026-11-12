//! Page Setup › Layout: Suppress endnotes (`w:noEndnote`), available only while endnotes are
//! placed at the end of each section, as in Word.

use super::*;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use wordcraft_doc::section::{NotePos, SectionProps, SectionStart};
use wordcraft_engine::Session;

const BOX: &str = "Suppress endnotes";

fn sect(app: &WordApp) -> SectionProps {
    wordcraft_engine::cmd::page::sect(&app.session)
}

/// A one-section document with its endnotes at the end of each section (`section_end`) or at
/// the end of the document (Word's default), and the section's stored Suppress endnotes.
fn app_with(section_end: bool, stored: Option<bool>) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": "Text"}));
    if section_end {
        app.run("references.noteOptions", json!({"kind": "endnote", "pos": "sectEnd"})).unwrap();
    }
    let want = if section_end { NotePos::SectEnd } else { NotePos::DocEnd };
    assert_eq!(app.session.doc.note_options(&sect(&app), true).pos, want, "endnote placement set up");
    if stored.is_some() {
        let mut s = sect(&app);
        s.no_endnote = stored;
        app.run("layout.pageSetup", json!({"section": s})).unwrap();
    }
    assert_eq!(sect(&app).no_endnote, stored, "stored value set up");
    app
}

/// Page Setup open on `app`'s caret section, drawn.
fn opened(mut app: WordApp) -> egui_kittest::Harness<'static, WordApp> {
    app.dialog = Dialog::open("pageSetup", &mut app);
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn ok(h: &mut egui_kittest::Harness<'static, WordApp>) {
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
}

fn tick(h: &mut egui_kittest::Harness<'static, WordApp>) {
    h.get_by_role_and_label(Role::CheckBox, BOX).click();
    h.run_steps(3);
}

/// (greyed out, ticked) of the Suppress endnotes box.
fn state(h: &egui_kittest::Harness<'static, WordApp>) -> (bool, bool) {
    let node = h.get_by_role_and_label(Role::CheckBox, BOX);
    (node.accesskit_node().is_disabled(), node.accesskit_node().toggled() == Some(Toggled::True))
}

#[test]
fn the_box_is_greyed_while_endnotes_are_at_the_end_of_the_document() {
    let h = opened(app_with(false, None));
    assert_eq!(state(&h), (true, false));
}

#[test]
fn the_box_is_available_with_endnotes_at_the_end_of_each_section() {
    let h = opened(app_with(true, None));
    assert_eq!(state(&h), (false, false));
    // A stored explicit off shows unticked too; a stored on shows ticked.
    let h = opened(app_with(true, Some(false)));
    assert_eq!(state(&h), (false, false));
    let h = opened(app_with(true, Some(true)));
    assert_eq!(state(&h), (false, true));
}

#[test]
fn ticking_it_suppresses_the_sections_endnotes_in_one_undo_step() {
    let app = app_with(true, None);
    let depth = app.session.undo_depth();
    let mut h = opened(app);
    tick(&mut h);
    assert_eq!(state(&h), (false, true));
    ok(&mut h);
    assert_eq!(sect(h.state()).no_endnote, Some(true));
    assert_eq!(h.state().session.undo_depth(), depth + 1, "one undo step");
}

#[test]
fn it_shares_the_undo_step_with_the_other_layout_settings() {
    let app = app_with(true, None);
    let depth = app.session.undo_depth();
    let mut h = opened(app);
    h.get_by_role_and_label(Role::ComboBox, "Section start:").click();
    h.run_steps(2);
    h.get_by_label("Odd page").click();
    h.run_steps(3);
    tick(&mut h);
    ok(&mut h);
    let s = sect(h.state());
    assert_eq!((s.start, s.no_endnote), (SectionStart::OddPage, Some(true)));
    assert_eq!(h.state().session.undo_depth(), depth + 1, "one undo step for both");
}

#[test]
fn unticking_it_removes_the_setting() {
    let mut h = opened(app_with(true, Some(true)));
    tick(&mut h);
    assert_eq!(state(&h), (false, false));
    ok(&mut h);
    assert_eq!(sect(h.state()).no_endnote, None, "off is no w:noEndnote at all");
}

#[test]
fn an_unchanged_ok_keeps_what_the_section_had() {
    for (section_end, stored) in [(true, Some(false)), (true, Some(true)), (true, None), (false, Some(true)), (false, Some(false))] {
        let app = app_with(section_end, stored);
        let depth = app.session.undo_depth();
        let mut h = opened(app);
        ok(&mut h);
        assert_eq!(sect(h.state()).no_endnote, stored, "section end {section_end}, stored {stored:?}");
        assert_eq!(h.state().session.undo_depth(), depth, "no undo step");
    }
}

#[test]
fn cancel_changes_nothing() {
    let app = app_with(true, None);
    let depth = app.session.undo_depth();
    let mut h = opened(app);
    tick(&mut h);
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    assert_eq!(sect(h.state()).no_endnote, None);
    assert_eq!(h.state().session.undo_depth(), depth);
}
