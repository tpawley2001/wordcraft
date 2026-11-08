//! View › Window › Split (#3254): the document area as two panes, one above the other, on the
//! same document. Each pane keeps its own scroll; one selection and one caret are shared. The
//! pane last clicked is the active one: keys go to the document as always, and only it follows
//! the caret. The engine keeps the split (`ViewState::split`, the top pane's share of the height);
//! a divider between the panes resizes it (`view.split {"ratio"}`) and removes it when released
//! near an edge or double-clicked (`view.removeSplit`).
//!
//! The canvas draws each pane in turn (`canvas::show`); the scroll state of the pane not being
//! drawn waits here ([`PaneScroll`]). The active pane's state stays in the canvas between frames,
//! so the keys, the control channel and the scroll commands act on it.

use crate::WordApp;
use crate::theme::Tokens;
use egui::{Rect, Sense, Ui, pos2};
use serde_json::json;

/// Height of the divider bar between the panes.
pub const DIVIDER: f32 = 6.0;
/// The top pane's share of the height, as `view.split` takes it.
pub const MIN_RATIO: f32 = 0.1;
pub const MAX_RATIO: f32 = 0.9;

/// One of the two panes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Pane {
    Top,
    #[default]
    Bottom,
}

impl Pane {
    pub fn other(self) -> Self {
        match self {
            Pane::Top => Pane::Bottom,
            Pane::Bottom => Pane::Top,
        }
    }
}

/// Where the panes and the divider lie in the document area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitGeometry {
    pub top: Rect,
    pub divider: Rect,
    pub bottom: Rect,
}

impl SplitGeometry {
    pub fn pane(&self, pane: Pane) -> Rect {
        match pane {
            Pane::Top => self.top,
            Pane::Bottom => self.bottom,
        }
    }
}

/// The panes and divider of `area` split at `ratio` (clamped to the allowed range): the top pane
/// takes `ratio` of the height above the divider's centre line, the bottom pane the rest.
pub fn geometry(area: Rect, ratio: f32) -> SplitGeometry {
    let ratio = if ratio.is_finite() { ratio.clamp(MIN_RATIO, MAX_RATIO) } else { 0.5 };
    let h = area.height().max(0.0);
    let half = (DIVIDER / 2.0).min(h / 2.0);
    let line = area.min.y + h * ratio;
    let top = Rect::from_min_max(area.min, pos2(area.max.x, (line - half).max(area.min.y)));
    let divider = Rect::from_min_max(pos2(area.min.x, top.max.y), pos2(area.max.x, (line + half).min(area.max.y)));
    let bottom = Rect::from_min_max(pos2(area.min.x, divider.max.y), pos2(area.max.x, area.max.y.max(divider.max.y)));
    SplitGeometry { top, divider, bottom }
}

/// The pointer's place in `area` as a share of its height (0 at the top, 1 at the bottom), not
/// clamped; None for an empty area or a non-finite place.
fn share_at(area: Rect, y: f32) -> Option<f32> {
    let h = area.height();
    let r = (y - area.min.y) / h;
    (h > 0.0 && r.is_finite()).then_some(r)
}

/// The ratio a divider dragged to `y` sets, kept to 0.1..0.9.
pub fn ratio_at(area: Rect, y: f32) -> Option<f32> {
    share_at(area, y).map(|r| r.clamp(MIN_RATIO, MAX_RATIO))
}

/// Whether releasing the divider at `y` removes the split: within the top or bottom tenth.
pub fn release_removes(area: Rect, y: f32) -> bool {
    share_at(area, y).is_some_and(|r| !(MIN_RATIO..=MAX_RATIO).contains(&r))
}

/// The split the window shows: the view's, in the editing views. Read Mode, Focus and Full Screen
/// View show one pane and keep the split for later.
pub fn shown_ratio(view: &wordcraft_engine::ViewState) -> Option<f32> {
    if view.read_mode || crate::chrome::Screen::of(view) != crate::chrome::Screen::Normal {
        return None;
    }
    view.split
}

/// The scroll state of a pane while the other one is drawn or active.
#[derive(Default)]
pub(crate) struct PaneScroll {
    pub(crate) offset: egui::Vec2,
    pub(crate) max: egui::Vec2,
    pub(crate) request: Option<egui::Vec2>,
    pub(crate) viewport: Option<Rect>,
    pub(crate) page_rects: Vec<Rect>,
    pub(crate) canvas_rect: Option<Rect>,
    pub(crate) wheel: crate::scroll::CanvasScroll,
    pub(crate) auto_scroll: crate::autoscroll::AutoScroll,
}

/// The canvas's split state.
#[derive(Default)]
pub struct SplitPanes {
    /// The window was split last frame.
    on: bool,
    /// The pane keys go to and that follows the caret.
    active: Pane,
    /// The scroll state of the pane that isn't active.
    parked: PaneScroll,
    /// The pane being drawn now (None: the window isn't split).
    pub(crate) drawing: Option<Pane>,
    /// The pane being drawn isn't the active one: it doesn't take keys, focus or follow the caret.
    pub(crate) passive: bool,
    /// A press in the passive pane: it becomes the active one.
    pub(crate) activate: bool,
    /// What the passive pane shows (its viewport in content coordinates and the page tiles in
    /// it), so the active pane's pass keeps those textures.
    pub(crate) keep: Option<(Rect, std::collections::HashSet<(usize, usize)>)>,
}

impl SplitPanes {
    /// The page widget's id for the pane being drawn: the single pane's for the bottom one, so
    /// the keyboard focus stays on the page when the window is split.
    pub(crate) fn widget_id(&self) -> egui::Id {
        match self.drawing {
            Some(Pane::Top) => egui::Id::unique(("wordcraft_canvas", "top")),
            _ => egui::Id::unique("wordcraft_canvas"),
        }
    }
}

/// Exchanges the canvas's scroll state with a parked pane's.
fn swap(app: &mut WordApp, p: &mut PaneScroll) {
    let c = &mut app.canvas;
    std::mem::swap(&mut c.scroll_offset, &mut p.offset);
    std::mem::swap(&mut c.scroll_max, &mut p.max);
    std::mem::swap(&mut c.scroll_request, &mut p.request);
    std::mem::swap(&mut c.viewport, &mut p.viewport);
    std::mem::swap(&mut c.page_rects, &mut p.page_rects);
    std::mem::swap(&mut c.canvas_rect, &mut p.canvas_rect);
    std::mem::swap(&mut c.wheel, &mut p.wheel);
    std::mem::swap(&mut c.auto_scroll, &mut p.auto_scroll);
}

/// Draws the document area: one pane, or two with the divider between them.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let full = ui.available_rect_before_wrap();
    let ratio = shown_ratio(&app.session.view);
    match (app.canvas.split.on, ratio.is_some()) {
        (false, true) => {
            // Both panes start where the window was; the bottom one is active.
            let at = app.canvas.scroll_offset;
            app.canvas.split.parked = PaneScroll { offset: at, max: app.canvas.scroll_max, request: Some(at), ..Default::default() };
            app.canvas.split.active = Pane::Bottom;
            app.canvas.scroll_request = Some(at);
            app.canvas.split.on = true;
        }
        (true, false) => {
            // One pane again, where the active pane was.
            app.canvas.scroll_request = Some(app.canvas.scroll_offset);
            if app.canvas.split.active == Pane::Top {
                app.canvas.want_focus = true;
            }
            let split = &mut app.canvas.split;
            (split.parked, split.keep, split.on) = (PaneScroll::default(), None, false);
        }
        _ => {}
    }
    let Some(ratio) = ratio else {
        app.canvas.split.drawing = None;
        app.canvas.split.passive = false;
        crate::canvas::show(app, ui);
        return;
    };
    let g = geometry(full, ratio);
    let active = app.canvas.split.active;
    draw_passive(app, ui, g, active.other());
    if std::mem::take(&mut app.canvas.split.activate) {
        // The press made the other pane active: its state moves into the canvas, and the pane
        // that was active waits this frame out.
        let mut parked = std::mem::take(&mut app.canvas.split.parked);
        swap(app, &mut parked);
        app.canvas.split.parked = parked;
        app.canvas.split.active = active.other();
        app.canvas.want_focus = true;
        draw_passive(app, ui, g, active);
        std::mem::take(&mut app.canvas.split.activate);
    } else {
        app.canvas.split.drawing = Some(active);
        app.canvas.split.passive = false;
        draw_pane(app, ui, g.pane(active), active);
    }
    app.canvas.split.drawing = None;
    app.canvas.split.passive = false;
    divider(app, ui, full, g.divider);
    // The passive pane was drawn before this frame's keys reached the document: draw it again.
    if ui.input(|i| !i.events.is_empty()) {
        ui.ctx().request_repaint();
    }
}

fn draw_passive(app: &mut WordApp, ui: &mut Ui, g: SplitGeometry, pane: Pane) {
    let mut parked = std::mem::take(&mut app.canvas.split.parked);
    swap(app, &mut parked);
    app.canvas.split.drawing = Some(pane);
    app.canvas.split.passive = true;
    draw_pane(app, ui, g.pane(pane), pane);
    swap(app, &mut parked);
    app.canvas.split.parked = parked;
}

fn draw_pane(app: &mut WordApp, ui: &mut Ui, rect: Rect, pane: Pane) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("split_pane", pane)).max_rect(rect));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    crate::canvas::show(app, &mut child);
}

/// The bar between the panes: drag it to resize them, release it near an edge or double-click it
/// to remove the split.
fn divider(app: &mut WordApp, ui: &mut Ui, area: Rect, bar: Rect) {
    let resp = ui.interact(bar, egui::Id::unique("wordcraft_split_divider"), Sense::click_and_drag());
    let t = Tokens::get(ui.ctx());
    let painter = ui.painter_at(bar);
    painter.rect_filled(bar, 0.0, if resp.hovered() || resp.dragged() { t.hover } else { t.panel });
    let stroke = egui::Stroke::new(1.0, t.border);
    painter.hline(bar.x_range(), bar.min.y + 0.5, stroke);
    painter.hline(bar.x_range(), bar.max.y - 0.5, stroke);
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let y = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.latest_pos())).map(|p| p.y);
    if resp.double_clicked() {
        let _ = app.run("view.removeSplit", json!({}));
    } else if resp.drag_stopped() {
        if y.is_some_and(|y| release_removes(area, y)) {
            let _ = app.run("view.removeSplit", json!({}));
        }
    } else if resp.dragged()
        && let Some(r) = y.and_then(|y| ratio_at(area, y))
        && app.session.view.split.is_none_or(|old| (old - r).abs() > 1e-4)
    {
        let _ = app.run("view.split", json!({"ratio": r}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_max(pos2(10.0, 100.0), pos2(810.0, 700.0))
    }

    #[test]
    fn panes_and_divider_tile_the_area() {
        let g = geometry(area(), 0.5);
        assert_eq!(g.top.min, area().min);
        assert_eq!(g.top.max.y, 397.0);
        assert_eq!(g.divider.height(), DIVIDER);
        assert_eq!(g.divider.min.y, g.top.max.y);
        assert_eq!(g.bottom.min.y, g.divider.max.y);
        assert_eq!(g.bottom.max, area().max);
        assert_eq!(g.pane(Pane::Top), g.top);
        assert_eq!(g.pane(Pane::Bottom), g.bottom);
        let g = geometry(area(), 0.25);
        assert_eq!(g.top.height(), 150.0 - DIVIDER / 2.0);
    }

    #[test]
    fn geometry_clamps_odd_ratios_and_areas() {
        assert_eq!(geometry(area(), 0.0), geometry(area(), MIN_RATIO));
        assert_eq!(geometry(area(), 7.0), geometry(area(), MAX_RATIO));
        assert_eq!(geometry(area(), f32::NAN), geometry(area(), 0.5));
        let flat = Rect::from_min_max(pos2(0.0, 50.0), pos2(100.0, 52.0));
        let g = geometry(flat, 0.5);
        assert!(g.top.height() >= 0.0 && g.divider.height() >= 0.0 && g.bottom.height() >= 0.0);
        assert_eq!(g.bottom.max.y, 52.0);
    }

    #[test]
    fn a_drag_sets_a_ratio_within_range() {
        assert_eq!(ratio_at(area(), 250.0), Some(0.25));
        assert_eq!(ratio_at(area(), 0.0), Some(MIN_RATIO));
        assert_eq!(ratio_at(area(), 5000.0), Some(MAX_RATIO));
        assert_eq!(ratio_at(area(), f32::NAN), None);
        assert_eq!(ratio_at(Rect::from_min_max(pos2(0.0, 5.0), pos2(9.0, 5.0)), 5.0), None);
    }

    #[test]
    fn releasing_near_an_edge_removes_the_split() {
        assert!(release_removes(area(), 120.0));
        assert!(release_removes(area(), 690.0));
        assert!(release_removes(area(), 20.0), "above the area");
        assert!(!release_removes(area(), 400.0));
        assert!(!release_removes(area(), 160.0), "the edge of the range keeps it");
        assert!(!release_removes(area(), f32::NAN));
    }

    #[test]
    fn read_mode_focus_and_full_screen_show_one_pane() {
        let mut v = wordcraft_engine::ViewState { split: Some(0.4), ..Default::default() };
        assert_eq!(shown_ratio(&v), Some(0.4));
        v.read_mode = true;
        assert_eq!(shown_ratio(&v), None);
        v.read_mode = false;
        v.focus_mode = true;
        assert_eq!(shown_ratio(&v), None);
        v.focus_mode = false;
        v.full_screen_view = true;
        assert_eq!(shown_ratio(&v), None);
        assert_eq!(v.split, Some(0.4), "the split is kept");
    }
}
