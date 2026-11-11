use super::{Stroke, Tokens, Ui, Value, WordApp, json, regular};

/// Items collected by Copy and Cut, newest first; clicking one pastes it at the caret.
pub(super) fn clipboard(app: &mut WordApp, ui: &mut Ui) {
    let items: Vec<String> = app.session.clip_history.items().iter().map(|i| i.preview()).collect();
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!items.is_empty(), egui::Button::new(tl!("Paste All"))).clicked() {
            let _ = app.run("edit.pasteAllClipboard", json!({}));
            app.canvas.want_focus = true;
        }
        if ui.add_enabled(!items.is_empty(), egui::Button::new(tl!("Clear All"))).clicked() {
            let _ = app.run("edit.clearClipboard", json!({}));
            app.canvas.want_focus = true;
        }
    });
    ui.add_space(4.0);
    let hint = if items.is_empty() { "Nothing collected yet. Items you copy or cut appear here." } else { "Click an item to paste it." };
    ui.label(egui::RichText::new(tl!(hint)).small().weak());
    ui.separator();
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, preview) in items.iter().enumerate() {
            egui::Frame::NONE.fill(t.input).stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let text = if preview.is_empty() { tl!("(picture or object)").to_string() } else { preview.clone() };
                let mut job =
                    egui::text::LayoutJob::single_section(text, egui::TextFormat { font_id: regular(12.5), color: t.text, ..Default::default() });
                job.wrap.max_width = ui.available_width();
                job.wrap.max_rows = 3;
                if ui.add(egui::Button::new(job).frame(false)).on_hover_text(tl!("Paste")).clicked() {
                    let _ = app.run("edit.pasteClipboardItem", json!({"index": i}));
                    app.canvas.want_focus = true;
                }
                ui.with_layout(crate::rtl::far_side(ui, egui::Align::Min), |ui| {
                    if ui.small_button(tl!("Delete")).clicked() {
                        let _ = app.run("edit.deleteClipboardItem", json!({"index": i}));
                        app.canvas.want_focus = true;
                    }
                });
            });
            ui.add_space(6.0);
        }
    });
    ui.separator();
    clipboard_options(app, ui);
}

/// The Clipboard pane's Options ▾ menu at its foot (`edit.clipboardOptions`).
fn clipboard_options(app: &mut WordApp, ui: &mut Ui) {
    let o = app.session.prefs.clipboard.clone();
    ui.menu_button(format!("{} ▾", tl!("Options")), |ui| {
        for (key, on, label) in [
            ("showAutomatically", o.show_automatically, tl!("Show Clipboard Automatically")),
            ("showOnCopyTwice", o.show_on_copy_twice, tl!("Show Clipboard When Ctrl+C Pressed Twice")),
            ("collectWithoutShowing", o.collect_without_showing, tl!("Collect Without Showing Clipboard")),
            ("showStatus", o.show_status, tl!("Show Status When Copying")),
        ] {
            let mut checked = on;
            if ui.checkbox(&mut checked, label).changed() {
                let _ = app.run("edit.clipboardOptions", json!({ key: checked }));
            }
        }
    });
}

pub(super) fn styles(app: &mut WordApp, ui: &mut Ui) {
    let st = app.session.run("format.state", &json!({})).unwrap_or_default();
    let cur = st.get("style").and_then(Value::as_str).unwrap_or("Normal").to_string();
    let current_name = app
        .session
        .doc
        .styles
        .get(&cur)
        .map(crate::i18n::style_name)
        .unwrap_or_else(|| st.get("styleName").and_then(Value::as_str).unwrap_or("").to_string());
    ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Current style: {style}"), &[("style", &current_name)])).small());
    // Wrapped: the host gives the pane a fixed width, and the row is wider than its default.
    ui.horizontal_wrapped(|ui| {
        if ui.button(tl!("New Style…")).clicked() {
            app.dialog = crate::dialogs::Dialog::open("newStyle", app);
        }
        if ui.button(tl!("Update to Match")).clicked() {
            let _ = app.run("styles.updateToMatch", json!({}));
        }
        if ui.button(tl!("Clear")).clicked() {
            let _ = app.run("format.clear", json!({}));
        }
    });
    if ui.button(tl!("Manage Styles…")).clicked() {
        let _ = app.run("styles.manage", json!({}));
    }
    if ui.selectable_label(app.session.view.style_inspector, tl!("Style Inspector")).clicked() {
        let _ = app.run("styles.inspector", json!({}));
    }
    // A solid scroll bar takes its own room beside the list instead of floating over the a / ¶ markers.
    // Its handle keeps the text colour: the solid default fills it like a button, white on white here.
    ui.spacing_mut().scroll = egui::style::ScrollStyle { foreground_color: true, ..egui::style::ScrollStyle::solid() };
    if ui.selectable_label(app.session.view.reveal_formatting_pane, tl!("Reveal Formatting")).clicked() {
        let _ = app.run("view.revealFormattingPane", json!({}));
    }
    let mut disable_linked = app.session.view.disable_linked_styles;
    if ui.checkbox(&mut disable_linked, tl!("Disable Linked Styles")).changed() {
        let _ = app.run("styles.disableLinked", json!({"value": disable_linked}));
    }
    ui.separator();
    // The pane's styles, in its order (Options… chooses which and how they sort).
    let listed = app.session.run("styles.paneList", &json!({})).unwrap_or_default();
    let sheet = &app.session.doc.styles;
    let list: Vec<(String, String, &str, bool)> = listed
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| {
            let st = sheet.get(x.get("id").and_then(Value::as_str)?)?;
            let mark = match x.get("type").and_then(Value::as_str) {
                Some("character") => "a",
                Some("linked") => "¶a",
                _ => "¶",
            };
            Some((st.id.clone(), crate::i18n::style_name(st), mark, st.builtin))
        })
        .collect();
    // Leave room below the list for Options….
    let list_height = (ui.available_height() - 32.0).max(60.0);
    egui::ScrollArea::vertical().max_height(list_height).show(ui, |ui| {
        for (id, name, mark, builtin) in list {
            ui.horizontal(|ui| {
                let r = ui.selectable_label(id == cur, &name);
                ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(mark).weak());
                });
                if r.clicked() {
                    let _ = app.run("para.style", json!({"style": id}));
                    app.canvas.want_focus = true;
                }
                crate::rtl::context_menu(&r, |ui| {
                    if ui.button(tl!("Modify…")).clicked() {
                        app.dialog = crate::dialogs::Dialog::modify_style(app, &id);
                        ui.close();
                    }
                    if ui.button(tl!("Update to Match Selection")).clicked() {
                        let _ = app.run("styles.updateToMatch", json!({"style": id}));
                        app.canvas.want_focus = true;
                        ui.close();
                    }
                    if ui.add_enabled(!builtin, egui::Button::new(tl!("Delete"))).clicked() {
                        let _ = app.run("styles.delete", json!({"style": id}));
                        app.canvas.want_focus = true;
                        ui.close();
                    }
                });
            });
        }
    });
    ui.separator();
    if ui.button(tl!("Options…")).clicked() {
        app.dialog = crate::dialogs::Dialog::open("stylePaneOptions", app);
    }
}
