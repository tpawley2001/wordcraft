use super::{Ui, Value, WordApp};
use serde_json::json;

/// Formatting resolved at the current caret, refreshed on every frame.
pub(super) fn reveal(app: &mut WordApp, ui: &mut Ui) {
    let formatting = wordcraft_engine::cmd::reveal::reveal(&app.session);
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut compare = app.session.view.reveal_compare.is_some();
        if ui.checkbox(&mut compare, tl!("Compare to another selection")).changed() {
            let _ = app.run("view.revealCompare", json!({"value": compare}));
        }
        let mut style_source = app.session.view.reveal_style_source;
        if ui.checkbox(&mut style_source, tl!("Distinguish style source")).changed() {
            let _ = app.run("view.revealStyleSource", json!({"value": style_source}));
        }
        if let Some(differences) = formatting.get("differences").and_then(Value::as_array) {
            ui.label(egui::RichText::new(tl!("Formatting differences")).strong());
            ui.indent("revealDifferences", |ui| {
                for difference in differences.iter().take(256) {
                    let first = side(difference.get("first"), difference.get("second"));
                    let second = side(difference.get("second"), difference.get("first"));
                    let line = crate::i18n::fmt(tl!("{first} -> {second}"), &[("first", &first), ("second", &second)]);
                    ui.add(egui::Label::new(line).wrap());
                }
            });
            ui.separator();
        }
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
                                ui.horizontal_wrapped(|ui| {
                                    ui.add(egui::Label::new(describe(entry)).wrap());
                                    if let Some(source) = source(entry) {
                                        ui.label(egui::RichText::new(source).weak());
                                    }
                                });
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

/// Where an entry's formatting comes from, as Distinguish style source shows it after the entry:
/// `(Heading 1)` for a style, `(direct)` for direct formatting, nothing for the defaults.
fn source(entry: &Value) -> Option<String> {
    match entry.get("source").and_then(Value::as_str)? {
        "style" => entry.get("styleName").and_then(Value::as_str).map(|name| format!("({name})")),
        "direct" => Some(format!("({})", tl!("direct"))),
        _ => None,
    }
}

/// One side of a difference: a missing FONT effect reads `Not Bold`, any other missing side is empty.
fn side(entry: Option<&Value>, other: Option<&Value>) -> String {
    let Some(entry) = entry.filter(|e| !e.is_null()) else {
        let effect =
            other.and_then(|e| e.get("text")).and_then(Value::as_str).filter(|_| other.is_some_and(|e| e.get("label").is_some_and(Value::is_null)));
        return effect.map(|text| format!("{} {text}", tl!("Not"))).unwrap_or_default();
    };
    describe(entry)
}

fn trim_num(x: f64) -> String {
    let s = format!("{:.2}", if x.is_finite() { x } else { 0.0 });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}
