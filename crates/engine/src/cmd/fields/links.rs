//! Edit Links to Files (Word's Links dialog): the main text's LINK fields (ECMA-376 Part 1
//! §17.16.5.32) listed with their source, item and settings, and one link at a time set to update
//! automatically (`\a`) or manually, locked, kept formatted after update (`\* MERGEFORMAT`),
//! pointed at another source, updated now, or broken (its result stays as ordinary text).
//!
//! Only the body's links are listed; INCLUDETEXT, INCLUDEPICTURE and HYPERLINK fields are not
//! links here. A setting changes only its own part of the field code.

use std::ops::Range;

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;
use wordcraft_doc::para::{InlineObject, OBJ};

use super::{FieldAt, MAX_CODE, story_fields, with_field};
use crate::cmd::references::{FieldCode, quoted, update_link_at};
use crate::{CmdError, CmdResult, Session, p};

/// The body's LINK fields in document order.
pub fn link_fields(s: &Session) -> Vec<FieldAt> {
    story_fields(s, StoryRef::Body).into_iter().filter(|f| FieldCode::parse(&f.instr).name == "LINK").collect()
}

/// One link as `fields.links` lists it.
fn link_json(index: usize, f: &FieldAt) -> Value {
    let code = FieldCode::parse(&f.instr);
    let arg = |i: usize| code.args.get(i).map(|a| a.trim().to_string()).unwrap_or_default();
    let merge = code.switches.iter().any(|(c, a)| *c == '*' && a.as_deref().is_some_and(|a| a.eq_ignore_ascii_case("MERGEFORMAT")));
    json!({
        "index": index,
        "source": arg(1),
        "item": arg(2),
        "progId": arg(0),
        "update": if code.has('a') { "auto" } else { "manual" },
        "locked": f.locked,
        "preserveFormatting": merge,
    })
}

/// `fields.links`: every link to another file in the main text.
pub(super) fn links(s: &mut Session, _: &Value) -> CmdResult {
    let list: Vec<Value> = link_fields(s).iter().enumerate().map(|(i, f)| link_json(i, f)).collect();
    Ok(json!({"links": list}))
}

/// How a word of a field code is used.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Name,
    /// The n-th positional argument.
    Arg(usize),
    /// A switch, by its letter (lower-cased).
    Switch(char),
    /// The argument of the switch before it.
    SwitchArg,
}

/// A field code's words: each one's byte range in the code (quotes included) and role, split as
/// [`crate::cmd::references::field_words`] splits them.
fn words(code: &str) -> Vec<(Range<usize>, Role)> {
    let mut spans: Vec<(Range<usize>, bool)> = Vec::new();
    let mut chars = code.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut end = code.len();
            while let Some((i, c)) = chars.next() {
                match c {
                    '"' => {
                        end = i + 1;
                        break;
                    }
                    '\\' if chars.peek().is_some() => {
                        chars.next();
                    }
                    _ => {}
                }
            }
            spans.push((start..end, true));
        } else {
            let mut end = start;
            while let Some(&(i, c)) = chars.peek() {
                if c.is_whitespace() || (c == '"' && i > start) {
                    break;
                }
                end = i + c.len_utf8();
                chars.next();
            }
            spans.push((start..end, false));
        }
    }
    let mut out = Vec::with_capacity(spans.len());
    let mut args = 0;
    let mut i = 0;
    while let Some((span, quoted)) = spans.get(i).cloned() {
        let text = code.get(span.clone()).unwrap_or_default();
        let switch = if quoted { None } else { text.strip_prefix('\\').and_then(|r| r.chars().next()) };
        if i == 0 {
            out.push((span, Role::Name));
        } else if let Some(c) = switch {
            out.push((span, Role::Switch(c.to_ascii_lowercase())));
            if let Some((next, q)) = spans.get(i + 1).cloned()
                && (q || !code.get(next.clone()).unwrap_or_default().starts_with('\\'))
            {
                out.push((next, Role::SwitchArg));
                i += 1;
            }
        } else {
            out.push((span, Role::Arg(args)));
            args += 1;
        }
        i += 1;
    }
    out
}

/// `code` without the bytes `cut` and the space before them.
fn without(code: &str, words: &[(Range<usize>, Role)], k: usize, last: usize) -> String {
    let from = k.checked_sub(1).and_then(|p| words.get(p)).map(|(r, _)| r.end).unwrap_or(0);
    let to = words.get(last).map(|(r, _)| r.end).unwrap_or(from);
    format!("{}{}", code.get(..from).unwrap_or_default(), code.get(to..).unwrap_or_default())
}

/// `code` with `text` put in at byte `at` (after a space).
fn with_at(code: &str, at: usize, text: &str) -> String {
    format!("{} {text}{}", code.get(..at).unwrap_or_default(), code.get(at..).unwrap_or_default())
}

/// The end of the positional arguments: after the last one before any switch, else the name.
fn args_end(words: &[(Range<usize>, Role)]) -> usize {
    words.iter().take_while(|(_, r)| !matches!(r, Role::Switch(_) | Role::SwitchArg)).last().map(|(r, _)| r.end).unwrap_or(0)
}

/// `code` with the switch `\a` there or not.
fn set_auto(code: &str, auto: bool) -> String {
    let mut code = code.to_string();
    loop {
        let w = words(&code);
        let found = w.iter().position(|(_, r)| *r == Role::Switch('a'));
        match (found, auto) {
            (Some(_), true) => return code,
            (Some(k), false) => {
                let last = if matches!(w.get(k + 1), Some((_, Role::SwitchArg))) { k + 1 } else { k };
                code = without(&code, &w, k, last);
            }
            (None, true) => return with_at(&code, args_end(&w), "\\a"),
            (None, false) => return code,
        }
    }
}

/// `code` with `\* MERGEFORMAT` there or not (other `\*` switches stay).
fn set_merge(code: &str, merge: bool) -> String {
    let mut code = code.to_string();
    loop {
        let w = words(&code);
        let found = w.iter().enumerate().position(|(k, (_, r))| {
            *r == Role::Switch('*')
                && w.get(k + 1).is_some_and(|(span, role)| {
                    *role == Role::SwitchArg && code.get(span.clone()).is_some_and(|t| t.trim_matches('"').eq_ignore_ascii_case("MERGEFORMAT"))
                })
        });
        match (found, merge) {
            (Some(_), true) => return code,
            (Some(k), false) => code = without(&code, &w, k, k + 1),
            (None, true) => {
                let end = code.trim_end().len();
                return with_at(&code, end, "\\* MERGEFORMAT");
            }
            (None, false) => return code,
        }
    }
}

/// `code` with its positional argument `n` (1 the path, 2 the item) set to `value` (quoted, its
/// backslashes doubled), or taken out when `value` is empty; ProgID and switches stay.
fn set_arg(code: &str, n: usize, value: &str) -> Result<String, CmdError> {
    let w = words(code);
    let at = w.iter().position(|(_, r)| *r == Role::Arg(n));
    match (at, value.is_empty()) {
        (Some(k), true) => Ok(without(code, &w, k, k)),
        (Some(k), false) => {
            let span = w.get(k).map(|(r, _)| r.clone()).unwrap_or(0..0);
            Ok(format!("{}{}{}", code.get(..span.start).unwrap_or_default(), quoted(value), code.get(span.end..).unwrap_or_default()))
        }
        (None, true) => Ok(code.to_string()),
        (None, false) => {
            // Only the next argument can be added: an item needs a path before it.
            let before = n.checked_sub(1).and_then(|m| w.iter().find(|(_, r)| *r == Role::Arg(m)));
            match before {
                Some((r, _)) => Ok(with_at(code, r.end, &quoted(value))),
                None => Err(CmdError::Failed("the link has no source to change".into())),
            }
        }
    }
}

/// A string parameter for a field code argument: bounded and without object characters.
fn code_text(v: &Value, k: &str) -> Result<Option<String>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) if t.len() <= MAX_CODE && !t.contains(OBJ) && !t.contains(['\r', '\n']) => Ok(Some(t.trim().to_string())),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be a path or name of at most {MAX_CODE} bytes"))),
    }
}

fn flag(v: &Value, k: &str) -> Result<Option<bool>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be true or false"))),
    }
}

/// `fields.editLink`: change the link `index` (as `fields.links` numbers them). Every setting
/// given applies, in one undo step; a bad parameter or index changes nothing.
pub(super) fn edit_link(s: &mut Session, v: &Value) -> CmdResult {
    let index = p::u64(v, "index").ok_or_else(|| CmdError::Params("`index` (number) is required".into()))?;
    let auto = match p::str(v, "update") {
        None if v.get("update").is_none_or(Value::is_null) => None,
        Some("auto") => Some(true),
        Some("manual") => Some(false),
        _ => return Err(CmdError::Params("`update` must be \"auto\" or \"manual\"".into())),
    };
    let locked = flag(v, "locked")?;
    let merge = flag(v, "preserveFormatting")?;
    let source = code_text(v, "source")?;
    if source.as_deref().is_some_and(str::is_empty) {
        return Err(CmdError::Params("`source` must name a file".into()));
    }
    let item = code_text(v, "item")?;
    let action = match p::str(v, "action") {
        None if v.get("action").is_none_or(Value::is_null) => None,
        Some(a @ ("updateNow" | "breakLink")) => Some(a),
        _ => return Err(CmdError::Params("`action` must be \"updateNow\" or \"breakLink\"".into())),
    };
    let all = link_fields(s);
    let f = usize::try_from(index).ok().and_then(|i| all.get(i)).cloned();
    let Some(f) = f else { return Err(CmdError::Params(format!("there is no link {index} (the document has {})", all.len()))) };

    let mut code = f.instr.clone();
    if let Some(auto) = auto {
        code = set_auto(&code, auto);
    }
    if let Some(merge) = merge {
        code = set_merge(&code, merge);
    }
    if let Some(source) = &source {
        code = set_arg(&code, 1, source)?;
    }
    if let Some(item) = &item {
        code = set_arg(&code, 2, item)?;
    }
    if code.len() > MAX_CODE {
        return Err(CmdError::Params("the field code would be too long".into()));
    }
    if action == Some("breakLink") {
        super::unlink_fields(s, std::slice::from_ref(&f))?;
        return Ok(json!({"index": index, "broken": true}));
    }
    let lock = locked.unwrap_or(f.locked);
    with_field(s, &f.at, |o| {
        if let InlineObject::Field { instr, locked, .. } | InlineObject::FieldStart { instr, locked, .. } = o {
            instr.clone_from(&code);
            *locked = lock;
        }
    })?;
    // Change Source shows the new source's text, as Update Now does; a locked link keeps its result.
    let update = action == Some("updateNow") || source.is_some() || item.is_some();
    if update && !lock {
        update_link_at(s, &f.at)?;
    }
    let after = FieldAt { instr: code, locked: lock, ..f };
    Ok(link_json(usize::try_from(index).unwrap_or(0), &after))
}

/// `fields.updateLinks`: Update Now on every link `fields.links` lists, in document order. A
/// locked link keeps its result; a link whose source can't be read keeps its stored result. Each
/// link is found again after every update, since an update can move the links after it.
pub(super) fn update_links(s: &mut Session, _: &Value) -> CmdResult {
    let count = link_fields(s).len();
    let mut updated = 0usize;
    for i in 0..count {
        let Some(f) = link_fields(s).get(i).cloned() else { break };
        if f.locked {
            continue;
        }
        update_link_at(s, &f.at)?;
        updated = updated.saturating_add(1);
    }
    Ok(json!({"updated": updated}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_switch_comes_and_goes_without_touching_the_rest() {
        let code = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \t"#;
        let auto = set_auto(code, true);
        assert_eq!(auto, r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#);
        assert_eq!(set_auto(&auto, false), code);
        assert_eq!(set_auto(&auto, true), auto);
    }

    #[test]
    fn mergeformat_is_added_at_the_end_and_only_it_is_removed() {
        let code = r#"LINK Word.Document.12 "a.docx" "" \t \* Upper"#;
        let merged = set_merge(code, true);
        assert_eq!(merged, r#"LINK Word.Document.12 "a.docx" "" \t \* Upper \* MERGEFORMAT"#);
        assert_eq!(set_merge(&merged, false), code);
    }

    #[test]
    fn source_and_item_are_rewritten_or_added() {
        let code = r#"LINK Word.Document.12 "C:\\d\\a.docx" \a \t"#;
        let moved = set_arg(code, 1, r"D:\new place\b.docx").unwrap();
        assert_eq!(moved, r#"LINK Word.Document.12 "D:\\new place\\b.docx" \a \t"#);
        let marked = set_arg(&moved, 2, "OLE_LINK3").unwrap();
        assert_eq!(marked, r#"LINK Word.Document.12 "D:\\new place\\b.docx" "OLE_LINK3" \a \t"#);
        assert_eq!(set_arg(&marked, 2, "").unwrap(), moved);
        assert!(set_arg("LINK", 2, "x").is_err());
    }

    #[test]
    fn hostile_codes_split_without_panicking() {
        for code in ["", "\"", "LINK \"", "LINK \\", "LINK é\"ü\" \\a \\* \\", "\\\\\\"] {
            let _ = set_auto(code, true);
            let _ = set_auto(code, false);
            let _ = set_merge(code, true);
            let _ = set_merge(code, false);
            let _ = set_arg(code, 1, "x");
            let _ = set_arg(code, 2, "");
        }
    }
}
