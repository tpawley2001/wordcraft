use super::{ENVELOPE_SIZES, LABEL_PRODUCTS, MeasureExt, Serialize, Ui, Value, WordApp, buttons, heading, json, run, unit};

// ---------------------------------------------------------------------------------------------
// Envelopes

/// An address's optional position (points) and style font overrides.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeAddressForm {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub font_name: String,
    pub font_size: Option<f32>,
    pub show_font: bool,
}

impl EnvelopeAddressForm {
    fn params(&self, v: &mut Value, prefix: &str) {
        if let Some(n) = self.left {
            v[format!("{prefix}Left")] = json!(n);
        }
        if let Some(n) = self.top {
            v[format!("{prefix}Top")] = json!(n);
        }
        let mut font = json!({});
        if !self.font_name.is_empty() {
            font["name"] = json!(self.font_name);
        }
        if let Some(n) = self.font_size {
            font["size"] = json!(n);
        }
        v[format!("{prefix}Font")] = font;
    }

    fn ui(&mut self, ui: &mut Ui, w: f32, h: f32, defaults: (f32, f32), size: f32) {
        if ui.button(tl!("Font…")).clicked() {
            self.show_font = !self.show_font;
        }
        if self.show_font {
            ui.horizontal(|ui| {
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(&mut self.font_name);
            });
            ui.horizontal(|ui| {
                let mut change = self.font_size.is_some();
                if ui.checkbox(&mut change, tl!("Size:")).changed() {
                    self.font_size = change.then_some(size);
                }
                if let Some(n) = &mut self.font_size {
                    ui.add(egui::DragValue::new(n).range(1.0..=1638.0).suffix(" pt"));
                }
            });
        }
        for (label, value, max, default) in [(tl!("From left:"), &mut self.left, w, defaults.0), (tl!("From top:"), &mut self.top, h, defaults.1)] {
            ui.horizontal(|ui| {
                ui.label(label);
                let mut auto = value.is_none();
                if ui.checkbox(&mut auto, tl!("Auto")).changed() {
                    *value = (!auto).then_some(default);
                }
                if let Some(n) = value {
                    let k = unit().pt_per_unit();
                    let mut display = *n / k;
                    if ui.add(egui::DragValue::new(&mut display).speed(0.01).range(0.0..=max / k).measure(unit()).max_decimals(2)).changed() {
                        *n = display * k;
                    }
                }
            });
        }
    }
}

/// The Envelopes dialog's fields. `size` indexes [`ENVELOPE_SIZES`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeForm {
    pub delivery: String,
    pub ret: String,
    pub omit_return: bool,
    pub size: usize,
    pub delivery_options: EnvelopeAddressForm,
    pub return_options: EnvelopeAddressForm,
    #[serde(skip)]
    pub options_draft: Option<Box<EnvelopeForm>>,
}

impl EnvelopeForm {
    /// The delivery address starts as the selected text (an address in the letter).
    pub fn read(app: &WordApp) -> EnvelopeForm {
        let sel = if app.session.sel.is_collapsed() { String::new() } else { app.session.selected_text() };
        EnvelopeForm {
            delivery: sel.trim().replace("\r\n", "\n").replace('\r', "\n"),
            ret: String::new(),
            omit_return: false,
            size: 0,
            delivery_options: EnvelopeAddressForm::default(),
            return_options: EnvelopeAddressForm::default(),
            options_draft: None,
        }
    }

    /// `mailings.envelopes` parameters; `add` puts the envelope at the start of this document.
    pub fn params(&self, add: bool) -> Value {
        let size = ENVELOPE_SIZES.get(self.size).map_or("Envelope #10", |s| s.0);
        let mut v = json!({"delivery": self.delivery, "return": self.ret, "omitReturn": self.omit_return, "size": size, "add": add});
        self.delivery_options.params(&mut v, "delivery");
        self.return_options.params(&mut v, "return");
        v
    }
}

pub(super) fn envelopes_ui(app: &mut WordApp, ui: &mut Ui, f: &mut EnvelopeForm) -> bool {
    if let Some(mut draft) = f.options_draft.take() {
        heading(ui, tl!("Envelope Options"));
        envelope_size_ui(ui, &mut draft.size);
        let (_, w, h) = ENVELOPE_SIZES.get(draft.size).copied().unwrap_or(("Envelope #10", 684.0, 297.0));
        ui.push_id("delivery_options", |ui| {
            heading(ui, tl!("Delivery address:"));
            draft.delivery_options.ui(ui, w, h, (w * 0.42, h * 0.45), 12.0);
        });
        ui.push_id("return_options", |ui| {
            heading(ui, tl!("Return address:"));
            draft.return_options.ui(ui, w, h, (22.0, 22.0), 10.0);
        });
        let (ok, cancel) = buttons(ui, tl!("OK"), true);
        if ok {
            *f = *draft;
        } else if !cancel {
            f.options_draft = Some(draft);
        }
        return false;
    }
    ui.label(tl!("Delivery address:"));
    ui.add(egui::TextEdit::multiline(&mut f.delivery).desired_rows(4).desired_width(340.0));
    ui.horizontal(|ui| {
        ui.label(tl!("Return address:"));
        ui.checkbox(&mut f.omit_return, tl!("Omit"));
    });
    ui.add_enabled(!f.omit_return, egui::TextEdit::multiline(&mut f.ret).desired_rows(3).desired_width(340.0));
    if ui.button(tl!("Options…")).clicked() {
        f.options_draft = Some(Box::new(f.clone()));
    }
    let ready = !f.delivery.trim().is_empty();
    ui.add_space(8.0);
    let (mut add, mut new, mut cancel) = (false, false, false);
    ui.horizontal(|ui| {
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            cancel = ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
            new = ui.add_enabled(ready, egui::Button::new(tl!("New Document"))).clicked();
            add = ui.add_enabled(ready, crate::widgets::primary_button(ui, tl!("Add to Document"))).clicked();
        });
    });
    if add || new {
        return run(app, "mailings.envelopes", f.params(add));
    }
    cancel
}

fn envelope_size_ui(ui: &mut Ui, size: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(tl!("Envelope size:"));
        let label = |i: usize| ENVELOPE_SIZES.get(i).map(|(n, w, h)| envelope_label(n, *w, *h)).unwrap_or_default();
        craft_ui::ComboBox::from_id_salt("env_size").selected_text(label(*size)).width(220.0).show_ui(ui, |ui| {
            for i in 0..ENVELOPE_SIZES.len() {
                ui.selectable_value(size, i, label(i));
            }
        });
    });
}

/// `DL   110 × 220 mm` / `#10   4.13 × 9.5 in`: metric sizes in millimetres, the others in
/// inches. The pattern is translated (the unit's name, the order) and the numbers use the
/// region's decimal separator.
pub(crate) fn envelope_label(name: &str, w: f32, h: f32) -> String {
    let short = name.strip_prefix("Envelope ").unwrap_or(name);
    let (template, w, h) = if short.starts_with('#') || short == "Monarch" {
        let i = |pt: f32| ((pt / 72.0) * 100.0).round() / 100.0;
        (tl!("{name}   {height} × {width} in"), i(w), i(h))
    } else {
        let mm = |pt: f32| (pt * 25.4 / 72.0).round();
        (tl!("{name}   {height} × {width} mm"), mm(w), mm(h))
    };
    crate::i18n::fmt(template, &[("name", short), ("height", &with_decimal(h.to_string())), ("width", &with_decimal(w.to_string()))])
}

/// A number written with `.` rewritten with the region's decimal separator (`2,625`).
fn with_decimal(n: String) -> String {
    let sep = crate::options::decimal();
    if sep == '.' { n } else { n.replace('.', sep.encode_utf8(&mut [0; 4])) }
}

// ---------------------------------------------------------------------------------------------
// Labels

/// The Labels dialog's fields. `product` indexes [`LABEL_PRODUCTS`]; `row`/`col` count from 1.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelsForm {
    pub text: String,
    pub product: usize,
    pub single: bool,
    pub row: usize,
    pub col: usize,
    /// One label per recipient (when a recipient list is loaded).
    pub recipients: bool,
}

impl LabelsForm {
    pub fn read(app: &WordApp) -> LabelsForm {
        let sel = if app.session.sel.is_collapsed() { String::new() } else { app.session.selected_text() };
        LabelsForm {
            text: sel.trim().replace("\r\n", "\n").replace('\r', "\n"),
            product: 0,
            single: false,
            row: 1,
            col: 1,
            recipients: !app.session.merge.rows.is_empty(),
        }
    }

    /// `mailings.labels` parameters.
    pub fn params(&self) -> Value {
        let id = LABEL_PRODUCTS.get(self.product).map_or("letter-3x10", |p| p.id);
        let mut v = json!({"text": self.text, "product": id, "fromRecipients": self.recipients && !self.single});
        if self.single {
            v["single"] = json!({"row": self.row, "col": self.col});
        }
        v
    }
}

/// `Address labels, 3 × 10, 2.625" × 1" (Letter)`.
pub(super) fn product_label(i: usize) -> String {
    let Some(p) = LABEL_PRODUCTS.get(i) else { return String::new() };
    let kind = match p.kind {
        "shipping" => tl!("Shipping labels"),
        "return" => tl!("Return address labels"),
        _ => tl!("Address labels"),
    };
    // Inches on Letter, millimetres on A4, as the sheets are sold.
    let size = if p.paper == "A4" {
        let mm = |pt: f32| with_decimal(format!("{:.1}", pt * 25.4 / 72.0));
        crate::i18n::fmt(tl!("{width} × {height} mm"), &[("width", &mm(p.width)), ("height", &mm(p.height))])
    } else {
        let i = |pt: f32| with_decimal((((pt / 72.0) * 1000.0).round() / 1000.0).to_string());
        format!("{}\" × {}\"", i(p.width), i(p.height))
    };
    let (columns, rows) = (p.cols.to_string(), p.rows.to_string());
    crate::i18n::fmt(
        tl!("{kind}, {columns} × {rows}, {size} ({paper})"),
        &[("kind", kind), ("columns", &columns), ("rows", &rows), ("size", &size), ("paper", tl!(p.paper))],
    )
}

pub(super) fn labels_ui(app: &mut WordApp, ui: &mut Ui, f: &mut LabelsForm) -> bool {
    let has_recipients = !app.session.merge.rows.is_empty();
    if has_recipients {
        ui.checkbox(&mut f.recipients, tl!("One label per recipient"));
    }
    ui.add_enabled_ui(!(has_recipients && f.recipients && !f.single), |ui| {
        ui.label(tl!("Address:"));
        ui.add(egui::TextEdit::multiline(&mut f.text).desired_rows(4).desired_width(340.0));
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Label:"));
        craft_ui::ComboBox::from_id_salt("label_product").selected_text(product_label(f.product)).width(300.0).show_ui(ui, |ui| {
            for i in 0..LABEL_PRODUCTS.len() {
                ui.selectable_value(&mut f.product, i, product_label(i));
            }
        });
    });
    let (rows, cols) = LABEL_PRODUCTS.get(f.product).map_or((1, 1), |p| (p.rows, p.cols));
    heading(ui, "Print");
    ui.radio_value(&mut f.single, false, tl!("Full page of the same label"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.single, true, tl!("Single label"));
        ui.add_enabled_ui(f.single, |ui| {
            ui.label(tl!("Row:"));
            ui.add(egui::DragValue::new(&mut f.row).range(1..=rows));
            ui.label(tl!("Column:"));
            ui.add(egui::DragValue::new(&mut f.col).range(1..=cols));
        });
    });
    f.row = f.row.clamp(1, rows.max(1));
    f.col = f.col.clamp(1, cols.max(1));
    let ready = (has_recipients && f.recipients && !f.single) || !f.text.trim().is_empty();
    let (ok, cancel) = buttons(ui, "New Document", ready);
    if ok {
        return run(app, "mailings.labels", f.params());
    }
    cancel
}
