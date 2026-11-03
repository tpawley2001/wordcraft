//! Text Direction dialog for the caret cell, text box or main document.

use egui::{Color32, FontId, Pos2, Ui, Vec2, vec2};
use serde::Serialize;
use wordcraft_doc::props::TextDirection;

use crate::WordApp;

/// What the dialog turns, from where the caret is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TextDirectionTarget {
    MainDocument,
    TableCell,
    TextBox,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextDirectionForm {
    pub target: TextDirectionTarget,
    pub value: TextDirection,
}

impl TextDirectionForm {
    pub fn new(app: &WordApp) -> Self {
        let target = if app.session.sel.focus.path.cell().is_some() {
            TextDirectionTarget::TableCell
        } else if wordcraft_engine::cmd::objects::text_box_target(&app.session).is_some() {
            TextDirectionTarget::TextBox
        } else {
            TextDirectionTarget::MainDocument
        };
        Self { target, value: wordcraft_engine::cmd::page::text_direction_at(&app.session) }
    }

    pub fn title(&self) -> &'static str {
        match self.target {
            TextDirectionTarget::MainDocument => tl!("Text Direction - Main Document"),
            TextDirectionTarget::TableCell => tl!("Text Direction - Table Cell"),
            TextDirectionTarget::TextBox => tl!("Text Direction - Text Box"),
        }
    }

    pub fn choices(&self) -> [TextDirection; 3] {
        [TextDirection::Horizontal, TextDirection::Down, TextDirection::Up]
    }

    pub fn ok(self, app: &mut WordApp) -> Result<(), String> {
        if self.value != wordcraft_engine::cmd::page::text_direction_at(&app.session) {
            app.run("layout.textDirection", serde_json::json!({"value": self.value.ooxml()}))?;
        }
        Ok(())
    }
}

fn angle(direction: TextDirection) -> f32 {
    match direction {
        TextDirection::Horizontal => 0.0,
        TextDirection::Down => std::f32::consts::FRAC_PI_2,
        TextDirection::Up => -std::f32::consts::FRAC_PI_2,
    }
}

/// Centre the rotated galley around the same point in all three orientations.
fn turned_text(ui: &Ui, center: Pos2, text: &str, size: f32, color: Color32, direction: TextDirection) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
    let rotation = egui::emath::Rot2::from_angle(angle(direction));
    let origin = center - rotation * (galley.size() * 0.5);
    let mut shape = egui::epaint::TextShape::new(origin, galley, color);
    shape.angle = angle(direction);
    ui.painter().add(shape);
}

pub(super) fn text_direction_ui(app: &mut WordApp, ui: &mut Ui, form: &mut TextDirectionForm) -> bool {
    ui.group(|ui| {
        ui.label(tl!("Orientation"));
        ui.horizontal(|ui| {
            for direction in form.choices() {
                let label = match direction {
                    TextDirection::Horizontal => tl!("Horizontal"),
                    TextDirection::Down => tl!("Rotate all text 90°"),
                    TextDirection::Up => tl!("Rotate all text 270°"),
                };
                let selected = form.value == direction;
                let (rect, response) = ui.allocate_exact_size(vec2(84.0, 84.0), egui::Sense::click());
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label));
                if response.clicked() {
                    form.value = direction;
                }
                let visuals = ui.style().interact_selectable(&response, selected);
                ui.painter().rect(rect, 4.0, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
                turned_text(ui, rect.center(), tl!("Text"), 16.0, visuals.text_color(), direction);
                let rotation = egui::emath::Rot2::from_angle(angle(direction));
                ui.painter().arrow(rect.center() + rotation * vec2(-17.0, 18.0), rotation * vec2(34.0, 0.0), visuals.fg_stroke);
                response.on_hover_text(label);
            }
        });
    });
    ui.add_space(8.0);
    ui.group(|ui| {
        ui.label(tl!("Preview"));
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(320.0), egui::Sense::hover());
        ui.painter().rect(rect, 2.0, ui.visuals().extreme_bg_color, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
        turned_text(ui, rect.center(), tl!("The quick brown fox jumps over the lazy dog"), 13.0, ui.visuals().text_color(), form.value);
    });
    if let Some((message, _)) = &app.status_msg {
        ui.colored_label(ui.visuals().error_fg_color, message);
    }
    let (ok, cancel) = crate::dialogs_insert::buttons(ui, "OK", "Cancel", true);
    if cancel {
        return true;
    }
    if ok {
        match form.clone().ok(app) {
            Ok(()) => return true,
            Err(message) => app.status(message),
        }
    }
    false
}

#[cfg(test)]
mod tests;
