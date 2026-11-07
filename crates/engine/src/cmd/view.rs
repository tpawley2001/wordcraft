//! View tab and status bar: views, zoom, show/hide, panes.

use serde_json::{Value, json};
use wordcraft_doc::{Zoom, ZoomFit};
use wordcraft_layout::{NumeralMode, ViewMode};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

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
            .params(r#"{"id"?: n (make this open document the active one)} → {"documents": [{"id", "title", "path", "dirty", "active"}]} in the order they were opened"#)
            .pure(),
        // Arrange All: the window layer tiles every document window top to bottom.
        CommandSpec::new("view.arrangeAll", "Arrange All", "View › Window", |s, _| {
            s.push_ui_request(json!({"open": "arrangeAll"}));
            switch_windows(s, &Value::Null)
        })
        .params(r#"{} → {"documents": [...]} as view.switchWindows; the desktop app tiles the windows top to bottom"#)
        .pure(),
        CommandSpec::new("view.sideBySide", "View Side by Side", "View › Window", side_by_side)
            .params(r#"{"value"?: bool (default: toggle), "id"?: n (the other open document; default the only other one)} → {"sideBySide": {"left", "right", "syncScroll"} | null}; with three or more documents and no id: {"sideBySide": null, "choose": [ids]} and the app asks which one"#)
            .when(side_by_side_available)
            .pure(),
        CommandSpec::new("view.syncScroll", "Synchronous Scrolling", "View › Window", |s, v| {
            let Some(pair) = s.side_by_side.as_mut() else { return Err(CmdError::Disabled(NOT_SIDE_BY_SIDE.into())) };
            pair.sync_scroll = p::bool(v, "value").unwrap_or(!pair.sync_scroll);
            Ok(json!({"syncScroll": pair.sync_scroll}))
        })
        .params(r#"{"value"?: bool (default: toggle)} → {"syncScroll": bool}"#)
        .when(side_by_side_on)
        .pure(),
        CommandSpec::new("view.resetWindowPosition", "Reset Window Position", "View › Window", |s, _| {
            let Some(pair) = s.side_by_side else { return Err(CmdError::Disabled(NOT_SIDE_BY_SIDE.into())) };
            s.push_ui_request(json!({"open": "resetWindowPosition"}));
            Ok(side_by_side_state(Some(pair)))
        })
        .when(side_by_side_on)
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
    if let Some(x) = v.get("id").filter(|x| !x.is_null()) {
        let id = x.as_u64().ok_or_else(|| CmdError::Params("`id` must be an open document's id".into()))?;
        s.activate(id)?;
    }
    Ok(json!({"documents": s.documents()}))
}

const NOT_SIDE_BY_SIDE: &str = "turn on View Side by Side first";

/// The pair, if both of its documents are still open.
fn current_pair(s: &Session) -> Option<crate::SideBySide> {
    s.side_by_side.filter(|p| s.is_open(p.left) && s.is_open(p.right))
}

fn side_by_side_available(s: &Session) -> Option<&'static str> {
    (s.documents().len() < 2).then_some("open another document first")
}

fn side_by_side_on(s: &Session) -> Option<&'static str> {
    current_pair(s).is_none().then_some(NOT_SIDE_BY_SIDE)
}

fn side_by_side_state(pair: Option<crate::SideBySide>) -> Value {
    match pair {
        Some(p) => json!({"sideBySide": {"left": p.left, "right": p.right, "syncScroll": p.sync_scroll}}),
        None => json!({"sideBySide": null}),
    }
}

/// View Side by Side: the active document on the left, another one on the right, scrolling
/// together. With three or more documents and no `id`, the app asks which one.
fn side_by_side(s: &mut Session, v: &Value) -> CmdResult {
    let current = current_pair(s);
    if current.is_none() {
        s.side_by_side = None;
    }
    let on = p::bool(v, "value").unwrap_or(current.is_none());
    if !on {
        if s.side_by_side.take().is_some() {
            s.push_ui_request(json!({"sideBySide": null}));
        }
        return Ok(side_by_side_state(None));
    }
    let active = s.document_id();
    let others: Vec<u64> = s.documents().iter().map(|d| d.id).filter(|&id| id != active).collect();
    let other = match v.get("id").filter(|x| !x.is_null()) {
        Some(x) => {
            let id = x.as_u64().ok_or_else(|| CmdError::Params("`id` must be another open document's id".into()))?;
            if !others.contains(&id) {
                return Err(CmdError::Params(format!("`id` {id} is not another open document")));
            }
            id
        }
        None => match others.as_slice() {
            [] => return Err(CmdError::Disabled("view.sideBySide: open another document first".into())),
            [one] => *one,
            _ => {
                s.push_ui_request(json!({"open": "sideBySide"}));
                return Ok(json!({"sideBySide": null, "choose": others}));
            }
        },
    };
    let pair = crate::SideBySide { left: active, right: other, sync_scroll: true };
    s.side_by_side = Some(pair);
    s.push_ui_request(json!({"sideBySide": {"left": active, "right": other}}));
    Ok(side_by_side_state(Some(pair)))
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
