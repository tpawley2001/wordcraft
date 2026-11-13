//! Paste Special: paste the clipboard in a chosen format (formatted, RTF, HTML, picture,
//! unformatted), and the formatted content Paste and Merge Formatting take from other programs.
//!
//! The clipboard can hold several representations of the same content. WordCraft's own copy keeps
//! a rich fragment; the system clipboard gives text, and callers (agents, the CLI, a front end
//! with a richer clipboard) may pass `html`, `rtf` or a `picture` too. Text that is itself RTF or
//! HTML source can also be pasted as that format. Paste keeps the source formatting of another
//! program's HTML or RTF; Merge Formatting keeps only its emphasis (#687).
//! Plain text has no formatting to keep, so Paste inserts it as Keep Text Only does.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use wordcraft_doc::edit::Fragment;
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::{Block, Document, Paragraph, Pos, StoryRef};

use super::{delete_selection, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// Largest clipboard payload Paste Special accepts, in bytes.
pub const MAX_PASTE_BYTES: usize = 16 << 20;
/// Largest clipboard picture Paste Special accepts (base64), in bytes.
pub const MAX_PICTURE_BYTES: usize = 96 << 20;
/// Nesting depth we follow when adjusting pasted tables.
const MAX_DEPTH: usize = 64;

/// The formats, in the order the dialog lists them: (id, label).
pub const FORMATS: [(&str, &str); 6] = [
    ("formatted", "Formatted Text"),
    ("rtf", "Formatted Text (RTF)"),
    ("html", "HTML Format"),
    ("picture", "Picture"),
    ("text", "Unformatted Text"),
    ("unicode", "Unformatted Unicode Text"),
];

/// What OK does with a format, as the Paste Special dialog's Result box says it.
pub fn result_text(id: &str) -> &'static str {
    match id {
        "formatted" | "rtf" => "Inserts the contents of the Clipboard as text with font and table formatting.",
        "html" => "Inserts the contents of the Clipboard as HTML Format.",
        "picture" => "Inserts the contents of the Clipboard as a picture.",
        "text" | "unicode" => "Inserts the contents of the Clipboard as text without any formatting.",
        _ => "",
    }
}

/// Where the clipboard's copy was taken from, for Paste Special > Paste link: set by Copy of a
/// plain text range ([`crate::Session::clipboard_link`]), cleared by anything else that fills the
/// clipboard.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardLink {
    /// `Session::document_id()` of the document the copy was taken from.
    pub doc: u64,
    /// The copied range in that document (selection start and end, in document order).
    pub start: Pos,
    pub end: Pos,
    /// The `OLE_LINK<n>` bookmark a Paste link of this copy added around the range, once there is one.
    pub bookmark: Option<String>,
}

/// What the dialog's Result box adds for Paste link.
const LINK_NOTE: &str = " Paste Link creates a shortcut to the source file. Changes to the source file will be reflected in your document.";

/// Most `OLE_LINK<n>` names tried before giving up.
const MAX_LINK_BOOKMARKS: u32 = 100_000;

/// The LINK field instruction (ECMA-376 Part 1, 17.16.5.31) Paste Special > Paste link inserts
/// for a copy from `path` whose source range is the bookmark `bookmark`, pasted as format `fmt`;
/// `None` for a format that can't be linked.
pub fn link_instr(path: &std::path::Path, bookmark: &str, fmt: &str) -> Option<String> {
    let switch = match fmt {
        "formatted" | "rtf" => r"\r",
        "html" => r"\h",
        "text" => r"\t",
        "unicode" => r"\u",
        _ => return None,
    };
    let file = path.to_string_lossy();
    let old_format = path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["doc", "dot", "rtf"].iter().any(|x| e.eq_ignore_ascii_case(x)));
    let class = if old_format { "Word.Document.8" } else { "Word.Document.12" };
    let quote = |t: &str| format!("\"{}\"", t.replace('\\', r"\\").replace('"', r#"\""#));
    Some(format!(r"LINK {class} {} {} \a {switch}", quote(&file), quote(bookmark)))
}

/// What OK does with a format when Paste link is chosen, as the dialog's Result box says it.
pub fn link_result_text(id: &str) -> String {
    let base = result_text(id);
    if base.is_empty() || id == "picture" { String::new() } else { format!("{base}{LINK_NOTE}") }
}

/// The text of `a..b` in `doc`, or `None` when either end no longer exists there.
fn range_text(doc: &Document, a: &Pos, b: &Pos) -> Option<String> {
    let ok = |p: &Pos| doc.para_at(p).is_some_and(|para| para.text.is_char_boundary(p.off));
    if !ok(a) || !ok(b) || a.story != b.story || a > b {
        return None;
    }
    Some(doc.copy_range(a, b).plain_text())
}

/// Where bookmark `name`'s start mark is in `story`.
fn find_bookmark(doc: &Document, story: StoryRef, name: &str) -> Option<Pos> {
    doc.para_paths(story).into_iter().find_map(|path| {
        let para = doc.para(story, &path)?;
        let off =
            para.objects_with_offsets().find_map(|(off, o)| matches!(o, InlineObject::BookmarkStart { name: n } if n == name).then_some(off))?;
        Some(Pos { story, path, off })
    })
}

/// Every bookmark name in `doc`, in any story.
fn bookmark_names(doc: &Document) -> Vec<String> {
    let stories = std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|k| StoryRef::Part(*k)));
    let mut out = Vec::new();
    for story in stories {
        for path in doc.para_paths(story) {
            if let Some(p) = doc.para(story, &path) {
                out.extend(p.objects_with_offsets().filter_map(|(_, o)| match o {
                    InlineObject::BookmarkStart { name } => Some(name.clone()),
                    _ => None,
                }));
            }
        }
    }
    out
}

/// The text the copy's source range reads now: between its bookmark's marks once Paste link
/// added one, else `start..end`.
fn source_text(doc: &Document, link: &ClipboardLink) -> Option<String> {
    match &link.bookmark {
        Some(name) => {
            let start = find_bookmark(doc, link.start.story, name)?;
            let (a, b) = super::insert::bookmark_span(doc, name, &start);
            range_text(doc, &a, &b)
        }
        None => range_text(doc, &link.start, &link.end),
    }
}

/// The file the clipboard's copy came from, when Paste link can point back at it: WordCraft's
/// own copy (still on the system clipboard, when the params say what is there) of a range in a
/// document that is still open, saved and unchanged there.
pub fn link_source(s: &Session, v: &Value) -> Option<PathBuf> {
    let frag = s.clipboard.as_ref()?;
    let link = s.clipboard_link.as_ref()?;
    if let Some(t) = p::str(v, "text")
        && frag.clipboard_text() != t.replace("\r\n", "\n")
    {
        return None;
    }
    let (doc, path) = s.open_document(link.doc)?;
    let path = path?;
    (source_text(doc, link)? == frag.plain_text()).then(|| path.to_path_buf())
}

/// The formats Paste link offers: those of [`available`] but Picture, when [`link_source`] has a
/// source; else none.
pub fn link_formats(s: &Session, v: &Value) -> Vec<&'static str> {
    if link_source(s, v).is_none() {
        return Vec::new();
    }
    available(s, v).into_iter().filter(|id| *id != "picture").collect()
}

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("edit.pasteSpecial", "Paste Special", "Home › Clipboard › Paste", paste_special).key("Mod+Alt+V").params(
        r#"{"as"?: "formatted"|"rtf"|"html"|"picture"|"text"|"unicode", "text"?: string, "html"?: string, "rtf"?: string, "picture"?: base64, "privateClipboard"?: bool, "link"?: bool (Paste link: a LINK field to the copy's saved source document)}"#,
    )]
}

/// Clipboard text from the params (system clipboard), else WordCraft's own copy.
fn clip_text(s: &Session, v: &Value) -> Option<String> {
    match p::str(v, "text") {
        Some(t) => Some(t.replace("\r\n", "\n")),
        None => s.clipboard.as_ref().map(Fragment::plain_text),
    }
}

fn looks_like_rtf(t: &str) -> bool {
    t.trim_start().starts_with("{\\rtf")
}

fn looks_like_html(t: &str) -> bool {
    let t = t.trim_start();
    t.starts_with('<') && (t.contains("</") || t.contains("/>"))
}

/// Whether the system clipboard still holds this window's copy. A front end that can check says
/// so in `privateClipboard`; callers that cannot (an agent, the CLI) omit it, and the session's
/// copy is used as before.
fn owns_clipboard(v: &Value) -> bool {
    p::bool(v, "privateClipboard").unwrap_or(true)
}

/// The source for a format, if the clipboard has it.
fn source(s: &Session, v: &Value, fmt: &str) -> Option<String> {
    let non_empty = |t: String| if t.is_empty() { None } else { Some(t) };
    match fmt {
        "formatted" if owns_clipboard(v) => {
            let f = s.clipboard.as_ref()?;
            // System clipboard text that isn't our own copy means someone else copied since.
            match p::str(v, "text") {
                Some(t) if f.clipboard_text() != t.replace("\r\n", "\n") => None,
                _ => Some(String::new()),
            }
        }
        "formatted" => None,
        "rtf" => p::str(v, "rtf").map(str::to_string).or_else(|| clip_text(s, v).filter(|t| looks_like_rtf(t))).and_then(non_empty),
        "html" => p::str(v, "html").map(str::to_string).or_else(|| clip_text(s, v).filter(|t| looks_like_html(t))).and_then(non_empty),
        "picture" => p::str(v, "picture").map(str::to_string).and_then(non_empty),
        "text" | "unicode" => clip_text(s, v).and_then(non_empty),
        _ => None,
    }
}

/// Formats the clipboard can be pasted as, best first.
pub fn available(s: &Session, v: &Value) -> Vec<&'static str> {
    FORMATS.iter().map(|(id, _)| *id).filter(|id| source(s, v, id).is_some()).collect()
}

fn formats_json(ids: &[&str], result: impl Fn(&str) -> String) -> Value {
    json!(
        ids.iter()
            .map(|id| json!({"id": id, "label": FORMATS.iter().find(|(f, _)| f == id).map(|(_, l)| *l).unwrap_or(*id), "result": result(id)}))
            .collect::<Vec<_>>()
    )
}

fn paste_special(s: &mut Session, v: &Value) -> CmdResult {
    for k in ["text", "html", "rtf"] {
        if p::str(v, k).is_some_and(|t| t.len() > MAX_PASTE_BYTES) {
            return Err(CmdError::Params(format!("`{k}` is larger than {} MB", MAX_PASTE_BYTES >> 20)));
        }
    }
    if p::str(v, "picture").is_some_and(|t| t.len() > MAX_PICTURE_BYTES) {
        return Err(CmdError::Params(format!("`picture` is larger than {} MB", MAX_PICTURE_BYTES >> 20)));
    }
    let ids = available(s, v);
    let Some(fmt) = p::str(v, "as") else {
        // No format chosen: list them and ask a front end (if any) to show the dialog.
        let links = link_formats(s, v);
        let mut req = json!({"open": "pasteSpecial", "formats": ids, "linkFormats": links});
        for k in ["text", "html", "rtf", "picture"] {
            if let Some(t) = p::str(v, k) {
                req[k] = json!(t);
            }
        }
        // Keep the ownership result so the dialog's list is the same as this one.
        if let Some(own) = p::bool(v, "privateClipboard") {
            req["privateClipboard"] = json!(own);
        }
        // Only the newest dialog request matters; older ones would pin their payloads.
        s.ui_requests.retain(|r| r["open"] != "pasteSpecial");
        s.push_ui_request(req);
        return Ok(json!({
            "formats": formats_json(&ids, |id| result_text(id).to_string()),
            "linkFormats": formats_json(&links, link_result_text),
        }));
    };
    let fmt = match fmt.to_ascii_lowercase().as_str() {
        "formatted" | "keep" | "wordcraft" => "formatted",
        "rtf" => "rtf",
        "html" => "html",
        "picture" | "png" | "image" => "picture",
        "text" | "unformatted" | "plain" => "text",
        "unicode" => "unicode",
        other => return Err(CmdError::Params(format!("unknown format `{other}` (formatted, rtf, html, picture, text or unicode)"))),
    };
    let link = p::bool(v, "link").unwrap_or(false);
    if link && link_source(s, v).is_none() {
        return Err(CmdError::Failed("Paste link needs text copied from a saved document that is still open".into()));
    }
    let Some(src) = source(s, v, fmt) else {
        return Err(CmdError::Failed(if ids.is_empty() {
            "the clipboard is empty".into()
        } else {
            format!("the clipboard has no {fmt} content (available: {})", ids.join(", "))
        }));
    };
    if link {
        return paste_link(s, v, fmt, &src);
    }
    if fmt == "picture" {
        let mut r = super::insert::picture(s, &json!({"data": src}))?;
        r["pastedAs"] = json!(fmt);
        return Ok(r);
    }
    paste_as(s, fmt, &src)?;
    let mut r = sel_result(s)?;
    r["pastedAs"] = json!(fmt);
    Ok(r)
}

/// Paste the clipboard's `src` for format `fmt` (not Picture) at the selection.
fn paste_as(s: &mut Session, fmt: &str, src: &str) -> Result<(), CmdError> {
    let frag = match fmt {
        "text" | "unicode" => return super::paste_text(s, &src.replace("\r\n", "\r").replace('\n', "\r")),
        "formatted" => s.clipboard.clone().ok_or_else(|| CmdError::Failed("the clipboard is empty".into()))?,
        "html" => adopt(s, import_html(src)),
        _ => adopt(s, wordcraft_formats::rtf::import(src.as_bytes()).map_err(|e| CmdError::Failed(format!("bad RTF: {e}")))?),
    };
    insert(s, frag)
}

/// Paste link: bookmark the copy's source range (once per copy) and paste the clipboard as format
/// `fmt` as the result of a LINK field pointing at it.
fn paste_link(s: &mut Session, v: &Value, fmt: &str, src: &str) -> CmdResult {
    let Some(path) = link_source(s, v) else {
        return Err(CmdError::Failed("Paste link needs text copied from a saved document that is still open".into()));
    };
    if !link_formats(s, v).contains(&fmt) {
        return Err(CmdError::Failed(format!("Paste link can't paste as {fmt}")));
    }
    let Some(link) = s.clipboard_link.clone() else {
        return Err(CmdError::Failed("Paste link needs text copied from a saved document that is still open".into()));
    };
    if replaces_source(s, &link) {
        return Err(CmdError::Failed("Paste link can't replace the text it links to".into()));
    }
    // `run` rolls back only the active document: a failure from here on also takes back the
    // bookmark this paste put in another open document, and the copy's link.
    let mut source_before = None;
    let r = link_and_paste(s, &link, &path, fmt, src, &mut source_before);
    if r.is_err() {
        s.clipboard_link = Some(link.clone());
        if let Some(c) = source_before {
            let _ = s.with_document(link.doc, |s| s.restore_edit_checkpoint(c));
        }
    }
    r
}

/// Whether the selection would replace any of the copy's source text (in its document, between
/// its bookmark's marks once a Paste link added one): pasting there would delete the text, and
/// the bookmark, the new LINK field points at.
fn replaces_source(s: &Session, link: &ClipboardLink) -> bool {
    if link.doc != s.document_id() || s.sel.is_collapsed() {
        return false;
    }
    let (start, end) = match &link.bookmark {
        Some(name) => match find_bookmark(&s.doc, link.start.story, name) {
            Some(at) => super::insert::bookmark_span(&s.doc, name, &at),
            None => return false,
        },
        None => (link.start.clone(), link.end.clone()),
    };
    let (a, b) = s.sel.ordered();
    a.story == start.story && a < end && start < b
}

/// Paste link's edits: bookmark the source range (once per copy), then the LINK field with the
/// clipboard pasted as its result. `source_before` gets the source document as it was when the
/// bookmark goes into another open document.
fn link_and_paste(
    s: &mut Session,
    link: &ClipboardLink,
    path: &std::path::Path,
    fmt: &str,
    src: &str,
    source_before: &mut Option<crate::session::EditCheckpoint>,
) -> CmdResult {
    let name = match &link.bookmark {
        Some(name) => name.clone(),
        None => {
            let name = add_link_bookmark(s, link, source_before)?;
            if let Some(l) = s.clipboard_link.as_mut() {
                l.bookmark = Some(name.clone());
            }
            name
        }
    };
    let instr = link_instr(path, &name, fmt).ok_or_else(|| CmdError::Failed(format!("Paste link can't paste as {fmt}")))?;
    // The field's start, then the content, then its end after the last pasted character.
    super::controls::prepare_edit(s)?;
    let at = delete_selection(s)?;
    let at = s.doc.past_field_ends(&at);
    let props = s.typing_props();
    let after = s.doc.insert_object(&at, InlineObject::FieldStart { instr: instr.clone(), locked: false, dirty: false }, &props)?;
    s.sel = Selection::caret(after.clone());
    paste_as(s, fmt, src)?;
    let (end, _) = s.sel.ordered();
    // Content ending with a paragraph mark: the field ends with the last pasted paragraph's text.
    let end_at = if end.off == 0 && end.path != after.path && end.path.last() > 0 {
        let prev = end.path.with_last(end.path.last().saturating_sub(1));
        match s.doc.para(end.story, &prev) {
            Some(p) => Pos { story: end.story, path: prev, off: p.len() },
            None => end.clone(),
        }
    } else {
        end.clone()
    };
    let field_end = s.doc.insert_object(&end_at, InlineObject::FieldEnd, &props)?;
    s.sel = Selection::caret(if end_at == end { field_end } else { end });
    let mut r = sel_result(s)?;
    r["pastedAs"] = json!(fmt);
    r["linked"] = json!(true);
    r["instr"] = json!(instr);
    r["bookmark"] = json!(name);
    Ok(r)
}

/// Add an `OLE_LINK<n>` bookmark around the copy's source range in its document: the smallest n
/// that document doesn't use yet. In the active document it is part of this command's edit (and
/// the selection keeps its place); in another open document it is that document's own undoable
/// edit.
fn add_link_bookmark(s: &mut Session, link: &ClipboardLink, source_before: &mut Option<crate::session::EditCheckpoint>) -> Result<String, CmdError> {
    let (doc, _) = s.open_document(link.doc).ok_or_else(|| CmdError::Failed("the copied text's document is closed".into()))?;
    let taken = bookmark_names(doc);
    let name = (1..=MAX_LINK_BOOKMARKS)
        .map(|n| format!("OLE_LINK{n}"))
        .find(|n| !taken.iter().any(|t| t.eq_ignore_ascii_case(n)))
        .ok_or_else(|| CmdError::Failed("the source document has too many OLE_LINK bookmarks".into()))?;
    if link.doc == s.document_id() {
        mark_range(s, link, &name)?;
        return Ok(name);
    }
    s.with_document(link.doc, |s| {
        // A nested command so the source gets its own undo step and is marked changed; it isn't
        // what the user ran, so Repeat and a macro being recorded don't see it.
        *source_before = Some(s.edit_checkpoint());
        let last = s.last_command.clone();
        let recorded = s.recording.as_ref().map(|(_, steps)| steps.len());
        let sel = s.sel.clone();
        s.sel = Selection { anchor: link.start.clone(), focus: link.end.clone() };
        let r = s.run("insert.bookmark", &json!({"name": name}));
        s.sel = sel;
        shift_for_marks(s, link);
        s.clamp_selection();
        s.last_command = last;
        if let (Some(n), Some((_, steps))) = (recorded, s.recording.as_mut()) {
            steps.truncate(n);
        }
        r
    })??;
    Ok(name)
}

/// Put bookmark `name`'s marks around the copy's range in the active document, keeping the
/// selection on the same text.
fn mark_range(s: &mut Session, link: &ClipboardLink, name: &str) -> Result<(), CmdError> {
    let props = wordcraft_doc::props::CharProps::default();
    s.doc.insert_object(&link.end, InlineObject::BookmarkEnd { name: name.to_string() }, &props)?;
    s.doc.insert_object(&link.start, InlineObject::BookmarkStart { name: name.to_string() }, &props)?;
    shift_for_marks(s, link);
    Ok(())
}

/// Move the selection's ends past the bookmark marks just put at the copy's start and end.
fn shift_for_marks(s: &mut Session, link: &ClipboardLink) {
    let len = OBJ.len_utf8();
    for at in [&mut s.sel.anchor, &mut s.sel.focus] {
        let mut by = 0;
        for (mark, inclusive) in [(&link.end, true), (&link.start, false)] {
            if at.story == mark.story && at.path == mark.path && (at.off > mark.off || (inclusive && at.off == mark.off)) {
                by += len;
            }
        }
        at.off = at.off.saturating_add(by);
    }
}

/// Indent each side of a pasted `<blockquote>` whose page gives it no margins, points.
const BLOCKQUOTE_INDENT: f32 = 36.0;

/// Another program's HTML clipboard payload as a document. A `<blockquote>` there is how
/// LibreOffice (and many web pages) write a paragraph indented on both sides, not a quotation
/// (#1270): it pastes as a Normal paragraph indented by the page's `blockquote` margins, else half
/// an inch each side, rather than in the (centred, italic) Quote style. Opening an .html file keeps
/// `<blockquote>` as Quote, so HTML round-trips.
fn import_html(html: &str) -> Document {
    let mut doc = wordcraft_formats::html::import(html_fragment(html));
    let (left, right) = blockquote_margins(html);
    for b in &mut doc.body {
        each_para(Arc::make_mut(b), 0, &mut |p| {
            if p.props.style.as_deref() == Some("Quote") {
                p.props.style = None;
                p.props.indent_left.get_or_insert(left);
                p.props.indent_right.get_or_insert(right);
                p.touch();
            }
        });
    }
    doc
}

/// The left and right margins (points) the payload's `<style>` sheets give `blockquote` — LibreOffice
/// writes `blockquote { margin-left: 1cm; margin-right: 1cm }` — else [`BLOCKQUOTE_INDENT`].
fn blockquote_margins(html: &str) -> (f32, f32) {
    let len = |v: &str| wordcraft_formats::html::css_len_pt(v).map(|x| x.clamp(0.0, 1584.0));
    let (mut left, mut right) = (BLOCKQUOTE_INDENT, BLOCKQUOTE_INDENT);
    let lower = html.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("<style") {
        let tag = rest.get(at..).unwrap_or("");
        let sheet = tag.find('>').and_then(|j| tag.get(j + 1..)).unwrap_or("");
        let end = sheet.find("</style").unwrap_or(sheet.len());
        for rule in sheet.get(..end).unwrap_or("").split('}') {
            let Some((sel, decls)) = rule.split_once('{') else { continue };
            if !sel.split(',').any(|s| s.trim().trim_start_matches("<!--").trim() == "blockquote") {
                continue;
            }
            for d in decls.split(';') {
                let Some((name, v)) = d.split_once(':') else { continue };
                match name.trim() {
                    "margin-left" => left = len(v).unwrap_or(left),
                    "margin-right" => right = len(v).unwrap_or(right),
                    "margin" => {
                        let vals: Vec<&str> = v.split_whitespace().take(4).collect();
                        let (r, l) = match vals.as_slice() {
                            [a] => (*a, *a),
                            [_, h] | [_, h, _] => (*h, *h),
                            [_, r, _, l] => (*r, *l),
                            _ => continue,
                        };
                        right = len(r).unwrap_or(right);
                        left = len(l).unwrap_or(left);
                    }
                    _ => {}
                }
            }
        }
        rest = sheet.get(end..).unwrap_or("");
    }
    (left, right)
}

/// The HTML itself from a Windows "HTML Format" (CF_HTML) clipboard payload — a `Version:` header
/// giving byte offsets — or the text unchanged. Hostile offsets fall back to the text after the
/// header.
pub fn html_fragment(html: &str) -> &str {
    if !html.starts_with("Version:") {
        return html;
    }
    let offset = |key: &str| -> Option<usize> {
        let at = html.find(key)?.checked_add(key.len())?;
        let digits: &str = html.get(at..)?.split(['\r', '\n']).next()?;
        digits.trim().parse().ok()
    };
    if let (Some(a), Some(b)) = (offset("StartFragment:"), offset("EndFragment:"))
        && let Some(frag) = html.get(a..b)
    {
        return frag;
    }
    html.find('<').and_then(|i| html.get(i..)).unwrap_or("")
}

/// Whether an imported document has anything to paste: text, a table, or a picture it carries.
fn has_content(d: &Document) -> bool {
    d.body.iter().any(|b| match &**b {
        Block::Para(p) => !p.text.trim().is_empty() || (!p.objects.is_empty() && !d.media.is_empty()),
        Block::Table(_) => true,
    })
}

/// Another program's formatted copy, from the system clipboard's HTML, else its RTF (#687), as a
/// fragment of this document. `None` when neither is given, either is larger than
/// [`MAX_PASTE_BYTES`], or it holds nothing to paste (the caller pastes the text then).
pub fn rich_fragment(s: &mut Session, v: &Value) -> Option<Fragment> {
    let fits = |k: &str| p::str(v, k).filter(|t| !t.trim().is_empty() && t.len() <= MAX_PASTE_BYTES);
    let doc = fits("html").map(import_html);
    let doc = match doc.filter(has_content) {
        Some(d) => d,
        None => {
            let rtf = fits("rtf").filter(|t| looks_like_rtf(t))?;
            wordcraft_formats::rtf::import(rtf.as_bytes()).ok().filter(has_content)?
        }
    };
    Some(adopt(s, doc))
}

/// What a Paste brings in.
pub enum Pasted {
    /// WordCraft's own copy, or another program's HTML or RTF: it keeps the formatting it carries.
    Fragment(Fragment),
    /// The system clipboard's text, lines separated by `\n`.
    Text(String),
}

/// What Paste inserts: WordCraft's own copy while the system clipboard still holds it, else
/// another program's formatted copy (HTML, RTF), else the system clipboard's text. A front end
/// that knows another program owns the clipboard passes `privateClipboard: false`, which keeps
/// the fragment out even when the text matches.
pub fn paste_content(s: &mut Session, v: &Value) -> Result<Pasted, CmdError> {
    let text = p::str(v, "text").map(|t| t.replace("\r\n", "\n"));
    if owns_clipboard(v)
        && let Some(f) = &s.clipboard
        && text.as_ref().is_none_or(|t| f.clipboard_text() == *t)
        && (text.is_some() || (p::str(v, "html").is_none() && p::str(v, "rtf").is_none()))
    {
        return Ok(Pasted::Fragment(f.clone()));
    }
    if let Some(f) = rich_fragment(s, v) {
        return Ok(Pasted::Fragment(f));
    }
    match text {
        Some(t) if !t.is_empty() => Ok(Pasted::Text(t)),
        _ => Err(CmdError::Failed("the clipboard is empty".into())),
    }
}

/// [`paste_content`] as a fragment. Text becomes paragraphs with no formatting of their own.
pub fn paste_fragment(s: &mut Session, v: &Value) -> Result<Fragment, CmdError> {
    Ok(match paste_content(s, v)? {
        Pasted::Fragment(f) => f,
        Pasted::Text(t) => Fragment::from_text(&t),
    })
}

/// Paste plain text the way Keep Text Only does, which is what Word does with text that carries
/// no formatting: the text takes the formatting before the caret, and each line after the first
/// starts a paragraph formatted like the one the text lands in (direction, style, alignment,
/// indents, list). Empty lines become empty paragraphs.
pub fn insert_text(s: &mut Session, text: &str) -> Result<(), CmdError> {
    // Only our own cut, pasted straight after it, becomes a move (see `insert`).
    s.pending_move = None;
    super::paste_text(s, &text.replace("\r\n", "\r").replace('\n', "\r"))
}

/// Merge Formatting (#687): the pasted text takes the formatting where it lands — the paragraph's
/// and the caret's — keeping only its emphasis (bold, italic, underline, strikethrough).
pub fn merge_formatting(s: &Session, frag: &mut Fragment) {
    let mut base = s.typing_props();
    base.ins = None;
    base.del = None;
    base.fmt_change = None;
    let (start, _) = s.sel.ordered();
    let para = s.doc.para_at(&start).map(|p| p.props.clone()).unwrap_or_default();
    for b in &mut frag.blocks {
        each_para(b, 0, &mut |p| {
            p.props = para.clone();
            for run in &mut p.runs {
                let src = std::mem::replace(&mut run.props, base.clone());
                if src.bold == Some(true) {
                    run.props.bold = Some(true);
                }
                if src.italic == Some(true) {
                    run.props.italic = Some(true);
                }
                if src.underline.is_some_and(|u| u != wordcraft_doc::props::Underline::None) {
                    run.props.underline = src.underline;
                }
                if src.strike == Some(true) {
                    run.props.strike = Some(true);
                }
                run.props.link = src.link;
                // Tracked changes are not formatting: text deleted in the source stays deleted.
                run.props.ins = src.ins;
                run.props.del = src.del;
            }
            p.touch();
        });
    }
}

/// Replace the selection with `frag`, marking it as an insertion under Track Changes.
pub fn insert(s: &mut Session, mut frag: Fragment) -> Result<(), CmdError> {
    super::controls::prepare_edit(s)?;
    let at = delete_selection(s)?;
    let track = s.doc.settings.track_changes;
    let mut rid = None;
    if track {
        // What the last command cut, pasted again: the two ends of a move.
        let r = match super::moves::take_paste(s, &frag) {
            Some(r) => r,
            None => super::new_revision(s, wordcraft_doc::RevisionKind::Insert),
        };
        rid = Some(r);
        for b in &mut frag.blocks {
            each_para(b, 0, &mut |p| {
                for run in &mut p.runs {
                    // Text deleted in the source stays deleted: inserted, then deleted.
                    run.props.ins = Some(r);
                    run.props.fmt_change = None;
                }
                p.mark.ins = None;
                p.mark.del = None;
            });
        }
    }
    let end = s.doc.insert_fragment(&at, &frag)?;
    // The paragraph marks the paste added are inserted too, so rejecting it joins the
    // paragraphs again. The last paragraph ends with the original mark and keeps it.
    if let Some(r) = rid
        && at.story == end.story
        && at.path.parent() == end.path.parent()
    {
        for i in at.path.last()..end.path.last() {
            if let Ok(p) = s.doc.para_mut(at.story, &at.path.with_last(i)) {
                p.mark.ins = Some(r);
                p.mark.del = None;
                p.touch();
            }
        }
    }
    s.sel = Selection::caret(end);
    Ok(())
}

/// Every paragraph in a block, tables included (bounded depth).
pub(crate) fn each_para(b: &mut Block, depth: usize, f: &mut dyn FnMut(&mut Paragraph)) {
    if depth > MAX_DEPTH {
        return;
    }
    match b {
        Block::Para(p) => f(p),
        Block::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for cb in &mut cell.blocks {
                        each_para(Arc::make_mut(cb), depth + 1, f);
                    }
                }
            }
        }
    }
}

/// Section breaks brought in from another document keep their page, but their header and
/// footer ids are that document's: clear them (and the different first page that went with
/// them) so the sections link to the previous section's headers and footers, as in Word.
pub(crate) fn unlink_sections(blocks: &mut [Block]) {
    for b in blocks {
        each_para(b, 0, &mut |p| {
            if let Some(sec) = p.section.as_deref_mut() {
                sec.headers = Default::default();
                sec.footers = Default::default();
                sec.title_page = false;
            }
        });
    }
}

/// Turn an imported document into a fragment of the session's document: bring over the media,
/// lists and styles it refers to (renumbered so they can't collide with ours).
pub(crate) fn adopt(s: &mut Session, src: Document) -> Fragment {
    let mut frag = s.doc.adopt(src);
    // Pasted pictures fit our text width, like Insert › Pictures (layout keeps their size).
    let max_w = crate::io::text_width(&s.doc);
    for b in &mut frag.blocks {
        crate::io::fit_block_pictures(b, max_w);
    }
    unlink_sections(&mut frag.blocks);
    frag
}

#[cfg(test)]
mod link_tests {
    use serde_json::json;
    use wordcraft_doc::{Document, Pos, StoryRef};

    use crate::Session;

    fn saved(text: &str) -> Session {
        let mut s = Session::new(Document::from_text(text));
        s.path = Some("/home/ann/Report.docx".into());
        s
    }

    /// Two whole paragraphs copied (ending with a paragraph mark): the field ends with the last
    /// pasted paragraph's text, not in the paragraph after it.
    #[test]
    fn a_linked_copy_ending_with_a_paragraph_mark_ends_the_field_in_its_last_paragraph() {
        let mut s = saved("One\nTwo\nThree");
        s.run("select.range", &json!({"anchor": Pos::body(0, 0), "focus": Pos::body(2, 0)})).unwrap();
        s.run("edit.copy", &json!({})).unwrap();
        let src = s.document_id();
        s.add_document(Document::from_text("Target"));
        s.run("caret.set", &json!({"pos": Pos::body(0, 6)})).unwrap();
        s.run("edit.pasteSpecial", &json!({"as": "formatted", "link": true})).unwrap();
        let f = s.doc.field_ranges(StoryRef::Body);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].start.path, Pos::body(0, 0).path);
        assert_eq!(f[0].end.path, Pos::body(1, 0).path, "{:?}", s.doc.plain_text(StoryRef::Body));
        let last = s.doc.para(StoryRef::Body, &f[0].end.path).map(|p| p.len());
        assert_eq!(Some(f[0].end.off.saturating_add(wordcraft_doc::para::OBJ.len_utf8())), last, "the end mark closes that paragraph");
        // The source's bookmark spans the two paragraphs.
        let marks = s.with_document(src, |s| s.doc.bookmarks()).unwrap();
        assert_eq!(marks.len(), 1);
    }

    /// Picture can't be linked; a refused Paste link changes nothing, and Undo takes a Paste link
    /// (bookmark and field together) out of the active document in one step.
    #[test]
    fn paste_link_refuses_picture_and_undoes_in_one_step() {
        let mut s = saved("Alpha beta gamma");
        s.run("select.range", &json!({"anchor": Pos::body(0, 0), "focus": Pos::body(0, 5)})).unwrap();
        s.run("edit.copy", &json!({})).unwrap();
        s.run("caret.set", &json!({"pos": Pos::body(0, 16)})).unwrap();
        let before = s.doc.clone();
        let e = s.run("edit.pasteSpecial", &json!({"as": "picture", "link": true, "picture": "AAAA"})).unwrap_err();
        assert!(e.to_string().contains("picture"), "{e}");
        assert_eq!(s.doc, before);
        s.run("edit.pasteSpecial", &json!({"as": "text", "link": true})).unwrap();
        assert_eq!(s.doc.field_ranges(StoryRef::Body).len(), 1);
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.plain_text(StoryRef::Body), before.plain_text(StoryRef::Body));
        assert!(s.doc.bookmarks().is_empty() && s.doc.field_ranges(StoryRef::Body).is_empty());
    }
}
