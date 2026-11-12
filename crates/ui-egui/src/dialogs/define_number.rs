//! Define New Number Format (Home › Paragraph › Numbering ▾): a number style, the text around
//! the number, its alignment and a preview. OK ends in one `para.defineNumber`, so agents get the
//! same result without the dialog.

use egui::{Sense, Ui, vec2};
use serde::Serialize;
use serde_json::{Value, json};
use wordcraft_doc::props::Align;
use wordcraft_doc::section::NumFormat;
use wordcraft_engine::cmd::lists::{caret_list, levels_of};

use crate::WordApp;
use crate::dialogs::{buttons, color_menu};
use crate::theme::Tokens;

/// Longest text kept on either side of the number (the level text holds 64 characters).
const SIDE_MAX: usize = 30;

/// Longest font family name sent to `para.defineNumber`.
const FONT_MAX: usize = 64;

/// `s` as an sRGB colour when it is exactly six hex digits (no `#`).
fn six_hex(s: &str) -> Option<wordcraft_doc::Rgb> {
    let s = s.trim();
    if s.len() == 6 { wordcraft_doc::Rgb::parse(s) } else { None }
}

/// The Define New Number Format form.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberForm {
    /// The number style, an OOXML number format name.
    pub style: String,
    /// The text before the number.
    pub before: String,
    /// The text after the number.
    pub after: String,
    /// `left`, `center` or `right`.
    pub align: String,
    /// The number's font family; empty for the paragraph text's font.
    pub font: String,
    /// The number is bold.
    pub bold: bool,
    /// The number is italic.
    pub italic: bool,
    /// The number's colour, `RRGGBB`; empty for Automatic.
    pub color: String,
    /// The number's size in points as the Size box shows it (`20`, `10.5`); empty for not set.
    pub size: String,
    /// The number's underline, a `w:u` name (`none` is a choice of its own); empty for not set.
    pub underline: String,
    /// Font… was clicked: the font controls are showing.
    #[serde(skip)]
    pub show_font: bool,
}

impl Default for NumberForm {
    fn default() -> Self {
        NumberForm {
            style: "decimal".into(),
            before: String::new(),
            after: ".".into(),
            align: "left".into(),
            font: String::new(),
            bold: false,
            italic: false,
            color: String::new(),
            size: String::new(),
            underline: String::new(),
            show_font: false,
        }
    }
}

/// A level text split at its first `%1`: the text before the number and the text after it. A text
/// without `%1` is all before.
pub fn split_number_text(text: &str) -> (String, String) {
    match text.split_once("%1") {
        Some((before, after)) => (before.to_string(), after.to_string()),
        None => (text.to_string(), String::new()),
    }
}

/// The level text for the text on either side of the number. `%` typed by the user is not a
/// placeholder, so it is dropped.
pub fn join_number_text(before: &str, after: &str) -> String {
    format!("{}%1{}", without_percent(before), without_percent(after))
}

fn without_percent(s: &str) -> String {
    s.chars().filter(|c| *c != '%').take(SIDE_MAX).collect()
}

/// Number styles the dialog offers: every format the model knows but bullets and none.
pub fn number_styles() -> impl Iterator<Item = NumFormat> {
    NumFormat::ALL.into_iter().filter(|f| !matches!(f, NumFormat::Bullet | NumFormat::None))
}

/// A style's first three numbers, as the Number style list shows it ("1, 2, 3").
pub fn style_sample(f: NumFormat) -> String {
    [1, 2, 3].map(|n| f.format(n)).join(", ")
}

impl NumberForm {
    /// The caret's numbered list's first level, or decimal `%1.` left-aligned.
    pub fn read(app: &WordApp) -> NumberForm {
        let s = &app.session;
        let Some(level) = caret_list(s).and_then(|n| levels_of(s, n)).and_then(|l| l.into_iter().next()) else {
            return NumberForm::default();
        };
        if matches!(level.format, NumFormat::Bullet | NumFormat::None) {
            return NumberForm::default();
        }
        let (before, after) = split_number_text(&level.text);
        let align = match level.align {
            Align::Center => "center",
            Align::Right => "right",
            _ => "left",
        };
        let font: String = level.chr.font.as_deref().unwrap_or_default().chars().take(FONT_MAX).collect();
        let bold = level.chr.bold == Some(true);
        let italic = level.chr.italic == Some(true);
        let color = match level.chr.color {
            Some(wordcraft_doc::TextColor::Rgb(c)) => c.hex(),
            _ => String::new(),
        };
        let show_font = !font.is_empty() || bold || italic || !color.is_empty();
        NumberForm {
            style: level.format.ooxml().into(),
            before: without_percent(&before),
            after: without_percent(&after),
            align: align.into(),
            font,
            bold,
            italic,
            color,
            size: String::new(),
            underline: String::new(),
            show_font,
        }
    }

    /// `para.defineNumber` parameters. The number's font keys are there only when set: the new
    /// list starts from a plain level, so a missing key means "not set".
    pub fn params(&self) -> Value {
        let mut p = serde_json::Map::new();
        p.insert("format".into(), json!(self.style));
        p.insert("text".into(), json!(join_number_text(&self.before, &self.after)));
        p.insert("align".into(), json!(self.align));
        let font: String = self.font.trim().chars().take(FONT_MAX).collect();
        let font = font.trim_end();
        if !font.is_empty() {
            p.insert("font".into(), json!(font));
        }
        if self.bold {
            p.insert("bold".into(), json!(true));
        }
        if self.italic {
            p.insert("italic".into(), json!(true));
        }
        if let Some(c) = six_hex(&self.color) {
            p.insert("color".into(), json!(c.hex()));
        }
        Value::Object(p)
    }

    fn format(&self) -> NumFormat {
        NumFormat::from_ooxml(&self.style)
    }

    /// How the preview draws a label of `size` points: bold, italic and the number's colour.
    pub fn label_format(&self, size: f32) -> egui::TextFormat {
        let font_id = if self.bold { crate::theme::semibold(size) } else { egui::FontId::proportional(size) };
        let color = six_hex(&self.color).map_or(egui::Color32::BLACK, |c| egui::Color32::from_rgb(c.0, c.1, c.2));
        egui::TextFormat { font_id, italics: self.italic, color, ..Default::default() }
    }

    /// The label of item `n` as the preview draws it.
    pub fn label(&self, n: u32) -> String {
        format!("{}{}{}", without_percent(&self.before), self.format().format(n), without_percent(&self.after))
    }
}

/// Define New Number Format. Returns true to close.
pub fn define_number(app: &mut WordApp, ui: &mut Ui, f: &mut NumberForm) -> bool {
    let t = Tokens::get(ui.ctx());
    let style_label = ui.label(tl!("Number style:"));
    ui.horizontal(|ui| {
        let combo = craft_ui::ComboBox::from_id_salt("dnf_style").width(240.0).selected_text(style_sample(f.format())).show_ui(ui, |ui| {
            for k in number_styles() {
                if ui.selectable_label(f.format() == k, style_sample(k)).clicked() {
                    f.style = k.ooxml().into();
                }
            }
        });
        combo.response.labelled_by(style_label.id);
        if ui.button(tl!("Font…")).clicked() {
            f.show_font = !f.show_font;
        }
    });
    if f.show_font {
        number_font(app, ui, f);
    }
    ui.add_space(6.0);
    let format_label = ui.label(tl!("Number format:"));
    egui::Frame::new().stroke(egui::Stroke::new(1.0, t.border_strong)).inner_margin(2.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add(egui::TextEdit::singleline(&mut f.before).desired_width(80.0).char_limit(SIDE_MAX).frame(egui::Frame::NONE))
                .labelled_by(format_label.id);
            // The number itself: shaded, not typed over.
            let number = f.format().format(1);
            let galley = ui.painter().layout_no_wrap(number, egui::FontId::proportional(13.0), t.text);
            let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 6.0, galley.size().y.max(16.0)), Sense::hover());
            ui.painter().rect_filled(rect, 2.0, t.border_strong.gamma_multiply(0.35));
            ui.painter().galley(egui::pos2(rect.left() + 3.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
            ui.add(egui::TextEdit::singleline(&mut f.after).desired_width(80.0).char_limit(SIDE_MAX).frame(egui::Frame::NONE))
                .labelled_by(format_label.id);
        });
    });
    f.before.retain(|c| c != '%');
    f.after.retain(|c| c != '%');
    ui.add_space(6.0);
    let align_label = ui.label(tl!("Alignment:"));
    let aligns = [("left", tl!("Left")), ("center", tl!("Centered")), ("right", tl!("Right"))];
    let shown = aligns.iter().find(|(k, _)| *k == f.align).map(|(_, s)| s.to_string()).unwrap_or_default();
    let combo = craft_ui::ComboBox::from_id_salt("dnf_align").width(240.0).selected_text(shown).show_ui(ui, |ui| {
        for (k, s) in &aligns {
            if ui.selectable_label(f.align == *k, *s).clicked() {
                f.align = (*k).into();
            }
        }
    });
    combo.response.labelled_by(align_label.id);
    ui.add_space(6.0);
    ui.label(egui::RichText::new(tl!("Preview")).small().weak());
    preview(ui, f);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && let Err(e) = app.run("para.defineNumber", f.params()) {
        app.status(e);
    }
    ok || cancel
}

/// The number's own font, bold, italic and colour (Font…).
fn number_font(app: &mut WordApp, ui: &mut Ui, f: &mut NumberForm) {
    let theme = app.session.doc.settings.theme_colors.clone();
    let fams = app.previews.families();
    let preview_fn = app.previews.font_preview_fn();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let font_label = ui.label(tl!("Font:"));
        ui.scope(|ui| {
            if let Some(name) = crate::widgets::font_combo(ui, "dnf_font", 170.0, &f.font, &fams, Some(&*preview_fn))
                && name != crate::widgets::ADD_FONT
                && name != crate::widgets::LOAD_SYSTEM_FONTS
            {
                f.font = name.chars().take(FONT_MAX).collect();
            }
        })
        .response
        .labelled_by(font_label.id);
        ui.checkbox(&mut f.bold, tl!("Bold"));
        ui.checkbox(&mut f.italic, tl!("Italic"));
    });
    ui.horizontal(|ui| {
        ui.push_id("dnf_color", |ui| {
            let color_label = ui.label(tl!("Font color:"));
            let response = ui.scope(|ui| color_menu(ui, &theme, &mut f.color)).response;
            response.labelled_by(color_label.id);
        });
    });
    // The colour menu may give `#RRGGBB` or lower case; the form keeps upper-case `RRGGBB`.
    f.color = wordcraft_doc::Rgb::parse(&f.color).map(|c| c.hex()).unwrap_or_default();
}

/// Three numbered lines, each label in a fixed column aligned as chosen, then a grey line of text.
fn preview(ui: &mut Ui, f: &NumberForm) {
    let t = Tokens::get(ui.ctx());
    let row = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(280.0, row * 3.0 + 12.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, egui::Color32::WHITE);
    p.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
    let (col_left, col_right) = (rect.left() + 16.0, rect.left() + 76.0);
    for i in 0..3u32 {
        let y = rect.top() + 6.0 + row * i as f32 + row / 2.0;
        let mut job = egui::text::LayoutJob::default();
        job.append(&f.label(i + 1), 0.0, f.label_format(11.0));
        let galley = p.layout_job(job);
        let w = galley.size().x;
        let x = match f.align.as_str() {
            "center" => (col_left + col_right) / 2.0 - w / 2.0,
            "right" => col_right - w,
            _ => col_left,
        };
        let text_x = (x + w + 6.0).max(col_right + 8.0);
        p.galley(egui::pos2(x, y - galley.size().y / 2.0), galley, egui::Color32::BLACK);
        let bar = egui::Rect::from_min_max(egui::pos2(text_x, y - 2.0), egui::pos2(rect.right() - 12.0, y + 2.0));
        if bar.width() > 0.0 {
            p.rect_filled(bar, 1.0, egui::Color32::from_gray(200));
        }
    }
}
