//! References tab: table of contents, footnotes/endnotes, captions; field updates.

use serde_json::{Value, json};
use wordcraft_doc::numbering::{Counters, LevelSuffix};
use wordcraft_doc::para::{InlineObject, NoteKind, OBJ};
use wordcraft_doc::props::{CharProps, TabAlign, TabLeader, TabStop};
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::{Block, Paragraph, PartKind, Path, Pos, StoryRef, para_block};

use super::{delete_selection, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

mod captions;
mod fields;
mod includetext;
mod notes;
mod numbers;
mod properties;
mod toc;

pub use captions::{AUTO_EQUATION, AUTO_OBJECTS, AUTO_TABLE, all_labels as all_caption_labels, label_options as caption_label_options};
pub(crate) use captions::{add_auto_caption, below_caption};
use captions::{auto_caption, caption_label};
use fields::caption;
pub use fields::{
    CHAPTER_SEPARATORS, REFERENCE_NOT_FOUND, output_fields, refresh_date_fields, seq_format, update_fields, update_fields_in, update_seq,
};
pub(crate) use fields::{page_at, page_formats, page_label, page_text};
pub(crate) use includetext::update_source;
pub(super) use notes::note_options;
use notes::{note, note_refs};
pub(crate) use properties::bookmark_text_at;
pub use properties::{bookmark_text, date_picture};
#[cfg(test)]
use toc::cached_toc_tabs;
pub(crate) use toc::{bookmark_heading, headings, toc_bookmark};
pub use toc::{caret_in_toc, leader, level_range, toc_blocks, toc_instr, update_toc, update_toc_pages};
use toc::{insert_toc, remove_toc, toc_range};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("references.toc", "Table of Contents", "References › Table of Contents", insert_toc).params(
            r#"{"levels"?: 1-9, "title"?: string, "pageNumbers"?: bool, "rightAlign"?: bool, "tabLeader"?: "dot|hyphen|underscore|none", "hyperlinks"?: bool, "headingStyles"?: bool, "outlineLevels"?: bool, "tcFields"?: bool, "styleLevels"?: {style name: 1-9}} (replaces an existing table; {} updates it)"#,
        ),
        CommandSpec::new("references.updateToc", "Update Table", "References › Table of Contents", |s, v| {
            if p::bool(v, "pageNumbersOnly").unwrap_or(false) {
                update_toc_pages(s)?;
            } else {
                update_toc(s)?;
            }
            sel_result(s)
        })
        .params(r#"{"pageNumbersOnly"?: bool (keep the entries as they are and update only their page numbers; default: rebuild the entire table)}"#),
        CommandSpec::new("references.removeToc", "Remove Table of Contents", "References › Table of Contents", |s, _| {
            if let Some((start, end)) = toc_range(s) {
                remove_toc(s, start, end)?;
                s.clamp_selection();
            }
            sel_result(s)
        }),
        CommandSpec::new("references.addText", "Add Text", "References › Table of Contents", |s, v| {
            let level = p::u64(v, "level").unwrap_or(0).min(9) as u8;
            super::para::fmt(s, &|pp| pp.outline_level = if level == 0 { Some(9) } else { Some(level - 1) })
        })
        .params(r#"{"level": 0 (do not show) | 1-9}"#),
        CommandSpec::new("references.markTocEntry", "Mark Table of Contents Entry", "References › Table of Contents", mark_toc_entry)
            .key(super::non_mac("Alt+Shift+O"))
            .params(r#"{"text"?: string (the selection), "identifier"?: one letter A-Z (default "C"), "level"?: 1-9 (default 1)}"#),
        CommandSpec::new("references.footnote", "Insert Footnote", "References › Footnotes", |s, v| note(s, v, NoteKind::Footnote))
            .key("Mod+Alt+F")
            .params(r#"{"text"?: string, "mark"?: string (a custom mark instead of the number)}"#),
        CommandSpec::new("references.endnote", "Insert Endnote", "References › Footnotes", |s, v| note(s, v, NoteKind::Endnote))
            // macOS takes ⌘⌥D to show and hide the Dock; Word for Mac uses ⌘⌥E.
            .key(super::mac_or("Mod+Alt+E", "Mod+Alt+D"))
            .params(r#"{"text"?: string, "mark"?: string (a custom mark instead of the number)}"#),
        CommandSpec::new("references.convertNotes", "Convert Notes", "References › Footnotes", |s, v| {
            let (foot, end) = match p::req_str(v, "to")? {
                "endnote" | "endnotes" => (true, false),
                "footnote" | "footnotes" => (false, true),
                "swap" => (true, true),
                t => return Err(CmdError::Params(format!("unknown `to` {t:?}; use endnote, footnote or swap"))),
            };
            let n = s.doc.convert_notes(foot, end);
            Ok(json!({"converted": n}))
        })
        .params(r#"{"to": "endnote" (footnotes become endnotes) | "footnote" (endnotes become footnotes) | "swap"}"#),
        CommandSpec::new("references.nextFootnote", "Next Footnote", "References › Footnotes", |s, _| {
            let caret = s.sel.focus.clone();
            let refs = note_refs(s, Some(NoteKind::Footnote));
            if let Some(p) = refs.iter().find(|p| **p > caret).or(refs.first()) {
                s.sel = Selection::caret(p.clone());
            }
            sel_result(s)
        })
        .key(super::non_mac("Alt+Shift+> / Alt+Shift+."))
        .pure(),
        CommandSpec::new("references.notes", "Show Notes", "References › Footnotes", |s, _| {
            let mut out = Vec::new();
            for (id, part) in &s.doc.parts {
                if matches!(part.kind, PartKind::Footnote | PartKind::Endnote) {
                    out.push(json!({"id": id, "kind": part.kind, "text": s.doc.plain_text(StoryRef::Part(*id))}));
                }
            }
            Ok(Value::Array(out))
        })
        .pure(),
        CommandSpec::new("references.caption", "Insert Caption", "References › Captions", caption)
            .params(r#"{"label"?: "Figure|Table|Equation|any label", "text"?: string, "position"?: "below|above", "excludeLabel"?: bool, "format"?: "ARABIC|alphabetic|ALPHABETIC|roman|ROMAN", "chapter"?: heading level 1-9, "separator"?: "-|.|:|—|–"}"#),
        CommandSpec::new("references.captionLabel", "New Label", "References › Captions", caption_label)
            .params(r#"{"add"?: string, "delete"?: string (not Figure, Table or Equation)} -> {"labels": [Figure, Table, Equation, the document's own…]}"#),
        CommandSpec::new("references.autoCaption", "AutoCaption", "References › Captions", auto_caption).params(
            r#"{"set"?: {"object": "Microsoft Word Table|Microsoft Equation|…", "label": label name or null (off), "position"?: "above|below", "format"?: "decimal|upperLetter|lowerLetter|upperRoman|lowerRoman", "chapter"?: heading level 1-9 or 0 (none), "separator"?: "-|.|:|—|–"}} -> {"objects": [{"name", "label", "position", "format"?, "chapter"?, "separator"?}…], "labels": […]}"#,
        ),
        // F9: update fields; the selected ones that showed their code show their result again.
        CommandSpec::new("references.updateFields", "Update Field", "References", |s, v| {
            update_fields_in(s, p::bool(v, "all").unwrap_or(false), p::bool(v, "pageNumbersOnly").unwrap_or(false))?;
            super::fields::show_results(s)?;
            sel_result(s)
        })
        .params(r#"{"all"?: bool (the body and every header, footer, note and text box, wherever the caret is; default: the story the caret is in), "pageNumbersOnly"?: bool (the table of contents keeps its entries and updates only their page numbers; default: rebuilt)}"#)
        .key("F9"),
    ]
}

/// The TC field for `references.markTocEntry`'s parameters: `TC "text" \f C \l 1` (ECMA-376
/// §17.16.5.70), hidden as Word hides it, at the end of the selection.
fn mark_toc_entry(s: &mut Session, v: &Value) -> CmdResult {
    let text = super::index::clean(&p::str(v, "text").map(str::to_string).unwrap_or_else(|| s.selected_text()), 255);
    if text.is_empty() {
        return Err(CmdError::Params("select text or give `text`".into()));
    }
    let identifier = p::str(v, "identifier").unwrap_or("C").to_ascii_uppercase();
    if identifier.len() != 1 || !identifier.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(CmdError::Params(format!("`identifier` must be one letter A-Z, not {identifier:?}")));
    }
    let level = match v.get("level") {
        None | Some(Value::Null) => 1,
        Some(l) => l.as_u64().filter(|l| (1..=9).contains(l)).ok_or_else(|| CmdError::Params(format!("`level` must be 1-9, not {l}")))?,
    };
    let instr = format!("TC \"{text}\" \\f {identifier} \\l {level}");
    let hidden = CharProps { hidden: Some(true), ..Default::default() };
    let field = InlineObject::Field { instr, result: String::new(), locked: false, dirty: false, code: false, form: String::new() };
    let (_, b) = s.sel.ordered();
    s.doc.insert_object(&b, field, &hidden)?;
    Ok(json!({"text": text, "identifier": identifier, "level": level}))
}

/// A field instruction split into its name, positional arguments and switches (ECMA-376
/// §17.16.1): `TOC \o "1-3" \h` → name `TOC`, switches `o` = `1-3` and `h`. Quoted text stays
/// whole (without the quotes; a backslash inside it makes the next character literal, as in Word:
/// `\"` is a quote, `\\` a backslash, `\*` an asterisk). A switch takes the word after it as its
/// argument unless that word is another switch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldCode {
    pub name: String,
    pub args: Vec<String>,
    /// Switch letter (lower-cased; `*`, `@`, `#` for the general switches) and argument.
    pub switches: Vec<(char, Option<String>)>,
}

/// A field instruction's words, each with whether it was quoted (see [`FieldCode`]).
pub fn field_words(instr: &str) -> Vec<(String, bool)> {
    let mut words: Vec<(String, bool)> = Vec::new();
    let mut chars = instr.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut w = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' if chars.peek().is_some() => w.extend(chars.next()),
                    c => w.push(c),
                }
            }
            words.push((w, true));
        } else {
            let mut w = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || (c == '"' && !w.is_empty()) {
                    break;
                }
                w.push(c);
                chars.next();
            }
            words.push((w, false));
        }
    }
    words
}

/// `s` as quoted field-code text, which [`field_words`] reads back as it was.
pub fn quoted(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '\\') {
            o.push('\\');
        }
        o.push(c);
    }
    o.push('"');
    o
}

/// `s` as one field-code argument: as it is when it's a single plain word, else [`quoted`].
pub fn field_arg(s: &str) -> String {
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || matches!(c, '"' | '\\')) { quoted(s) } else { s.to_string() }
}

impl FieldCode {
    pub fn parse(instr: &str) -> FieldCode {
        let mut words = field_words(instr).into_iter().peekable();
        let mut f =
            FieldCode { name: words.next().map(|(w, _)| w.trim_start_matches('=').to_ascii_uppercase()).unwrap_or_default(), ..Default::default() };
        while let Some((w, quoted)) = words.next() {
            let sw = if quoted { None } else { w.strip_prefix('\\').and_then(|r| r.chars().next()) };
            match sw {
                Some(c) => {
                    let arg = match words.peek() {
                        Some((n, q)) if *q || !n.starts_with('\\') => words.next().map(|(n, _)| n),
                        _ => None,
                    };
                    f.switches.push((c.to_ascii_lowercase(), arg));
                }
                None => f.args.push(w),
            }
        }
        f
    }

    pub fn has(&self, c: char) -> bool {
        self.switches.iter().any(|(k, _)| *k == c)
    }

    /// A table of figures (`TOC \c` or `TOC \a`, ECMA-376 §17.16.5.68), not a table of contents.
    pub fn is_table_of_figures(&self) -> bool {
        self.name == "TOC" && (self.has('c') || self.has('a'))
    }

    /// The argument of switch `c`, if the switch is there with one.
    pub fn arg(&self, c: char) -> Option<&str> {
        self.switches.iter().find(|(k, _)| *k == c).and_then(|(_, a)| a.as_deref())
    }
}

#[cfg(test)]
mod tests;
