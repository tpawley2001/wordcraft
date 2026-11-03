//! Compatibility Mode: a document laid out for an older Word (`compatibilityMode` below 15, or a
//! Word 97-2003 binary file) and File › Info › Convert, which upgrades it.

use serde_json::{Value, json};
use wordcraft_doc::COMPAT_MODE_CURRENT;

use crate::{CmdError, CmdResult, Session};

/// The Word version a `compatibilityMode` value lays out like.
pub fn mode_version(mode: u32) -> &'static str {
    match mode {
        0..=11 => "Word 97-2003",
        12 | 13 => "Word 2007",
        14 => "Word 2010",
        _ => "Word 2013 and later",
    }
}

/// Whether the path is a Word 97-2003 binary document or template (`.doc`, `.dot`).
fn is_binary_word(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("doc") || e.eq_ignore_ascii_case("dot"))
}

/// Whether the open document is in Compatibility Mode: laid out for a Word older than 2013, or
/// opened from a Word 97-2003 binary file (always in Compatibility Mode).
pub fn compatibility_mode(s: &Session) -> bool {
    s.doc.settings.compat_mode < COMPAT_MODE_CURRENT || s.path.as_deref().is_some_and(is_binary_word)
}

/// `file.convert`: upgrades the document to the current layout mode, as one undo step. A Word
/// 97-2003 file moves to the new format's extension, so the next Save writes a new file and the
/// old one is left alone.
pub(super) fn convert(s: &mut Session, _: &Value) -> CmdResult {
    if !compatibility_mode(s) {
        return Err(CmdError::Disabled("file.convert: the document is not in Compatibility Mode".into()));
    }
    let previous = s.doc.settings.compat_mode;
    s.doc.settings.compat_mode = COMPAT_MODE_CURRENT;
    s.dirty = true;
    let mut out = json!({"mode": COMPAT_MODE_CURRENT, "previous": previous});
    if let Some(path) = s.path.as_ref().filter(|p| is_binary_word(p)) {
        let template = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("dot"));
        let path = path.with_extension(if template { "dotx" } else { "docx" });
        if let Some(m) = out.as_object_mut() {
            m.insert("path".into(), json!(path.to_string_lossy()));
        }
        s.path = Some(path);
    }
    Ok(out)
}
