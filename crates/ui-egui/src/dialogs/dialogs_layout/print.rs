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

/// Custom sheet measurements, stored in points like the engine command.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomLabel {
    pub name: String,
    pub top: f32,
    pub side: f32,
    pub width: f32,
    pub height: f32,
    pub h_pitch: f32,
    pub v_pitch: f32,
    pub cols: usize,
    pub rows: usize,
    pub page_width: f32,
    pub page_height: f32,
}

impl CustomLabel {
    fn from_product(p: &wordcraft_engine::cmd::mailings::LabelProduct) -> Self {
        let (page_width, page_height) = if p.paper == "A4" { (595.3, 841.9) } else { (612.0, 792.0) };
        Self {
            name: tl!("Custom label").into(),
            top: p.top,
            side: p.side,
            width: p.width,
            height: p.height,
            h_pitch: p.h_pitch,
            v_pitch: p.v_pitch,
            cols: p.cols,
            rows: p.rows,
            page_width,
            page_height,
        }
    }

    pub fn fits(&self) -> bool {
        let extent = |margin: f32, pitch: f32, count: usize, size: f32| {
            f64::from(margin) + f64::from(pitch) * count.saturating_sub(1) as f64 + f64::from(size)
        };
        [self.top, self.side, self.width, self.height, self.h_pitch, self.v_pitch, self.page_width, self.page_height]
            .iter()
            .all(|n| n.is_finite() && *n >= 0.0)
            && self.width > 0.0
            && self.height > 0.0
            && self.page_width > 0.0
            && self.page_height > 0.0
            && (1..=10).contains(&self.cols)
            && (1..=40).contains(&self.rows)
            && self.h_pitch >= self.width
            && self.v_pitch >= self.height
            && extent(self.side, self.h_pitch, self.cols, self.width) <= f64::from(self.page_width) + 0.01
            && extent(self.top, self.v_pitch, self.rows, self.height) <= f64::from(self.page_height) + 0.01
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(tl!("Label name:"));
            ui.text_edit_singleline(&mut self.name);
        });
        let k = unit().pt_per_unit();
        crate::rtl::Table::new("custom_label_lengths").columns(2).show(ui, |t| {
            for (label, pt) in [
                (tl!("Top margin:"), &mut self.top),
                (tl!("Side margin:"), &mut self.side),
                (tl!("Vertical pitch:"), &mut self.v_pitch),
                (tl!("Horizontal pitch:"), &mut self.h_pitch),
                (tl!("Label height:"), &mut self.height),
                (tl!("Label width:"), &mut self.width),
            ] {
                t.row(|r| {
                    let name = r.cell(|ui| ui.label(label));
                    let mut display = *pt / k;
                    if r.cell(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut display)
                                .speed(1.0 / k)
                                .range(0.0..=4000.0 / k)
                                .clamp_existing_to_range(false)
                                .measure(unit())
                                .max_decimals(2)
                                .update_while_editing(false),
                        )
                        .labelled_by(name.id)
                        .changed()
                    }) {
                        *pt = display * k;
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl!("Number across:"));
            ui.add(egui::DragValue::new(&mut self.cols).range(1..=10));
            ui.label(tl!("Number down:"));
            ui.add(egui::DragValue::new(&mut self.rows).range(1..=40));
        });
        ui.horizontal(|ui| {
            ui.label(tl!("Page size:"));
            let mut a4 = self.page_width == 595.3 && self.page_height == 841.9;
            craft_ui::ComboBox::from_id_salt("custom_label_page").selected_text(if a4 { tl!("A4") } else { tl!("Letter") }).show_ui(ui, |ui| {
                if ui.selectable_value(&mut a4, false, tl!("Letter")).clicked() {
                    (self.page_width, self.page_height) = (612.0, 792.0);
                }
                if ui.selectable_value(&mut a4, true, tl!("A4")).clicked() {
                    (self.page_width, self.page_height) = (595.3, 841.9);
                }
            });
        });
    }
}

/// The Labels dialog's fields. `product` indexes [`LABEL_PRODUCTS`]; `row`/`col` count from 1.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelsForm {
    pub text: String,
    pub product: usize,
    pub custom: Option<CustomLabel>,
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
            custom: None,
            single: false,
            row: 1,
            col: 1,
            recipients: !app.session.merge.rows.is_empty(),
        }
    }

    /// `mailings.labels` parameters.
    pub fn params(&self) -> Value {
        let id = LABEL_PRODUCTS.get(self.product).map_or("letter-3x10", |p| p.id);
        let mut v = json!({"text": self.text, "fromRecipients": self.recipients && !self.single});
        if let Some(custom) = &self.custom {
            v["custom"] = json!(custom);
        } else {
            v["product"] = json!(id);
        }
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
        let selected = f.custom.as_ref().map_or_else(
            || product_label(f.product),
            |c| {
                if c.name.trim().is_empty() { tl!("Custom label").into() } else { c.name.clone() }
            },
        );
        craft_ui::ComboBox::from_id_salt("label_product").selected_text(selected).width(300.0).show_ui(ui, |ui| {
            for i in 0..LABEL_PRODUCTS.len() {
                if ui.selectable_label(f.custom.is_none() && f.product == i, product_label(i)).clicked() {
                    f.product = i;
                    f.custom = None;
                }
            }
        });
        if ui.button(tl!("New Label…")).clicked() {
            f.custom = LABEL_PRODUCTS.get(f.product).map(CustomLabel::from_product);
        }
    });
    if let Some(custom) = &mut f.custom {
        egui::CollapsingHeader::new(tl!("Label Details")).default_open(true).show(ui, |ui| custom.ui(ui));
    }
    let (rows, cols) = f.custom.as_ref().map_or_else(|| LABEL_PRODUCTS.get(f.product).map_or((1, 1), |p| (p.rows, p.cols)), |c| (c.rows, c.cols));
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
    let fits = f.custom.as_ref().is_none_or(CustomLabel::fits);
    if !fits {
        ui.label(tl!("The labels don't fit on the page."));
    }
    let (ok, cancel) = buttons(ui, tl!("New Document"), ready && fits);
    if ok {
        return run(app, "mailings.labels", f.params());
    }
    cancel
}

#[cfg(test)]
mod custom_label_tests {
    use super::*;

    #[test]
    fn custom_params_replace_product() {
        let custom = CustomLabel::from_product(&LABEL_PRODUCTS[0]);
        let form = LabelsForm { text: "Address".into(), product: 0, custom: Some(custom.clone()), single: false, row: 1, col: 1, recipients: false };
        let params = form.params();
        assert!(params.get("product").is_none());
        assert_eq!(
            params["custom"],
            json!({"name": custom.name, "top": 36.0, "side": 13.5,
            "width": 189.0, "height": 72.0, "hPitch": 198.0, "vPitch": 72.0,
            "cols": 3, "rows": 10, "pageWidth": 612.0, "pageHeight": 792.0})
        );
        assert!(custom.fits());
        assert!(!CustomLabel { cols: 4, ..custom }.fits());
    }
}
