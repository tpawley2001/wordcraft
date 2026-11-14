use super::state::{SavedDock, labels, move_before, open, pane_state};
use super::{Edge, Pane, PaneHost, PaneRequest, PaneState, Place, SidePolicy, Stroke, Tokens, Ui, Value, WordApp, json};

pub(super) struct Content<'a> {
    pub(super) app: &'a mut WordApp,
}

/// The pane host's name. Its focus and drag state is per window ([`WordApp::window_id`]); where
/// each pane sits is the app's.
const HOST: &str = "task_panes";

/// Whether a dialog, file browser, colors dialog, content-control popup, Zotero alert, keytips
/// or egui popup is open: the keyboard belongs to it, not to the panes or the document.
pub(crate) fn overlay_open(app: &WordApp, ctx: &egui::Context) -> bool {
    app.dialog.is_some()
        || app.file_dialog_open()
        || app.colors.is_some()
        || app.controls.popup_open()
        || app.zotero.alert_open()
        || app.ui.keytips != crate::keytips::Phase::Off
        || egui::Popup::is_any_open(ctx)
}

/// The part of the window that has the keyboard, for the F6 / Shift+F6 cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRegion {
    Document,
    Pane,
    StatusBar,
    Ribbon,
    Other,
}

/// A stop of the F6 cycle outside the task panes. The status bar and the ribbon note where
/// they are each pass they are drawn; one that was not drawn is not on screen and F6 skips it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum F6Stop {
    StatusBar,
    Ribbon,
}

/// What a stop drew in one pass: the widget F6 focuses and the area its controls sit in.
#[derive(Clone, Copy, Debug, Default)]
struct StopSeen {
    pass: u64,
    /// The widget F6 focuses, and whether it was the preferred one (the selected ribbon tab).
    target: Option<(egui::Id, bool)>,
    area: Option<egui::Rect>,
}

fn stop_key(app: &WordApp, stop: F6Stop) -> egui::Id {
    app.window_id("f6_stops").with(stop as u8)
}

fn note(app: &WordApp, ctx: &egui::Context, stop: F6Stop, f: impl FnOnce(&mut StopSeen)) {
    let key = stop_key(app, stop);
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let seen = d.get_temp_mut_or_default::<StopSeen>(key);
        if seen.pass != pass {
            *seen = StopSeen { pass, ..StopSeen::default() };
        }
        f(seen);
    });
}

/// The widget F6 focuses in `stop` this pass. The first one noted wins unless a later one is
/// `preferred` (the selected tab over the first tab shown).
pub(crate) fn note_f6_target(app: &WordApp, ctx: &egui::Context, stop: F6Stop, id: egui::Id, preferred: bool) {
    note(app, ctx, stop, |seen| {
        if preferred || seen.target.is_none() {
            seen.target = Some((id, preferred));
        }
    });
}

/// Part of the area `stop`'s controls sit in this pass: a focused widget inside it is in the stop.
pub(crate) fn note_f6_area(app: &WordApp, ctx: &egui::Context, stop: F6Stop, rect: egui::Rect) {
    note(app, ctx, stop, |seen| seen.area = Some(seen.area.map_or(rect, |a| a.union(rect))));
}

/// The stop as drawn in this pass or the one before (read between passes), if it was.
fn seen(app: &WordApp, ctx: &egui::Context, stop: F6Stop) -> Option<StopSeen> {
    let seen = ctx.data(|d| d.get_temp::<StopSeen>(stop_key(app, stop)))?;
    (seen.pass.saturating_add(1) >= ctx.cumulative_pass_nr()).then_some(seen)
}

/// The widget F6 focuses in `stop`, when the stop is on screen.
fn stop_target(app: &WordApp, ctx: &egui::Context, stop: F6Stop) -> Option<egui::Id> {
    seen(app, ctx, stop)?.target.map(|(id, _)| id)
}

/// Whether the pane host had a pane holding the keyboard when it last drew.
fn pane_key(app: &WordApp) -> egui::Id {
    app.window_id(HOST).with("f6_pane")
}

/// The region of the window the keyboard was in when the panes were last drawn, for Escape.
fn last_region_key(app: &WordApp) -> egui::Id {
    app.window_id(HOST).with("f6_region")
}

/// Which part of the window `focused` (an egui widget id) belongs to.
fn region_of(app: &WordApp, ctx: &egui::Context, focused: Option<egui::Id>) -> KeyRegion {
    let Some(focused) = focused else { return KeyRegion::Other };
    if app.canvas.widget_id.is_some_and(|(id, _)| id == focused) {
        return KeyRegion::Document;
    }
    let rect = ctx.read_response(focused).map(|r| r.rect);
    for (stop, region) in [(F6Stop::StatusBar, KeyRegion::StatusBar), (F6Stop::Ribbon, KeyRegion::Ribbon)] {
        if let Some(seen) = seen(app, ctx, stop)
            && (seen.target.is_some_and(|(id, _)| id == focused) || rect.zip(seen.area).is_some_and(|(r, a)| a.contains_rect(r)))
        {
            return region;
        }
    }
    if ctx.data(|d| d.get_temp::<bool>(pane_key(app))).unwrap_or(false) { KeyRegion::Pane } else { KeyRegion::Other }
}

/// Which part of the window has the keyboard: the page, a task pane, the status bar, the ribbon
/// or something else. Derived from egui's focus, so it cannot drift from it.
pub(crate) fn key_region(app: &WordApp, ctx: &egui::Context) -> KeyRegion {
    if app.canvas.focused {
        return KeyRegion::Document;
    }
    region_of(app, ctx, ctx.memory(|m| m.focused()))
}

/// The F6 cycle, forwards: the task panes, the status bar, the ribbon, the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Panes,
    StatusBar,
    Ribbon,
    Document,
}

/// Give the keyboard to `step`, or to the next step on in the direction of travel when `step`
/// is not on screen. Returns the direction when the pane host is to cycle its panes.
fn enter(app: &mut WordApp, ctx: &egui::Context, step: Step, reverse: bool) -> Option<bool> {
    let stop = match step {
        Step::Panes => {
            craft_ui::panes::focus_next(ctx, app.window_id(HOST), reverse);
            return Some(reverse);
        }
        Step::Document => {
            app.canvas.want_focus = true;
            return None;
        }
        Step::StatusBar => F6Stop::StatusBar,
        Step::Ribbon => F6Stop::Ribbon,
    };
    if let Some(id) = stop_target(app, ctx, stop) {
        ctx.memory_mut(|m| m.request_focus(id));
        return None;
    }
    let next = match (step, reverse) {
        (Step::StatusBar, false) => Step::Ribbon,
        (Step::StatusBar, true) => Step::Panes,
        (_, false) => Step::Document,
        (_, true) => Step::StatusBar,
    };
    enter(app, ctx, next, reverse)
}

/// F6 and Shift+F6 move the keyboard through the task panes, the status bar and the ribbon and
/// back to the document, as Word does. The key stays with an open dialog, file browser, popup or
/// keytips (Customize Keyboard records it), and an F6 the user bound to a command runs that
/// command instead. Escape from the status bar or the ribbon gives the page the keyboard back.
/// Returns the direction when the pane host is to cycle its panes this pass.
fn route_f6(app: &mut WordApp, ctx: &egui::Context) -> Option<bool> {
    if overlay_open(app, ctx) {
        return None;
    }
    // egui drops a widget's focus on Escape before the pass starts: the region it was in is
    // the one noted when the panes were last drawn.
    let last = ctx.data(|d| d.get_temp::<KeyRegion>(last_region_key(app)));
    if matches!(last, Some(KeyRegion::StatusBar | KeyRegion::Ribbon))
        && ctx.memory(|m| m.focused()).is_none()
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        app.canvas.want_focus = true;
    }
    let reg = &app.session.registry;
    let bound = |m: egui::Modifiers| app.session.keymap.resolve(reg, &crate::keys::combo(m, "F6", true)).is_some();
    let mut cycle = None;
    ctx.input_mut(|input| {
        input.events.retain(|e| match e {
            egui::Event::Key { key: egui::Key::F6, pressed: true, modifiers: m, .. } if !m.command && !m.ctrl && !m.mac_cmd && !bound(*m) => {
                cycle = Some(m.shift);
                false
            }
            _ => true,
        });
    });
    let reverse = cycle?;
    let step = match (key_region(app, ctx), reverse) {
        (KeyRegion::StatusBar, false) => Step::Ribbon,
        (KeyRegion::StatusBar, true) => Step::Panes,
        (KeyRegion::Ribbon, false) => Step::Document,
        (KeyRegion::Ribbon, true) => Step::StatusBar,
        (KeyRegion::Pane, _) => Step::Panes,
        (KeyRegion::Document | KeyRegion::Other, false) => Step::Panes,
        (KeyRegion::Document | KeyRegion::Other, true) => Step::Ribbon,
    };
    enter(app, ctx, step, reverse)
}

/// Draw the open panes around the document. Call before the canvas's `CentralPanel`.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let open = open(app);
    if !open.contains(&Pane::FormatShape) {
        crate::format_shape::forget_drag(ui.ctx());
    }
    let cycle = route_f6(app, ui.ctx());
    let states: Vec<(Pane, PaneState)> = open.iter().map(|pane| (*pane, pane_state(app, *pane))).collect();
    let out = PaneHost::new(app.window_id(HOST))
        .policy(SidePolicy::Columns)
        .f6(false)
        .labels(labels())
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)))
        .body_margin(egui::Margin::same(10))
        .show(ui, &open, |pane| states.iter().find(|(p, _)| p == pane).map(|(_, state)| *state), &mut Content { app });
    for request in out.requests {
        apply(app, request);
    }
    let ctx = ui.ctx().clone();
    if out.to_document {
        // Past the last pane comes the status bar; before the first one, the document.
        match cycle {
            Some(false) => {
                enter(app, &ctx, Step::StatusBar, false);
            }
            _ => app.canvas.want_focus = true,
        }
    }
    ctx.data_mut(|d| d.insert_temp(pane_key(app), out.focused.is_some()));
    let focused = ctx.memory(|m| m.focused());
    let region = region_of(app, &ctx, focused);
    ctx.data_mut(|d| d.insert_temp(last_region_key(app), region));
}

/// Run a request from the pane host through the pane's commands and the UI preferences.
pub(crate) fn apply(app: &mut WordApp, request: PaneRequest<Pane>) {
    match request {
        PaneRequest::Close(pane) => close(app, pane),
        PaneRequest::Place(Pane::Styles, place) => {
            // Physical sides: the host is not RTL-flipped, and Dock Left / Dock Right mean the
            // window's left and right (same as `ui.paneDock` / `view.stylesPaneDock`).
            let dock = match place {
                Place::Docked(Edge::Start) => "left",
                Place::Docked(Edge::End) => "right",
                Place::Floating => "float",
                Place::Docked(Edge::Bottom) => return,
            };
            let _ = app.run("view.stylesPaneDock", json!({"dock": dock}));
        }
        PaneRequest::Resize(Pane::Styles, width) => app.ui.styles_pane_width = Some(width),
        PaneRequest::FloatRect(Pane::Styles, rect) => app.ui.styles_pane_float = Some(rect),
        PaneRequest::Place(pane, place) => {
            let dock = match place {
                Place::Docked(Edge::Start) => "left",
                Place::Docked(Edge::End) => "right",
                Place::Floating => "float",
                Place::Docked(Edge::Bottom) => return,
            };
            let _ = app.run("ui.paneDock", json!({"pane": pane.key(), "dock": dock}));
        }
        PaneRequest::Resize(pane, width) => app.ui.pane_places.entry(pane.key().into()).or_default().width = Some(width),
        PaneRequest::FloatRect(pane, rect) => app.ui.pane_places.entry(pane.key().into()).or_default().float = Some(rect),
        PaneRequest::Before(pane, before) => {
            let _ = app.run("ui.paneDock", json!({"pane": pane.key(), "before": before.map(Pane::key)}));
        }
        PaneRequest::Focus(_) => {}
    }
}

/// `ui.paneDock {"pane": key, "dock"?: "left" | "right" | "float", "before"?: key | null}` (#1756):
/// docks or floats a task pane, as its menu's Dock Left, Dock Right and Float do; without `dock`
/// it reads the pane's place. `pane` is the pane's `panePlaces` key; Styles goes through
/// `view.stylesPaneDock`. `before` puts the pane directly before that pane in the order, outside
/// it on their edge, as a drop over a docked pane does; `null` puts it after every pane.
pub(crate) fn dock_command(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    let key = p.get("pane").and_then(Value::as_str).ok_or("`pane` must be a task pane's key, such as \"navigation\"")?;
    let pane = Pane::from_key(key).ok_or_else(|| format!("unknown task pane `{key}`"))?;
    let dock = match p.get("dock") {
        None => None,
        Some(v) => Some(match v.as_str() {
            Some("left") => SavedDock::Left,
            Some("right") => SavedDock::Right,
            Some("float") => SavedDock::Float,
            _ => return Err("`dock` must be left, right or float".into()),
        }),
    };
    let before = match p.get("before") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => match v.as_str().and_then(Pane::from_key) {
            Some(before) if before != pane => Some(Some(before)),
            _ => return Err("`before` must be another task pane's key or null".into()),
        },
    };
    if let Some(before) = before {
        move_before(app, pane, before);
    }
    if pane == Pane::Styles {
        let params = p.get("dock").map_or_else(|| json!({}), |d| json!({"dock": d}));
        let result = app.run("view.stylesPaneDock", params)?;
        return Ok(json!({"pane": key, "dock": result.get("dock")}));
    }
    if let Some(dock) = dock {
        app.ui.pane_places.entry(key.into()).or_default().dock = Some(dock);
    }
    // Report the physical side the pane occupies (matches what was stored / requested).
    let name = match pane_state(app, pane).place {
        Place::Docked(Edge::Start) => "left",
        Place::Floating => "float",
        Place::Docked(_) => "right",
    };
    Ok(json!({"pane": key, "dock": name}))
}

fn close(app: &mut WordApp, pane: Pane) {
    match pane.close_command() {
        Some(id) => {
            let _ = app.run(id, json!({"value": false}));
        }
        None if pane == Pane::Proofing => app.spelling_review = None,
        None => app.restrict.open = false,
    }
}
