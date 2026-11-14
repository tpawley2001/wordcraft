//! View tab and status bar: views, zoom, show/hide, panes.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wordcraft_doc::{Zoom, ZoomFit};
use wordcraft_layout::{NumeralMode, ViewMode};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Toggles or sets a boolean view property, returning the updated state.
fn toggle(s: &mut Session, v: &Value, f: fn(&mut crate::ViewState) -> &mut bool) -> CmdResult {
    let cur = *f(&mut s.view);
    *f(&mut s.view) = p::bool(v, "value").unwrap_or(!cur);
    Ok(json!({"value": *f(&mut s.view)}))
}

/// A grid spacing or origin in points: 0.5..=1584 for a spacing, 0..=1584 for an origin.
fn grid_pt(v: &Value, k: &str, min: f32) -> Result<Option<f32>, CmdError> {
    let Some(x) = v.get(k) else { return Ok(None) };
    match x.as_f64().map(|x| x as f32).filter(|x| x.is_finite()) {
        Some(x) if (min..=1584.0).contains(&x) => Ok(Some(x)),
        _ => Err(CmdError::Params(format!("`{k}` must be a number of points from {min} to 1584"))),
    }
}

/// A gridline interval: an integer 0..=127 (0 draws none).
fn grid_every(v: &Value, k: &str) -> Result<Option<u8>, CmdError> {
    let Some(x) = v.get(k) else { return Ok(None) };
    match x.as_f64().filter(|x| x.is_finite() && x.fract() == 0.0 && (0.0..=127.0).contains(x)) {
        Some(x) => Ok(Some(x as u8)),
        None => Err(CmdError::Params(format!("`{k}` must be a whole number from 0 to 127"))),
    }
}

fn grid_settings(s: &mut Session, v: &Value) -> CmdResult {
    let h = grid_pt(v, "horizontalSpacing", 0.5)?;
    let vv = grid_pt(v, "verticalSpacing", 0.5)?;
    let every_h = grid_every(v, "horizontalEvery")?;
    let every_v = grid_every(v, "verticalEvery")?;
    let use_margins = match v.get("useMargins") {
        None => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return Err(CmdError::Params("`useMargins` must be true or false".into())),
    };
    let ox = grid_pt(v, "horizontalOrigin", 0.0)?;
    let oy = grid_pt(v, "verticalOrigin", 0.0)?;
    let g = &mut s.doc.settings;
    if let Some(x) = h {
        g.grid_h = x;
    }
    if let Some(x) = vv {
        g.grid_v = x;
    }
    if let Some(x) = every_h {
        g.grid_every_h = x;
    }
    if let Some(x) = every_v {
        g.grid_every_v = x;
    }
    if let Some(x) = use_margins {
        g.grid_use_margins = x;
    }
    if let Some(x) = ox {
        g.grid_origin_x = x;
    }
    if let Some(x) = oy {
        g.grid_origin_y = x;
    }
    let g = &s.doc.settings;
    Ok(json!({
        "horizontalSpacing": g.grid_h,
        "verticalSpacing": g.grid_v,
        "horizontalEvery": g.grid_every_h,
        "verticalEvery": g.grid_every_v,
        "useMargins": g.grid_use_margins,
        "horizontalOrigin": g.grid_origin_x,
        "verticalOrigin": g.grid_origin_y,
    }))
}

/// Returns the command specifications for the View ribbon tab and window management commands.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view.marks", "Show/Hide ¶", "Home › Paragraph", |s, v| {
            let r = toggle(s, v, |x| &mut x.marks);
            s.relayout();
            r
        })
        .key("Mod+Shift+8 / Mod+8")
        .pure(),
        CommandSpec::new("view.printLayout", "Print Layout", "View › Views", |s, _| mode(s, ViewMode::Print, false)).key("Mod+Alt+P").pure(),
        CommandSpec::new("view.webLayout", "Web Layout", "View › Views", |s, _| mode(s, ViewMode::Web, false)).pure(),
        CommandSpec::new("view.draft", "Draft", "View › Views", |s, _| mode(s, ViewMode::Draft, false)).key("Mod+Alt+N").pure(),
        // The body as an outline of its headings, with the Outlining tab (`outline.*`).
        CommandSpec::new("view.outline", "Outline", "View › Views", |s, _| mode(s, ViewMode::Outline, false)).key("Mod+Alt+O").pure(),
        CommandSpec::new("view.readMode", "Read Mode", "View › Views", |s, _| mode(s, ViewMode::Print, true)).pure(),
        CommandSpec::new("view.readColumnWidth", "Reading Column Width", "Reading › Layout", read_column_width)
            .params(r#"{"value": "auto" | "narrow" | "medium" | "wide" | "page"}"#)
            .pure(),
        CommandSpec::new("view.readTextSize", "Reading Text Size", "Reading › Layout", read_text_size)
            .params(r#"{"value": percent (75–150)}"#)
            .pure(),
        CommandSpec::new("view.readLineSpacing", "Reading Line Spacing", "Reading › Layout", read_line_spacing)
            .params(r#"{"value": "document" | 1.0 | 1.25 | 1.5 | 2.0}"#)
            .pure(),
        CommandSpec::new("view.readPageColor", "Reading Page Color", "Reading › Layout", read_page_color)
            .params(r#"{"value": "none" | "sepia" | "inverse"}"#)
            .pure(),
        CommandSpec::new("view.readLayout", "Reading Layout", "Reading › Layout", read_layout)
            .params(r#"{"value": "column" | "paper"}"#)
            .pure(),
        CommandSpec::new("view.paperColor", "Screen Paper Color", "View › Display", |s, v| {
            let color: PaperColor = serde_json::from_value(v.get("value").cloned().unwrap_or(Value::Null))
                .map_err(|_| CmdError::Params("screen paper color must be document, green, cream, or gray".into()))?;
            s.prefs.paper_color = color;
            if color != PaperColor::Document {
                s.view.dark_mode = false;
            }
            Ok(json!({"paperColor": color}))
        }).params(r#"{"value": "document" | "green" | "cream" | "gray"}"#).pure(),
        // Print Layout: hide the white space (margins, headers, footers) between pages. Screen only.
        CommandSpec::new("view.hideWhiteSpace", "Hide White Space", "View › Show", |s, v| toggle(s, v, |x| &mut x.hide_white_space))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        // Focus and Full Screen View (#1678) fill the screen; turning one on turns the other off.
        // Full Screen View has no key of its own: Customize Keyboard binds one.
        CommandSpec::new("view.focus", "Focus", "View › Immersive", |s, v| {
            let r = toggle(s, v, |x| &mut x.focus_mode);
            s.view.full_screen_view &= !s.view.focus_mode;
            r
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("view.fullScreenView", "Full Screen View", "View › Immersive", |s, v| {
            let r = toggle(s, v, |x| &mut x.full_screen_view);
            s.view.focus_mode &= !s.view.full_screen_view;
            r
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        // Immersive Reader: its own view and tab (View › Immersive). Its settings are screen only
        // and work whether it is open or not.
        CommandSpec::new("view.immersive", "Immersive Reader", "View › Immersive", |s, v| {
            let on = p::bool(v, "value").unwrap_or(!s.view.immersive);
            set_immersive(s, on)
        })
        .params(r#"{"value"?: bool} — no value toggles. Opening leaves Read Mode, Focus and Full Screen View; the view under it (Print Layout, Web Layout, Draft, Outline) is kept for Close."#)
        .pure(),
        CommandSpec::new("view.closeImmersive", "Close Immersive Reader", "Immersive Reader › Close", |s, _| set_immersive(s, false)).pure(),
        CommandSpec::new("view.immersiveColumnWidth", "Column Width", "Immersive Reader › Immersive Reader", immersive_column_width)
            .params(r#"{"value": "veryNarrow" | "narrow" | "moderate" | "wide"}"#)
            .pure(),
        CommandSpec::new("view.immersivePageColor", "Page Color", "Immersive Reader › Immersive Reader", immersive_page_color)
            .params(r##"{"value": "none" | "sepia" | "inverse" | "#RRGGBB"}"##)
            .pure(),
        CommandSpec::new("view.lineFocus", "Line Focus", "Immersive Reader › Immersive Reader", line_focus)
            .params(r#"{"value": 0 | 1 | 3 | 5} — the number of lines left uncovered (0: none)"#)
            .pure(),
        CommandSpec::new("view.textSpacing", "Text Spacing", "Immersive Reader › Immersive Reader", |s, v| {
            toggle(s, v, |x| &mut x.text_spacing)
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("view.syllables", "Syllables", "Immersive Reader › Immersive Reader", |s, v| toggle(s, v, |x| &mut x.syllables))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.ruler", "Ruler", "View › Show", |s, v| toggle(s, v, |x| &mut x.ruler)).pure(),
        // The drawing grid over the page (screen only; never printed or exported).
        CommandSpec::new("view.gridlines", "Gridlines", "View › Show", |s, v| toggle(s, v, |x| &mut x.gridlines))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        // The drawing grid's spacing, origin and how often gridlines are shown (Layout › Align ›
        // Grid Settings…), stored in settings.xml as Word does (ECMA-376 §17.15.1).
        CommandSpec::new("view.gridSettings", "Grid Settings", "Layout › Align", grid_settings)
            .params(
                r#"{"horizontalSpacing"?: pt (0.5-1584), "verticalSpacing"?: pt (0.5-1584), "horizontalEvery"?: n (0-127; vertical gridlines every n units, 0 = none), "verticalEvery"?: n (0-127; horizontal gridlines every n units, 0 = none), "useMargins"?: bool, "horizontalOrigin"?: pt (0-1584), "verticalOrigin"?: pt (0-1584)} → the resulting values"#,
            ),
        // Header & Footer › Show Document Text (Word's `View.ShowMainTextLayer`): screen only.
        CommandSpec::new("view.showDocumentText", "Show Document Text", "Header & Footer › Options", |s, v| toggle(s, v, |x| &mut x.show_document_text))
            .params(r#"{"value"?: bool (omit to toggle)}"#)
            .pure(),
        // Outlines of table cells (screen only; never printed or exported).
        CommandSpec::new("table.viewGridlines", "View Gridlines", "Table Layout › Table", |s, v| toggle(s, v, |x| &mut x.table_gridlines))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.navigationPane", "Navigation Pane", "View › Show", |s, v| toggle(s, v, |x| &mut x.nav_pane)).pure(),
        CommandSpec::new("view.zoom", "Zoom", "View › Zoom", zoom)
            .params(r#"{"value": percent (10-500) | "pageWidth" | "onePage" | "multiplePages"}"#)
            .pure(),
        CommandSpec::new("view.zoom100", "100%", "View › Zoom", |s, _| zoom(s, &json!({"value": 100}))).pure(),
        CommandSpec::new("view.zoomIn", "Zoom In", "View › Zoom", |s, _| {
            let z = ((s.view.zoom * 10.0).round() / 10.0 + 0.1).min(5.0);
            zoom(s, &json!({"value": z * 100.0}))
        })
        .pure(),
        CommandSpec::new("view.zoomOut", "Zoom Out", "View › Zoom", |s, _| {
            let z = ((s.view.zoom * 10.0).round() / 10.0 - 0.1).max(0.1);
            zoom(s, &json!({"value": z * 100.0}))
        })
        .pure(),
        CommandSpec::new("view.onePage", "One Page", "View › Zoom", |s, _| zoom(s, &json!({"value": "onePage"}))).pure(),
        CommandSpec::new("view.multiplePages", "Multiple Pages", "View › Zoom", |s, _| zoom(s, &json!({"value": "multiplePages"}))).pure(),
        CommandSpec::new("view.pageWidth", "Page Width", "View › Zoom", |s, _| zoom(s, &json!({"value": "pageWidth"}))).pure(),
        CommandSpec::new("view.darkMode", "Switch Modes", "View › Dark Mode", |s, v| toggle(s, v, |x| &mut x.dark_mode)).pure(),
        // The interface theme itself (light, dark, or follow the OS), next to dark pages (#1861).
        // The session can't paint: like split and new windows, this queues a request the front
        // end applies to its theme setting (which persists per user). Without a value it toggles
        // light and dark, as the manual switch did.
        CommandSpec::new("view.interfaceTheme", "Interface Theme", "View › Dark Mode", |s, v| match v.get("value") {
            None => {
                s.push_ui_request(json!({"toggle": "interfaceTheme"}));
                Ok(json!({"value": "toggle"}))
            }
            Some(Value::String(name)) => match name.as_str() {
                "light" | "dark" | "system" => {
                    s.push_ui_request(json!({"interfaceTheme": name}));
                    Ok(json!({"value": name}))
                }
                _ => Err(CmdError::Params("interface theme: light|dark|system".into())),
            },
            _ => Err(CmdError::Params("interface theme: light|dark|system".into())),
        })
        .params(r#"{"value"?: "light" | "dark" | "system"; without it, toggles light and dark}"#)
        .pure(),
        CommandSpec::new("view.stylesPane", "Styles Pane", "Home › Styles", |s, v| toggle(s, v, |x| &mut x.styles_pane)).pure(),
        // Where the Styles pane sits: docked right or left, or floating (#1756). No `dock` reads it.
        CommandSpec::new("view.stylesPaneDock", "Styles Pane Position", "Home › Styles", styles_pane_dock)
            .params(r#"{"dock"?: "left" | "right" | "float"}"#)
            .pure(),
        CommandSpec::new("view.commentsPane", "Comments Pane", "Review › Comments", |s, v| toggle(s, v, |x| &mut x.comments_pane)).pure(),
        CommandSpec::new("view.accessibilityPane", "Accessibility Pane", "Review › Accessibility", |s, v| {
            toggle(s, v, |x| &mut x.accessibility_pane)
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("view.altTextPane", "Alt Text Pane", "Review › Accessibility", |s, v| toggle(s, v, |x| &mut x.alt_text_pane))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.formatShapePane", "Format Shape Pane", "Shape Format › Shape Styles", |s, v| {
            toggle(s, v, |x| &mut x.format_shape_pane)
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("view.newWindow", "New Window", "View › Window", |s, _| {
            s.push_ui_request(json!({"open": "newWindow"}));
            sel_result(s)
        })
        .pure(),
        CommandSpec::new("view.switchWindows", "Switch Windows", "View › Window", switch_windows)
            .params(r#"{"id"?: n (make this open document the active one), "window"?: n (make this open window the active one)} → {"documents": [{"id", "title", "path", "dirty", "active"}]} in the order they were opened"#)
            .pure(),
        CommandSpec::new("view.nextWindow", "Next Window", "View › Window", |s, _| cycle_window(s, false))
            .key(super::mac_or("", "Mod+F6 / Alt+F6"))
            .pure(),
        CommandSpec::new("view.previousWindow", "Previous Window", "View › Window", |s, _| cycle_window(s, true))
            .key(super::mac_or("", "Mod+Shift+F6 / Alt+Shift+F6"))
            .pure(),
        // Word's Selection Pane (Layout › Arrange), a side pane listing the body's drawings.
        CommandSpec::new("view.selectionPane", "Selection Pane", "Layout › Arrange", |s, v| {
            s.view.selection_pane = p::bool(v, "value").unwrap_or(!s.view.selection_pane);
            Ok(json!({"pane": s.view.selection_pane}))
        })
        .params(r#"{"value"?: bool (show/hide the pane)}"#)
        .key(super::mac_or("", "Alt+F10"))
        .pure(),
        // Split (#3254): two panes, one above the other, on the same document. The session keeps
        // the top pane's share of the height; the desktop app draws the panes and sends the ratio
        // back while the divider is dragged.
        CommandSpec::new("view.split", "Split", "View › Window", split)
            .params(r#"{"value"?: bool (default: toggle), "ratio"?: 0.1..0.9 (the top pane's share; turns the split on)} → {"split": bool, "ratio": number | null}"#)
            .key("Mod+Alt+S")
            .pure(),
        // Word's ClosePane key on Windows; Word for Mac has none.
        CommandSpec::new("view.removeSplit", "Remove Split", "View › Window", |s, _| {
            s.view.split = None;
            Ok(json!({"split": false}))
        })
        .key(super::non_mac("Alt+Shift+C"))
        .pure(),
        // View › Window across WordCraft windows (#322): the desktop app's native windows
        // perform these (see `Windows`), elsewhere they are disabled.
        CommandSpec::new("view.arrangeAll", "Arrange All", "View › Window", |s, _| {
            s.ui_requests.push(json!({"windows": "arrange"}));
            Ok(json!({"windows": s.windows.others.len() + 1}))
        })
        .when(windows_available)
        .pure(),
        CommandSpec::new("view.sideBySide", "View Side by Side", "View › Window", side_by_side)
            .params(r#"{"value"?: bool, "window"?: id of the other window (optional when one other window is open)}"#)
            .when(|s| match windows_available(s) {
                None if s.windows.side_by_side.is_none() => has_other_window(s),
                r => r,
            })
            .pure(),
        CommandSpec::new("view.syncScroll", "Synchronous Scrolling", "View › Window", |s, v| {
            let Some(with) = s.windows.side_by_side else { return Err(CmdError::Disabled("turn on View Side by Side first".into())) };
            s.windows.sync_scroll = p::bool(v, "value").unwrap_or(!s.windows.sync_scroll);
            s.ui_requests.push(json!({"windows": "syncScroll", "with": with, "value": s.windows.sync_scroll}));
            Ok(json!({"value": s.windows.sync_scroll}))
        })
        .params(r#"{"value"?: bool}"#)
        .when(|s| if s.windows.side_by_side.is_some() { None } else { Some("turn on View Side by Side first") })
        .pure(),
        // Puts the pair back on equal halves of the screen; pairing and scrolling stay as they are.
        CommandSpec::new("view.resetWindowPosition", "Reset Window Position", "View › Window", |s, _| {
            let Some(with) = s.windows.side_by_side else { return Err(CmdError::Disabled("turn on View Side by Side first".into())) };
            s.ui_requests.push(json!({"windows": "resetPosition", "with": with}));
            Ok(json!({"window": with}))
        })
        .when(|s| if s.windows.side_by_side.is_some() { None } else { Some("turn on View Side by Side first") })
        .pure(),
        // The window fills the screen, OS-level full screen (#980). F11 as in browsers and most
        // desktop apps; ⌃⌘F first on macOS, where Ctrl is a key of its own (elsewhere Mod is Ctrl,
        // so "Mod+Ctrl+F" would read Ctrl+Ctrl+F). Escape also leaves it. The front end applies it.
        CommandSpec::new("view.fullScreen", "Full Screen", "View › Window", |s, v| toggle(s, v, |x| &mut x.full_screen))
            .params(r#"{"value"?: bool}"#)
            .key(super::mac_or("Mod+Ctrl+F / F11", "F11"))
            .when(full_screen_available)
            .pure(),
        CommandSpec::new("view.state", "View State", "View", |s, _| serde_json::to_value(&s.view).map_err(|e| CmdError::Failed(e.to_string())))
            .pure(),
        // Word › File › Options › Advanced › Numeral: how digits look on screen, in print, PDFs
        // and page images (display only; the document text keeps the typed codepoints).
        CommandSpec::new("view.numeral", "Numerals", "File › Options › Advanced", |s, v| match v.get("value") {
            None => Ok(json!({"value": s.view.numeral})),
            Some(Value::String(name)) => {
                let Some(mode) = NumeralMode::parse(name) else {
                    return Err(CmdError::Params("numeral mode: arabic|hindi|context|system".into()));
                };
                s.view.numeral = mode;
                s.relayout();
                Ok(json!({"value": s.view.numeral}))
            }
            Some(_) => Err(CmdError::Params("numeral mode: arabic|hindi|context|system".into())),
        })
        .params(r#"{"value"?: "arabic|hindi|context|system"}"#)
        .pure(),
    ]
}

/// The top pane's share of the height while split, and the default when splitting.
const SPLIT_RATIO: std::ops::RangeInclusive<f32> = 0.1..=0.9;
const SPLIT_EVEN: f32 = 0.5;

fn split(s: &mut Session, v: &Value) -> Result<Value, CmdError> {
    let ratio = match v.get("ratio") {
        None | Some(Value::Null) => None,
        Some(r) => match r.as_f64().map(|r| r as f32) {
            Some(r) if r.is_finite() && SPLIT_RATIO.contains(&r) => Some(r),
            _ => return Err(CmdError::Params("ratio: a number from 0.1 to 0.9".into())),
        },
    };
    let on = match v.get("value") {
        None | Some(Value::Null) => ratio.is_some() || s.view.split.is_none(),
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(CmdError::Params("value: bool".into())),
    };
    s.view.split = on.then(|| ratio.or(s.view.split).unwrap_or(SPLIT_EVEN));
    // Reported to four places: 0.4, not the f32's 0.4000000059604645.
    let ratio = s.view.split.map(|r| (f64::from(r) * 1e4).round() / 1e4);
    Ok(json!({"split": s.view.split.is_some(), "ratio": ratio}))
}

/// View › Window (#322): the other native windows of the app as the window being drawn sees them
/// (by window id, the ids of `ui.inspect`'s `windows`), and its Side by Side pairing. The front
/// end keeps these current as each window draws; elsewhere (web, CLI, MCP without the app) they
/// stay empty and the window commands are disabled.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Windows {
    /// The host can list and move its windows (the desktop app).
    pub available: bool,
    /// The active window, when known.
    pub current: Option<u64>,
    /// The other open windows, by id.
    pub others: Vec<OtherWindow>,
    /// View Side by Side is on, with this window.
    pub side_by_side: Option<u64>,
    /// Synchronous Scrolling (only while Side by Side is on).
    pub sync_scroll: bool,
}

/// Another open WordCraft window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OtherWindow {
    pub id: u64,
    pub title: String,
}

fn windows_available(s: &Session) -> Option<&'static str> {
    if s.windows.available { None } else { Some("only the desktop app has several windows") }
}

fn has_other_window(s: &Session) -> Option<&'static str> {
    windows_available(s).or(if s.windows.others.is_empty() { Some("no other WordCraft window is open") } else { None })
}

/// The window a command means: `window`, or the only other one.
fn pick_window(s: &Session, v: &Value) -> Result<u64, CmdError> {
    let others = &s.windows.others;
    match p::u64(v, "window") {
        Some(id) if others.iter().any(|w| w.id == id) => Ok(id),
        Some(id) => Err(CmdError::Params(format!("no open window {id}"))),
        None => match others.as_slice() {
            [only] => Ok(only.id),
            [] => Err(CmdError::Disabled("no other WordCraft window is open".into())),
            several => {
                let list: Vec<String> = several.iter().map(|w| format!("{} ({})", w.id, w.title)).collect();
                Err(CmdError::Params(format!("pass `window`: one of {}", list.join(", "))))
            }
        },
    }
}

/// View Side by Side: on with a window (left this one, right that one, scrolling together), or
/// off.
fn side_by_side(s: &mut Session, v: &Value) -> CmdResult {
    let on = p::bool(v, "value").unwrap_or(s.windows.side_by_side.is_none() || v.get("window").is_some());
    if !on {
        if let Some(id) = s.windows.side_by_side.take() {
            s.ui_requests.push(json!({"windows": "sideBySide", "off": id}));
        }
        s.windows.sync_scroll = false;
        return Ok(json!({"value": false}));
    }
    let id = match (s.windows.side_by_side, v.get("window")) {
        (Some(id), None) => id,
        _ => pick_window(s, v)?,
    };
    if let Some(old) = s.windows.side_by_side.filter(|old| *old != id) {
        s.ui_requests.push(json!({"windows": "sideBySide", "off": old}));
    }
    // The partner it already has keeps the user's Synchronous Scrolling choice and is not asked
    // again; a new pairing starts with scrolling on.
    if s.windows.side_by_side != Some(id) {
        s.windows.side_by_side = Some(id);
        s.windows.sync_scroll = true;
        s.ui_requests.push(json!({"windows": "sideBySide", "with": id}));
    }
    Ok(json!({"value": true, "window": id, "syncScroll": s.windows.sync_scroll}))
}

/// The browser keeps full screen to itself (its own F11); the web app can't ask for it.
fn full_screen_available(_: &Session) -> Option<&'static str> {
    cfg!(target_arch = "wasm32").then_some("Use the browser's full screen (F11)")
}

fn mode(s: &mut Session, m: ViewMode, read: bool) -> CmdResult {
    if read && !s.view.read_mode {
        // Width, size and spacing reset on each entry. Page Color follows the saved preference.
        s.view.read_column_width = None;
        s.view.read_zoom = 1.0;
        s.view.read_line_spacing = None;
        s.view.read_page_color = s.prefs.read_page_color;
        s.view.read_layout = ReadLayout::Column;
    }
    s.view.mode = m;
    s.view.read_mode = read;
    // Read Mode and the other views close Immersive Reader.
    s.view.immersive = false;
    s.relayout();
    Ok(json!({"mode": m, "readMode": read}))
}

fn styles_pane_dock(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(d) = v.get("dock") {
        let Some(dock) = d.as_str().and_then(crate::StylesPaneDock::parse) else {
            return Err(CmdError::Params("dock must be left, right or float".into()));
        };
        s.view.styles_pane_dock = dock;
    }
    Ok(json!({"dock": s.view.styles_pane_dock.name()}))
}

fn read_column_width(s: &mut Session, v: &Value) -> CmdResult {
    let width = match v.get("value").and_then(Value::as_str) {
        Some("auto") => None,
        Some("narrow") => Some(180.0),
        Some("medium") => Some(240.0),
        Some("wide") => Some(350.0),
        Some("page") => Some(612.0),
        _ => return Err(CmdError::Params("column width must be auto, narrow, medium, wide, or page".into())),
    };
    s.view.read_column_width = width;
    Ok(json!({"width": width}))
}

fn read_text_size(s: &mut Session, v: &Value) -> CmdResult {
    let Some(pct) = v.get("value").and_then(Value::as_f64) else {
        return Err(CmdError::Params("reading text size must be a percentage".into()));
    };
    if !pct.is_finite() || !(75.0..=150.0).contains(&pct) {
        return Err(CmdError::Params("reading text size must be 75–150%".into()));
    }
    s.view.read_zoom = pct as f32 / 100.0;
    Ok(json!({"percent": pct}))
}

fn read_line_spacing(s: &mut Session, v: &Value) -> CmdResult {
    let value = v.get("value");
    let spacing = match value {
        Some(Value::String(s)) if s == "document" => None,
        Some(Value::Number(n)) if matches!(n.as_f64(), Some(1.0 | 1.25 | 1.5 | 2.0)) => n.as_f64().map(|n| n as f32),
        _ => return Err(CmdError::Params("reading line spacing must be document, 1, 1.25, 1.5, or 2".into())),
    };
    s.view.read_line_spacing = spacing;
    Ok(json!({"spacing": spacing}))
}

/// Opens or closes Immersive Reader. Opening leaves Read Mode, Focus and Full Screen View and
/// keeps `mode`, the view Close goes back to.
fn set_immersive(s: &mut Session, on: bool) -> CmdResult {
    if on {
        s.view.read_mode = false;
        s.view.focus_mode = false;
        s.view.full_screen_view = false;
    }
    s.view.immersive = on;
    Ok(json!({"value": on}))
}

/// Immersive Reader › Column Width: how long the reading lines are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImmersiveColumnWidth {
    VeryNarrow,
    Narrow,
    Moderate,
    /// The window's width less an inch each side.
    #[default]
    Wide,
}

impl ImmersiveColumnWidth {
    pub const ALL: [Self; 4] = [Self::VeryNarrow, Self::Narrow, Self::Moderate, Self::Wide];

    /// `veryNarrow`, `narrow`, `moderate` or `wide`; anything else is `None`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::VeryNarrow => "veryNarrow",
            Self::Narrow => "narrow",
            Self::Moderate => "moderate",
            Self::Wide => "wide",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::VeryNarrow => "Very Narrow",
            Self::Narrow => "Narrow",
            Self::Moderate => "Moderate",
            Self::Wide => "Wide",
        }
    }

    /// The column's width in points in a window `window` points wide: fixed for the first
    /// three, the window less 72 pt each side for Wide, never below Very Narrow's 252 pt.
    pub fn points(self, window: f32) -> f32 {
        match self {
            Self::VeryNarrow => 252.0,
            Self::Narrow => 360.0,
            Self::Moderate => 468.0,
            Self::Wide => {
                let w = window - 144.0;
                if w.is_finite() { w.clamp(252.0, 20_000.0) } else { 252.0 }
            }
        }
    }
}

fn immersive_column_width(s: &mut Session, v: &Value) -> CmdResult {
    let Some(w) = v.get("value").and_then(Value::as_str).and_then(ImmersiveColumnWidth::parse) else {
        return Err(CmdError::Params("column width must be veryNarrow, narrow, moderate or wide".into()));
    };
    s.view.immersive_column_width = w;
    Ok(json!({"columnWidth": w.name()}))
}

/// An Immersive Reader page colour as it is stored: `none`, `sepia`, `inverse`, or `#RRGGBB`
/// with upper-case digits. `None` for anything else.
pub fn immersive_page_color_name(value: &str) -> Option<String> {
    match value {
        "none" | "sepia" | "inverse" => Some(value.to_string()),
        _ => {
            let hex = value.strip_prefix('#')?;
            (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("#{}", hex.to_ascii_uppercase()))
        }
    }
}

fn immersive_page_color(s: &mut Session, v: &Value) -> CmdResult {
    let Some(color) = v.get("value").and_then(Value::as_str).and_then(immersive_page_color_name) else {
        return Err(CmdError::Params("page color must be none, sepia, inverse or #RRGGBB".into()));
    };
    s.view.immersive_page_color = color.clone();
    Ok(json!({"pageColor": color}))
}

fn line_focus(s: &mut Session, v: &Value) -> CmdResult {
    let Some(n) = v.get("value").and_then(Value::as_u64).filter(|n| matches!(n, 0 | 1 | 3 | 5)) else {
        return Err(CmdError::Params("line focus must be 0, 1, 3 or 5 lines".into()));
    };
    s.view.line_focus = n as u8;
    Ok(json!({"lineFocus": n}))
}

/// Immersive Reader › Syllables: where `word`, written in `lang` (a BCP 47 tag; `None` is US
/// English), breaks between syllables, as byte offsets strictly inside the word. They are the
/// automatic hyphenation points of the language's patterns, with at least two letters before
/// the first and after the last. A language without patterns gives none.
pub fn syllable_breaks(word: &str, lang: Option<&str>) -> Vec<usize> {
    use wordcraft_proof::hyphen::{Limits, Source, hyphen_points_in, word_breaks_in};
    let lang = wordcraft_proof::ProofLang::from_tag(Some(lang.unwrap_or("en-US")));
    // The heuristic fallback guesses; only a language's own patterns, dictionary or rules count.
    if matches!(word_breaks_in(word, lang).1, Source::Heuristic | Source::None) {
        return Vec::new();
    }
    let lim = Limits { min_word: 4, after_first: 2, before_last: 2, capitalized: true };
    let offs: Vec<usize> = word.char_indices().map(|(o, _)| o).collect();
    hyphen_points_in(word, &lim, lang).into_iter().filter_map(|i| offs.get(i).copied()).filter(|&b| b > 0 && b < word.len()).collect()
}

/// Screen-only paper presets, saved with user preferences, never in the document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaperColor {
    #[default]
    Document,
    Green,
    Cream,
    Gray,
}

impl PaperColor {
    pub const ALL: [Self; 4] = [Self::Document, Self::Green, Self::Cream, Self::Gray];

    pub fn label(self) -> &'static str {
        match self {
            Self::Document => "Document background",
            Self::Green => "Soft green",
            Self::Cream => "Warm cream",
            Self::Gray => "Soft gray",
        }
    }

    pub fn color(self) -> wordcraft_doc::props::Rgb {
        use wordcraft_doc::props::Rgb;
        match self {
            Self::Document => Rgb::WHITE,
            Self::Green => Rgb(0xCC, 0xE8, 0xCF),
            Self::Cream => Rgb(0xFA, 0xF0, 0xDC),
            Self::Gray => Rgb(0xE8, 0xEA, 0xED),
        }
    }
}

/// Read Mode › View › Page Color, as in Word. Screen only: the document and its own page color
/// are unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadPageColor {
    /// White paper; the document's page color and page picture are not shown.
    #[default]
    None,
    /// Sepia paper (Word's RGB 112, 66, 20 at 80% transparency over white); the text keeps its colors.
    Sepia,
    /// A dark page with light text, as View › Switch Modes draws it.
    Inverse,
}

/// Read Mode › View › Layout: reading columns (Column Layout) or the printed pages (Paper Layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadLayout {
    /// The text reflows into reading columns sized to the window.
    #[default]
    Column,
    /// The document's pages as they print.
    Paper,
}

fn read_page_color(s: &mut Session, v: &Value) -> CmdResult {
    let color = match v.get("value").and_then(Value::as_str) {
        Some("none") => ReadPageColor::None,
        Some("sepia") => ReadPageColor::Sepia,
        Some("inverse") => ReadPageColor::Inverse,
        _ => return Err(CmdError::Params("reading page color must be none, sepia, or inverse".into())),
    };
    s.view.read_page_color = color;
    s.prefs.read_page_color = color;
    Ok(json!({"pageColor": color}))
}

/// Read Mode › View › Layout. Paper Layout keeps Column Width and Line Spacing for when Column
/// Layout comes back.
fn read_layout(s: &mut Session, v: &Value) -> CmdResult {
    let layout = match v.get("value").and_then(Value::as_str) {
        Some("column") => ReadLayout::Column,
        Some("paper") => ReadLayout::Paper,
        _ => return Err(CmdError::Params("reading layout must be column or paper".into())),
    };
    s.view.read_layout = layout;
    Ok(json!({"layout": layout}))
}

/// The zoom a document is saved with (#995): the fit mode, else the percentage; `None` at 100%.
pub(crate) fn saved_zoom(v: &crate::ViewState) -> Option<Zoom> {
    let fit = match v.fit.as_str() {
        "onePage" => Some(ZoomFit::OnePage),
        "pageWidth" => Some(ZoomFit::PageWidth),
        "textWidth" => Some(ZoomFit::TextWidth),
        _ => None,
    };
    let percent = if v.zoom.is_finite() { (v.zoom * 100.0).round().clamp(10.0, 500.0) as u32 } else { 100 };
    Some(Zoom { percent, fit }).filter(|z| z.percent != 100 || z.fit.is_some())
}

/// Show an opened document at the zoom it was saved with.
pub(crate) fn restore_zoom(s: &mut Session, z: Zoom) {
    let fit = match z.fit {
        Some(ZoomFit::OnePage) => "onePage",
        Some(ZoomFit::PageWidth) => "pageWidth",
        Some(ZoomFit::TextWidth) => "textWidth",
        None => "",
    };
    s.view.zoom = z.percent.clamp(10, 500) as f32 / 100.0;
    s.view.fit = fit.into();
    s.view.multi_page = false;
    s.view.single_column = fit == "onePage" || fit == "pageWidth";
}

/// Sets the view zoom level or fit mode, or requests opening the Zoom dialog.
fn zoom(s: &mut Session, v: &Value) -> CmdResult {
    match v.get("value") {
        Some(Value::String(f)) if ["pageWidth", "onePage", "multiplePages", "textWidth"].contains(&f.as_str()) => {
            s.view.fit = f.clone();
            s.view.multi_page = f == "multiplePages";
            s.view.single_column = f == "onePage" || f == "pageWidth";
        }
        Some(x) => {
            let pct = x.as_f64().ok_or_else(|| CmdError::Params("zoom percent or fit mode".into()))? as f32;
            if !(10.0..=500.0).contains(&pct) {
                return Err(CmdError::Params("zoom must be 10–500%".into()));
            }
            s.view.zoom = pct / 100.0;
            s.view.fit.clear();
            s.view.multi_page = false;
        }
        None => {
            s.push_ui_request(json!({"open": "zoom"}));
        }
    }
    Ok(json!({"zoom": s.view.zoom, "fit": s.view.fit}))
}

/// View › Window › Switch Windows: the open documents, or make one of them active.
fn switch_windows(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(w) = v.get("window").filter(|x| !x.is_null()) {
        let wid = w.as_u64().ok_or_else(|| CmdError::Params("`window` must be an open window's id".into()))?;
        if s.windows.available && s.windows.current != Some(wid) && !s.windows.others.iter().any(|other| other.id == wid) {
            return Err(CmdError::Params(format!("no open window {wid}")));
        }
        s.push_ui_request(json!({"windows": "switchWindow", "window": wid}));
    } else if let Some(x) = v.get("id").filter(|x| !x.is_null()) {
        let id = x.as_u64().ok_or_else(|| CmdError::Params("`id` must be an open document's id".into()))?;
        s.activate(id)?;
    }
    Ok(json!({"documents": s.documents()}))
}

/// Move through the open documents without changing their opening order or edit history.
fn cycle_window(s: &mut Session, previous: bool) -> CmdResult {
    let documents = s.documents();
    if let Some(active) = documents.iter().position(|d| d.active) {
        let target = if previous {
            documents.iter().take(active).next_back().or_else(|| documents.last())
        } else {
            documents.get(active.saturating_add(1)).or_else(|| documents.first())
        };
        if let Some(document) = target {
            s.activate(document.id)?;
        }
    }
    switch_windows(s, &Value::Null)
}

#[cfg(test)]
mod tests {
    use super::syllable_breaks;

    #[test]
    fn syllable_breaks_fall_strictly_inside_the_word_on_char_boundaries() {
        let word = "information";
        let breaks = syllable_breaks(word, Some("en-US"));
        assert!(!breaks.is_empty(), "in-for-ma-tion has syllable breaks");
        for b in &breaks {
            assert!(*b > 0 && *b < word.len() && word.is_char_boundary(*b), "{b}");
        }
        assert!(syllable_breaks("a", Some("en-US")).is_empty());
        let accented = "désinformation";
        for b in syllable_breaks(accented, Some("fr-FR")) {
            assert!(b >= 3 && b < accented.len() && accented.is_char_boundary(b), "{b}");
        }
    }

    /// Immersive Reader lays the text out in one column as wide as Column Width, and Text
    /// Spacing widens it; each change lays the text out again. Print Layout and export don't move.
    #[test]
    fn immersive_column_width_and_text_spacing_lay_the_text_out_again() {
        use serde_json::json;
        let text = "Reading is easier with fewer distractions on the page. ".repeat(20);
        let mut s = crate::Session::new(wordcraft_doc::Document::from_text(&text));
        let print = s.layout();
        let export = s.export_layout().pages.len();
        s.run("view.immersive", &json!({"value": true})).unwrap();
        s.run("view.immersiveColumnWidth", &json!({"value": "veryNarrow"})).unwrap();
        let narrow = s.layout();
        assert!(narrow.pages.iter().all(|p| p.w == 252.0), "one column 252 pt wide");
        s.run("view.immersiveColumnWidth", &json!({"value": "moderate"})).unwrap();
        assert!(s.layout().pages.iter().all(|p| p.w == 468.0));
        let lines = |l: &wordcraft_layout::DocLayout| -> usize {
            l.pages.iter().flat_map(|p| &p.items).map(|i| if let wordcraft_layout::Placed::Lines { l0, l1, .. } = i { l1 - l0 } else { 0 }).sum()
        };
        let plain = lines(&s.layout());
        s.run("view.textSpacing", &json!({"value": true})).unwrap();
        assert!(lines(&s.layout()) > plain, "wider letters and words take more lines");
        s.run("view.closeImmersive", &json!({})).unwrap();
        assert_eq!(s.layout().pages[0].w, print.pages[0].w);
        assert_eq!(s.export_layout().pages.len(), export);
    }
}
