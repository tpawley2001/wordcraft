//! INCLUDETEXT "FileName" [Bookmark] (ECMA-376 §17.16.5.30): on an explicit update the field shows
//! the text of another local file, or of one bookmark in it. Network paths and URLs are refused
//! without touching the file system; the file is read as `insert.textFromFile` reads it, so the
//! automation read root and the document size cap apply.
//!
//! Update Source (Ctrl+Shift+F7) goes the other way: the field's (edited) result is written back
//! into the file, replacing its main text or only the named bookmark's text.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::{Document, Paragraph, Pos, StoryRef, para_block};

use super::FieldCode;
use crate::cmd::fields::FieldAt;
use crate::{CmdError, CmdResult, Session};

/// The result for a path that is refused or can't be read, as Word shows it.
const BAD_FILE: &str = "Error! Not a valid filename.";
/// The result when the file has no bookmark of the given name.
const NO_BOOKMARK: &str = "Error! Bookmark not defined.";

/// The new result of the INCLUDETEXT field `instr` in `s`: the included text, its paragraphs
/// separated by `\r` (a break in the field result), or an `Error!` text.
pub(super) fn include_text(s: &Session, instr: &str) -> String {
    let (raw, mark) = path_and_bookmark(instr);
    let Some(path) = local_path(s, &raw) else { return BAD_FILE.into() };
    let Ok(mut doc) = crate::io::open_path(&path) else { return BAD_FILE.into() };
    if let Some(name) = mark.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        let Some((found, start)) = doc.bookmarks().into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) else {
            return NO_BOOKMARK.into();
        };
        let (from, to) = crate::cmd::insert::bookmark_span(&doc, &found, &start);
        doc = doc.copy_range(&from, &to).to_document();
    }
    let paras: Vec<String> = doc
        .para_paths(wordcraft_doc::StoryRef::Body)
        .iter()
        .filter_map(|p| doc.para(wordcraft_doc::StoryRef::Body, p))
        .map(|p| p.plain_text())
        .collect();
    paras.join("\r")
}

/// The path argument and the bookmark name (if any) of an INCLUDETEXT instruction.
fn path_and_bookmark(instr: &str) -> (String, Option<String>) {
    let f = FieldCode::parse(instr);
    let raw = f.args.first().cloned().unwrap_or_default();
    // `\!` takes no argument: a word after it is the bookmark (`INCLUDETEXT "a.docx" \! Part`).
    // `\c`, `\t`, `\x` and `\n` (converter, XML transforms) are read and ignored.
    let mark = f.args.get(1).cloned().or_else(|| f.arg('!').map(str::to_string));
    (raw, mark.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()))
}

/// Update Source (`fields.updateSource`): write the result of each selected INCLUDETEXT field back
/// into the file it includes, in document order. The document itself is not changed. The first
/// failure stops and is returned; files already written stay written.
pub(crate) fn update_source(s: &mut Session, _: &Value) -> CmdResult {
    let fields: Vec<FieldAt> =
        crate::cmd::fields::selected(s).into_iter().filter(|f| FieldCode::parse(&f.instr).name.eq_ignore_ascii_case("INCLUDETEXT")).collect();
    if fields.is_empty() {
        return Err(CmdError::Failed("there is no INCLUDETEXT field in the selection".into()));
    }
    let mut updated = Vec::new();
    for f in &fields {
        if f.locked {
            return Err(CmdError::Failed("the INCLUDETEXT field is locked".into()));
        }
        let (raw, mark) = path_and_bookmark(&f.instr);
        let path = local_path(s, &raw).ok_or_else(|| CmdError::Failed(BAD_FILE.into()))?;
        let mut doc = crate::io::open_path(&path).map_err(CmdError::Failed)?;
        let text = field_result(s, f);
        let text = text.strip_suffix('\r').unwrap_or(&text);
        match mark {
            Some(name) => replace_bookmark(&mut doc, &name, text)?,
            None => replace_body(&mut doc, text),
        }
        crate::io::save_path(&path, &doc).map_err(CmdError::Failed)?;
        updated.push(path.display().to_string());
    }
    Ok(json!({"updated": updated}))
}

/// A field's current result: a simple field's cached text, or a range field's content with its
/// paragraphs separated by `\r`.
fn field_result(s: &Session, f: &FieldAt) -> String {
    match &f.end {
        None => match s.doc.para(f.at.story, &f.at.path).and_then(|p| p.object_at(f.at.off)) {
            Some(InlineObject::Field { result, .. }) => result.clone(),
            _ => String::new(),
        },
        Some(end) => {
            let from = Pos { off: f.at.off.saturating_add(OBJ.len_utf8()), ..f.at.clone() };
            let doc = s.doc.copy_range(&from, end).to_document();
            let paras: Vec<String> =
                doc.para_paths(StoryRef::Body).iter().filter_map(|p| doc.para(StoryRef::Body, p)).map(|p| p.plain_text()).collect();
            paras.join("\r")
        }
    }
}

/// Replace the main text of `doc` with `text`, one paragraph per `\r`-separated part, formatted as
/// its first paragraph and first run were.
fn replace_body(doc: &mut Document, text: &str) {
    let paths = doc.para_paths(StoryRef::Body);
    let first = paths.first().and_then(|p| doc.para(StoryRef::Body, p));
    let props = first.map(|p| p.props.clone()).unwrap_or_default();
    let chars = first.map(|p| p.props_at(0).clone()).unwrap_or_default();
    // The body's section properties ride on its last paragraph (OOXML `w:sectPr`); keep them.
    let section = paths.last().and_then(|p| doc.para(StoryRef::Body, p)).and_then(|p| p.section.clone());
    let mut paras: Vec<Paragraph> = text
        .split('\r')
        .map(|t| {
            let mut p = Paragraph::with_text(t, chars.clone());
            p.props = props.clone();
            p.mark = chars.clone();
            p
        })
        .collect();
    if let Some(last) = paras.last_mut() {
        last.section = section;
    }
    doc.body = paras.into_iter().map(para_block).collect();
}

/// Replace the text of bookmark `name` (found case-insensitively) in `doc` with `text`, one
/// paragraph per `\r`-separated part; the bookmark then spans the new text.
fn replace_bookmark(doc: &mut Document, name: &str, text: &str) -> Result<(), CmdError> {
    let Some((found, start)) = doc.bookmarks().into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) else {
        return Err(CmdError::Failed(NO_BOOKMARK.into()));
    };
    let (from, to) = crate::cmd::insert::bookmark_span(doc, &found, &start);
    let chars = doc.para(from.story, &from.path).map(|p| p.props_at(from.off).clone()).unwrap_or_default();
    let mut at = doc.delete_range(&from, &to)?;
    for (i, part) in text.split('\r').enumerate() {
        if i > 0 {
            at = doc.split_paragraph(&at)?;
        }
        at = doc.insert_text(&at, part, &chars)?;
    }
    Ok(())
}

/// The local file the field's path names: `raw` as the field code gives it (a quoted path's
/// doubled backslashes already single), relative to the document's folder. `None` for an empty path, a network path, a URL, or a relative path with no document
/// folder: none of them is looked at on disk.
pub(super) fn local_path(s: &Session, raw: &str) -> Option<PathBuf> {
    let text = raw.trim();
    if text.is_empty() || text.starts_with("\\\\") || text.starts_with("//") || has_scheme(text) {
        return None;
    }
    let path = Path::new(text);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    let folder = s.path.as_deref()?.parent()?;
    Some(folder.join(path))
}

/// Does `text` start with a URL scheme (`http:`, `file:`…)? A drive letter (`C:`) is not one.
fn has_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once(':') else { return false };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.len() > 1
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::has_scheme;

    #[test]
    fn schemes_but_not_drive_letters() {
        for url in ["http://a", "https://a", "ftp://a", "file:///a", "x-y.z+w:rest", "mailto:a"] {
            assert!(has_scheme(url), "{url}");
        }
        for path in ["C:\\a.txt", "c:/a.txt", "a.txt", "/tmp/a:b", "1x:a"] {
            assert!(!has_scheme(path), "{path}");
        }
    }
}
