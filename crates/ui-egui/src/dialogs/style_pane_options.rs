//! Styles pane › Options… (Style Pane Options): which styles the pane shows and how it sorts
//! them. OK runs `styles.paneOptions`, so agents get the same result without the dialog.

use egui::Ui;
use serde::Serialize;
use serde_json::{Value, json};

use crate::WordApp;
use crate::dialogs::buttons;

/// "Select styles to show:" — (parameter value, label).
const SHOW: [(&str, &str); 2] = [("inUse", "In use"), ("all", "All styles")];
/// "Select how list is sorted:" — (parameter value, label).
const SORT: [(&str, &str); 5] =
    [("alphabetical", "Alphabetical"), ("recommended", "As Recommended"), ("font", "Font"), ("basedOn", "Based on"), ("type", "By type")];

/// The dialog's fields: `styles.paneOptions`' parameter values.
#[derive(Clone, Debug, Serialize)]
pub struct StylePaneForm {
    pub show: String,
    pub sort: String,
}

impl StylePaneForm {
    pub fn read(app: &mut WordApp) -> StylePaneForm {
        let v = app.session.run("styles.paneOptions", &json!({})).unwrap_or_default();
        let get = |k: &str, default: &str| v.get(k).and_then(Value::as_str).unwrap_or(default).to_string();
        StylePaneForm { show: get("show", "all"), sort: get("sort", "alphabetical") }
    }
}

/// A combo box over `options`, choosing a parameter value.
fn combo(ui: &mut Ui, id: &str, label: &str, value: &mut String, options: &[(&str, &str)]) {
    let label = ui.label(label);
    let shown = options.iter().find(|(v, _)| v == value).map(|(_, l)| tl!(l)).unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(220.0)
        .show_ui(ui, |ui| {
            for (v, l) in options {
                ui.selectable_value(value, (*v).to_string(), tl!(l));
            }
        })
        .response
        .labelled_by(label.id);
}

/// Returns true to close.
pub fn style_pane_options(app: &mut WordApp, ui: &mut Ui, f: &mut StylePaneForm) -> bool {
    combo(ui, "stylePaneShow", tl!("Select styles to show:"), &mut f.show, &SHOW);
    ui.add_space(6.0);
    combo(ui, "stylePaneSort", tl!("Select how list is sorted:"), &mut f.sort, &SORT);
    ui.add_space(6.0);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let _ = app.run("styles.paneOptions", json!({"show": f.show, "sort": f.sort}));
    }
    ok || cancel
}
