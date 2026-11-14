use super::cross_refs::{caption_label, is_caption};
use super::render::entry;
use super::sources::bib_order;
use super::styles::style;
use super::{
    Block, CharProps, CmdError, CmdResult, Document, FieldCode, InlineObject, Paragraph, Path, Pos, Session, SortKey, StoryRef, TabAlign, TabLeader,
    TabStop, Value, delete_selection, heading_letter, p, sel_result, sort_key,
};
use crate::cmd::references::{bookmark_heading, page_at, page_formats, page_text};
use std::sync::Arc;
use wordcraft_doc::section::NumFormat;

/// The TOC field for `references.tableOfFigures` (ECMA-376 §17.16.5.68): `\c "<label>"` lists the
/// captions that label's SEQ field numbers, `\a "<label>"` lists them without their label and
/// number; `\n` leaves page numbers out and `\p " "` puts them right after the text. With
/// `style` (`\t "<style>"`) and/or `entryFields` (`\f <ID>`), Table of Figures Options, it lists
/// that style's paragraphs and the TC fields with that identifier instead, a bare `\c` marking it a
/// table of figures.
pub fn figures_instr(v: &Value) -> String {
    let on = |k: &str| p::bool(v, k).unwrap_or(true);
    let links = if on("hyperlinks") { " \\h \\z" } else { "" };
    let style = p::str(v, "style").map(figures_style_name).filter(|n| !n.is_empty());
    let entry = p::str(v, "entryFields").and_then(entry_identifier);
    let mut s = if style.is_some() || entry.is_some() {
        let mut s = format!("TOC{links}");
        if let Some(style) = style {
            s.push_str(&format!(" \\t \"{style}\""));
        }
        if let Some(id) = entry {
            s.push_str(&format!(" \\f {id}"));
        }
        s.push_str(" \\c");
        s
    } else {
        let label: String = p::str(v, "label").unwrap_or("Figure").chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).take(40).collect();
        let ident = seq_ident(&label);
        let ident = if ident.is_empty() { "Figure".to_string() } else { ident };
        format!("TOC{links} \\{} \"{ident}\"", if on("includeLabel") { 'c' } else { 'a' })
    };
    if !on("pageNumbers") {
        s.push_str(" \\n");
    } else if !on("rightAlign") {
        s.push_str(" \\p \" \"");
    }
    s
}

/// A style name as `\t` holds it: no quotes, backslashes or control characters, at most 253
/// characters.
fn figures_style_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).take(253).collect::<String>().trim().to_string()
}

/// A table entry field identifier: one ASCII letter, upper case.
fn entry_identifier(id: &str) -> Option<char> {
    let mut c = id.chars();
    match (c.next(), c.next()) {
        (Some(l), None) if l.is_ascii_alphabetic() => Some(l.to_ascii_uppercase()),
        _ => None,
    }
}

/// The SEQ identifier of a caption label: Word joins a multi-word label with underscores.
fn seq_ident(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join("_")
}

pub(super) fn table_of_figures(s: &mut Session, v: &Value) -> CmdResult {
    let mut v = v.clone();
    if let Some(id) = p::str(&v, "entryFields")
        && entry_identifier(id).is_none()
    {
        return Err(CmdError::Params(format!("`entryFields` must be one letter A-Z, not `{id}`")));
    }
    if let Some(name) = p::str(&v, "style") {
        let st = s
            .doc
            .styles
            .find(&figures_style_name(name))
            .filter(|st| st.kind == wordcraft_doc::StyleKind::Paragraph)
            .ok_or_else(|| CmdError::Params(format!("unknown style `{name}`")))?;
        let shown = figures_style_name(&st.name);
        if let Some(o) = v.as_object_mut() {
            o.insert("style".into(), Value::String(shown));
        }
    }
    generated_list_with(s, &figures_instr(&v), "", p::str(&v, "tabLeader").map(crate::cmd::references::leader))?;
    sel_result(s)
}

/// Generated lists (BIBLIOGRAPHY, INDEX, table of figures, TOA): a heading paragraph holding
/// the field, then entry paragraphs marked with the "GeneratedEntry" style family.
pub(crate) fn generated_list(s: &mut Session, instr: &str, title: &str) -> Result<(), CmdError> {
    generated_list_with(s, instr, title, None)
}

/// [`generated_list`] whose entries take `leader`. The leader lives in the entries' tab stops (as
/// in a table of contents) and an update keeps the first entry's, so a placeholder entry carries
/// it through the rebuild, however the lists above it change.
fn generated_list_with(s: &mut Session, instr: &str, title: &str, leader: Option<TabLeader>) -> Result<(), CmdError> {
    let at = delete_selection(s)?;
    if at.story != StoryRef::Body || at.path.depth() > 0 {
        return Err(CmdError::Failed("lists go in the main text".into()));
    }
    let mut head = if title.is_empty() { Paragraph::new() } else { Paragraph::with_text(title, CharProps::default()).styled("Heading1") };
    let end = head.len();
    head.insert_object(
        end,
        InlineObject::Field {
            instr: instr.to_string(),
            result: String::new(),
            locked: false,
            dirty: false,
            code: false,
            form: String::new(),
            code_paras: None,
        },
        &CharProps::default(),
    )?;
    let i = if s.doc.para_at(&at).is_some_and(|p| p.is_empty()) { at.path.last() } else { s.doc.split_paragraph(&at)?.path.last() };
    s.doc.insert_block(StoryRef::Body, &Path::top(i), Block::Para(head))?;
    if let Some(leader) = leader {
        let mut seed = Paragraph::new().styled(ENTRY_STYLE);
        seed.props.tabs = Some(vec![TabStop { pos: 0.0, align: TabAlign::Right, leader }]);
        s.doc.insert_block(StoryRef::Body, &Path::top(i.saturating_add(1)), Block::Para(seed))?;
    }
    update_generated(s)
}

/// Most paragraphs a list held as a field around its entries spans.
const MAX_WRAPPED: usize = 10_000;

/// A generated list (index, bibliography, table of figures or authorities) held as a complex
/// field that starts at body block `i` and runs over its entries: its field code and the block
/// it ends in. `None` for anything else (a TOC has its own update).
fn wrapped_list(s: &Session, i: usize) -> Option<(String, usize, usize, usize)> {
    let p = s.doc.body.get(i)?.as_para()?;
    let (k, off, instr) = p.objects_with_offsets().enumerate().find_map(|(k, (off, o))| match o {
        InlineObject::FieldStart { instr, .. } => {
            let t = instr.trim_start();
            (t.starts_with("BIBLIOGRAPHY") || t.starts_with("INDEX") || t.starts_with("TOA") || figures_of(t).is_some())
                .then(|| (k, off, t.to_string()))
        }
        _ => None,
    })?;
    // Only when it starts the paragraph: text before it is the document's, not the list's.
    if !p.text.get(..off).is_some_and(|t| t.chars().all(|c| c.is_whitespace() || c == wordcraft_doc::para::OBJ)) {
        return None;
    }
    // Where it ends: the field end that closes it, nested fields counted from its own start (a
    // field before it in its paragraph, an empty `PAGE`…, closes before it and isn't it).
    let mut depth = 0usize;
    for (j, b) in s.doc.body.iter().enumerate().skip(i).take(MAX_WRAPPED) {
        let Some(q) = b.as_para() else { continue };
        for (at, o) in q.objects_with_offsets().skip(if j == i { k } else { 0 }) {
            match o {
                InlineObject::FieldStart { .. } => depth += 1,
                InlineObject::FieldEnd => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some((instr, off, j, at + wordcraft_doc::para::OBJ.len_utf8()));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn list_kind(p: &Paragraph) -> Option<String> {
    p.objects.iter().find_map(list_field)
}

/// The field code of `o` when it is a generated list's field.
fn list_field(o: &InlineObject) -> Option<String> {
    match o {
        InlineObject::Field { instr, .. } => {
            let t = instr.trim_start();
            (t.starts_with("BIBLIOGRAPHY") || t.starts_with("INDEX") || t.starts_with("TOA") || figures_of(t).is_some()).then(|| t.to_string())
        }
        _ => None,
    }
}

/// What a table-of-figures field (`TOC \c` or `\a`) lists.
#[derive(Clone, Debug, PartialEq)]
enum Figures {
    /// The captions a caption label's SEQ identifier numbers.
    Captions(String),
    /// Paragraphs in the `\t` styles and `TC` fields with the `\f` identifier (Table of Figures
    /// Options): a bare `\c` then lists no captions.
    Marked { styles: Vec<String>, entries: Option<String> },
}

/// Most style names a `\t` list is read for.
const MAX_STYLES: usize = 64;

/// What table-of-figures field `instr` lists; `None` for anything else.
fn figures_of(instr: &str) -> Option<Figures> {
    let f = FieldCode::parse(instr);
    if !f.is_table_of_figures() {
        return None;
    }
    if f.has('t') || f.has('f') {
        // `\t "Style,Level,Style,Level"`: every second item is a level, which a flat list ignores.
        let styles = f
            .arg('t')
            .map(|list| list.split(',').step_by(2).map(str::trim).filter(|n| !n.is_empty()).take(MAX_STYLES).map(str::to_string).collect())
            .unwrap_or_default();
        // A TC field without `\f` has identifier C, as does a bare `\f`.
        let entries = f.has('f').then(|| f.arg('f').unwrap_or("C").to_string());
        return Some(Figures::Marked { styles, entries });
    }
    Some(Figures::Captions(f.arg('c').or_else(|| f.arg('a')).map_or_else(|| "Figure".to_string(), seq_ident)))
}

/// The caption label (SEQ identifier) a caption table of figures lists, `\c` or `\a`; `None`
/// for a list built from styles or table entry fields.
#[cfg(test)]
fn figures_label(instr: &str) -> Option<String> {
    match figures_of(instr)? {
        Figures::Captions(ident) => Some(ident),
        Figures::Marked { .. } => None,
    }
}

/// Whether paragraph `p`'s style is one of `styles` (display name or id, any case).
fn styled_as(doc: &Document, p: &Paragraph, styles: &[String]) -> bool {
    let id = p.props.style.as_deref().unwrap_or("Normal");
    let name = doc.styles.get(id).map(|st| st.name.as_str());
    styles.iter().any(|w| w.eq_ignore_ascii_case(id) || name.is_some_and(|n| w.eq_ignore_ascii_case(n)))
}

/// The entries a table of figures lists, in document order: (text, position).
fn figure_marks(doc: &Document, figures: &Figures, bare: bool) -> Vec<(String, Pos)> {
    let mut marks = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        match figures {
            Figures::Captions(ident) => {
                if captioned_as(doc, p, ident) {
                    let text = if bare { caption_body(p, &ident.replace('_', " ")) } else { p.plain_text() };
                    marks.push((text, Pos { story: StoryRef::Body, path: path.clone(), off: 0 }));
                }
            }
            Figures::Marked { styles, entries } => {
                // The list's own paragraphs are never listed.
                if p.props.style.as_deref() == Some(ENTRY_STYLE) || list_kind(p).is_some() {
                    continue;
                }
                if styled_as(doc, p, styles) {
                    marks.push((p.plain_text(), Pos { story: StoryRef::Body, path: path.clone(), off: 0 }));
                }
                let Some(want) = entries else { continue };
                for off in p.object_offsets() {
                    let instr = match p.object_at(off) {
                        Some(InlineObject::Field { instr, .. } | InlineObject::FieldStart { instr, .. }) => instr,
                        _ => continue,
                    };
                    let tc = FieldCode::parse(instr);
                    if tc.name != "TC" || !tc.arg('f').unwrap_or("C").eq_ignore_ascii_case(want) {
                        continue;
                    }
                    let text = tc.args.first().cloned().unwrap_or_default();
                    marks.push((text, Pos { story: StoryRef::Body, path: path.clone(), off }));
                }
            }
        }
    }
    marks
}

/// Whether `p` is a caption numbered by SEQ identifier `ident` (any case, as Word matches it). A
/// caption typed by hand, without a SEQ field, goes by its label.
fn captioned_as(doc: &Document, p: &Paragraph, ident: &str) -> bool {
    if !is_caption(doc, p) {
        return false;
    }
    let seq = p.objects.iter().find_map(|o| match o {
        InlineObject::Field { instr, .. } => Some(FieldCode::parse(instr)).filter(|f| f.name == "SEQ"),
        _ => None,
    });
    match seq {
        Some(f) => f.args.first().is_some_and(|id| id.eq_ignore_ascii_case(ident)),
        None => caption_label(p, &p.plain_text()).is_some_and(|l| seq_ident(&l).eq_ignore_ascii_case(ident)),
    }
}

/// A caption's text without its label and number (`\a`): what follows its SEQ field, or for a
/// caption typed by hand what follows the label and the number, less the separator.
fn caption_body(p: &Paragraph, label: &str) -> String {
    let seq_end = p.object_offsets().into_iter().find_map(|o| match p.object_at(o) {
        Some(InlineObject::Field { instr, .. }) if FieldCode::parse(instr).name == "SEQ" => Some(o + wordcraft_doc::para::OBJ.len_utf8()),
        _ => None,
    });
    let rest = match seq_end {
        Some(end) => {
            let mut k = 0;
            let mut rest = String::new();
            for (at, c) in p.text.char_indices() {
                if c == wordcraft_doc::para::OBJ {
                    if at >= end
                        && let Some(o) = p.objects.get(k)
                    {
                        rest.push_str(o.plain_text());
                    }
                    k += 1;
                } else if at >= end {
                    rest.push(c);
                }
            }
            rest
        }
        None => {
            let text = p.plain_text();
            // The label in any case, as captions are matched (ASCII case keeps the byte length).
            let start = text.trim_start();
            let after = start
                .get(..label.len())
                .filter(|head| head.eq_ignore_ascii_case(label))
                .and_then(|_| start.get(label.len()..))
                .unwrap_or(&text)
                .trim_start();
            // The number: the word after the label, when it starts with a digit.
            match after.find(char::is_whitespace) {
                Some(n) if after.starts_with(|c: char| c.is_ascii_digit()) => after[n..].to_string(),
                _ if after.starts_with(|c: char| c.is_ascii_digit()) => String::new(),
                _ => after.to_string(),
            }
        }
    };
    rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '.' | '-' | '\u{2013}' | '\u{2014}')).trim_end().to_string()
}

pub(super) const ENTRY_STYLE: &str = "ListEntry";

/// Rebuild every generated list.
pub fn update_generated(s: &mut Session) -> Result<(), CmdError> {
    let mut i = 0;
    while i < s.doc.body.len() {
        // A list held as a field around its entries (as Word and ODT files hold them): made the
        // field's own paragraph, as WordCraft inserts one, its old entries rebuilt below.
        // What follows its closing field end, and its first paragraph's properties and section
        // break, are the document's and stay.
        // (The field it is made: what comes before it in its paragraph may hold other fields.)
        let mut converted = None;
        if let Some((instr, start, last, end)) = wrapped_list(s, i) {
            // The entries between its first and last paragraphs go.
            for _ in i.saturating_add(1)..last {
                s.doc.remove_block(StoryRef::Body, &Path::top(i + 1))?;
            }
            // Its first paragraph keeps what comes before the field (bookmarks, other fields)
            // and the field; its last (or, ending in its first, the
            // same) loses what is up to and through the field end, and goes when nothing is left
            // of it and it ends no section.
            let first_end = if last > i {
                let q = s.doc.para_mut(StoryRef::Body, &Path::top(i + 1))?;
                q.delete(0, end)?;
                if q.is_empty() && q.section.is_none() {
                    s.doc.remove_block(StoryRef::Body, &Path::top(i + 1))?;
                }
                s.doc.para_mut(StoryRef::Body, &Path::top(i))?.len()
            } else {
                end
            };
            let p = s.doc.para_mut(StoryRef::Body, &Path::top(i))?;
            p.delete(start, first_end)?;
            converted = Some(instr.trim_start().to_string());
            p.insert_object(
                start,
                InlineObject::Field { instr, result: String::new(), locked: false, dirty: false, code: false, form: String::new(), code_paras: None },
                &Default::default(),
            )?;
            // What followed the field end in its own paragraph comes after the list, in a
            // paragraph of its own that takes the section break.
            let field = start + wordcraft_doc::para::OBJ.len_utf8();
            if p.len() > field {
                let rest = p.split_off(field)?;
                s.doc.insert_block(StoryRef::Body, &Path::top(i + 1), Block::Para(rest))?;
            }
            // Another list's field before it would be the paragraph's list on later updates (and
            // have its shown list cleared with this one's): the list gets a paragraph of its own,
            // after it, and each is rebuilt in its turn.
            let p = s.doc.para_mut(StoryRef::Body, &Path::top(i))?;
            if p.objects_with_offsets().any(|(off, o)| off < start && list_field(o).is_some()) {
                let list = p.split_off(start)?;
                s.doc.insert_block(StoryRef::Body, &Path::top(i + 1), Block::Para(list))?;
                converted = None;
            }
        }
        let kind = converted.or_else(|| s.doc.body.get(i).and_then(|b| b.as_para()).and_then(list_kind));
        let Some(kind) = kind else {
            i += 1;
            continue;
        };
        add_entry_style(s);
        if kind.starts_with("INDEX") {
            crate::cmd::index::rebuild_from(s, i, None, kind)?;
            i += 1;
            continue;
        }
        // A table of figures from Word can hold the list it saved as the field's result, in the
        // field's own paragraph: the rebuilt entries replace it, keeping its leader.
        let mut saved_leader = None;
        if figures_of(&kind).is_some()
            && let Ok(p) = s.doc.para_mut(StoryRef::Body, &Path::top(i))
        {
            let leader = p.props.tabs.as_ref().and_then(|t| t.last()).map(|t| t.leader);
            let mut cleared = false;
            for o in &mut p.objects {
                if let InlineObject::Field { instr, result, .. } = o
                    && !result.is_empty()
                    && figures_of(instr).is_some()
                {
                    result.clear();
                    cleared = true;
                }
            }
            if cleared {
                saved_leader = leader;
                p.touch();
            }
        }
        // Remove old entries, keeping their tab leader.
        let leader = s
            .doc
            .body
            .get(i + 1)
            .and_then(|b| b.as_para())
            .filter(|p| p.props.style.as_deref() == Some(ENTRY_STYLE))
            .and_then(|p| p.props.tabs.as_ref()?.last().map(|t| t.leader))
            .or(saved_leader)
            .unwrap_or(TabLeader::Dot);
        while s.doc.body.get(i + 1).and_then(|b| b.as_para()).is_some_and(|p| p.props.style.as_deref() == Some(ENTRY_STYLE)) {
            s.doc.remove_block(StoryRef::Body, &Path::top(i + 1))?;
        }
        let lines = entries_for(s, &kind)?;
        let width = crate::cmd::page::sect(s).text_width();
        for (k, mut para) in lines.into_iter().enumerate() {
            if para.plain_text().contains('\t') {
                para.props.tabs = Some(vec![TabStop { pos: width - 0.5, align: TabAlign::Right, leader }]);
            }
            s.doc.insert_block(StoryRef::Body, &Path::top(i + 1 + k), Block::Para(para))?;
        }
        i += 1;
    }
    s.touch();
    Ok(())
}

/// Add the style generated list entries take, if the document doesn't have it yet. Only when a
/// list is there: a new style changes the formatting every paragraph is laid out with, so the
/// layout cache starts over (and the saved file would gain a style it doesn't use).
fn add_entry_style(s: &mut Session) {
    if s.doc.styles.get(ENTRY_STYLE).is_none() {
        Arc::make_mut(&mut s.doc.styles).upsert(wordcraft_doc::Style {
            id: ENTRY_STYLE.into(),
            name: "List Entry".into(),
            kind: wordcraft_doc::StyleKind::Paragraph,
            based_on: Some("Normal".into()),
            para: wordcraft_doc::ParaProps { space_after: Some(4.0), ..Default::default() },
            hidden: true,
            ..Default::default()
        });
    }
}

/// The page holding `pos`: its index in the layout (to order and deduplicate by) and its number
/// in its section's format (`ii`), as the list writes it. Page "1" (index 0) when no page holds it.
fn page_of(s: &mut Session, formats: &[NumFormat], pos: &Pos) -> (usize, String) {
    let l = s.layout();
    page_at(&l, formats, pos).map_or_else(|| (0, "1".into()), |(i, n, fmt, chapter)| (i, page_text(n, fmt, chapter)))
}

/// Entry paragraphs for a generated list, including caption links when requested.
fn entries_for(s: &mut Session, kind: &str) -> Result<Vec<Paragraph>, CmdError> {
    if kind.starts_with("BIBLIOGRAPHY") {
        let st = style(&s.bib_style);
        return Ok(bib_order(&s.doc, st, crate::cmd::default_lang(s))
            .into_iter()
            .enumerate()
            .filter_map(|(k, i)| s.doc.sources.get(i).map(|x| (entry(x, st.name, k.saturating_add(1)), !st.numbered())))
            .map(|(text, hanging)| plain_entry(text, hanging))
            .collect());
    }
    // Collect marks with positions.
    let figures = figures_of(kind);
    let bare = FieldCode::parse(kind).has('a');
    let mut marks: Vec<(String, Pos)> = match &figures {
        Some(fig) => figure_marks(&s.doc, fig, bare),
        None => Vec::new(),
    };
    if figures.is_none() {
        for path in s.doc.para_paths(StoryRef::Body) {
            let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
            for off in p.object_offsets() {
                if let Some(InlineObject::Field { instr, .. }) = p.object_at(off) {
                    let t = instr.trim_start();
                    let want = if kind.starts_with("INDEX") { "XE" } else { "TA" };
                    if t.starts_with(want) {
                        let e = t.split('"').nth(1).unwrap_or("").to_string();
                        marks.push((e, Pos { story: StoryRef::Body, path: path.clone(), off }));
                    }
                }
            }
        }
    }
    let f = FieldCode::parse(kind);
    // Bookmark objects change paragraph offsets, so collect all links before laying out pages.
    let mut links = Vec::new();
    if figures.is_some() && f.has('h') {
        for (_, pos) in &marks {
            let lead = s.doc.para(StoryRef::Body, &pos.path).map_or(0, |p| p.text.bytes().take_while(|b| *b == 0x0C).count());
            links.push(bookmark_heading(s, &pos.path, lead)?);
        }
        s.touch();
    }
    let mut with_pages: Vec<(String, (usize, String))> = marks.into_iter().map(|(e, _)| (e, (0, String::new()))).collect();
    // Pages (layout once).
    let positions: Vec<Pos> = match &figures {
        Some(fig) => figure_marks(&s.doc, fig, bare).into_iter().map(|(_, pos)| pos).collect(),
        None => {
            let mut v = Vec::new();
            for path in s.doc.para_paths(StoryRef::Body) {
                let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
                for off in p.object_offsets() {
                    if let Some(InlineObject::Field { instr, .. }) = p.object_at(off) {
                        let want = if kind.starts_with("INDEX") { "XE" } else { "TA" };
                        if instr.trim_start().starts_with(want) {
                            v.push(Pos { story: StoryRef::Body, path: path.clone(), off });
                        }
                    }
                }
            }
            v
        }
    };
    let formats = page_formats(s);
    for (i, pos) in positions.iter().enumerate() {
        let pg = page_of(s, &formats, pos);
        if let Some(x) = with_pages.get_mut(i) {
            x.1 = pg;
        }
    }
    if figures.is_some() {
        // `\n` without page numbers; `\p` the separator before them (a tab by default).
        let sep = f.arg('p').unwrap_or("\t");
        return with_pages
            .into_iter()
            .enumerate()
            .map(|(k, (text, page))| {
                let bookmark = links.get(k);
                let props = CharProps { link: bookmark.map(|b| format!("#{b}")), ..Default::default() };
                let text = if f.has('n') {
                    text
                } else if bookmark.is_some() {
                    format!("{text}{sep}")
                } else {
                    format!("{text}{sep}{}", page.1)
                };
                let mut para = Paragraph::with_text(&text, props.clone()).styled(ENTRY_STYLE);
                if let Some(bookmark) = bookmark.filter(|_| !f.has('n')) {
                    let field = InlineObject::Field {
                        instr: format!("PAGEREF {bookmark} \\h"),
                        result: page.1,
                        locked: false,
                        dirty: false,
                        code: false,
                        form: String::new(),
                        code_paras: None,
                    };
                    para.insert_object(para.len(), field, &props)?;
                }
                Ok(para)
            })
            .collect();
    }
    // Index / TOA: group by entry, sort, list pages.
    let lang = crate::cmd::default_lang(s);
    // Each page once, in page order (by its place in the layout, not its label: `ii` before `1`).
    let mut grouped: std::collections::BTreeMap<(SortKey, String), Vec<(usize, String)>> = Default::default();
    for (e, p) in with_pages {
        let pages = grouped.entry((sort_key(&e, lang.as_deref()), e)).or_default();
        if !pages.iter().any(|(i, _)| *i == p.0) {
            pages.push(p);
        }
    }
    let mut out = Vec::new();
    let mut letter = None;
    for ((_, e), pages) in grouped {
        let first = heading_letter(&e, lang.as_deref());
        if kind.starts_with("INDEX") && first != letter {
            letter = first.clone();
            if let Some(l) = first {
                out.push((l, false));
            }
        }
        let list = pages.iter().map(|(_, label)| label.as_str()).collect::<Vec<_>>().join(", ");
        out.push(if kind.starts_with("INDEX") { (format!("{e}, {list}"), true) } else { (format!("{e}\t{list}"), false) });
    }
    Ok(out.into_iter().map(|(text, hanging)| plain_entry(text, hanging)).collect())
}

fn plain_entry(text: String, hanging: bool) -> Paragraph {
    let mut para = Paragraph::with_text(&text, CharProps::default()).styled(ENTRY_STYLE);
    if hanging {
        para.props.indent_left = Some(36.0);
        para.props.indent_first = Some(-36.0);
    }
    para
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare `\c` beside `\t` or `\f` marks a table of figures, not captions labelled Figure.
    #[test]
    fn a_bare_c_with_styles_or_entries_lists_no_captions() {
        assert_eq!(figures_label(r#"TOC \h \z \c "Table""#), Some("Table".to_string()));
        assert_eq!(figures_label(r#"TOC \c"#), Some("Figure".to_string()));
        assert_eq!(figures_label(r#"TOC \h \z \t "Caption" \c"#), None);
        assert_eq!(figures_label(r#"TOC \f T \c"#), None);
        assert_eq!(
            figures_of(r#"TOC \t "Heading 2,1,Caption,2" \f t \c"#),
            Some(Figures::Marked { styles: vec!["Heading 2".into(), "Caption".into()], entries: Some("t".into()) })
        );
        assert_eq!(figures_of(r#"TOC \o "1-3""#), None);
    }

    /// A field before a list's in its paragraph (an empty `PAGE`) closes first: the list still
    /// ends at its own field end, paragraphs below, not in its first paragraph.
    #[test]
    fn a_wrapped_list_ends_at_its_own_field_end() {
        let mut d = Document::from_text("Old one\nOld two\nAfter");
        let props = CharProps::default();
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(0)) {
            p.insert_object(0, InlineObject::FieldStart { instr: "BIBLIOGRAPHY".into(), locked: false, dirty: false }, &props).unwrap();
            p.insert_object(0, InlineObject::FieldEnd, &props).unwrap();
            p.insert_object(0, InlineObject::FieldStart { instr: "PAGE".into(), locked: false, dirty: false }, &props).unwrap();
        }
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(1)) {
            let n = p.len();
            p.insert_object(n, InlineObject::FieldEnd, &props).unwrap();
        }
        let s = Session::new(d);
        assert_eq!(wrapped_list(&s, 0).map(|(instr, _, last, _)| (instr, last)), Some(("BIBLIOGRAPHY".to_string(), 1)));
    }

    /// A wrapped list as a session holds it: the field starting paragraph `first`'s text, ending
    /// in paragraph `last` at byte `at`.
    fn wrapped(text: &str, first: usize, last: usize, at: usize) -> Session {
        let mut d = Document::from_text(text);
        let props = CharProps::default();
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(last)) {
            p.insert_object(at, InlineObject::FieldEnd, &props).unwrap();
        }
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(first)) {
            p.insert_object(0, InlineObject::FieldStart { instr: "BIBLIOGRAPHY".into(), locked: false, dirty: false }, &props).unwrap();
        }
        Session::new(d)
    }

    fn texts(s: &Session) -> Vec<String> {
        s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
    }

    /// What follows a wrapped list's closing field end, in its last paragraph or in its only one,
    /// stays, after the list; the last paragraph's section break stays too.
    #[test]
    fn what_follows_a_wrapped_lists_end_stays() {
        // Ending in a later paragraph, with text after the end and a section break.
        let mut s = wrapped("Old one\nOld two\nOld three. After\nBelow", 0, 2, "Old three.".len());
        if let Ok(p) = s.doc.para_mut(StoryRef::Body, &Path::top(2)) {
            p.section = Some(Box::default());
        }
        if let Ok(p) = s.doc.para_mut(StoryRef::Body, &Path::top(0)) {
            p.props.indent_left = Some(9.0);
        }
        update_generated(&mut s).unwrap();
        let t = texts(&s);
        assert!(t.iter().all(|t| !t.contains("Old")), "{t:?}");
        assert!(t.iter().any(|t| t == " After"), "{t:?}");
        assert_eq!(t.last().map(String::as_str), Some("Below"));
        let after = s.doc.body.iter().filter_map(|b| b.as_para()).find(|p| p.plain_text() == " After");
        assert!(after.is_some_and(|p| p.section.is_some()), "the section break went");
        let first = s.doc.body.first().and_then(|b| b.as_para());
        assert!(first.is_some_and(|p| p.props.indent_left == Some(9.0) && list_kind(p).is_some()), "{first:?}");
        // An empty last paragraph without a section break goes.
        let mut s = wrapped("Old one\nOld two\nBelow", 0, 1, "Old two".len());
        update_generated(&mut s).unwrap();
        // (The field's paragraph shows nothing: there are no sources to list.)
        assert_eq!(texts(&s), ["", "Below"]);
        // Ending in its own paragraph: what follows comes after the list.
        let mut s = wrapped("Old entry. Then this\nBelow", 0, 0, "Old entry.".len());
        update_generated(&mut s).unwrap();
        let t = texts(&s);
        assert!(t.first().is_some_and(|t| !t.contains("Old")), "{t:?}");
        assert_eq!(t.get(1).map(String::as_str), Some(" Then this"), "{t:?}");
        assert_eq!(t.last().map(String::as_str), Some("Below"));
    }

    /// What comes before a wrapped list's field in its first paragraph (a bookmark, an empty
    /// `PAGE` field) stays there, before the rebuilt list's field.
    #[test]
    fn what_comes_before_a_wrapped_lists_field_stays() {
        let mut s = wrapped("Old one\nOld two\nBelow", 0, 1, "Old two".len());
        let props = CharProps::default();
        if let Ok(p) = s.doc.para_mut(StoryRef::Body, &Path::top(0)) {
            p.insert_object(0, InlineObject::FieldEnd, &props).unwrap();
            p.insert_object(0, InlineObject::FieldStart { instr: "PAGE".into(), locked: false, dirty: false }, &props).unwrap();
            p.insert_object(0, InlineObject::BookmarkStart { name: "Sources".into() }, &props).unwrap();
        }
        update_generated(&mut s).unwrap();
        let first = s.doc.body.first().and_then(|b| b.as_para());
        let kinds: Vec<&str> = first
            .map(|p| {
                p.objects
                    .iter()
                    .map(|o| match o {
                        InlineObject::BookmarkStart { .. } => "bookmark",
                        InlineObject::FieldStart { instr, .. } if instr == "PAGE" => "page",
                        InlineObject::FieldEnd => "end",
                        InlineObject::Field { instr, .. } if instr.starts_with("BIBLIOGRAPHY") => "list",
                        _ => "other",
                    })
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(kinds, ["bookmark", "page", "end", "list"]);
        assert_eq!(texts(&s), ["", "Below"]);
    }

    /// A list field before a wrapped list's field in its paragraph doesn't take its place: the
    /// wrapped list is the one rebuilt.
    #[test]
    fn a_wrapped_list_is_rebuilt_as_itself_after_another_list_field() {
        let mut s = wrapped("Old one\nOld two\nBelow", 0, 1, "Old two".len());
        s.doc.sources.push(wordcraft_doc::Source { tag: "a".into(), kind: "book".into(), title: "Listed Source".into(), ..Default::default() });
        let index = InlineObject::Field {
            instr: "INDEX".into(),
            result: String::new(),
            locked: false,
            dirty: false,
            code: false,
            form: String::new(),
            code_paras: None,
        };
        if let Ok(p) = s.doc.para_mut(StoryRef::Body, &Path::top(0)) {
            p.insert_object(0, index, &CharProps::default()).unwrap();
        }
        // Updated again, each list stays itself.
        for _ in 0..2 {
            update_generated(&mut s).unwrap();
            let t = texts(&s);
            assert_eq!(t.iter().filter(|t| t.contains("Listed Source")).count(), 1, "the bibliography was rebuilt: {t:?}");
            let lists: Vec<Option<String>> = s.doc.body.iter().filter_map(|b| b.as_para()).filter_map(list_kind).map(Some).collect();
            assert_eq!(lists, [Some("INDEX".to_string()), Some("BIBLIOGRAPHY".to_string())]);
        }
    }
}
