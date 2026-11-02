use super::{Ui, Value, WordApp};

/// Formatting resolved at the current caret, refreshed on every frame.
pub(super) fn reveal(app: &mut WordApp, ui: &mut Ui) {
    let formatting = wordcraft_engine::cmd::reveal::reveal(&app.session);
    egui::ScrollArea::vertical().show(ui, |ui| {
        let Some(groups) = formatting.get("groups").and_then(Value::as_array) else { return };
        for group in groups {
            let name = group.get("name").and_then(Value::as_str).unwrap_or_default();
            egui::CollapsingHeader::new(tl!(name)).id_salt(("revealFormatting", name)).default_open(true).show(ui, |ui| {
                let Some(sections) = group.get("sections").and_then(Value::as_array) else { return };
                for (index, section) in sections.iter().enumerate() {
                    let heading = section.get("heading").and_then(Value::as_str).unwrap_or_default();
                    ui.label(egui::RichText::new(tl!(heading)).small().strong());
                    ui.indent(index, |ui| {
                        if let Some(items) = section.get("items").and_then(Value::as_array) {
                            for item in items.iter().filter_map(Value::as_str) {
                                ui.add(egui::Label::new(item).wrap());
                            }
                        }
                    });
                }
            });
        }
    });
}
