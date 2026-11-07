use super::{Tokens, Ui, Value, WordApp, json};

/// The row being renamed and its draft name, kept in egui memory while the pane is open.
#[derive(Clone, Default)]
struct Renaming {
    index: Option<usize>,
    draft: String,
}

/// Selection Pane (Layout › Arrange, Alt+F10): the body's drawings by name, front-most first.
/// Clicking a row selects its object; the eye shows or hides it; double-click or F2 renames it.
pub(super) fn selection(app: &mut WordApp, ui: &mut Ui) {
    let list = app.session.run("arrange.selectionPane", &json!({})).unwrap_or_default();
    let entries: Vec<(String, bool, bool)> = list
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|e| {
            let name = e.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            (name, e.get("hidden").and_then(Value::as_bool).unwrap_or(false), e.get("selected").and_then(Value::as_bool).unwrap_or(false))
        })
        .collect();
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!entries.is_empty(), egui::Button::new(tl!("Show All"))).clicked() {
            let _ = app.run("arrange.showAll", json!({}));
        }
        if ui.add_enabled(!entries.is_empty(), egui::Button::new(tl!("Hide All"))).clicked() {
            let _ = app.run("arrange.hideAll", json!({}));
        }
    });
    ui.separator();
    if entries.is_empty() {
        ui.label(egui::RichText::new(tl!("There are no pictures or shapes in the document.")).small().weak());
        return;
    }
    let id = ui.id().with("selection_pane_rename");
    let mut renaming: Renaming = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
    if renaming.index.is_some_and(|i| i >= entries.len()) {
        renaming = Renaming::default();
    }
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, (name, hidden, selected)) in entries.iter().enumerate() {
            ui.horizontal(|ui| {
                let tip = if *hidden { tl!("Show") } else { tl!("Hide") };
                if eye_button(ui, !hidden, tip, &t).clicked() {
                    let _ = app.run("arrange.setVisible", json!({"index": i, "visible": *hidden}));
                }
                if renaming.index == Some(i) {
                    let r = ui.add(egui::TextEdit::singleline(&mut renaming.draft).desired_width(ui.available_width()));
                    if !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    let (enter, escape) = ui.input(|inp| (inp.key_pressed(egui::Key::Enter), inp.key_pressed(egui::Key::Escape)));
                    if escape {
                        renaming = Renaming::default();
                    } else if r.lost_focus() || enter {
                        if enter || !renaming.draft.trim().is_empty() {
                            let _ = app.run("arrange.rename", json!({"index": i, "name": renaming.draft}));
                        }
                        renaming = Renaming::default();
                    }
                    return;
                }
                let text = if *hidden { egui::RichText::new(name).weak() } else { egui::RichText::new(name) };
                let r = ui.add(egui::Button::selectable(*selected, text).truncate());
                if r.clicked() {
                    let _ = app.run("arrange.selectObject", json!({"index": i}));
                }
                if r.double_clicked() || (r.has_focus() && ui.input(|inp| inp.key_pressed(egui::Key::F2))) {
                    renaming = Renaming { index: Some(i), draft: name.clone() };
                }
            });
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, renaming));
}

/// The eye that shows (open) or hides (closed) a row's object; `label` is its accessible name.
fn eye_button(ui: &mut Ui, open: bool, label: &str, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, t.hover);
    }
    let icon = if open { "eye" } else { "eyeClosed" };
    crate::icons::paint(ui.painter(), egui::Rect::from_center_size(r.center(), egui::vec2(14.0, 14.0)), icon, t.icon, t.accent);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp.on_hover_text(label)
}
