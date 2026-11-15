//! Customize Status Bar: the items the status bar can show, which are shown, and each one's value.

use egui::Ui;
use serde_json::{Value, json};
use wordcraft_layout::{DocLayout, Placed};

use crate::{StatusCount, StatusCounts, WordApp};

/// An item of the status bar's Customize Status Bar list, in Word's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatusItem {
    FormattedPageNumber,
    Section,
    PageNumber,
    VerticalPagePosition,
    LineNumber,
    Column,
    WordCount,
    CharacterCount,
    Language,
    TrackChanges,
    CapsLock,
    Overtype,
    SelectionMode,
    ViewShortcuts,
    ZoomSlider,
    Zoom,
}

impl StatusItem {
    pub const ALL: [StatusItem; 16] = [
        StatusItem::FormattedPageNumber,
        StatusItem::Section,
        StatusItem::PageNumber,
        StatusItem::VerticalPagePosition,
        StatusItem::LineNumber,
        StatusItem::Column,
        StatusItem::WordCount,
        StatusItem::CharacterCount,
        StatusItem::Language,
        StatusItem::TrackChanges,
        StatusItem::CapsLock,
        StatusItem::Overtype,
        StatusItem::SelectionMode,
        StatusItem::ViewShortcuts,
        StatusItem::ZoomSlider,
        StatusItem::Zoom,
    ];

    /// The item's name in the list.
    pub fn label(self) -> &'static str {
        match self {
            StatusItem::FormattedPageNumber => "Formatted Page Number",
            StatusItem::Section => "Section",
            StatusItem::PageNumber => "Page Number",
            StatusItem::VerticalPagePosition => "Vertical Page Position",
            StatusItem::LineNumber => "Line Number",
            StatusItem::Column => "Column",
            StatusItem::WordCount => "Word Count",
            StatusItem::CharacterCount => "Character Count (with spaces)",
            StatusItem::Language => "Language",
            StatusItem::TrackChanges => "Track Changes",
            StatusItem::CapsLock => "Caps Lock",
            StatusItem::Overtype => "Overtype",
            StatusItem::SelectionMode => "Selection Mode",
            StatusItem::ViewShortcuts => "View Shortcuts",
            StatusItem::ZoomSlider => "Zoom Slider",
            StatusItem::Zoom => "Zoom",
        }
    }

    /// Shown before the person changes anything: what the status bar showed before it could be
    /// customized.
    pub fn default_on(self) -> bool {
        matches!(
            self,
            StatusItem::PageNumber
                | StatusItem::WordCount
                | StatusItem::Language
                | StatusItem::TrackChanges
                | StatusItem::Overtype
                | StatusItem::ViewShortcuts
                | StatusItem::ZoomSlider
                | StatusItem::Zoom
        )
    }

    /// The name used in `ui.json` and by the `ui.statusItem` command.
    pub fn code(self) -> &'static str {
        match self {
            StatusItem::FormattedPageNumber => "formattedPageNumber",
            StatusItem::Section => "section",
            StatusItem::PageNumber => "pageNumber",
            StatusItem::VerticalPagePosition => "verticalPagePosition",
            StatusItem::LineNumber => "lineNumber",
            StatusItem::Column => "column",
            StatusItem::WordCount => "wordCount",
            StatusItem::CharacterCount => "characterCount",
            StatusItem::Language => "language",
            StatusItem::TrackChanges => "trackChanges",
            StatusItem::CapsLock => "capsLock",
            StatusItem::Overtype => "overtype",
            StatusItem::SelectionMode => "selectionMode",
            StatusItem::ViewShortcuts => "viewShortcuts",
            StatusItem::ZoomSlider => "zoomSlider",
            StatusItem::Zoom => "zoom",
        }
    }

    /// The item named `code` (ignoring case and surrounding space).
    pub fn from_code(code: &str) -> Option<StatusItem> {
        StatusItem::ALL.into_iter().find(|i| i.code().eq_ignore_ascii_case(code.trim()))
    }

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// Which status bar items are shown, kept between runs in `ui.json` as an object of item codes
/// and booleans. A missing item, or one whose value isn't true or false, reads as its default;
/// unknown names are ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "Value", into = "Value")]
pub struct StatusItems {
    shown: u32,
}

impl Default for StatusItems {
    fn default() -> Self {
        StatusItems { shown: StatusItem::ALL.into_iter().filter(|i| i.default_on()).fold(0, |m, i| m | i.bit()) }
    }
}

impl StatusItems {
    pub fn shows(self, item: StatusItem) -> bool {
        self.shown & item.bit() != 0
    }

    pub fn set(&mut self, item: StatusItem, on: bool) {
        if on {
            self.shown |= item.bit();
        } else {
            self.shown &= !item.bit();
        }
    }
}

impl From<Value> for StatusItems {
    fn from(v: Value) -> Self {
        let mut items = StatusItems::default();
        if let Some(map) = v.as_object() {
            for item in StatusItem::ALL {
                if let Some(on) = map.get(item.code()).and_then(Value::as_bool) {
                    items.set(item, on);
                }
            }
        }
        items
    }
}

impl From<StatusItems> for Value {
    fn from(items: StatusItems) -> Self {
        Value::Object(StatusItem::ALL.into_iter().map(|i| (i.code().to_string(), Value::Bool(items.shows(i)))).collect())
    }
}

/// Where the caret is on its page: the page (layout index), the line counted from the top of
/// the page's area for the caret's story (1-based), the character in that line (1-based) and the
/// line's top, in points from the page's top edge.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CaretSpot {
    page: usize,
    line: usize,
    column: usize,
    top: f32,
}

/// Lines closer than this (points) are one line of the page (side-by-side table cells).
const SAME_LINE: f32 = 0.5;

/// The caret's [`CaretSpot`] in layout `l`; `None` when the caret's line isn't laid out.
fn caret_spot_in(app: &WordApp, l: &DocLayout) -> Option<CaretSpot> {
    let focus = &app.session.sel.focus;
    let c = l.caret_on(focus, app.session.page_hint)?;
    let page = l.pages.get(c.page)?;
    let horizontal = wordcraft_doc::props::TextDirection::Horizontal;
    // Lines of the caret's story on the page, unturned: (paragraph lines, first, end, top of first).
    let lines = || {
        page.items.iter().chain(page.header.iter()).chain(page.footer.iter()).filter_map(move |it| match it {
            Placed::Lines { story, path, para, l0, l1, y, turn, .. } if *story == focus.story && *turn == horizontal => {
                Some((path, para, *l0, *l1, *y))
            }
            _ => None,
        })
    };
    let top_of = |para: &wordcraft_layout::ParaLayout, l0: usize, li: usize, y: f32| -> Option<f32> {
        Some(y + para.lines.get(li)?.top - para.lines.get(l0)?.top)
    };
    let text = app.session.doc.para_at(focus).map(|p| p.text.as_str()).unwrap_or("");
    let (top, column) = lines().find_map(|(path, para, l0, l1, y)| {
        if path != &focus.path {
            return None;
        }
        let li = para.line_of(focus.off);
        if li < l0 || li >= l1 {
            return None;
        }
        let start = para.lines.get(li)?.start.min(focus.off);
        let column = text.get(start..focus.off).map_or(0, |s| s.chars().count()) + 1;
        Some((top_of(para, l0, li, y)?, column))
    })?;
    let mut above: Vec<f32> =
        lines().flat_map(|(_, para, l0, l1, y)| (l0..l1).filter_map(move |li| top_of(para, l0, li, y))).filter(|t| *t < top - SAME_LINE).collect();
    above.sort_by(f32::total_cmp);
    above.dedup_by(|a, b| (*a - *b).abs() < SAME_LINE);
    Some(CaretSpot { page: c.page, line: above.len() + 1, column, top })
}

/// The page the status bar's page number names: the page in view while the caret is off screen
/// (#3880), the remembered page while the whole story is selected, else the caret's.
pub(crate) fn bar_page(app: &WordApp, l: &DocLayout) -> usize {
    let last = l.pages.len().saturating_sub(1);
    if let Some(p) = app.canvas.viewed_page {
        p.min(last) + 1
    } else if app.session.selects_whole_story() {
        app.session.page_hint.min(last) + 1
    } else {
        l.caret_on(&app.session.sel.focus, app.session.page_hint).map(|c| c.page + 1).unwrap_or(1)
    }
}

/// The caret page's number in its section's page number format (`iii`, `A-2`).
fn formatted_page_number(app: &WordApp, l: &DocLayout) -> Option<String> {
    let c = l.caret_on(&app.session.sel.focus, app.session.page_hint)?;
    let page = l.pages.get(c.page)?;
    let fmt = app.session.doc.sections().get(page.section).map(|(_, s)| s.page_num_format).unwrap_or_default();
    Some(wordcraft_layout::fields::page_label("", fmt, page.number, &page.chapter))
}

/// The document's counts: the status bar's cached ones while they're current.
fn counts(app: &WordApp) -> StatusCounts {
    let key = app.session.rev().wrapping_mul(2) | u64::from(app.session.prefs.count_notes);
    if app.status_counts.0 == key {
        return app.status_counts.1;
    }
    let text = app.session.counted_text();
    StatusCounts {
        words: app.session.word_count(),
        chars: wordcraft_doc::count_chars(&text),
        chars_no_spaces: wordcraft_doc::count_chars_no_spaces(&text),
    }
}

/// The count in `unit` as the status bar words it: the document's, or "selected of total".
pub(crate) fn count_text(app: &WordApp, unit: StatusCount, counts: StatusCounts) -> String {
    // A selected picture/shape/text box isn't a text selection: show the total.
    if app.session.sel.is_collapsed() || crate::objects::selected(app).is_some() {
        super::status_count_text(unit, counts.in_unit(unit), None)
    } else {
        let selected = crate::count_in_unit(&app.session.selected_text(), unit);
        super::status_count_text(unit, counts.in_unit(unit), Some(selected))
    }
}

/// The zoom percentage the status bar shows (the reading text size in Read Mode).
pub(crate) fn zoom_percent(app: &WordApp) -> f32 {
    let reading = app.session.view.read_mode;
    (if reading { app.session.view.read_zoom } else { app.canvas.scale / crate::canvas::PX_PER_PT } * 100.0).round()
}

fn on_off(on: bool) -> String {
    if on { tl!("On") } else { tl!("Off") }.to_string()
}

/// The value the list shows beside `item` ("" when it has none). The caret's line, column and
/// position read the layout already made: empty before there is one.
pub fn status_item_value(app: &WordApp, item: StatusItem) -> String {
    let layout = app.session.laid_out();
    let spot = || layout.as_deref().and_then(|l| caret_spot_in(app, l));
    match item {
        StatusItem::FormattedPageNumber => layout.as_deref().and_then(|l| formatted_page_number(app, l)).unwrap_or_default(),
        StatusItem::Section => (wordcraft_engine::cmd::page::section_index_of_focus(&app.session) + 1).to_string(),
        StatusItem::PageNumber => layout.as_deref().map_or_else(String::new, |l| {
            crate::i18n::fmt(tl!("Page {page} of {pages}"), &[("page", &bar_page(app, l).to_string()), ("pages", &l.pages.len().to_string())])
        }),
        StatusItem::VerticalPagePosition => spot().map_or_else(String::new, |s| crate::options::length(s.top)),
        StatusItem::LineNumber => spot().map_or_else(String::new, |s| s.line.to_string()),
        StatusItem::Column => spot().map_or_else(String::new, |s| s.column.to_string()),
        StatusItem::WordCount => count_text(app, app.ui.status_count, counts(app)),
        StatusItem::CharacterCount => count_text(app, StatusCount::Characters, counts(app)),
        StatusItem::Language => crate::ribbon::caret_language(app),
        StatusItem::TrackChanges => on_off(app.session.doc.settings.track_changes),
        // egui doesn't report the Caps Lock key's state, so there's nothing to show.
        StatusItem::CapsLock => String::new(),
        StatusItem::Overtype => if app.session.prefs.overtype { tl!("Overtype") } else { tl!("Insert") }.to_string(),
        StatusItem::SelectionMode => {
            if app.session.extend_mode_on() {
                tl!("Extend Selection").to_string()
            } else {
                String::new()
            }
        }
        StatusItem::ViewShortcuts | StatusItem::ZoomSlider => String::new(),
        StatusItem::Zoom => format!("{}%", zoom_percent(app)),
    }
}

/// What the status bar shows for the caret-position items while they are on: `None` when the
/// item is off, has nothing to show, or isn't one of them.
pub(crate) fn bar_text(app: &WordApp, l: &DocLayout, item: StatusItem) -> Option<String> {
    if !status_item_shown(app, item) {
        return None;
    }
    let spot = || caret_spot_in(app, l);
    let one = |key: &str, text: &str, value: String| crate::i18n::fmt(tl!(text), &[(key, &value)]);
    match item {
        StatusItem::FormattedPageNumber => formatted_page_number(app, l).map(|p| one("page", "Page: {page}", p)),
        StatusItem::Section => Some(one("section", "Section: {section}", status_item_value(app, item))),
        StatusItem::VerticalPagePosition => spot().map(|s| one("position", "At {position}", crate::options::length(s.top))),
        StatusItem::LineNumber => spot().map(|s| one("line", "Line: {line}", s.line.to_string())),
        StatusItem::Column => spot().map(|s| one("column", "Column: {column}", s.column.to_string())),
        StatusItem::SelectionMode => app.session.extend_mode_on().then(|| tl!("Extend Selection").to_string()),
        _ => None,
    }
}

/// Is `item` shown on the status bar?
pub fn status_item_shown(app: &WordApp, item: StatusItem) -> bool {
    app.ui.status_items.shows(item)
}

/// Show `item` if hidden, hide it if shown.
pub fn toggle_status_item(app: &mut WordApp, item: StatusItem) {
    let on = !status_item_shown(app, item);
    app.ui.status_items.set(item, on);
}

/// `ui.statusItem`: `{"item": code, "value"?: bool}` shows (true) or hides (false) the item, or
/// toggles it without a value; with no item it reads every item.
pub(crate) fn command(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    let all = |app: &WordApp| Value::from(app.ui.status_items);
    let Some(code) = p.get("item") else { return Ok(json!({"statusItems": all(app)})) };
    let Some(item) = code.as_str().and_then(StatusItem::from_code) else {
        let known: Vec<&str> = StatusItem::ALL.iter().map(|i| i.code()).collect();
        return Err(format!("unknown status bar item {code}; use one of {}", known.join(", ")));
    };
    match p.get("value") {
        None => toggle_status_item(app, item),
        Some(Value::Bool(on)) => app.ui.status_items.set(item, *on),
        Some(_) => return Err("`value` must be true or false".into()),
    }
    Ok(json!({"item": item.code(), "shown": status_item_shown(app, item), "statusItems": all(app)}))
}

/// Right-click on the status bar (Customize Status Bar): every item, checked when shown, with its
/// value at the right. A click shows or hides the item and the list stays open.
pub(crate) fn customize_menu(app: &mut WordApp, resp: &egui::Response) {
    let popup = egui::Popup::context_menu(resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let popup = if crate::rtl::on() { popup.layout(crate::rtl::menu_layout()) } else { popup };
    popup.show(|ui| list(app, ui));
}

fn list(app: &mut WordApp, ui: &mut Ui) {
    ui.label(egui::RichText::new(tl!("Customize Status Bar")).strong());
    ui.separator();
    for item in StatusItem::ALL {
        let shown = status_item_shown(app, item);
        let value = status_item_value(app, item);
        let label = tl!(item.label());
        let resp = ui.add(crate::rtl::menu_item(ui, label, &value, 300.0).selected(shown));
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, shown, label));
        if resp.clicked() {
            let _ = app.run("ui.statusItem", json!({"item": item.code()}));
        }
    }
}

#[cfg(test)]
mod tests;
