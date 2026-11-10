//! Styles pane › Options… (Style Pane Options): which styles the pane shows and how it sorts
//! them, kept in the settings part (`w:stylePaneFormatFilter`, `w:stylePaneSortMethod`).

use std::collections::HashSet;

use wordcraft_doc::{PANE_ALL_STYLES, PANE_IN_USE, StoryRef, StylePaneOptions, StylePaneSort};

use super::{CmdError, CmdResult, Session, Style, StyleKind, Value, json, p};

/// "By type": the order of the pane's style types (Word's own order isn't measured).
const TYPE_ORDER: [&str; 3] = ["paragraph", "linked", "character"];

/// Deepest `based_on` chain followed for a style's inherited font.
const MAX_BASED_ON: usize = 32;

/// The sort a `sort` parameter names.
fn sort_of(name: &str) -> Option<StylePaneSort> {
    Some(match name {
        "alphabetical" => StylePaneSort::Name,
        "recommended" => StylePaneSort::Priority,
        "font" => StylePaneSort::Font,
        "basedOn" => StylePaneSort::BasedOn,
        "type" => StylePaneSort::Type,
        _ => return None,
    })
}

/// A sort's parameter name.
fn sort_name(sort: Option<StylePaneSort>) -> &'static str {
    match sort {
        None | Some(StylePaneSort::Name) => "alphabetical",
        Some(StylePaneSort::Priority | StylePaneSort::Default) => "recommended",
        Some(StylePaneSort::Font) => "font",
        Some(StylePaneSort::BasedOn) => "basedOn",
        Some(StylePaneSort::Type) => "type",
    }
}

/// Whether the pane shows only the styles in use.
fn in_use_only(pane: Option<&StylePaneOptions>) -> bool {
    pane.is_some_and(|o| o.filter & PANE_IN_USE != 0 && o.filter & PANE_ALL_STYLES == 0)
}

fn state(s: &Session) -> Value {
    let pane = s.doc.settings.style_pane.as_ref();
    json!({"show": if in_use_only(pane) { "inUse" } else { "all" }, "sort": sort_name(pane.and_then(|o| o.sort))})
}

/// `styles.paneOptions`: change the pane's options (none given: a read) and return them.
pub(super) fn pane_options(s: &mut Session, v: &Value) -> CmdResult {
    let show = match p::str(v, "show") {
        None => None,
        Some("inUse") => Some(true),
        Some("all") => Some(false),
        Some(x) => return Err(CmdError::Params(format!("unknown show `{x}` (inUse|all)"))),
    };
    let sort = match p::str(v, "sort") {
        None => None,
        Some(x) => Some(sort_of(x).ok_or_else(|| CmdError::Params(format!("unknown sort `{x}` (alphabetical|recommended|font|basedOn|type)")))?),
    };
    if show.is_none() && sort.is_none() {
        return Ok(state(s));
    }
    let pane = s.doc.settings.style_pane.get_or_insert(StylePaneOptions { filter: 0, filter_attrs: true, sort: None });
    match show {
        Some(true) => pane.filter = (pane.filter | PANE_IN_USE) & !PANE_ALL_STYLES,
        Some(false) => pane.filter = (pane.filter | PANE_ALL_STYLES) & !PANE_IN_USE,
        None => {}
    }
    if sort.is_some() {
        pane.sort = sort;
    }
    Ok(state(s))
}

/// The ids of the styles applied somewhere: paragraph styles and run character styles in every
/// story, and the style of the paragraph at the caret.
fn styles_in_use(s: &Session) -> HashSet<String> {
    let doc = &s.doc;
    let para_style = |st: Option<&String>| st.cloned().unwrap_or_else(|| "Normal".into());
    let mut used = HashSet::new();
    for story in std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|k| StoryRef::Part(*k))) {
        for path in doc.para_paths(story) {
            let Some(para) = doc.para(story, &path) else { continue };
            used.insert(para_style(para.props.style.as_ref()));
            used.extend(para.runs.iter().filter_map(|r| r.props.style.clone()));
        }
    }
    if let Some(para) = doc.para_at(&s.sel.focus) {
        used.insert(para_style(para.props.style.as_ref()));
    }
    used
}

/// A style's own font, else the one it inherits through `based_on`, else empty.
fn font_of(sheet: &wordcraft_doc::StyleSheet, st: &Style) -> String {
    let mut cur = Some(st);
    for _ in 0..MAX_BASED_ON {
        let Some(x) = cur else { break };
        if let Some(f) = &x.chr.font {
            return f.to_lowercase();
        }
        cur = x.based_on.as_deref().and_then(|b| sheet.get(b));
    }
    String::new()
}

/// `styles.paneList`: the styles the pane lists, in its order.
pub(super) fn pane_list(s: &mut Session, _: &Value) -> CmdResult {
    let pane = s.doc.settings.style_pane.as_ref();
    let used = in_use_only(pane).then(|| styles_in_use(s));
    let sort = pane.and_then(|o| o.sort).unwrap_or(StylePaneSort::Name);
    // Manage Styles' rules: no numbering styles, a linked pair once as its paragraph half.
    let all = super::manage_list(s, true);
    let sheet = &s.doc.styles;
    let mut list: Vec<(&Style, &str)> = all
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| {
            let ty = match x.get("type").and_then(Value::as_str)? {
                "paragraph" => "paragraph",
                "linked" => "linked",
                "character" => "character",
                _ => return None,
            };
            let st = sheet.get(x.get("id").and_then(Value::as_str)?)?;
            (!st.hidden).then_some((st, ty))
        })
        .filter(|(st, _)| {
            used.as_ref().is_none_or(|u| u.contains(&st.id) || (st.kind == StyleKind::Paragraph && st.linked.as_ref().is_some_and(|l| u.contains(l))))
        })
        .collect();
    let name = |st: &Style| st.name.to_lowercase();
    match sort {
        StylePaneSort::Name => list.sort_by_cached_key(|(st, _)| name(st)),
        StylePaneSort::Priority | StylePaneSort::Default => list.sort_by_cached_key(|(st, _)| (st.priority.unwrap_or(99), name(st))),
        StylePaneSort::Font => list.sort_by_cached_key(|(st, _)| (font_of(sheet, st), name(st))),
        StylePaneSort::BasedOn => {
            list.sort_by_cached_key(|(st, _)| (st.based_on.as_deref().and_then(|b| sheet.get(b)).map(|b| b.name.to_lowercase()), name(st)))
        }
        StylePaneSort::Type => list.sort_by_cached_key(|(st, ty)| (TYPE_ORDER.iter().position(|t| t == ty).unwrap_or(TYPE_ORDER.len()), name(st))),
    }
    Ok(Value::Array(list.into_iter().map(|(st, ty)| json!({"id": st.id, "name": st.name, "type": ty})).collect()))
}
