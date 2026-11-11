use super::{CharProps, CmdError, CmdResult, InlineObject, Pos, Selection, Session, StoryRef, Value, json, p, sel_result};

/// What a link can point to inside the document (Insert › Link › Place in This Document): the top
/// of the document, its headings (body block, level, text) and its bookmarks, without the hidden
/// ones (names starting with `_`), plus distinct body e-mail link targets in document order, and
/// the ScreenTip of the link at the selection (`screenTip`, "" when none).
pub(super) fn link_targets(s: &mut Session, _: &Value) -> CmdResult {
    let headings: Vec<Value> = crate::cmd::references::headings(s)
        .into_iter()
        .map(|(block, level, text, _)| json!({"block": block, "level": level, "text": text}))
        .collect();
    let mut bookmarks: Vec<String> = Vec::new();
    for (name, _) in s.doc.bookmarks() {
        if !name.starts_with('_') && !bookmarks.contains(&name) {
            bookmarks.push(name);
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut emails = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        let Some(para) = s.doc.para(StoryRef::Body, &path) else { continue };
        for (_, props) in para.run_ranges() {
            let Some(target) = props.link.as_deref() else { continue };
            let Some(rest) = mailto_address(target) else { continue };
            if !seen.insert(target) {
                continue;
            }
            let (address, headers) = rest.split_once('?').unwrap_or((rest, ""));
            let subject = headers.split('&').filter_map(|h| h.split_once('=')).find(|(name, _)| name.eq_ignore_ascii_case("subject"));
            emails.push(json!({"address": address, "subject": subject.map(|(_, value)| decode_subject(value)).unwrap_or_default()}));
        }
    }
    Ok(json!({"top": true, "headings": headings, "bookmarks": bookmarks, "emails": emails, "screenTip": selected_link_tip(s)}))
}

/// Accept the URI scheme case-insensitively, without slicing inside a UTF-8 character.
fn mailto_address(value: &str) -> Option<&str> {
    value.get(..7).filter(|prefix| prefix.eq_ignore_ascii_case("mailto:"))?;
    value.get(7..)
}

fn decode_subject(value: &str) -> String {
    let mut bytes = value.bytes();
    let mut decoded = Vec::with_capacity(value.len());
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let mut next = bytes.clone();
            if let (Some(a), Some(b)) = (next.next(), next.next())
                && let (Some(a), Some(b)) = ((a as char).to_digit(16), (b as char).to_digit(16))
            {
                decoded.push((a * 16 + b) as u8);
                bytes = next;
                continue;
            }
        }
        decoded.push(byte);
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn email_target(email: &str, subject: &str) -> Result<String, CmdError> {
    let email = email.trim();
    let address = mailto_address(email).unwrap_or(email);
    if address.trim().is_empty() || address.len() > 4089 {
        return Err(CmdError::Params("bad email address".into()));
    }
    let mut target = format!("mailto:{address}");
    if !subject.is_empty() {
        target.push_str("?subject=");
        for byte in subject.bytes() {
            // Bound the encoded URI as it grows, even for a hostile oversized subject.
            if target.len() > 4096 {
                return Err(CmdError::Params("bad url".into()));
            }
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                target.push(byte as char);
            } else {
                use std::fmt::Write;
                let _ = write!(target, "%{byte:02X}");
            }
        }
    }
    Ok(target)
}

/// The address and the default text of a link to a place in this document, from `top`, `bookmark`
/// or `heading` (a heading gets a hidden `_Toc` bookmark, as in Word); None when none is given.
fn place_target(s: &mut Session, v: &Value) -> Result<Option<(String, String)>, CmdError> {
    if p::bool(v, "top") == Some(true) {
        return Ok(Some(("#_top".into(), "Top of the Document".into())));
    }
    if let Some(name) = p::str(v, "bookmark") {
        if !s.doc.bookmarks().iter().any(|(n, _)| n == name) {
            return Err(CmdError::Params(format!("no bookmark `{name}`")));
        }
        return Ok(Some((format!("#{name}"), name.to_string())));
    }
    if let Some(block) = p::u64(v, "heading") {
        let Some((i, _, text, lead)) = crate::cmd::references::headings(s).into_iter().find(|h| h.0 as u64 == block) else {
            return Err(CmdError::Params(format!("block {block} is not a heading")));
        };
        let name = crate::cmd::references::toc_bookmark(s, i, lead)?;
        return Ok(Some((format!("#{name}"), text)));
    }
    Ok(None)
}

pub(super) fn link(s: &mut Session, v: &Value) -> CmdResult {
    let (url, label) = match p::str(v, "url") {
        Some(url) => (url.to_string(), url.to_string()),
        None if p::str(v, "email").is_some() => {
            let target = email_target(p::req_str(v, "email")?, p::str(v, "subject").unwrap_or_default())?;
            (target.clone(), target)
        }
        None => match place_target(s, v)? {
            Some(t) => t,
            None => {
                s.push_ui_request(json!({"open": "link"}));
                return sel_result(s);
            }
        },
    };
    if url.is_empty() || url.len() > 4096 {
        return Err(CmdError::Params("bad url".into()));
    }
    let tip = screen_tip(v)?;
    let style = Some("Hyperlink".to_string());
    if s.sel.is_collapsed() {
        let text = p::str(v, "text").unwrap_or(&label).to_string();
        let mut props = s.typing_props();
        props.link = Some(url.clone());
        // A new link has only the ScreenTip it is given.
        props.link_tip = tip.clone().flatten();
        props.style = style;
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &text, &props)?;
        s.sel = Selection::caret(end);
        s.pending = Some(CharProps::default());
    } else if let Some(text) = retyped_text(s, v) {
        // Text to display changed: the selection is retyped with it, then linked (as in Word).
        crate::cmd::paste_text(s, &text)?;
        let end = s.sel.focus.clone();
        let start = Pos { off: end.off.saturating_sub(text.len()), ..end.clone() };
        s.doc.format_range(&start, &end, &|c| {
            c.link = Some(url.clone());
            if let Some(tip) = &tip {
                c.link_tip = tip.clone();
            }
            c.style = style.clone();
        })?;
        s.sel = Selection::caret(end);
        s.pending = Some(CharProps::default());
    } else {
        let (a, b) = s.sel.ordered();
        s.doc.format_range(&a, &b, &|c| {
            c.link = Some(url.clone());
            if let Some(tip) = &tip {
                c.link_tip = tip.clone();
            }
            c.style = style.clone();
        })?;
    }
    sel_result(s)
}

/// Word's ScreenTip box holds at most this many characters.
pub const MAX_SCREEN_TIP: usize = 255;

/// The `screenTip` param of `insert.link`: absent (`None`: a selected link keeps its tip), empty
/// (`Some(None)`: the tip is cleared) or the text to show on hover. Longer than
/// [`MAX_SCREEN_TIP`] characters, holding a control character or not a string: refused.
fn screen_tip(v: &Value) -> Result<Option<Option<String>>, CmdError> {
    let Some(tip) = v.get("screenTip") else { return Ok(None) };
    let bad = || CmdError::Params("bad screenTip".into());
    let tip = tip.as_str().ok_or_else(bad)?;
    if tip.chars().count() > MAX_SCREEN_TIP || tip.chars().any(char::is_control) {
        return Err(bad());
    }
    Ok(Some((!tip.is_empty()).then(|| tip.to_string())))
}

/// The ScreenTip of the link at the selection (the character at its start, or just before a
/// caret at a link's end), "" when it has none: what Insert Hyperlink shows when opened on a link.
pub fn selected_link_tip(s: &Session) -> String {
    let (a, _) = s.sel.ordered();
    let Some(para) = s.doc.para_at(&a) else { return String::new() };
    [Some(a.off).filter(|o| *o < para.len()), (a.off > 0).then(|| para.prev_boundary(a.off))]
        .into_iter()
        .flatten()
        .map(|off| para.props_of_char(off))
        .find(|c| c.link.is_some())
        .and_then(|c| c.link_tip.clone())
        .unwrap_or_default()
}

/// The `text` to retype a selection with before linking it: only when it differs from the selected
/// text and both fit in one line of one paragraph without pictures or fields, outside any field's
/// result. Otherwise (as Word shows `<<Selection in Document>>`) the selection keeps its text.
fn retyped_text(s: &Session, v: &Value) -> Option<String> {
    use wordcraft_doc::para::{COLUMN_BREAK, LINE_BREAK, OBJ, PAGE_BREAK};
    /// What can't stand in one line of text: breaks, and pictures or fields.
    const NOT_ONE_LINE: [char; 7] = [LINE_BREAK, '\r', '\u{2029}', '\u{b}', PAGE_BREAK, COLUMN_BREAK, OBJ];
    let text = p::str(v, "text").filter(|t| !t.is_empty() && t.len() <= 4096)?;
    if text.contains(NOT_ONE_LINE) {
        return None;
    }
    let (a, b) = s.sel.ordered();
    if a.story != b.story || a.path != b.path {
        return None;
    }
    let shown = s.doc.para(a.story, &a.path)?.text.get(a.off..b.off)?;
    if shown.contains(NOT_ONE_LINE) || shown == text {
        return None;
    }
    // A selection in a field's result (between its markers, which it doesn't hold) is retyped by
    // the next update of the field, not here.
    if s.doc.field_ranges(a.story).iter().any(|r| r.start < b && a <= r.end) {
        return None;
    }
    Some(text.to_string())
}

/// Whether `name` can name a bookmark: starts with a letter or underscore, contains only alphanumeric characters
/// or underscores, and is at most 40 characters (#3491). Word allows leading underscores for hidden bookmarks (_Toc, _Ref).
pub fn bookmark_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_') && name.chars().count() <= 40
}

/// Where bookmark `name` is: see [`bookmark_span`]. `None` when there is no such bookmark.
pub fn bookmark_range(doc: &wordcraft_doc::Document, name: &str) -> Option<(Pos, Pos)> {
    let (_, start) = doc.bookmarks().into_iter().find(|(n, _)| n == name)?;
    Some(bookmark_span(doc, name, &start))
}

/// The text of bookmark `name` whose start mark is at `start`: from just after that mark to its
/// end mark — in a later paragraph when it spans several (the start again when it has no end).
pub fn bookmark_span(doc: &wordcraft_doc::Document, name: &str, start: &Pos) -> (Pos, Pos) {
    let from = Pos { off: start.off.saturating_add(wordcraft_doc::para::OBJ.len_utf8()), ..start.clone() };
    let end = doc.para_paths(start.story).into_iter().skip_while(|p| *p != start.path).find_map(|path| {
        let para = doc.para(start.story, &path)?;
        let off = para.object_offsets().into_iter().find(|o| {
            (path != start.path || *o > start.off) && matches!(para.object_at(*o), Some(InlineObject::BookmarkEnd { name: n }) if n == name)
        })?;
        Some(Pos { story: start.story, path, off })
    });
    let end = end.unwrap_or_else(|| from.clone());
    (from, end)
}

/// Remove bookmark `name`'s start and end marks, in whichever paragraphs and stories they are,
/// keeping the selection on the same text. Returns whether the bookmark was there.
fn remove_bookmark(s: &mut Session, name: &str) -> Result<bool, CmdError> {
    let mark_len = wordcraft_doc::para::OBJ.len_utf8();
    let mut found = false;
    let stories: Vec<StoryRef> = std::iter::once(StoryRef::Body).chain(s.doc.parts.keys().map(|k| StoryRef::Part(*k))).collect();
    for story in stories {
        for path in s.doc.para_paths(story) {
            let Some(p) = s.doc.para(story, &path) else { continue };
            let mut marks = Vec::new();
            for off in p.object_offsets() {
                match p.object_at(off) {
                    Some(InlineObject::BookmarkStart { name: n }) if n == name => {
                        found = true;
                        marks.push(off);
                    }
                    Some(InlineObject::BookmarkEnd { name: n }) if n == name => marks.push(off),
                    _ => {}
                }
            }
            if marks.is_empty() {
                continue;
            }
            let para = s.doc.para_mut(story, &path)?;
            for off in marks.into_iter().rev() {
                para.delete(off, off + mark_len)?;
                for at in [&mut s.sel.anchor, &mut s.sel.focus] {
                    if at.story == story && at.path == path && at.off > off {
                        at.off = at.off.saturating_sub(mark_len);
                    }
                }
            }
        }
    }
    s.clamp_selection();
    Ok(found)
}

pub(super) fn delete_bookmark(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?.trim();
    if !remove_bookmark(s, name)? {
        return Err(CmdError::Params(format!("no bookmark `{name}`")));
    }
    sel_result(s)
}

pub(super) fn bookmark(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?.trim().to_string();
    if !bookmark_name_ok(&name) {
        return Err(CmdError::Params("bookmark names start with a letter, have no spaces and at most 40 characters".into()));
    }
    // Replace an existing bookmark of the same name.
    remove_bookmark(s, &name)?;
    let (a, b) = s.sel.ordered();
    let props = CharProps::default();
    let end = s.doc.insert_object(&b, InlineObject::BookmarkEnd { name: name.clone() }, &props)?;
    s.doc.insert_object(&a, InlineObject::BookmarkStart { name: name.clone() }, &props)?;
    let _ = end;
    if !name.starts_with('_') {
        s.last_bookmark = Some(name);
    }
    sel_result(s)
}

#[cfg(test)]
mod email_tests {
    use super::*;

    #[test]
    fn email_target_bounds_and_unreserved_bytes() {
        assert_eq!(email_target("a@b.c", "AZaz09-._~").unwrap(), "mailto:a@b.c?subject=AZaz09-._~");
        assert!(email_target("MAILTO:   ", "").is_err());
        assert_eq!(email_target(&"a".repeat(4089), "").unwrap().len(), 4096);
        assert!(email_target(&"a".repeat(4090), "").is_err());
        let mut s = Session::new(wordcraft_doc::Document::from_text("selected"));
        s.run("select.all", &json!({})).unwrap();
        let before = s.doc.clone();
        assert!(s.run("insert.link", &json!({"email": "a@b.c", "subject": "é".repeat(4096)})).is_err());
        assert_eq!(s.doc, before);
        s.run("insert.link", &json!({"email": "a@b.c", "subject": "Hi"})).unwrap();
        let para = s.doc.body.first().and_then(|b| b.as_para()).unwrap();
        assert_eq!(para.text, "selected");
        assert!(para.run_ranges().all(|(_, props)| props.link.as_deref() == Some("mailto:a@b.c?subject=Hi")));
    }

    #[test]
    fn malformed_percent_escapes_and_plus_stay_literal() {
        assert_eq!(decode_subject("%4%41%+%a%"), "%4A%+%a%");
        assert_eq!(decode_subject("a+b%2Bc"), "a+b+c");
        assert_eq!(mailto_address("éééé"), None);
    }
}
