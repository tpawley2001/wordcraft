//! The style area (File › Options › Advanced › Display › Style area pane width in Draft and
//! Outline views): a column at the left of the text that names each paragraph's style level with
//! its first line, as in Word. Screen only: it never prints and isn't part of the document. The
//! line between it and the text drags to resize it; dragged to the column's left edge it closes.
//! Double-clicking a name selects that paragraph and opens Modify Style for its style.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::{Path, Pos, StoryRef};
use wordcraft_layout::{DocLayout, Placed, ViewMode};

use super::pages::page_screen_scale;
use crate::WordApp;
use crate::theme::{Tokens, TypeRung};

/// Widest style area, in points (22 in).
const MAX_WIDTH: f32 = 1584.0;
/// A release this close to the column's left edge (screen pixels) closes the style area.
const CLOSE_PX: f32 = 4.0;
/// Half the width of the separator's grip (screen pixels).
const GRIP_PX: f32 = 3.0;
/// Space between the column's left edge and the names (screen pixels).
const INSET_PX: f32 = 4.0;

/// The style area's width in points as the canvas shows it: the preference in Draft and Outline
/// views, 0 in every other view.
pub(super) fn width(app: &WordApp) -> f32 {
    let v = &app.session.view;
    if v.read_mode || !matches!(v.mode, ViewMode::Draft | ViewMode::Outline) {
        return 0.0;
    }
    let w = app.session.prefs.style_area_width;
    if w.is_finite() { w.clamp(0.0, MAX_WIDTH) } else { 0.0 }
}

/// Whether the style area names this body paragraph: every top-level paragraph, and in a table
/// the first paragraph of each row's first cell.
fn named(path: &Path) -> bool {
    match path.0.as_slice() {
        [_] => true,
        [_, _, cell, block] => *cell == 0 && *block == 0,
        _ => false,
    }
}

/// The name the style area shows for a body paragraph's style.
fn style_label(app: &WordApp, path: &Path) -> Option<(String, String)> {
    let para = app.session.doc.para(StoryRef::Body, path)?;
    let id = para.props.style.clone().unwrap_or_else(|| "Normal".into());
    let name = app.session.doc.styles.get(&id).map_or_else(|| id.clone(), crate::i18n::style_name);
    Some((id, name))
}

/// What a double-click on a name asks for, done once the style area is painted.
struct Open {
    path: Path,
    style: String,
}

/// Paint the style area beside each page on screen and handle its mouse: resizing by the
/// separator and double-clicks on the names.
pub(super) fn show(app: &mut WordApp, ui: &mut Ui, layout: &DocLayout, rects: &[Rect], scale: f32, paper: Color32, t: &Tokens) {
    if let Some(o) = paint(app, ui, layout, rects, scale, paper, t) {
        open(app, o);
    }
}

/// [`show`]'s painting and mouse; returns a double-clicked name's paragraph.
fn paint(app: &mut WordApp, ui: &mut Ui, layout: &DocLayout, rects: &[Rect], scale: f32, paper: Color32, t: &Tokens) -> Option<Open> {
    let w = width(app);
    if w <= 0.0 {
        return None;
    }
    let clip = ui.clip_rect();
    let font = TypeRung::Caption.regular();
    let mut open = None;
    for (i, pr) in rects.iter().enumerate() {
        let Some(page) = layout.pages.get(i) else { continue };
        let s = page_screen_scale(*pr, page, scale);
        let col = Rect::from_min_max(pos2(pr.left() - w * s, pr.top()), pos2(pr.left(), pr.bottom()));
        if !col.intersects(clip) {
            continue;
        }
        let painter = ui.painter().with_clip_rect(clip.intersect(col.expand2(vec2(GRIP_PX, 0.0))));
        painter.rect_filled(col, 0.0, paper);
        let names = painter.with_clip_rect(clip.intersect(Rect::from_min_max(col.min, pos2(col.right() - GRIP_PX, col.bottom()))));
        for (k, item) in page.items.iter().enumerate() {
            let Placed::Lines { story: StoryRef::Body, path, para, l0, y, .. } = item else { continue };
            if *l0 != 0 || !named(path) {
                continue;
            }
            let Some(line) = para.lines.first() else { continue };
            let base = col.top() + (y + line.baseline - line.top) * s;
            if !base.is_finite() || base < clip.top() - 40.0 || base > clip.bottom() + 40.0 {
                continue;
            }
            let Some((id, name)) = style_label(app, path) else { continue };
            let galley = names.layout_no_wrap(name.clone(), font.clone(), t.text);
            let size = galley.size();
            let top = base - size.y * 0.8;
            let at = pos2(col.left() + INSET_PX, top);
            names.galley(at, galley, t.text);
            let hit = Rect::from_min_size(pos2(col.left(), top), vec2((col.width() - GRIP_PX).max(1.0), size.y)).intersect(names.clip_rect());
            if hit.width() <= 0.0 || hit.height() <= 0.0 {
                continue;
            }
            let resp = ui.interact(hit, ui.scope_id().with(("style_area_name", i, k)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
            if resp.double_clicked() {
                open = Some(Open { path: path.clone(), style: id });
            }
        }
        // The separator, and its grip.
        painter.vline(col.right() - 0.5, col.y_range(), Stroke::new(1.0, t.border_strong));
        let grip = Rect::from_min_max(pos2(col.right() - GRIP_PX, col.top()), pos2(col.right() + GRIP_PX, col.bottom())).intersect(clip);
        if grip.height() <= 0.0 {
            continue;
        }
        let resp =
            ui.interact(grip, ui.scope_id().with(("style_area_grip", i)), Sense::drag()).on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);
        let pointer: Option<Pos2> = resp.interact_pointer_pos().or_else(|| ui.input(|inp| inp.pointer.latest_pos()));
        if resp.dragged()
            && let Some(p) = pointer
        {
            let x = p.x.max(col.left());
            ui.painter().with_clip_rect(clip).vline(x, clip.y_range(), Stroke::new(1.0, t.text));
        }
        if resp.drag_stopped()
            && let Some(p) = pointer
        {
            let dx = p.x - col.left();
            let value = if !dx.is_finite() || dx < CLOSE_PX { 0.0 } else { (dx / s.max(0.01)).clamp(0.0, MAX_WIDTH) };
            // One preference write, on release.
            let _ = app.run("view.styleAreaWidth", json!({"value": value}));
        }
    }
    open
}

/// Select the double-clicked name's paragraph and open Modify Style for its style.
fn open(app: &mut WordApp, open: Open) {
    app.session.sel = wordcraft_engine::Selection::caret(Pos::new(StoryRef::Body, open.path, 0));
    let _ = app.run("select.paragraph", json!({}));
    if let Some(d) = crate::dialogs::Dialog::modify_style(app, &open.style) {
        app.dialog = Some(d);
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use serde_json::json;
    use wordcraft_doc::{Block, Document};

    use crate::{Services, WordApp};

    fn harness(width: f32) -> egui_kittest::Harness<'static, WordApp> {
        let mut d = Document::from_text("Chapter one\nThe body text");
        if let Some(Block::Para(p)) = d.body.get_mut(0).map(std::sync::Arc::make_mut) {
            p.props.style = Some("Heading1".into());
        }
        let mut a = WordApp::new(wordcraft_engine::Session::new(d), Services::default());
        a.run("view.styleAreaWidth", json!({"value": width})).unwrap();
        egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        )
    }

    /// A name drawn in the style area (not the ribbon's style gallery, which has its own labels).
    fn shown(h: &egui_kittest::Harness<'static, WordApp>, name: &str) -> bool {
        let pages = h.state().canvas.page_rects.clone();
        h.query_all_by_label(name).any(|n| {
            let r = n.rect();
            pages.iter().any(|p| r.min.x < p.left() && r.min.y >= p.top() && r.max.y <= p.bottom())
        })
    }

    #[test]
    fn draft_view_names_each_paragraphs_style() {
        let mut h = harness(72.0);
        h.state_mut().run("view.draft", json!({})).unwrap();
        h.run_steps(4);
        assert!(shown(&h, "Heading 1"), "the heading's style is named");
        assert!(shown(&h, "Normal"), "the body paragraph's style is named");
    }

    #[test]
    fn print_layout_has_no_style_area() {
        let mut h = harness(72.0);
        h.run_steps(4);
        assert!(!shown(&h, "Heading 1"));
        assert!(!shown(&h, "Normal"));
    }

    #[test]
    fn no_style_area_at_width_zero() {
        let mut h = harness(0.0);
        h.state_mut().run("view.draft", json!({})).unwrap();
        h.run_steps(4);
        assert!(!shown(&h, "Heading 1"));
        assert!(!shown(&h, "Normal"));
    }

    #[test]
    fn the_text_moves_right_by_the_style_area() {
        let mut h = harness(0.0);
        h.state_mut().run("view.draft", json!({})).unwrap();
        h.run_steps(4);
        let before = h.state().canvas.page_rects.first().copied().unwrap();
        h.state_mut().run("view.styleAreaWidth", json!({"value": 72})).unwrap();
        h.run_steps(4);
        let after = h.state().canvas.page_rects.first().copied().unwrap();
        assert!(after.left() > before.left(), "{before:?} → {after:?}");
        assert_eq!(after.width(), before.width(), "the text column keeps its width");
    }
}
