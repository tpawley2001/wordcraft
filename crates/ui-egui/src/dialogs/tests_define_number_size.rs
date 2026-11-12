//! Define New Number Format › Font…: the number's own size and underline. Both are blank until
//! chosen, go to `para.defineNumber` only when set, are read back from the caret's list, and the
//! dialog's real OK button stores them on the new list's first level.

use egui_kittest::kittest::Queryable;
use serde_json::json;
use wordcraft_doc::props::Underline;
use wordcraft_engine::Session;

use crate::dialogs::Dialog;
use crate::dialogs::define_number::NumberForm;
use crate::{Services, WordApp};

fn app() -> WordApp {
    let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
    a.run("text.insert", json!({"text": "Item"})).unwrap();
    a
}

fn caret_level0(a: &WordApp) -> wordcraft_doc::numbering::Level {
    let num = wordcraft_engine::cmd::lists::caret_list(&a.session).expect("the caret is in a list");
    wordcraft_engine::cmd::lists::levels_of(&a.session, num).unwrap().into_iter().next().unwrap()
}

fn harness(a: WordApp) -> egui_kittest::Harness<'static, WordApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    )
}

fn form_of(a: &WordApp) -> &NumberForm {
    let Some(Dialog::DefineNumber { form }) = &a.dialog else { panic!("the Define New Number Format dialog is open") };
    form
}

fn form_mut(a: &mut WordApp) -> &mut NumberForm {
    let Some(Dialog::DefineNumber { form }) = &mut a.dialog else { panic!("the Define New Number Format dialog is open") };
    form
}

#[test]
fn a_new_form_has_no_size_and_no_underline() {
    let f = NumberForm::default();
    assert_eq!((f.size.as_str(), f.underline.as_str()), ("", ""));
    assert_eq!(f.params(), json!({"format": "decimal", "text": "%1.", "align": "left"}));
}

#[test]
fn params_carry_the_size_only_when_it_is_a_size_word_takes() {
    let with = |size: &str| NumberForm { size: size.into(), ..NumberForm::default() }.params();
    for (typed, want) in [("20", 20.0), (" 20 ", 20.0), ("10.5", 10.5), ("10.3", 10.5), ("1", 1.0), ("1638", 1638.0)] {
        let p = with(typed);
        assert_eq!(p.get("size").and_then(|v| v.as_f64()), Some(want), "size typed as `{typed}`");
        assert_eq!(p.get("underline"), None);
    }
    for typed in ["", "  ", "abc", "0", "0.4", "1639", "2000", "-5"] {
        assert_eq!(with(typed), NumberForm::default().params(), "`{typed}` is not sent");
    }
}

#[test]
fn params_carry_the_underline_only_when_it_is_chosen() {
    let with = |u: &str| NumberForm { underline: u.into(), ..NumberForm::default() }.params();
    for u in Underline::ALL {
        let p = with(u.ooxml());
        assert_eq!(p.get("underline").and_then(|v| v.as_str()), Some(u.ooxml()));
        assert_eq!(p.get("size"), None);
    }
    for u in ["", "dashed", "Single"] {
        assert_eq!(with(u), NumberForm::default().params(), "`{u}` is not sent");
    }
    let all = NumberForm { size: "20".into(), underline: "double".into(), bold: true, ..NumberForm::default() }.params();
    assert_eq!(all.get("size").and_then(|v| v.as_f64()), Some(20.0));
    assert_eq!(all.get("underline").and_then(|v| v.as_str()), Some("double"));
    assert_eq!(all.get("bold"), Some(&json!(true)));
}

#[test]
fn the_form_reads_the_caret_lists_number_size_and_underline() {
    let mut a = app();
    a.run("para.defineNumber", json!({"format": "decimal", "text": "%1.", "size": 20, "underline": "double"})).unwrap();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&a);
    assert_eq!((f.size.as_str(), f.underline.as_str()), ("20", "double"));
    assert!(f.show_font, "a number with its own size opens with the font controls showing");
    // A half-point size keeps its half; (none) is read back as (none), not as blank.
    let mut b = app();
    b.run("para.defineNumber", json!({"format": "decimal", "text": "%1.", "size": 10.5, "underline": "none"})).unwrap();
    b.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&b);
    assert_eq!((f.size.as_str(), f.underline.as_str(), f.show_font), ("10.5", "none", true));
    // Underline alone opens the controls too; a plain list leaves both blank and the controls closed.
    let mut c = app();
    c.run("para.defineNumber", json!({"format": "decimal", "text": "%1.", "underline": "wave"})).unwrap();
    c.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&c);
    assert_eq!((f.size.as_str(), f.underline.as_str(), f.show_font), ("", "wave", true));
    let mut d = app();
    d.run("para.defineNumber", json!({"format": "lowerLetter", "text": "%1."})).unwrap();
    d.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&d);
    assert_eq!((f.size.as_str(), f.underline.as_str(), f.show_font), ("", "", false));
}

#[test]
fn the_preview_label_is_underlined_only_when_a_line_is_chosen() {
    let line = |u: &str| NumberForm { underline: u.into(), ..NumberForm::default() }.label_format(11.0).underline;
    assert_eq!(line(""), egui::Stroke::NONE);
    assert_eq!(line("none"), egui::Stroke::NONE);
    for u in ["single", "double", "words", "wave", "dotted"] {
        assert!(line(u).width > 0.0, "`{u}` underlines the preview's number");
    }
    // The line has the number's colour.
    let red = NumberForm { underline: "single".into(), color: "FF0000".into(), ..NumberForm::default() }.label_format(11.0);
    assert_eq!(red.underline.color, egui::Color32::from_rgb(255, 0, 0));
    // The preview's point size is the caller's, whatever Size says.
    let big = NumberForm { size: "40".into(), ..NumberForm::default() }.label_format(11.0);
    assert_eq!(big.font_id, egui::FontId::proportional(11.0));
}

#[test]
fn font_button_shows_the_size_and_underline_style_boxes() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    assert_eq!(h.query_all_by_label("Underline style:").count(), 0, "hidden until Font… is clicked");
    h.get_by_label("Font…").click();
    h.run_steps(4);
    assert!(h.query_all_by_label("Underline style:").count() >= 1, "Font… shows the Underline style box");
    assert!(h.query_all_by_label("Size:").count() >= 1, "Font… shows the Size box");
    assert_eq!((form_of(h.state()).size.as_str(), form_of(h.state()).underline.as_str()), ("", ""), "both boxes start blank");
}

#[test]
fn ok_clicked_in_the_dialog_makes_the_list_with_the_number_size_and_underline() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    {
        let f = form_mut(h.state_mut());
        f.size = "20".into();
        f.underline = "single".into();
        f.show_font = true;
    }
    h.run_steps(2);
    h.get_by_label("OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    let level = caret_level0(h.state());
    assert_eq!(level.chr.size, Some(20.0));
    assert_eq!(level.chr.underline, Some(Underline::Single));
    assert_eq!((level.chr.font.as_deref(), level.chr.bold, level.chr.italic), (None, None, None));
}

/// OK with both boxes blank leaves the number's size and underline to the paragraph.
#[test]
fn ok_with_blank_boxes_sets_no_size_and_no_underline() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    h.get_by_label("OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    let level = caret_level0(h.state());
    assert_eq!((level.chr.size, level.chr.underline), (None, None));
}
