//! Define New Number Format › Font…: the number's own font, bold, italic and colour go to the new
//! list through `para.defineNumber`, the form reads them back from the caret's list, the preview
//! shows them, and the dialog's real OK button makes the list.

use egui::Color32;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::{Rgb, TextColor};
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
fn params_carry_the_number_font_only_when_it_is_set() {
    let plain = NumberForm::default();
    assert_eq!(plain.params(), json!({"format": "decimal", "text": "%1.", "align": "left"}));
    let all = NumberForm { font: " Arial ".into(), bold: true, italic: true, color: "FF0000".into(), ..NumberForm::default() };
    assert_eq!(
        all.params(),
        json!({"format": "decimal", "text": "%1.", "align": "left", "font": "Arial", "bold": true, "italic": true, "color": "FF0000"})
    );
    let bold = NumberForm { bold: true, ..NumberForm::default() };
    assert_eq!(bold.params(), json!({"format": "decimal", "text": "%1.", "align": "left", "bold": true}));
    // Showing the font controls is not a setting.
    let shown = NumberForm { show_font: true, ..NumberForm::default() };
    assert_eq!(shown.params(), plain.params());
}

#[test]
fn the_form_reads_the_caret_lists_number_font() {
    let mut a = app();
    a.run("para.defineNumber", json!({"format": "decimal", "text": "%1)", "font": "Courier New", "bold": true, "color": "0070c0"})).unwrap();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&a);
    assert_eq!((f.font.as_str(), f.bold, f.italic, f.color.as_str()), ("Courier New", true, false, "0070C0"));
    assert!(f.show_font, "a number with its own font opens with the font controls showing");
    assert_eq!(f.after, ")");
    // A list whose number has no font of its own: nothing set, the controls stay closed.
    let mut b = app();
    b.run("para.defineNumber", json!({"format": "lowerLetter", "text": "%1."})).unwrap();
    b.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&b);
    assert_eq!((f.font.as_str(), f.bold, f.italic, f.color.as_str(), f.show_font), ("", false, false, "", false));
    // Italic alone is read too.
    let mut c = app();
    c.run("para.defineNumber", json!({"format": "decimal", "text": "%1.", "italic": true})).unwrap();
    c.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let f = form_of(&c);
    assert_eq!((f.bold, f.italic, f.show_font), (false, true, true));
}

#[test]
fn the_preview_label_is_drawn_bold_italic_and_coloured() {
    let plain = NumberForm::default().label_format(11.0);
    assert_eq!(plain.font_id, egui::FontId::proportional(11.0));
    assert!(!plain.italics);
    assert_eq!(plain.color, Color32::BLACK);
    let f = NumberForm { bold: true, italic: true, color: "FF0000".into(), ..NumberForm::default() }.label_format(11.0);
    assert_eq!(f.font_id, crate::theme::semibold(11.0));
    assert!(f.italics);
    assert_eq!(f.color, Color32::from_rgb(255, 0, 0));
    // A colour that is not six hex digits is drawn as Automatic.
    let odd = NumberForm { color: "red".into(), ..NumberForm::default() }.label_format(11.0);
    assert_eq!(odd.color, Color32::BLACK);
}

#[test]
fn font_button_shows_and_hides_the_font_controls() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    assert!(!form_of(h.state()).show_font);
    assert_eq!(h.query_all_by_label("Font color:").count(), 0, "the font controls are hidden until Font… is clicked");
    h.get_by_label("Font…").click();
    h.run_steps(4);
    assert!(form_of(h.state()).show_font);
    assert!(h.query_all_by_label("Font color:").count() >= 1, "Font… shows the font controls");
    h.get_by_label("Font…").click();
    h.run_steps(4);
    assert!(!form_of(h.state()).show_font);
    assert_eq!(h.query_all_by_label("Font color:").count(), 0);
    assert!(matches!(h.state().dialog, Some(Dialog::DefineNumber { .. })), "Font… never closes the dialog");
}

#[test]
fn ok_clicked_in_the_dialog_makes_the_list_with_the_number_font() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    {
        let f = form_mut(h.state_mut());
        f.style = "upperRoman".into();
        f.before = "Step ".into();
        f.font = "Arial".into();
        f.bold = true;
        f.color = "FF0000".into();
        f.show_font = true;
    }
    h.run_steps(2);
    h.get_by_label("OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    let level = caret_level0(h.state());
    assert_eq!((level.format, level.text.as_str()), (NumFormat::UpperRoman, "Step %1."));
    assert_eq!(level.chr.font.as_deref(), Some("Arial"));
    assert_eq!(level.chr.bold, Some(true));
    assert_ne!(level.chr.italic, Some(true));
    assert_eq!(level.chr.color, Some(TextColor::Rgb(Rgb(0xFF, 0, 0))));
}

/// OK with nothing chosen under Font… leaves the number's font to the paragraph text.
#[test]
fn ok_without_a_font_sets_no_font_on_the_number() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    h.get_by_label("OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    let level = caret_level0(h.state());
    assert_eq!((level.format, level.text.as_str()), (NumFormat::Decimal, "%1."));
    assert_eq!((level.chr.font.as_deref(), level.chr.bold, level.chr.italic), (None, None, None));
    assert!(matches!(level.chr.color, None | Some(TextColor::Auto)));
}
