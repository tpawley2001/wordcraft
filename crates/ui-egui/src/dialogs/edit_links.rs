//! Edit Links to Files (title "Links"): the document's links to other files (`fields.links`) in
//! a list, and for the selected one its source, its update method (Automatic / Manual, Locked)
//! and Preserve formatting after update, with Update Now, Open Source, Change Source… and Break
//! Link (`fields.editLink`). Each change runs at once, as one undo step.

use serde::Serialize;
use serde_json::Value;

use super::{Tokens, Ui, WordApp, json, semibold, vec2};
use crate::file_dialogs::{AfterPick, FileDialogRequest};

/// One row of the list, as `fields.links` gives it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LinkRow {
    pub source: String,
    pub item: String,
    pub prog_id: String,
    pub auto: bool,
    pub locked: bool,
    pub preserve: bool,
}

impl LinkRow {
    fn read(v: &Value) -> LinkRow {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        LinkRow {
            source: s("source"),
            item: s("item"),
            prog_id: s("progId"),
            auto: v.get("update").and_then(Value::as_str) == Some("auto"),
            locked: b("locked"),
            preserve: b("preserveFormatting"),
        }
    }

    /// The Type column: "Word Document" for a Word document link, else its ProgID.
    fn kind(&self) -> String {
        if self.prog_id.to_ascii_lowercase().starts_with("word.document") { tl!("Word Document").to_string() } else { self.prog_id.clone() }
    }

    /// The Update column.
    fn update(&self) -> &'static str {
        match (self.locked, self.auto) {
            (true, _) => tl!("Locked"),
            (false, true) => tl!("Auto"),
            (false, false) => tl!("Manual"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EditLinksForm {
    pub links: Vec<LinkRow>,
    pub selected: usize,
    /// Change Source… is open: the new source file and item.
    pub change: Option<(String, String)>,
    /// Break Link asked "Are you sure…?" and waits for Yes or No.
    pub confirm_break: bool,
}

impl EditLinksForm {
    pub fn read(app: &mut WordApp) -> EditLinksForm {
        let mut f = EditLinksForm::default();
        f.reload(app);
        f
    }

    /// The links as the document has them now; the selection stays in the list.
    fn reload(&mut self, app: &mut WordApp) {
        let v = app.session.run("fields.links", &json!({})).unwrap_or_default();
        self.links = v.get("links").and_then(Value::as_array).map(|a| a.iter().map(LinkRow::read).collect()).unwrap_or_default();
        self.selected = self.selected.min(self.links.len().saturating_sub(1));
    }
}

/// Run `fields.editLink` on the selected link with `params`, then read the list again.
fn edit(app: &mut WordApp, f: &mut EditLinksForm, mut params: Value) {
    params["index"] = json!(f.selected);
    if let Err(e) = app.run("fields.editLink", params) {
        app.status(e);
    }
    f.reload(app);
}

/// The source file as a path to open: relative to the document's folder when it is relative.
fn source_path(app: &WordApp, source: &str) -> String {
    let p = std::path::Path::new(source);
    match app.session.path.as_deref().and_then(std::path::Path::parent) {
        Some(dir) if p.is_relative() => dir.join(p).display().to_string(),
        _ => source.to_string(),
    }
}

/// Returns true to close.
pub(super) fn edit_links(app: &mut WordApp, ui: &mut Ui, f: &mut EditLinksForm) -> bool {
    if let Some(picked) = app.dialog_pick.take()
        && let Some((path, _)) = f.change.as_mut()
    {
        *path = picked;
    }
    let t = Tokens::get(ui.ctx());
    let mut close = false;
    crate::rtl::horizontal_top(ui, |ui| {
        crate::rtl::vertical(ui, |ui| {
            list(ui, f, &t);
            ui.add_space(6.0);
            details(app, ui, f);
        });
        crate::rtl::vertical(ui, |ui| {
            ui.add_space(18.0);
            close = side_buttons(app, ui, f);
        });
    });
    if f.confirm_break {
        ui.add_space(6.0);
        ui.label(tl!("Are you sure you want to break the selected links?"));
        ui.horizontal(|ui| {
            if ui.button(tl!("Yes")).clicked() {
                f.confirm_break = false;
                edit(app, f, json!({"action": "breakLink"}));
            }
            if ui.button(tl!("No")).clicked() {
                f.confirm_break = false;
            }
        });
    }
    if f.change.is_some() {
        change_source(app, ui, f);
    }
    close || super::chrome::cancel_key_pressed(ui)
}

/// The list: Source file, Item, Type, Update.
fn list(ui: &mut Ui, f: &mut EditLinksForm, t: &Tokens) {
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.border)).inner_margin(4).show(ui, |ui| {
        ui.set_width(460.0);
        ui.set_height(130.0);
        egui::ScrollArea::vertical().id_salt("edit_links_list").max_height(130.0).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("edit_links_grid").num_columns(4).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                for head in ["Source file", "Item", "Type", "Update"] {
                    ui.label(egui::RichText::new(tl!(head)).font(semibold(12.5)));
                }
                ui.end_row();
                let mut pick = None;
                for (i, l) in f.links.iter().enumerate() {
                    if ui.selectable_label(i == f.selected, l.source.as_str()).clicked() {
                        pick = Some(i);
                    }
                    ui.label(l.item.as_str());
                    ui.label(l.kind());
                    ui.label(l.update());
                    ui.end_row();
                }
                if let Some(i) = pick {
                    f.selected = i;
                    f.confirm_break = false;
                }
            });
            if f.links.is_empty() {
                ui.label(egui::RichText::new(tl!("The document has no links to other files.")).small().color(t.text_dim));
            }
        });
    });
}

/// Source information, Method for updating selected link and Options for selected link.
fn details(app: &mut WordApp, ui: &mut Ui, f: &mut EditLinksForm) {
    let Some(l) = f.links.get(f.selected).cloned() else { return };
    ui.label(egui::RichText::new(tl!("Source information for selected link")).font(semibold(12.5)));
    egui::Grid::new("edit_links_source").num_columns(2).show(ui, |ui| {
        for (label, value) in [("Source file:", l.source.clone()), ("Item in file:", l.item.clone()), ("Link type:", l.kind())] {
            ui.label(tl!(label));
            ui.label(value);
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("Method for updating selected link")).font(semibold(12.5)));
    let mut auto = l.auto;
    let a = ui.radio_value(&mut auto, true, tl!("Automatic update")).changed();
    let m = ui.radio_value(&mut auto, false, tl!("Manual update")).changed();
    if (a || m) && auto != l.auto {
        edit(app, f, json!({"update": if auto { "auto" } else { "manual" }}));
    }
    let mut locked = l.locked;
    if ui.checkbox(&mut locked, tl!("Locked")).changed() {
        edit(app, f, json!({"locked": locked}));
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("Options for selected link")).font(semibold(12.5)));
    let mut preserve = l.preserve;
    if ui.checkbox(&mut preserve, tl!("Preserve formatting after update")).changed() {
        edit(app, f, json!({"preserveFormatting": preserve}));
    }
}

/// Update Now, Open Source, Change Source…, Break Link and OK. Returns true to close.
fn side_buttons(app: &mut WordApp, ui: &mut Ui, f: &mut EditLinksForm) -> bool {
    let w = vec2(120.0, 0.0);
    let row = f.links.get(f.selected).cloned();
    let any = row.is_some();
    let unlocked = row.as_ref().is_some_and(|l| !l.locked);
    if ui.add_enabled(unlocked, egui::Button::new(tl!("Update Now")).min_size(w)).clicked() {
        edit(app, f, json!({"action": "updateNow"}));
    }
    if ui.add_enabled(any, egui::Button::new(tl!("Open Source")).min_size(w)).clicked()
        && let Some(l) = &row
    {
        let path = source_path(app, &l.source);
        match app.open_file(&path, None) {
            Ok(_) => return true,
            Err(e) => app.status(e),
        }
    }
    if ui.add_enabled(any, egui::Button::new(tl!("Change Source…")).min_size(w)).clicked()
        && let Some(l) = &row
    {
        f.change = Some((l.source.clone(), l.item.clone()));
    }
    if ui.add_enabled(any, egui::Button::new(tl!("Break Link")).min_size(w)).clicked() {
        f.confirm_break = true;
    }
    ui.add_space(12.0);
    ui.add(crate::widgets::primary_button(ui, tl!("OK"))).clicked()
}

/// Change Source…: a new source file (typed or picked) and the item in it.
fn change_source(app: &mut WordApp, ui: &mut Ui, f: &mut EditLinksForm) {
    let mut apply = false;
    let mut cancel = false;
    let can_pick = app.services.open_async.is_none() && (app.services.file_dialog.is_some() || app.services.pick_open.is_some());
    let mut browse = false;
    if let Some((path, item)) = f.change.as_mut() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tl!("Change Source")).font(semibold(12.5)));
        ui.horizontal(|ui| {
            let label = ui.label(tl!("Source file:"));
            ui.add(egui::TextEdit::singleline(path).desired_width(300.0)).labelled_by(label.id);
            browse = ui.add_enabled(can_pick, egui::Button::new(tl!("Browse…"))).clicked();
        });
        ui.horizontal(|ui| {
            let label = ui.label(tl!("Item in file:"));
            ui.add(egui::TextEdit::singleline(item).desired_width(300.0)).labelled_by(label.id);
        });
        ui.horizontal(|ui| {
            apply = ui.add_enabled(!path.trim().is_empty(), egui::Button::new(tl!("Change"))).clicked();
            cancel = ui.button(tl!("Cancel")).clicked();
        });
    }
    if browse {
        let _ = app.ask_file(FileDialogRequest::Open { purpose: "document".into() }, AfterPick::ForDialog);
    }
    if apply && let Some((path, item)) = f.change.take() {
        edit(app, f, json!({"source": path.trim(), "item": item.trim()}));
    }
    if cancel {
        f.change = None;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::{CharProps, Document, InlineObject, Pos};

    use crate::WordApp;
    use crate::dialogs::Dialog;

    fn app_with_link() -> WordApp {
        let mut doc = Document::from_text("Here: ");
        let props = CharProps::default();
        let instr = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
        let at = doc.insert_object(&Pos::body(0, 6), InlineObject::FieldStart { instr: instr.into(), locked: false, dirty: false }, &props).unwrap();
        let at = doc.insert_text(&at, "OLD", &props).unwrap();
        doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
        WordApp::new(wordcraft_engine::Session::new(doc), crate::Services::default())
    }

    #[test]
    fn edit_link_without_an_index_opens_the_links_dialog_listing_the_link() {
        let mut app = app_with_link();
        app.run("fields.editLink", json!({})).unwrap();
        let Some(Dialog::EditLinks { form }) = &app.dialog else { panic!("the Links dialog opens: {:?}", app.dialog.as_ref().map(Dialog::name)) };
        assert_eq!(form.links.len(), 1);
        assert_eq!(form.links[0].source, r"C:\d\a.docx");
        assert_eq!(form.links[0].kind(), "Word Document");
        assert_eq!(form.links[0].update(), "Auto");
    }
}
