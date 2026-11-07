//! Undo, clipboard, find & replace, Go To, Format Painter.

use serde_json::{Value, json};
use wordcraft_doc::edit::Fragment;
use wordcraft_doc::{Path, Pos, StoryRef};

use super::{delete_selection, find_pattern, pos_json, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// Undo several steps at once: the Undo button's list (#701). `count` is validated here and
/// clamped to the undo depth, so hostile numbers never reach the stack.
fn undo_to(s: &mut Session, v: &Value) -> CmdResult {
    let n = p::u64(v, "count").ok_or_else(|| CmdError::Params("`count` (whole number, 1 or more) is required".into()))?;
    if n == 0 {
        return Err(CmdError::Params("`count` must be 1 or more".into()));
    }
    let n = usize::try_from(n).unwrap_or(usize::MAX).min(s.undo_depth());
    if s.step_lifts_protection(false) {
        return Err(CmdError::Disabled("edit.undoTo: use Stop Protection to lift the document's protection".into()));
    }
    let undone = s.undo_steps(n);
    let mut r = sel_result(s)?;
    if let Some(m) = r.as_object_mut() {
        m.insert("undone".into(), json!(undone));
        m.insert("depth".into(), json!(s.undo_depth()));
    }
    Ok(r)
}

/// Something the Spike can collect: text (it doesn't take empty cells).
fn has_text_sel(s: &Session) -> Option<&'static str> {
    if s.sel.is_collapsed() { Some("nothing is selected") } else { None }
}

/// Something to cut: text, or whole table cells (an empty cell's selection has no length, #1744).
fn has_sel(s: &Session) -> Option<&'static str> {
    if s.sel.is_collapsed() && s.cell_range().is_none() { Some("nothing is selected") } else { None }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.undo", "Undo", "Quick Access Toolbar", |s, _| {
            if s.step_lifts_protection(false) {
                return Err(CmdError::Disabled("edit.undo: use Stop Protection to lift the document's protection".into()));
            }
            s.undo();
            sel_result(s)
        })
        .key("Mod+Z")
        .pure(),
        CommandSpec::new("edit.undoTo", "Undo Actions", "Quick Access Toolbar", undo_to)
            .params(r#"{"count": n (1 = the newest step, like edit.undo; n undoes the newest n steps at once; at most the undo depth)}"#)
            .when(|s| if s.can_undo() { None } else { Some("there is nothing to undo") })
            .pure(),
        CommandSpec::new("edit.undoHistory", "Undo History", "Quick Access Toolbar", |s, _| {
            Ok(json!({"undo": s.undo_history(), "depth": s.undo_depth(), "redo": s.redo_label()}))
        })
        .pure(),
        CommandSpec::new("edit.redo", "Redo", "Quick Access Toolbar", |s, _| {
            if s.step_lifts_protection(true) {
                return Err(CmdError::Disabled("edit.redo: use Stop Protection to lift the document's protection".into()));
            }
            s.redo();
            sel_result(s)
        })
        .key("Mod+Y / Mod+Shift+Z")
        .pure(),
        CommandSpec::new("edit.cut", "Cut", "Home › Clipboard", cut).key("Mod+X").when(has_sel),
        CommandSpec::new("edit.copy", "Copy", "Home › Clipboard", copy).key("Mod+C / Mod+Insert").pure(),
        // Word's Spike (Ctrl+F3, Ctrl+Shift+F3; ⌘ on the Mac): the AutoText entry "Spike", so Quick
        // Parts and AutoText insert it without emptying it, as in Word.
        CommandSpec::new("edit.spike", "Spike", "Editing", spike).key("Mod+F3").when(has_text_sel),
        CommandSpec::new("edit.insertSpike", "Insert Spike", "Editing", insert_spike).key("Mod+Shift+F3").when(|s| {
            if s.math.is_some() {
                Some("leave the equation first")
            } else if s.building_blocks.contains_key(SPIKE) {
                None
            } else {
                Some("the Spike is empty")
            }
        }),
        CommandSpec::new("edit.paste", "Paste", "Home › Clipboard", paste).key("Mod+V / Shift+Insert").params(r#"{"text"?: string, "html"?: string, "rtf"?: string}"#),
        CommandSpec::new("edit.pasteText", "Paste: Keep Text Only", "Home › Clipboard › Paste", paste_text)
            .params(r#"{"text"?: string}"#)
            .key("Mod+Shift+Alt+V"),
        CommandSpec::new("edit.pasteMerge", "Paste: Merge Formatting", "Home › Clipboard › Paste", paste_merge)
            .params(r#"{"text"?: string, "html"?: string, "rtf"?: string}"#),
        CommandSpec::new("edit.clipboardPane", "Clipboard Pane", "Home › Clipboard", |s, v| {
            let on = p::bool(v, "value").unwrap_or(!s.view.clipboard_pane);
            s.view.clipboard_pane = on;
            Ok(json!({"value": on, "count": s.clip_history.len()}))
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("edit.clipboardOptions", "Clipboard Options", "Home › Clipboard › Clipboard Pane › Options", clipboard_options)
            .params(r#"{"showAutomatically"?: bool, "showOnCopyTwice"?: bool, "collectWithoutShowing"?: bool, "showStatus"?: bool}"#)
            .pure(),
        CommandSpec::new("edit.clipboardItems", "Clipboard Items", "Home › Clipboard › Clipboard Pane", |s, _| Ok(s.clip_history.describe())).pure(),
        CommandSpec::new("edit.pasteClipboardItem", "Paste Clipboard Item", "Home › Clipboard › Clipboard Pane", paste_clip_item)
            .params(r#"{"index": n (0 = newest)}"#)
            .when(has_clips),
        CommandSpec::new("edit.pasteAllClipboard", "Paste All", "Home › Clipboard › Clipboard Pane", paste_all_clips).when(has_clips),
        CommandSpec::new("edit.deleteClipboardItem", "Delete Clipboard Item", "Home › Clipboard › Clipboard Pane", |s, v| {
            let i = clip_index(s, v)?;
            s.clip_history.items.remove(i);
            Ok(s.clip_history.describe())
        })
        .params(r#"{"index": n (0 = newest)}"#)
        .when(has_clips)
        .pure(),
        CommandSpec::new("edit.clearClipboard", "Clear All", "Home › Clipboard › Clipboard Pane", |s, _| {
            s.clip_history.items.clear();
            Ok(s.clip_history.describe())
        })
        .pure(),
        CommandSpec::new("edit.find", "Find", "Home › Editing", find)
            .key("Mod+F")
            .params(r#"{"text": string, "matchCase"?: bool, "wholeWord"?: bool, "matchPrefix"?: bool, "matchSuffix"?: bool, "ignorePunct"?: bool, "ignoreSpace"?: bool, "wildcards"?: bool, "special"?: bool (^p ^t ^# … codes, default true), "regex"?: bool, "format"?: {"bold"?, "italic"?, "underline"?: bool, "font"?: string, "size"?: pt, "color"?: "RRGGBB|auto", "highlight"?: name|"none", "style"?: name} | null (No Formatting; text may be empty to find by format alone), "replaceFormat"?: {same} | null}"#)
            .pure(),
        CommandSpec::new("edit.advancedFind", "Advanced Find", "Home › Editing › Find", advanced_find)
            .params(r#"{"text"?: string, …options of edit.find, "in"?: "main|selection", "highlight"?: bool}"#)
            .pure(),
        CommandSpec::new("edit.findNext", "Find Next", "Home › Editing › Find", |s, v| {
            read_opts(s, v)?;
            step(s, 1)
        })
        // Elsewhere Ctrl+Page Down is Next by the browse object (`nav.browseNext`), Find by default.
        .key(super::mac_or("Mod+G / F3 / Shift+F4 / Mod+PageDown", "Mod+G / F3 / Shift+F4 / Mod+Alt+Y"))
        .params(r#"{"text"?: string, "matchCase"?: bool, "wholeWord"?: bool, "regex"?: bool, "format"?: {…} | null (as edit.find)}"#)
        .pure(),
        CommandSpec::new("edit.findPrevious", "Find Previous", "Home › Editing › Find", |s, v| {
            read_opts(s, v)?;
            step(s, -1)
        })
        // Elsewhere Ctrl+Shift+G is Word Count and Ctrl+Page Up is Previous by the browse
        // object (`nav.browsePrevious`), Find by default.
        .key(super::mac_or("Mod+Shift+G / Mod+PageUp", ""))
        .params(r#"{"text"?: string, "matchCase"?: bool, "wholeWord"?: bool, "regex"?: bool, "format"?: {…} | null (as edit.find)}"#)
        .pure(),
        CommandSpec::new("edit.replace", "Replace", "Home › Editing", replace).params(r#"{"text": string, "with": string, …options of edit.replaceAll}"#),
        // Ctrl+H, Home › Replace: the Find and Replace dialog on its Replace tab (#712). Opening
        // it changes nothing, so it is its own command and never marks the document edited.
        CommandSpec::new("edit.findReplace", "Find and Replace", "Home › Editing", |s, v| {
            let tab = match p::str(v, "tab").unwrap_or("replace") {
                t @ ("find" | "replace" | "goto") => t,
                other => return Err(CmdError::Params(format!("`tab` is find, replace or goto, not `{other}`"))),
            };
            s.push_ui_request(json!({"open": tab}));
            Ok(json!({"open": tab}))
        })
        // Word for Mac's own key is ⌃H.
        .key(super::mac_or("Mod+H / Ctrl+H", "Mod+H"))
        .params(r#"{"tab"?: "find|replace|goto" (default replace)}"#)
        .pure(),
        CommandSpec::new("edit.replaceAll", "Replace All", "Home › Editing › Replace", replace_all)
            .params(r#"{"text": string, "with": string (^p ^t ^& ^c …; \\1 with wildcards; with regex also $1 ${n} ${name} $0 $$ \\1 \\n (paragraph mark) \\t), "format"?, "replaceFormat"?: {…} | null (Replace with's formatting; empty `with` keeps the text and only formats it), …options of edit.find}"#),
        CommandSpec::new("edit.goto", "Go To", "Home › Editing › Find", goto)
            .key("Mod+Alt+G / F5")
            .params(r#"{"page"?: n, "pageBy"?: int (pages forward, negative back, from the caret's page), "bookmark"?: string (selects its text), "paragraph"?: n}; none opens Find and Replace on its Go To tab"#)
            .pure(),
        CommandSpec::new("edit.goBack", "Go Back", "Navigation", |s, _| {
            let p = s.go_back().ok_or_else(|| CmdError::Failed("no edits to go back to yet".into()))?;
            s.sel = Selection::caret(p);
            s.goal_x = None;
            sel_result(s)
        })
        .key("Shift+F5 / Mod+Alt+Z")
        .pure(),
        CommandSpec::new("edit.formatPainter", "Format Painter", "Home › Clipboard", painter).params(r#"{"sticky"?: bool}"#).pure(),
        CommandSpec::new("edit.copyFormat", "Copy Formatting", "Home › Clipboard", |s, _| {
            painter_pick(s);
            sel_result(s)
        })
        .key("Mod+Shift+C")
        .pure(),
        CommandSpec::new("edit.pasteFormat", "Paste Formatting", "Home › Clipboard", paste_format).key("Mod+Shift+V"),
    ]
}

fn copy(s: &mut Session, _: &Value) -> CmdResult {
    let whole = super::table::selected_table(s);
    // The same selection copied again with nothing changed in between: Ctrl+C pressed twice.
    let key = (s.sel.clone(), s.document_id(), s.rev());
    let twice = s.last_copy.as_ref() == Some(&key);
    let mut r = copy_with(s, whole.as_ref())?;
    s.last_copy = Some(key);
    if twice && s.prefs.clipboard.show_on_copy_twice {
        s.view.clipboard_pane = true;
        r["pane"] = json!(true);
    }
    Ok(r)
}

/// The Clipboard pane's Options: set the given ones, report all four.
fn clipboard_options(s: &mut Session, v: &Value) -> CmdResult {
    let mut o = s.prefs.clipboard.clone();
    for (key, field) in [
        ("showAutomatically", &mut o.show_automatically),
        ("showOnCopyTwice", &mut o.show_on_copy_twice),
        ("collectWithoutShowing", &mut o.collect_without_showing),
        ("showStatus", &mut o.show_status),
    ] {
        match v.get(key) {
            None => {}
            Some(Value::Bool(b)) => *field = *b,
            Some(_) => return Err(CmdError::Params(format!("{key} must be true or false"))),
        }
    }
    s.prefs.clipboard = o;
    let o = &s.prefs.clipboard;
    Ok(json!({
        "showAutomatically": o.show_automatically,
        "showOnCopyTwice": o.show_on_copy_twice,
        "collectWithoutShowing": o.collect_without_showing,
        "showStatus": o.show_status,
    }))
}

/// Put a copy or cut on the clipboard and, as the Clipboard pane's Options say, collect it in
/// the pane: what Copy and Cut return, with `collected` ("Item N of 24") when the status shows
/// and `pane` when collecting opened the pane.
pub(crate) fn take_copy(s: &mut Session, f: Fragment) -> Value {
    s.clipboard_text = f.clipboard_text();
    let o = s.prefs.clipboard.clone();
    let collected = (s.view.clipboard_pane || o.collect_without_showing) && s.clip_history.push(&f, &s.clipboard_text);
    s.clipboard = Some(f);
    let mut r = copy_result(s);
    if collected {
        let n = s.clip_history.len();
        if o.show_status {
            r["collected"] = json!({"item": n, "of": CLIP_MAX_ITEMS});
        }
        if o.show_automatically && n >= 2 && !s.view.clipboard_pane {
            s.view.clipboard_pane = true;
            r["pane"] = json!(true);
        }
    }
    r
}

/// Copy, `whole` being the table the selection covers whole, if it does.
fn copy_with(s: &mut Session, whole: Option<&Path>) -> CmdResult {
    let (a, b) = s.sel.ordered();
    // Whole table cells selected copy as a table, even empty ones (their selection has no
    // length): the whole table, or a table of just the selected rows or cells (#1744).
    let table = match (whole, s.cell_range()) {
        (Some(tp), _) => s.doc.copy_block(a.story, tp),
        (None, Some(r)) => s.doc.copy_table_cells(r.story, &r.table, r.rows, r.cols),
        (None, None) => None,
    };
    let f = match table {
        Some(f) => f,
        None if a == b => return Ok(json!({"text": ""})),
        None => s.doc.copy_range(&a, &b),
    };
    Ok(take_copy(s, f))
}

/// What Copy and Cut return: the clipboard's text, and the copy as HTML (equations as MathML,
/// #816) and as RTF for the host to put on the system clipboard too, so other programs keep the
/// formatting; and for a copy of one equation alone, its MathML (#1297).
pub(crate) fn copy_result(s: &mut Session) -> Value {
    (s.clipboard_html, s.clipboard_rtf) = match &s.clipboard {
        Some(f) => {
            let d = f.to_document();
            (wordcraft_formats::html::export_fragment(&d), wordcraft_formats::rtf::export(&d))
        }
        None => (String::new(), String::new()),
    };
    let mut r = json!({"text": s.clipboard_text});
    for (k, v) in [("html", &s.clipboard_html), ("rtf", &s.clipboard_rtf)] {
        if !v.is_empty() {
            r[k] = json!(v);
        }
    }
    // A copy of one equation alone, as MathML for the clipboard formats equation editors read
    // ("MathML" on Windows, `public.mathml.presentation` on macOS; #1297). Word looks for them
    // before anything else when it pastes; a copy with text around the equation doesn't offer
    // them, so pasting it keeps the text.
    if let Some((linear, display)) = s.clipboard.as_ref().and_then(|f| f.sole_equation()) {
        r["mathml"] = json!(wordcraft_formats::mathml::linear_to_mathml(linear, display));
    }
    r
}

fn cut(s: &mut Session, _: &Value) -> CmdResult {
    let whole = super::table::selected_table(s);
    let r = copy_with(s, whole.as_ref())?;
    // A whole table selected: Cut removes the table, as Delete Table does (#1744). With Track
    // Changes on, the cut below marks its text deleted (a tracked cut may become a move).
    if !s.doc.settings.track_changes
        && let Some(tp) = &whole
    {
        super::table::delete_table_at(s, tp)?;
        super::controls::restore_placeholder(s)?;
        return Ok(r);
    }
    // Whole rows or some cells: their text goes, the rows and cells stay, as in Word.
    if whole.is_none()
        && let Some(range) = s.cell_range().cloned()
    {
        super::table::clear_range_cells(s, &range)?;
        super::controls::restore_placeholder(s)?;
        return Ok(r);
    }
    // A tracked cut of a paragraph or sentence may become a move when pasted.
    if !super::moves::cut_tracked(s)? {
        delete_selection(s)?;
    }
    super::controls::restore_placeholder(s)?;
    Ok(r)
}

/// The AutoText entry the Spike collects into.
const SPIKE: &str = "Spike";

/// Ctrl+F3: cut the selection to the end of the Spike. The clipboard is left alone.
fn spike(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    // Column selection collapses the selection before the command runs.
    if a == b {
        return Err(CmdError::Disabled("edit.spike: nothing is selected".into()));
    }
    let piece = s.doc.copy_range(&a, &b);
    if piece.blocks.is_empty() {
        return Err(CmdError::Disabled("edit.spike: this selection can't be collected".into()));
    }
    delete_selection(s)?;
    super::controls::restore_placeholder(s)?;
    s.building_blocks.entry(SPIKE.to_string()).or_default().append_piece(piece);
    sel_result(s)
}

/// Ctrl+Shift+F3: insert everything the Spike collected, in order, and empty it.
fn insert_spike(s: &mut Session, _: &Value) -> CmdResult {
    let frag = s.building_blocks.get(SPIKE).cloned().ok_or_else(|| CmdError::Disabled("edit.insertSpike: the Spike is empty".into()))?;
    super::paste::insert(s, frag)?;
    s.building_blocks.remove(SPIKE);
    sel_result(s)
}

/// Paste keeps the source formatting: our own copy while the system clipboard still holds it,
/// else another program's HTML or RTF (#687). The system clipboard's text has none, so it pastes
/// as Keep Text Only does.
fn paste(s: &mut Session, v: &Value) -> CmdResult {
    match super::paste::paste_content(s, v)? {
        super::paste::Pasted::Fragment(frag) => super::paste::insert(s, frag)?,
        super::paste::Pasted::Text(text) => super::paste::insert_text(s, &text)?,
    }
    sel_result(s)
}

/// Merge Formatting: the pasted text takes the destination's formatting, keeping its emphasis.
fn paste_merge(s: &mut Session, v: &Value) -> CmdResult {
    let mut frag = super::paste::paste_fragment(s, v)?;
    super::paste::merge_formatting(s, &mut frag);
    super::paste::insert(s, frag)?;
    sel_result(s)
}

/// Most items the Clipboard pane keeps.
pub const CLIP_MAX_ITEMS: usize = 24;
/// A copy bigger than this (estimated) is not collected (it still reaches the clipboard).
const CLIP_ITEM_BYTES: usize = 4 << 20;
/// All collected items together stay under this; the oldest go first.
const CLIP_TOTAL_BYTES: usize = 24 << 20;
/// Characters of preview per item.
const CLIP_PREVIEW_CHARS: usize = 120;

/// One item collected by Copy or Cut.
#[derive(Clone, Debug)]
pub struct ClipItem {
    pub fragment: Fragment,
    /// Its plain text.
    pub text: String,
    /// Rough memory use.
    bytes: usize,
}

impl ClipItem {
    /// One line of the item's text for a list: whitespace runs become one space, objects drop out.
    pub fn preview(&self) -> String {
        let words = self.text.split(|c: char| c.is_whitespace() || c == wordcraft_doc::para::OBJ).filter(|w| !w.is_empty());
        let mut out = String::new();
        let mut n = 0;
        for w in words {
            if !out.is_empty() {
                out.push(' ');
                n += 1;
            }
            for c in w.chars() {
                if n >= CLIP_PREVIEW_CHARS {
                    out.push('…');
                    return out;
                }
                out.push(c);
                n += 1;
            }
        }
        out
    }
}

/// Items collected by Copy and Cut during the session, newest first: the Clipboard pane
/// (Home › Clipboard). At most [`CLIP_MAX_ITEMS`]; oversized copies are skipped.
#[derive(Clone, Debug, Default)]
pub struct ClipHistory {
    items: Vec<ClipItem>,
}

impl ClipHistory {
    pub fn items(&self) -> &[ClipItem] {
        &self.items
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Collect a copy; whether it was added (a repeat of the newest item or an oversized copy
    /// isn't).
    pub(crate) fn push(&mut self, f: &Fragment, text: &str) -> bool {
        // Text plus per-paragraph overhead; pictures are references into the document.
        let paras = text.matches('\n').count().saturating_add(f.blocks.len()).saturating_add(f.parts.len()).saturating_add(1);
        let bytes = text.len().saturating_mul(2).saturating_add(paras.saturating_mul(256));
        if bytes > CLIP_ITEM_BYTES || f.blocks.is_empty() {
            return false;
        }
        // Copying the same thing again doesn't add a second item.
        if self.items.first().is_some_and(|i| i.text == text && i.fragment == *f) {
            return false;
        }
        self.items.insert(0, ClipItem { fragment: f.clone(), text: text.to_string(), bytes });
        self.items.truncate(CLIP_MAX_ITEMS);
        while self.items.len() > 1 && self.items.iter().map(|i| i.bytes).sum::<usize>() > CLIP_TOTAL_BYTES {
            self.items.pop();
        }
        true
    }

    /// The items for agents and the pane, newest first.
    fn describe(&self) -> Value {
        Value::Array(
            self.items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    json!({
                        "index": i,
                        "preview": it.preview(),
                        "chars": it.text.chars().count(),
                        "objects": it.text.matches(wordcraft_doc::para::OBJ).count(),
                    })
                })
                .collect(),
        )
    }
}

fn has_clips(s: &Session) -> Option<&'static str> {
    if s.clip_history.is_empty() { Some("the Clipboard pane is empty") } else { None }
}

fn clip_index(s: &Session, v: &Value) -> Result<usize, CmdError> {
    let i = p::u64(v, "index").ok_or_else(|| CmdError::Params("`index` (number, 0 = newest) is required".into()))?;
    usize::try_from(i)
        .ok()
        .filter(|i| *i < s.clip_history.len())
        .ok_or_else(|| CmdError::Params(format!("no clipboard item {i} (there are {})", s.clip_history.len())))
}

/// Paste a collected item through the normal paste path. The clipboard itself stays as it was.
fn paste_clip(s: &mut Session, f: Fragment) -> CmdResult {
    let saved = s.clipboard.replace(f);
    let r = paste(s, &json!({}));
    s.clipboard = saved;
    r
}

fn paste_clip_item(s: &mut Session, v: &Value) -> CmdResult {
    let i = clip_index(s, v)?;
    let f = s.clip_history.items.get(i).map(|x| x.fragment.clone()).ok_or_else(|| CmdError::Params("no such clipboard item".into()))?;
    paste_clip(s, f)
}

/// Every collected item, in the order they were copied (one undo step).
fn paste_all_clips(s: &mut Session, _: &Value) -> CmdResult {
    let all: Vec<Fragment> = s.clip_history.items.iter().rev().map(|x| x.fragment.clone()).collect();
    let mut r = sel_result(s)?;
    for f in all {
        r = paste_clip(s, f)?;
    }
    Ok(r)
}

fn paste_text(s: &mut Session, v: &Value) -> CmdResult {
    let text = match p::str(v, "text") {
        Some(t) => t.to_string(),
        None => s.clipboard.as_ref().map(Fragment::plain_text).unwrap_or_default(),
    };
    if text.is_empty() {
        return Err(CmdError::Failed("the clipboard is empty".into()));
    }
    super::paste_text(s, &text.replace("\r\n", "\r").replace('\n', "\r"))?;
    sel_result(s)
}

/// One match: its range and, when asked for, the Replace with text filled in for it.
pub(crate) struct Found {
    pub a: Pos,
    pub b: Pos,
    pub rep: Option<String>,
}

/// A stretch of searched text: `len` bytes from `hay` are paragraph `para`'s bytes from `raw`.
/// A paragraph mark is a one-byte stretch at the paragraph's end (`mark`).
struct Seg {
    hay: usize,
    para: usize,
    raw: usize,
    len: usize,
    mark: bool,
}

/// All matches of the find state in the current story. Text marked deleted by
/// Track Changes is no longer part of the document, so the search reads each
/// paragraph without it: a match lies within one stretch of live text, and word
/// boundaries and anchors see the neighbouring live text, not the deleted text.
/// Otherwise Replace would find its own deleted text again.
pub(crate) fn search(s: &Session, story: StoryRef) -> Result<Vec<(Pos, Pos)>, CmdError> {
    Ok(search_full(s, story, false)?.into_iter().map(|f| (f.a, f.b)).collect())
}

fn find_opts(f: &crate::session::FindState) -> find_pattern::Opts {
    find_pattern::Opts {
        match_case: f.match_case,
        whole_word: f.whole_word,
        prefix: f.match_prefix,
        suffix: f.match_suffix,
        ignore_punct: f.ignore_punct,
        ignore_space: f.ignore_space,
        wildcards: f.wildcards,
        regex: f.regex,
        literal: f.literal,
    }
}

/// [`search`], with the Replace with text of each match when `with_rep` (special codes, `^&`,
/// wildcard groups). A pattern that can match a paragraph mark (`^p`) reads runs of sibling
/// paragraphs as one text with a mark between them; otherwise each paragraph is read alone.
pub(crate) fn search_full(s: &Session, story: StoryRef, with_rep: bool) -> Result<Vec<Found>, CmdError> {
    use regex_automata::{Input, meta, util::syntax};
    let f = &s.find;
    let cond = f.format.as_ref().map(|ff| super::find_format::compile(s, ff)).transpose()?;
    if f.query.is_empty() {
        // No text and a format: each stretch of text with the format.
        let Some(cond) = cond else { return Ok(Vec::new()) };
        let opts = find_opts(f);
        let pieces = find_pattern::resolve_names(find_pattern::parse_replace(&f.replace, &opts), &|_| None);
        let out = cond.stretches(s, story, 100_000).into_iter().map(|(a, b)| {
            let rep = with_rep.then(|| {
                find_pattern::expand(&pieces, s.doc.para_at(&a).and_then(|p| p.text.get(a.off..b.off)).unwrap_or(""), &|_| None, &s.clipboard_text)
            });
            Found { a, b, rep }
        });
        return Ok(out.collect());
    }
    let opts = find_opts(f);
    let c = find_pattern::compile(&f.query, &opts).map_err(CmdError::Params)?;
    let re = meta::Regex::builder()
        .syntax(syntax::Config::new().case_insensitive(c.case_insensitive))
        .configure(meta::Config::new().nfa_size_limit(Some(1 << 20)))
        .build(&c.pattern)
        .map_err(|e| CmdError::Params(format!("bad pattern: {e}")))?;
    let pieces = with_rep.then(|| {
        let index = |name: &str| re.group_info().to_index(regex_automata::PatternID::ZERO, name);
        find_pattern::resolve_names(find_pattern::parse_replace(&f.replace, &opts), &index)
    });
    let paths = s.doc.para_paths(story);
    // Paragraphs read as one text: consecutive siblings when the pattern spans paragraphs.
    let mut chunks: Vec<std::ops::Range<usize>> = Vec::new();
    for i in 0..paths.len() {
        let joins =
            c.spans_paras && i > 0 && paths.get(i - 1).zip(paths.get(i)).is_some_and(|(x, y)| x.parent() == y.parent() && x.last() + 1 == y.last());
        match chunks.last_mut() {
            Some(r) if joins => r.end = i + 1,
            _ => chunks.push(i..i + 1),
        }
    }
    let mut out = Vec::new();
    let mut caps = re.create_captures();
    for chunk in chunks {
        let mut hay = String::new();
        let mut segs: Vec<Seg> = Vec::new();
        // Where the live text breaks (tracked-deleted text, or a paragraph edge when not spanning).
        let mut cuts: Vec<usize> = Vec::new();
        let last = chunk.end.saturating_sub(1);
        for i in chunk {
            let Some(p) = paths.get(i).and_then(|path| s.doc.para(story, path)) else {
                cuts.push(hay.len());
                continue;
            };
            let mut expect = 0;
            for (r, ch) in p.run_ranges() {
                let Some(t) = p.text.get(r.clone()).filter(|_| ch.del.is_none()) else { continue };
                if t.is_empty() {
                    continue;
                }
                if r.start != expect {
                    cuts.push(hay.len());
                }
                match segs.last_mut() {
                    Some(sg) if !sg.mark && sg.para == i && sg.raw + sg.len == r.start => sg.len += t.len(),
                    _ => segs.push(Seg { hay: hay.len(), para: i, raw: r.start, len: t.len(), mark: false }),
                }
                hay.push_str(t);
                expect = r.end;
            }
            if i < last {
                if expect != p.text.len() {
                    cuts.push(hay.len());
                }
                segs.push(Seg { hay: hay.len(), para: i, raw: p.text.len(), len: 1, mark: true });
                hay.push(if p.section.is_some() { find_pattern::SECT_MARK } else { find_pattern::PARA_MARK });
            }
        }
        let pos_of = |x: usize, end: bool| -> Option<Pos> {
            let k = segs.partition_point(|sg| if end { sg.hay + sg.len < x } else { sg.hay + sg.len <= x });
            let sg = segs.get(k)?;
            if (end && sg.hay >= x) || (!end && sg.hay > x) {
                return None;
            }
            if sg.mark && x > sg.hay {
                let path = paths.get(sg.para)?;
                return Some(Pos { story, path: path.with_last(path.last() + 1), off: 0 });
            }
            Some(Pos { story, path: paths.get(sg.para)?.clone(), off: sg.raw + (x - sg.hay) })
        };
        cuts.push(hay.len());
        let mut from = 0;
        for cut in cuts {
            if cut <= from {
                continue;
            }
            let mut input = Input::new(&hay).range(from..cut);
            from = cut;
            loop {
                re.search_captures(&input, &mut caps);
                let Some(m) = caps.get_match() else { break };
                // Step past an empty match so the search moves on.
                let next = if m.is_empty() {
                    hay.get(m.end()..).and_then(|t| t.chars().next()).map_or(m.end() + 1, |c| m.end() + c.len_utf8())
                } else {
                    m.end()
                };
                if next > input.end() {
                    break;
                }
                input.set_start(next);
                if m.is_empty() {
                    continue;
                }
                let (Some(a), Some(b)) = (pos_of(m.start(), false), pos_of(m.end(), true)) else { continue };
                // Text and a format: every character of the match has the format.
                if cond.as_ref().is_some_and(|c| !c.range_matches(s, &a, &b)) {
                    continue;
                }
                let rep = pieces.as_ref().map(|pc| {
                    let group = |n: usize| caps.get_group(n).and_then(|g| hay.get(g.range())).map(str::to_string);
                    find_pattern::expand(pc, hay.get(m.range()).unwrap_or(""), &group, &s.clipboard_text)
                });
                out.push(Found { a, b, rep });
                if out.len() >= 100_000 {
                    return Ok(out);
                }
            }
        }
    }
    Ok(out)
}

fn read_opts(s: &mut Session, v: &Value) -> Result<(), CmdError> {
    // Checked before anything changes, so bad formatting leaves the search as it was.
    let format = super::find_format::read(s, v, "format")?;
    let replace_format = super::find_format::read(s, v, "replaceFormat")?;
    // A new search changes what Reading Highlight marks.
    s.find.highlights = None;
    if let Some(t) = p::str(v, "text") {
        s.find.query = t.to_string();
        // Finding new text makes Find the browse object again, as in Word.
        s.find.browse = super::browse::BrowseObject::Find;
    }
    if let Some(f) = format {
        s.find.format = f;
        s.find.browse = super::browse::BrowseObject::Find;
    }
    if let Some(f) = replace_format {
        s.find.replace_format = f;
    }
    if let Some(b) = p::bool(v, "matchCase") {
        s.find.match_case = b;
    }
    if let Some(b) = p::bool(v, "wholeWord") {
        s.find.whole_word = b;
    }
    if let Some(b) = p::bool(v, "regex") {
        s.find.regex = b;
    }
    if let Some(b) = p::bool(v, "wildcards") {
        s.find.wildcards = b;
    }
    if let Some(b) = p::bool(v, "special") {
        s.find.literal = !b;
    }
    if let Some(b) = p::bool(v, "matchPrefix") {
        s.find.match_prefix = b;
    }
    if let Some(b) = p::bool(v, "matchSuffix") {
        s.find.match_suffix = b;
    }
    if let Some(b) = p::bool(v, "ignorePunct") {
        s.find.ignore_punct = b;
    }
    if let Some(b) = p::bool(v, "ignoreSpace") {
        s.find.ignore_space = b;
    }
    if let Some(t) = p::str(v, "with") {
        s.find.replace = t.to_string();
    }
    Ok(())
}

/// Whether the call searches: it gives Find what's text or formatting.
fn searches(v: &Value) -> bool {
    p::str(v, "text").is_some() || v.get("format").is_some_and(Value::is_object)
}

fn find(s: &mut Session, v: &Value) -> CmdResult {
    read_opts(s, v)?;
    if !searches(v) {
        // Word's Ctrl+F: the Navigation pane with its search box ready to type in (#712); the
        // Find and Replace dialog is Advanced Find.
        s.view.nav_pane = true;
        s.push_ui_request(json!({"focus": "navSearch"}));
        return Ok(json!({"count": s.find.results.len()}));
    }
    let story = s.sel.focus.story;
    s.find.results = search(s, story)?;
    // Select the first match after the caret.
    let caret = s.sel.focus.clone();
    let idx = s.find.results.iter().position(|(a, _)| *a >= caret).unwrap_or(0);
    s.find.current = idx;
    if let Some((a, b)) = s.find.results.get(idx).cloned() {
        s.sel = Selection { anchor: a, focus: b };
    }
    Ok(json!({
        "count": s.find.results.len(),
        "matches": s.find.results.iter().take(200).map(|(a, b)| json!({"start": pos_json(a), "end": pos_json(b)})).collect::<Vec<_>>(),
    }))
}

/// Find in the classic dialog: every match in the body or the selection, optionally highlighted while you read.
fn advanced_find(s: &mut Session, v: &Value) -> CmdResult {
    if !searches(v) && v.get("highlight").is_none() && v.get("format").is_none() {
        s.push_ui_request(json!({"open": "find"}));
        return Ok(json!({"count": s.find.results.len()}));
    }
    read_opts(s, v)?;
    let within = match p::str(v, "in").unwrap_or("main") {
        "main" => None,
        "selection" if !s.sel.is_collapsed() => Some(s.sel.ordered()),
        "selection" => return Err(CmdError::Params("`in: selection` needs a selection".into())),
        other => return Err(CmdError::Params(format!("`in` is main or selection, not `{other}`"))),
    };
    let story = within.as_ref().map(|(a, _)| a.story).unwrap_or(StoryRef::Body);
    let mut found = search(s, story)?;
    if let Some((a, b)) = &within {
        found.retain(|(x, y)| x >= a && y <= b);
    }
    if let Some(h) = p::bool(v, "highlight") {
        s.find.highlight = h;
        s.find.highlights = None;
    }
    let n = found.len();
    let matches: Vec<Value> = found.iter().take(200).map(|(a, b)| json!({"start": pos_json(a), "end": pos_json(b)})).collect();
    s.find.current = 0;
    s.find.results = found;
    Ok(json!({"count": n, "matches": matches, "highlight": s.find.highlight}))
}

/// Find Next (`dir` 1) or Find Previous (-1) with the current options: Next / Previous when
/// browsing by Find.
pub(super) fn find_step(s: &mut Session, dir: i64) -> CmdResult {
    read_opts(s, &Value::Null)?;
    step(s, dir)
}

/// Go To: opens the dialog without params.
pub(super) fn goto_command(s: &mut Session, v: &Value) -> CmdResult {
    goto(s, v)
}

fn step(s: &mut Session, dir: i64) -> CmdResult {
    let story = s.sel.focus.story;
    s.find.results = search(s, story)?;
    let n = s.find.results.len();
    if n == 0 {
        return Err(CmdError::Failed("no matches".into()));
    }
    let caret = if dir > 0 { s.sel.ordered().1 } else { s.sel.ordered().0 };
    let idx = if dir > 0 {
        s.find.results.iter().position(|(a, _)| *a >= caret).unwrap_or(0)
    } else {
        s.find.results.iter().rposition(|(_, b)| *b <= caret).unwrap_or(n - 1)
    };
    s.find.current = idx;
    if let Some((a, b)) = s.find.results.get(idx).cloned() {
        s.sel = Selection { anchor: a, focus: b };
    }
    Ok(json!({"index": idx, "count": n}))
}

/// Replace needs something to look for: the `text` given, or the last search's (Find then Replace
/// with just a `with`). Neither is a call that would silently replace nothing (a misspelt `text`).
fn need_query(s: &Session, v: &Value) -> Result<(), CmdError> {
    if !searches(v) && s.find.query.is_empty() && s.find.format.is_none() {
        return Err(CmdError::Params("`text` (string) is required".into()));
    }
    Ok(())
}

fn replace(s: &mut Session, v: &Value) -> CmdResult {
    need_query(s, v)?;
    read_opts(s, v)?;
    let story = s.sel.focus.story;
    let results = search_full(s, story, true)?;
    let (a, b) = s.sel.ordered();
    // Replace the current match if selected, then move to the next.
    if let Some(f) = results.into_iter().find(|f| f.a == a && f.b == b) {
        let rf = s.find.replace_format.clone();
        replace_match(s, f.a, f.b, &f.rep.unwrap_or_default(), rf.as_ref(), false)?;
    }
    let _ = step(s, 1);
    Ok(json!({"remaining": search(s, story)?.len()}))
}

/// Replace one match `a..b` with `with`, then give the new text Replace with's formatting `rf`.
/// With a format and no replacement text the match keeps its text and only takes the format, as
/// in Word. `direct` edits the document without the typing path (Replace All without Track
/// Changes); otherwise the replacement is pasted, so it is a tracked revision when tracking.
fn replace_match(s: &mut Session, a: Pos, b: Pos, with: &str, rf: Option<&crate::session::FindFormat>, direct: bool) -> Result<(), CmdError> {
    if let Some(rf) = rf
        && with.is_empty()
    {
        super::find_format::apply(s, &a, &b, rf)?;
        s.sel = Selection { anchor: a, focus: b };
        return Ok(());
    }
    if !direct {
        s.sel = Selection { anchor: a.clone(), focus: b };
        super::paste_text(s, with)?;
        if let Some(rf) = rf {
            let end = s.sel.focus.clone();
            // The pasted text ends at the caret; tracked, the deleted match stays before it.
            let start = if with.contains(['\r', '\u{2029}']) || end.path != a.path || end.story != a.story {
                a
            } else {
                Pos { off: end.off.saturating_sub(with.len()).max(a.off), ..end.clone() }
            };
            super::find_format::apply(s, &start, &end, rf)?;
        }
        return Ok(());
    }
    // Replacing a symbol gives the new text the symbol's formatting, not its own font.
    let props = s.doc.para_at(&a).map(|p| p.props_of_char(a.off).clone().without_symbol()).unwrap_or_default();
    let mut at = s.doc.delete_range(&a, &b)?;
    for (k, part) in with.split(['\r', '\u{2029}']).enumerate() {
        if k > 0 {
            at = s.doc.split_paragraph(&at)?;
        }
        if !part.is_empty() {
            at = s.doc.insert_text(&at, part, &props)?;
        }
    }
    if let Some(rf) = rf {
        super::find_format::apply(s, &a, &at, rf)?;
    }
    Ok(())
}

/// Every match replaced, last first so earlier positions stay valid (one undo step). A `\r` in
/// the replacement (`^p`) splits the paragraph. Replace with's formatting goes on each
/// replacement (or each match, when Replace with is empty).
fn replace_all(s: &mut Session, v: &Value) -> CmdResult {
    need_query(s, v)?;
    read_opts(s, v)?;
    let story = s.sel.focus.story;
    let results = search_full(s, story, true)?;
    let n = results.len();
    let rf = s.find.replace_format.clone();
    // Only formatting changes when Replace with is empty and has a format: nothing is deleted.
    let deletes = rf.is_none() || results.iter().any(|f| f.rep.as_deref().is_some_and(|r| !r.is_empty()));
    if deletes {
        super::controls::check_deletions(s, story, results.iter().map(|f| (&f.a, &f.b)))?;
    }
    let direct = !s.doc.settings.track_changes;
    for Found { a, b, rep } in results.into_iter().rev() {
        // Tracked, it takes the same path as a single replace, so the change is a reviewable revision.
        replace_match(s, a, b, &rep.unwrap_or_default(), rf.as_ref(), direct)?;
    }
    s.status = format!("All done. We made {n} replacements.");
    Ok(json!({"replaced": n}))
}

pub(super) fn goto(s: &mut Session, v: &Value) -> CmdResult {
    // Relative pages, Go To's "+2" / "-1": from the page the caret is on.
    let by = v.get("pageBy").and_then(Value::as_i64);
    let rel = by.map(|by| {
        let here = s.layout().caret_on(&s.sel.focus, s.page_hint).map_or(0, |c| c.page) as i64;
        (here + 1).saturating_add(by).max(1) as u64
    });
    if let Some(pg) = rel.or_else(|| p::u64(v, "page")) {
        let l = s.layout();
        let idx = crate::p::page_index(pg).min(l.pages.len().saturating_sub(1));
        let target = l.pages.get(idx).and_then(|page| {
            page.items.iter().find_map(|it| match it {
                wordcraft_layout::Placed::Lines { story, path, para, l0, .. } if *story == StoryRef::Body => {
                    para.lines.get(*l0).map(|ln| Pos { story: *story, path: path.clone(), off: ln.start })
                }
                _ => None,
            })
        });
        if let Some(t) = target {
            s.sel = Selection::caret(t);
            s.page_hint = idx;
        }
        s.push_ui_request(json!({"scrollTo": "caret"}));
        return sel_result(s);
    }
    if let Some(name) = p::str(v, "bookmark") {
        // Word's built-in bookmark: the top of the document.
        if name == "_top" {
            s.sel = Selection::caret(Pos::body(0, 0));
            return sel_result(s);
        }
        // As in Word, the bookmark's text is selected (a caret for an empty bookmark).
        let (from, to) = super::insert::bookmark_range(&s.doc, name).ok_or_else(|| CmdError::Failed(format!("no bookmark `{name}`")))?;
        s.sel = Selection { anchor: from, focus: to };
        return sel_result(s);
    }
    if let Some(n) = p::u64(v, "paragraph") {
        let paths = s.doc.para_paths(StoryRef::Body);
        if let Some(path) = paths.get((n.max(1) - 1) as usize) {
            s.sel = Selection::caret(Pos { story: StoryRef::Body, path: path.clone(), off: 0 });
        }
        return sel_result(s);
    }
    s.push_ui_request(json!({"open": "goto"}));
    sel_result(s)
}

fn painter_pick(s: &mut Session) {
    let f = s.sel.ordered().0;
    // A symbol's own font isn't formatting to paint onto text.
    let chr = s.doc.para_at(&f).map(|p| p.props_of_char(f.off).clone().without_symbol()).unwrap_or_default();
    let para = s.doc.para_at(&f).map(|p| p.props.clone()).unwrap_or_default();
    s.painter = Some((chr, para, false));
}

fn painter(s: &mut Session, v: &Value) -> CmdResult {
    if s.painter.is_some() {
        s.painter = None;
        return Ok(json!({"active": false}));
    }
    painter_pick(s);
    if let Some((_, _, sticky)) = s.painter.as_mut() {
        *sticky = p::bool(v, "sticky").unwrap_or(false);
    }
    Ok(json!({"active": true}))
}

/// Apply the format painter's formatting to the selection.
fn paste_format(s: &mut Session, _: &Value) -> CmdResult {
    let Some((chr, para, sticky)) = s.painter.clone() else { return Err(CmdError::Failed("no formatting copied".into())) };
    let (a, b) = s.sel.ordered();
    if a == b {
        // No selection: apply paragraph formatting (and word formatting like Word).
        let f = s.sel.focus.clone();
        if let Some(pp) = s.doc.para_at(&f) {
            let (x, y) = pp.word_at(f.off);
            s.doc.format_range(&Pos { off: x, ..f.clone() }, &Pos { off: y, ..f.clone() }, &|c| *c = chr.cleared().overlaid(&chr))?;
        }
        s.doc.format_paragraphs(&a, &b, &|p| *p = para.clone())?;
    } else {
        s.doc.format_range(&a, &b, &|c| {
            let link = c.link.clone();
            *c = c.cleared().overlaid(&chr);
            c.link = link;
        })?;
        if a.path != b.path || a.off == 0 {
            s.doc.format_paragraphs(&a, &b, &|p| *p = para.clone())?;
        }
    }
    if !sticky {
        s.painter = None;
    }
    sel_result(s)
}
