//! Font Substitution (File › Options › Advanced › Show document content › Font Substitution…):
//! the fonts the document shows that aren't installed (`font.substitution`), each with the font
//! it is drawn in. The Substituted font box picks what a missing font is shown and printed in on
//! this computer (OK applies the choices with `font.setSubstitute`; Cancel drops them); Convert
//! Permanently… replaces the font in the document (`font.convertPermanently`, one undo step).
//! With nothing missing it is only a message.

use serde::Serialize;
use serde_json::Value;

use super::{Tokens, Ui, WordApp, json, semibold, vec2};

/// One row of the list, as `font.substitution` gives it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SubstitutionRow {
    /// The missing font, as the document names it.
    pub font: String,
    /// The families its text is drawn in now.
    pub shown_as: Vec<String>,
    /// The font picked in the dialog to show it in (`None`: Default).
    pub substitute: Option<String>,
    /// The choice when the dialog opened: OK changes only the rows that differ.
    pub was: Option<String>,
    /// The face Default draws the font in (`shown_as` holds the chosen font while there is a
    /// choice).
    pub default: Option<String>,
}

impl SubstitutionRow {
    fn read(v: &Value) -> SubstitutionRow {
        let substitute = v.get("substitute").and_then(Value::as_str).map(str::to_string);
        SubstitutionRow {
            font: v.get("font").and_then(Value::as_str).unwrap_or_default().to_string(),
            shown_as: v.get("shownAs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
            was: substitute.clone(),
            substitute,
            default: v.get("default").and_then(Value::as_str).map(str::to_string),
        }
    }

    /// The Substituted font column: the picked font, else the face Default draws it in.
    fn shown(&self) -> String {
        self.substitute.clone().or_else(|| self.default_face()).unwrap_or_else(|| tl!("Default").to_string())
    }

    /// The face the font is drawn in with no choice: as drawn now when there was none when the
    /// dialog opened, else Default's own substitute (not the choice being taken back).
    fn default_face(&self) -> Option<String> {
        match &self.was {
            None => self.shown_as.first().cloned(),
            Some(_) => self.default.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FontSubstitutionForm {
    pub rows: Vec<SubstitutionRow>,
    pub selected: usize,
    /// Convert Permanently… asked to confirm and waits for Yes or No.
    pub confirm_convert: bool,
}

impl FontSubstitutionForm {
    pub fn read(app: &mut WordApp) -> FontSubstitutionForm {
        let mut f = FontSubstitutionForm::default();
        f.reload(app);
        f
    }

    /// The missing fonts as the document has them now. Choices made in the dialog and not yet
    /// applied stay with their fonts.
    fn reload(&mut self, app: &mut WordApp) {
        let v = app.session.run("font.substitution", &json!({})).unwrap_or_default();
        let old = std::mem::take(&mut self.rows);
        self.rows = v.get("fonts").and_then(Value::as_array).map(|a| a.iter().map(SubstitutionRow::read).collect()).unwrap_or_default();
        for r in &mut self.rows {
            if let Some(o) = old.iter().find(|o| o.font.eq_ignore_ascii_case(&r.font)) {
                r.substitute = o.substitute.clone();
            }
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }
}

/// Returns true to close.
pub(super) fn font_substitution(app: &mut WordApp, ui: &mut Ui, f: &mut FontSubstitutionForm) -> bool {
    if f.rows.is_empty() {
        ui.label(tl!("No font substitution is occurring. All the fonts used in this document are available."));
        ui.add_space(8.0);
        let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close |= ui.add(crate::widgets::primary_button(ui, tl!("OK"))).clicked();
        });
        return close;
    }
    let t = Tokens::get(ui.ctx());
    list(ui, f, &t);
    ui.add_space(6.0);
    details(app, ui, f);
    if f.confirm_convert {
        confirm(app, ui, f);
    }
    let (ok, cancel) = super::buttons(ui, tl!("OK"));
    if ok {
        for r in f.rows.iter().filter(|r| r.substitute != r.was) {
            if let Err(e) = app.run("font.setSubstitute", json!({"font": r.font, "substitute": r.substitute})) {
                app.status(e);
            }
        }
    }
    ok || cancel
}

/// The list: Missing document font, Substituted font.
fn list(ui: &mut Ui, f: &mut FontSubstitutionForm, t: &Tokens) {
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.border)).inner_margin(4).show(ui, |ui| {
        ui.set_width(380.0);
        egui::ScrollArea::vertical().id_salt("font_substitution_list").max_height(140.0).auto_shrink([false, true]).show(ui, |ui| {
            egui::Grid::new("font_substitution_grid").num_columns(2).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                for head in ["Missing document font", "Substituted font"] {
                    ui.label(egui::RichText::new(tl!(head)).font(semibold(12.5)));
                }
                ui.end_row();
                let mut pick = None;
                for (i, r) in f.rows.iter().enumerate() {
                    if ui.selectable_label(i == f.selected, r.font.as_str()).clicked() {
                        pick = Some(i);
                    }
                    ui.label(r.shown());
                    ui.end_row();
                }
                if let Some(i) = pick {
                    f.selected = i;
                    f.confirm_convert = false;
                }
            });
        });
    });
}

/// The selected row: its name, and the Substituted font box (Default and every installed
/// family), with Convert Permanently….
fn details(app: &mut WordApp, ui: &mut Ui, f: &mut FontSubstitutionForm) {
    let Some(r) = f.rows.get(f.selected).cloned() else { return };
    ui.label(crate::i18n::fmt(tl!("Missing document font: {font}"), &[("font", &r.font)]));
    let mut pick: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        let label = ui.label(tl!("Substituted font:"));
        let shown = r.substitute.clone().unwrap_or_else(|| tl!("Default").to_string());
        craft_ui::ComboBox::from_id_salt("font_substitution_font")
            .width(200.0)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui.selectable_label(r.substitute.is_none(), tl!("Default")).clicked() {
                    pick = Some(None);
                }
                for fam in app.previews.families() {
                    let on = r.substitute.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(&fam));
                    if ui.selectable_label(on, fam.as_str()).clicked() {
                        pick = Some(Some(fam));
                    }
                }
            })
            .response
            .labelled_by(label.id);
    });
    if let Some(p) = pick
        && let Some(row) = f.rows.get_mut(f.selected)
    {
        row.substitute = p;
        f.confirm_convert = false;
    }
    ui.add_space(4.0);
    if ui.button(tl!("Convert Permanently…")).clicked() {
        f.confirm_convert = true;
    }
}

/// The font Convert Permanently… puts in place of `r`'s: the one picked, else the one it is
/// drawn in.
fn convert_to(r: &SubstitutionRow) -> Option<String> {
    r.substitute.clone().or_else(|| r.default_face())
}

/// Convert Permanently…'s question, Yes and No.
fn confirm(app: &mut WordApp, ui: &mut Ui, f: &mut FontSubstitutionForm) {
    let Some(r) = f.rows.get(f.selected).cloned() else {
        f.confirm_convert = false;
        return;
    };
    let Some(to) = convert_to(&r) else {
        f.confirm_convert = false;
        return;
    };
    ui.add_space(6.0);
    ui.label(crate::i18n::fmt(tl!("Replace {font} with {to} everywhere in the document?"), &[("font", &r.font), ("to", &to)]));
    ui.horizontal(|ui| {
        if ui.button(tl!("Yes")).clicked() {
            f.confirm_convert = false;
            if let Err(e) = app.run("font.convertPermanently", json!({"font": r.font, "to": to})) {
                app.status(e);
            }
            f.reload(app);
        }
        if ui.button(tl!("No")).clicked() {
            f.confirm_convert = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn going_back_to_default_shows_the_default_substitute() {
        let v = json!({"font": "Gone", "shownAs": ["Chosen Face"], "substitute": "Chosen Face", "default": "Default Face"});
        let mut r = SubstitutionRow::read(&v);
        assert_eq!(r.shown(), "Chosen Face");
        r.substitute = None;
        assert_eq!(r.shown(), "Default Face", "not the choice being taken back");
        assert_eq!(convert_to(&r).as_deref(), Some("Default Face"));
        // No choice when the dialog opened: the face it is drawn in now.
        let r = SubstitutionRow::read(&json!({"font": "Gone", "shownAs": ["Drawn Face"], "substitute": null, "default": "Other"}));
        assert_eq!(r.shown(), "Drawn Face");
    }
}
