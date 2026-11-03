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
                        if let Some(entries) = section.get("entries").and_then(Value::as_array) {
                            for entry in entries.iter().take(64) {
                                ui.add(egui::Label::new(describe(entry)).wrap());
                            }
                        }
                    });
                }
            });
        }
    });
}

/// One line of the pane in the interface language: `Left: 1"`, `14 pt`, `Line spacing: single`.
fn describe(entry: &Value) -> String {
    let fmt = crate::i18n::fmt;
    let number = |key: &str| entry.get(key).and_then(Value::as_f64).map(trim_num);
    let value = if let Some(font) = entry.get("font").and_then(Value::as_str) {
        font.to_string()
    } else if let Some(style) = entry.get("style").and_then(Value::as_str) {
        style.to_string()
    } else if let Some(lang) = entry.get("lang").and_then(Value::as_str) {
        tl!(&crate::ribbon::language_name(Some(lang))).to_string()
    } else if let Some(hex) = entry.get("hex").and_then(Value::as_str) {
        format!("#{hex}")
    } else if let Some(n) = number("pt") {
        fmt(tl!("{n} pt"), &[("n", &n)])
    } else if let Some(n) = number("inches") {
        fmt(tl!("{n}\""), &[("n", &n)])
    } else if let Some(n) = number("multiple") {
        fmt(tl!("Multiple {n} li"), &[("n", &n)])
    } else if let Some(n) = number("atLeast") {
        fmt(tl!("At least {n} pt"), &[("n", &n)])
    } else if let Some(n) = number("exactly") {
        fmt(tl!("Exactly {n} pt"), &[("n", &n)])
    } else if let Some(text) = entry.get("text").and_then(Value::as_str) {
        tl!(text).to_string()
    } else {
        String::new()
    };
    match entry.get("label").and_then(Value::as_str) {
        Some(label) => format!("{}: {value}", tl!(label)),
        None => value,
    }
}

fn trim_num(x: f64) -> String {
    let s = format!("{:.2}", if x.is_finite() { x } else { 0.0 });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}
