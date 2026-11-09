//! INCLUDETEXT "FileName" [Bookmark] (ECMA-376 §17.16.5.30): on an explicit update the field shows
//! the text of another local file, or of one bookmark in it. Network paths and URLs are refused
//! without touching the file system; the file is read as `insert.textFromFile` reads it, so the
//! automation read root and the document size cap apply.

use std::path::{Path, PathBuf};

use super::FieldCode;
use crate::Session;

/// The result for a path that is refused or can't be read, as Word shows it.
const BAD_FILE: &str = "Error! Not a valid filename.";
/// The result when the file has no bookmark of the given name.
const NO_BOOKMARK: &str = "Error! Bookmark not defined.";

/// The new result of the INCLUDETEXT field `instr` in `s`: the included text, its paragraphs
/// separated by `\r` (a break in the field result), or an `Error!` text.
pub(super) fn include_text(s: &Session, instr: &str) -> String {
    let f = FieldCode::parse(instr);
    let raw = f.args.first().cloned().unwrap_or_default();
    // `\!` takes no argument: a word after it is the bookmark (`INCLUDETEXT "a.docx" \! Part`).
    // `\c`, `\t`, `\x` and `\n` (converter, XML transforms) are read and ignored.
    let mark = f.args.get(1).cloned().or_else(|| f.arg('!').map(str::to_string));
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

/// The local file the field's path names: `raw` as the field code gives it (a quoted path's
/// doubled backslashes already single), relative to the document's folder. `None` for an empty path, a network path, a URL, or a relative path with no document
/// folder: none of them is looked at on disk.
fn local_path(s: &Session, raw: &str) -> Option<PathBuf> {
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
