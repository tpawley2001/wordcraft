//! Header & Footer › Insert Alignment Tab: where the positional tab goes (left, centre or right
//! of the margins or the indents) and its leader. OK runs `insert.alignmentTab`.

use super::{Serialize, Ui, Value, WordApp, buttons, json};

/// The leaders in the dialog's list: the command's value and the label shown.
pub const ALIGNMENT_TAB_LEADERS: [(&str, &str); 5] =
    [("none", "1 None"), ("dot", "2 ......."), ("hyphen", "3 -------"), ("underscore", "4 _______"), ("middleDot", "5 · · · ·")];

/// The Alignment Tab dialog's fields, with the command's values.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentTabForm {
    /// `left`, `center` or `right`.
    pub align: &'static str,
    /// `margin` or `indent`.
    pub relative_to: &'static str,
    /// `none`, `dot`, `hyphen`, `underscore` or `middleDot`.
    pub leader: &'static str,
}

impl Default for AlignmentTabForm {
    /// Left, relative to the margin, no leader, as Word opens it.
    fn default() -> Self {
        AlignmentTabForm { align: "left", relative_to: "margin", leader: "none" }
    }
}

impl AlignmentTabForm {
    /// The `insert.alignmentTab` parameters.
    pub fn params(&self) -> Value {
        json!({"align": self.align, "relativeTo": self.relative_to, "leader": self.leader})
    }
}

/// Draws the dialog; returns true to close it.
pub(super) fn alignment_tab_ui(app: &mut WordApp, ui: &mut Ui, f: &mut AlignmentTabForm) -> bool {
    ui.group(|ui| {
        ui.label(tl!("Alignment"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.align, "left", tl!("Left"));
            ui.radio_value(&mut f.align, "center", tl!("Center"));
            ui.radio_value(&mut f.align, "right", tl!("Right"));
        });
        ui.add_space(4.0);
        ui.label(tl!("Align relative to:"));
        let shown = if f.relative_to == "indent" { "Indent" } else { "Margin" };
        craft_ui::ComboBox::from_id_salt("alignment_tab_relative").selected_text(tl!(shown)).width(140.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut f.relative_to, "margin", tl!("Margin"));
            ui.selectable_value(&mut f.relative_to, "indent", tl!("Indent"));
        });
    });
    ui.add_space(8.0);
    ui.group(|ui| {
        ui.label(tl!("Leader"));
        for (value, label) in ALIGNMENT_TAB_LEADERS {
            ui.radio_value(&mut f.leader, value, tl!(label));
        }
    });
    let (ok, cancel) = buttons(ui, "OK", "Cancel", true);
    if ok {
        alignment_tab_ok(app, f);
    }
    ok || cancel
}

/// The dialog's OK: insert the alignment tab it describes at the caret.
pub fn alignment_tab_ok(app: &mut WordApp, f: &AlignmentTabForm) {
    if let Err(message) = app.run("insert.alignmentTab", f.params()) {
        app.status(message);
    }
}
