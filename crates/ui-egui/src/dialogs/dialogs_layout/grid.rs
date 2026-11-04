use super::{MeasureExt, Serialize, Ui, Value, WordApp, buttons, heading, json, unit, vec2};

// ---------------------------------------------------------------------------------------------
// Grid and Guides

/// The Grid and Guides dialog's fields (Layout › Align › Grid Settings…). Spacings and origins
/// are in the interface unit; the intervals are in grid units (0 shows none of that direction).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSettingsForm {
    pub horizontal_spacing: f32,
    pub vertical_spacing: f32,
    pub horizontal_every: u8,
    pub vertical_every: u8,
    pub use_margins: bool,
    pub horizontal_origin: f32,
    pub vertical_origin: f32,
    /// Display gridlines on screen (`view.gridlines`), and how it was when the dialog opened.
    pub gridlines: bool,
    pub gridlines_was: bool,
}

impl GridSettingsForm {
    pub fn read(app: &WordApp) -> GridSettingsForm {
        let s = &app.session.doc.settings;
        let ppu = unit().pt_per_unit();
        GridSettingsForm {
            horizontal_spacing: s.grid_h / ppu,
            vertical_spacing: s.grid_v / ppu,
            horizontal_every: s.grid_every_h.min(127),
            vertical_every: s.grid_every_v.min(127),
            use_margins: s.grid_use_margins,
            horizontal_origin: s.grid_origin_x / ppu,
            vertical_origin: s.grid_origin_y / ppu,
            gridlines: app.session.view.gridlines,
            gridlines_was: app.session.view.gridlines,
        }
    }

    /// `view.gridSettings` parameters.
    pub fn params(&self) -> Value {
        let ppu = unit().pt_per_unit();
        json!({
            "horizontalSpacing": self.horizontal_spacing * ppu,
            "verticalSpacing": self.vertical_spacing * ppu,
            "horizontalEvery": self.horizontal_every,
            "verticalEvery": self.vertical_every,
            "useMargins": self.use_margins,
            "horizontalOrigin": self.horizontal_origin * ppu,
            "verticalOrigin": self.vertical_origin * ppu,
        })
    }
}

pub(super) fn grid_settings_ui(app: &mut WordApp, ui: &mut Ui, f: &mut GridSettingsForm) -> bool {
    let ppu = unit().pt_per_unit();
    let speed = 3.6 / ppu;
    heading(ui, "Grid settings");
    egui::Grid::new("grid_settings").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        let h = ui.label(tl!("Horizontal spacing:"));
        ui.add(egui::DragValue::new(&mut f.horizontal_spacing).speed(speed).range(0.5 / ppu..=1584.0 / ppu).measure(unit()).max_decimals(2))
            .labelled_by(h.id);
        ui.end_row();
        let v = ui.label(tl!("Vertical spacing:"));
        ui.add(egui::DragValue::new(&mut f.vertical_spacing).speed(speed).range(0.5 / ppu..=1584.0 / ppu).measure(unit()).max_decimals(2))
            .labelled_by(v.id);
        ui.end_row();
    });
    heading(ui, "Grid origin");
    ui.checkbox(&mut f.use_margins, tl!("Use margins"));
    egui::Grid::new("grid_origin").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        let h = ui.add_enabled(!f.use_margins, egui::Label::new(tl!("Horizontal origin:")));
        ui.add_enabled(
            !f.use_margins,
            egui::DragValue::new(&mut f.horizontal_origin).speed(speed).range(0.0..=1584.0 / ppu).measure(unit()).max_decimals(2),
        )
        .labelled_by(h.id);
        ui.end_row();
        let v = ui.add_enabled(!f.use_margins, egui::Label::new(tl!("Vertical origin:")));
        ui.add_enabled(
            !f.use_margins,
            egui::DragValue::new(&mut f.vertical_origin).speed(speed).range(0.0..=1584.0 / ppu).measure(unit()).max_decimals(2),
        )
        .labelled_by(v.id);
        ui.end_row();
    });
    heading(ui, "Show grid");
    ui.checkbox(&mut f.gridlines, tl!("Display gridlines on screen"));
    egui::Grid::new("grid_show").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        // These names describe the lines drawn; the command's *Every names follow the OOXML
        // elements, whose horizontal/vertical names describe the grid units instead.
        let v = ui.label(tl!("Vertical gridlines every:"));
        ui.add(egui::DragValue::new(&mut f.horizontal_every).range(0..=127)).labelled_by(v.id);
        ui.end_row();
        let h = ui.label(tl!("Horizontal gridlines every:"));
        ui.add(egui::DragValue::new(&mut f.vertical_every).range(0..=127)).labelled_by(h.id);
        ui.end_row();
    });
    let (ok, cancel) = buttons(ui, "OK", true);
    if ok {
        // A refusal keeps the dialog open with the reason on the status bar.
        if let Err(e) = app.run("view.gridSettings", f.params()) {
            app.status(e);
            return false;
        }
        if f.gridlines != f.gridlines_was
            && let Err(e) = app.run("view.gridlines", json!({"value": f.gridlines}))
        {
            app.status(e);
            return false;
        }
        return true;
    }
    cancel
}
