use super::{CharProps, CmdError, CmdResult, Document, InlineObject, Paragraph, Pos, Selection, Session, StoryRef, Value, delete_selection, json, p};
use crate::cmd::references::page_label;
use wordcraft_doc::para::NoteKind;

const MARK: usize = wordcraft_doc::para::OBJ.len_utf8();

/// The note kind a cross-reference type names: `footnote` or `endnote`.
fn note_kind(to: &str) -> Option<NoteKind> {
    match to {
        "footnote" => Some(NoteKind::Footnote),
        "endnote" => Some(NoteKind::Endnote),
        _ => None,
    }
}

/// "above" when `pos` is before the selection, else "below" (Word's `\p` result).
fn above_below(s: &Session, pos: &Pos) -> String {
    if *pos < s.sel.focus { "above".into() } else { "below".into() }
}

/// The field instruction: `<name> <bookmark>` with its switches, and `\h` unless Insert as
/// hyperlink is off.
fn instruction(field: &str, name: &str, switches: &str, hyperlink: bool) -> String {
    let mut instr = format!("{field} {name}");
    for sw in [switches, if hyperlink { "\\h" } else { "" }] {
        if !sw.is_empty() {
            instr.push(' ');
            instr.push_str(sw);
        }
    }
    instr
}

pub(super) fn cross_ref(s: &mut Session, v: &Value) -> CmdResult {
    let to = p::str(v, "to").unwrap_or("heading");
    if let Some(kind) = note_kind(to) {
        return note_cross_ref(s, v, kind);
    }
    let targets = ref_targets(s, to);
    let Some(target) = p::str(v, "target") else {
        return Ok(Value::Array(targets.iter().map(|(t, _)| json!(t)).collect()));
    };
    let show = p::str(v, "show").unwrap_or("text");
    if show == "formattedNumber" {
        return Err(CmdError::Params("`formattedNumber` is for footnotes and endnotes".into()));
    }
    let hyperlink = p::bool(v, "hyperlink").unwrap_or(true);
    let find = |ts: Vec<(String, Pos)>| ts.into_iter().find(|(t, _)| t == target || t.starts_with(target));
    let (text, pos) = find(targets).ok_or_else(|| CmdError::Params(format!("no {to} `{target}`")))?;
    let result = match show {
        // The page the target's text starts on (after its leading page breaks), as its footer
        // shows it.
        "page" => page_label(s, &Pos { off: leading_breaks(&s.doc, &pos), ..pos.clone() }),
        "number" => text.split(':').next().unwrap_or(&text).trim().to_string(),
        "aboveBelow" => above_below(s, &pos),
        _ if to == "bookmark" => crate::cmd::references::bookmark_text(&s.doc, &text).unwrap_or_default(),
        _ => text.clone(),
    };
    let props = s.typing_props();
    let at = delete_selection(s)?;
    // The field points at a bookmark: the named one, or a hidden `_Ref` bookmark on the target
    // paragraph (as Word does), so Update Field and Word can resolve it.
    let (name, at) = if to == "bookmark" {
        (text, at)
    } else {
        // Deleting the selection can move paragraphs: look the target up again.
        let (_, pos) = find(ref_targets(s, to)).ok_or_else(|| CmdError::Params(format!("no {to} `{target}`")))?;
        ref_bookmark(s, &pos, show == "number", at)?
    };
    // Above/below keeps its `\p` switch, so Update Field shows "above" or "below" again
    // (ECMA-376 §17.16.5.51), not the target's text.
    let instr = match show {
        "page" => instruction("PAGEREF", &name, "", hyperlink),
        "aboveBelow" => instruction("REF", &name, "\\p", hyperlink),
        _ => instruction("REF", &name, "", hyperlink),
    };
    let end = s.doc.insert_object(
        &at,
        InlineObject::Field { instr, result: result.clone(), locked: false, dirty: false, code: false, form: String::new() },
        &props,
    )?;
    s.sel = Selection::caret(end);
    Ok(json!({"inserted": result}))
}

/// A body footnote or endnote reference: its part id, its mark as shown, the listing text
/// (`<mark> <start of its text>`) and where its mark is.
struct NoteTarget {
    id: u32,
    mark: String,
    listing: String,
    pos: Pos,
}

/// Characters of a note's text the Cross-reference list shows.
const NOTE_TEXT_CHARS: usize = 50;

/// The body's notes of `kind`, in document order.
fn note_targets(s: &Session, kind: NoteKind) -> Vec<NoteTarget> {
    let marks = wordcraft_layout::note_marks(&s.doc);
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        for (off, o) in p.objects_with_offsets() {
            let InlineObject::NoteRef { kind: k, id, custom } = o else { continue };
            if *k != kind {
                continue;
            }
            let mark = if custom.is_empty() { marks.get(id).cloned().unwrap_or_default() } else { custom.clone() };
            let text: String = s
                .doc
                .para_paths(StoryRef::Part(*id))
                .first()
                .and_then(|pp| s.doc.para(StoryRef::Part(*id), pp))
                .map(|np| np.text.chars().filter(|c| *c != wordcraft_doc::para::OBJ).collect())
                .unwrap_or_default();
            let text: String = text.trim().chars().take(NOTE_TEXT_CHARS).collect();
            let listing = format!("{mark} {}", text.trim_end()).trim().to_string();
            out.push(NoteTarget { id: *id, mark, listing, pos: Pos { story: StoryRef::Body, path: path.clone(), off } });
        }
    }
    out
}

/// `insert.crossReference` to a footnote or endnote: `NOTEREF` (number, formatted number,
/// above/below) or `PAGEREF` (page) to a hidden `_Ref` bookmark around its reference mark
/// (ECMA-376 §17.16.5.40, §17.16.5.45).
fn note_cross_ref(s: &mut Session, v: &Value, kind: NoteKind) -> CmdResult {
    let targets = note_targets(s, kind);
    let Some(target) = p::str(v, "target") else {
        return Ok(Value::Array(targets.iter().map(|t| json!(t.listing)).collect()));
    };
    let show = p::str(v, "show").unwrap_or("number");
    if !matches!(show, "number" | "formattedNumber" | "page" | "aboveBelow") {
        return Err(CmdError::Params(format!("a note cross-reference can't show `{show}`")));
    }
    let hyperlink = p::bool(v, "hyperlink").unwrap_or(true);
    let what = if kind == NoteKind::Footnote { "footnote" } else { "endnote" };
    let t = targets.into_iter().find(|t| t.mark == target || t.listing == target).ok_or_else(|| CmdError::Params(format!("no {what} `{target}`")))?;
    let result = match show {
        "page" => page_label(s, &t.pos),
        "aboveBelow" => above_below(s, &t.pos),
        _ => t.mark.clone(),
    };
    let mut props = s.typing_props();
    if show == "formattedNumber" {
        // Drawn like a reference mark.
        props.style = Some(if kind == NoteKind::Footnote { "FootnoteReference" } else { "EndnoteReference" }.into());
    }
    let at = delete_selection(s)?;
    // Deleting the selection can move the mark: look the note up again.
    let pos =
        note_targets(s, kind).into_iter().find(|n| n.id == t.id).map(|n| n.pos).ok_or_else(|| CmdError::Params(format!("no {what} `{target}`")))?;
    let (name, at) = note_bookmark(s, &pos, at)?;
    let instr = match show {
        "page" => instruction("PAGEREF", &name, "", hyperlink),
        "formattedNumber" => instruction("NOTEREF", &name, "\\f", hyperlink),
        "aboveBelow" => instruction("NOTEREF", &name, "\\p", hyperlink),
        _ => instruction("NOTEREF", &name, "", hyperlink),
    };
    let end = s.doc.insert_object(
        &at,
        InlineObject::Field { instr, result: result.clone(), locked: false, dirty: false, code: false, form: String::new() },
        &props,
    )?;
    s.sel = Selection::caret(end);
    Ok(json!({"inserted": result}))
}

/// The hidden `_Ref` bookmark around the note reference mark at `pos`, reusing one already
/// there. Returns its name and `at` moved past the bookmark marks inserted before it.
fn note_bookmark(s: &mut Session, pos: &Pos, at: Pos) -> Result<(String, Pos), CmdError> {
    let para = s.doc.para(pos.story, &pos.path).ok_or_else(|| CmdError::Params("cross-reference target is gone".into()))?;
    if let Some(before) = pos.off.checked_sub(MARK)
        && let Some(InlineObject::BookmarkStart { name }) = para.object_at(before)
        && name.starts_with("_Ref")
        && matches!(para.object_at(pos.off.saturating_add(MARK)), Some(InlineObject::BookmarkEnd { name: n }) if n == name)
    {
        return Ok((name.clone(), at));
    }
    add_ref_bookmark(s, pos, pos.off, pos.off.saturating_add(MARK), at)
}

/// A new hidden `_Ref` bookmark over `start..end` of the paragraph at `pos`, numbered after the
/// document's others. Returns its name and `at` moved past the marks inserted before it.
fn add_ref_bookmark(s: &mut Session, pos: &Pos, start: usize, end: usize, mut at: Pos) -> Result<(String, Pos), CmdError> {
    let next =
        s.doc.bookmarks().iter().filter_map(|(n, _)| n.strip_prefix("_Ref").and_then(|d| d.parse::<u64>().ok())).max().unwrap_or(0).saturating_add(1);
    let name = format!("_Ref{next:09}");
    let props = CharProps::default();
    s.doc.insert_object(&Pos { off: end, ..pos.clone() }, InlineObject::BookmarkEnd { name: name.clone() }, &props)?;
    s.doc.insert_object(&Pos { off: start, ..pos.clone() }, InlineObject::BookmarkStart { name: name.clone() }, &props)?;
    if at.story == pos.story && at.path == pos.path {
        if at.off >= end {
            at.off = at.off.saturating_add(2 * MARK);
        } else if at.off > start {
            at.off = at.off.saturating_add(MARK);
        }
    }
    Ok((name, at))
}

/// The label of a caption: the text before its first field (`Plate ` in `Plate `, `SEQ Plate`,
/// `: A rock`). When the caption starts with its chapter number, the text between that field and
/// the `SEQ` number, unless it is only a separator. Failing both, the `SEQ` identifier names it.
/// A caption without a `SEQ` field (typed by hand) has what comes before its first digit.
pub(super) fn caption_label(p: &Paragraph, text: &str) -> Option<String> {
    let fields: Vec<(usize, &str)> = p
        .object_offsets()
        .into_iter()
        .filter_map(|o| match p.object_at(o) {
            Some(InlineObject::Field { instr, .. }) => Some((o, instr.as_str())),
            _ => None,
        })
        .collect();
    let seq = fields.iter().position(|(_, instr)| instr.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("SEQ")));
    let label = match seq.and_then(|i| Some((i, fields.get(i)?))) {
        Some((i, (seq_at, instr))) => {
            // The text in `from..to` with the bookmark marks dropped, if it says anything.
            let said = |from: usize, to: usize| {
                let t: String = p.text.get(from..to).unwrap_or_default().chars().filter(|c| *c != wordcraft_doc::para::OBJ).collect();
                let t = t.trim();
                t.chars().any(char::is_alphanumeric).then(|| t.to_string())
            };
            let first = fields.first().map_or(0, |(o, _)| *o);
            let after_previous =
                i.checked_sub(1).and_then(|k| fields.get(k)).map_or(0, |(o, _)| o.saturating_add(wordcraft_doc::para::OBJ.len_utf8()));
            said(0, first).or_else(|| said(after_previous, *seq_at)).or_else(|| instr.split_whitespace().nth(1).map(|id| id.replace('_', " ")))?
        }
        None => text.get(..text.find(|c: char| c.is_ascii_digit())?)?.trim().to_string(),
    };
    (!label.is_empty() && label.chars().count() <= 40).then_some(label)
}

/// Whether `p` is a caption: its style is Caption or one based on it, as a caption restyled with a
/// style of its own (a `Table Caption` based on Caption) still is (#4411).
pub(super) fn is_caption(doc: &Document, p: &Paragraph) -> bool {
    p.props.style.as_deref().is_some_and(|id| id == "Caption" || doc.styles.chain(id).iter().any(|s| s.id == "Caption"))
}

/// The caption paragraphs of the body: (text, position, label).
fn captions(s: &Session) -> Vec<(String, Pos, Option<String>)> {
    s.doc
        .para_paths(StoryRef::Body)
        .into_iter()
        .filter_map(|path| {
            let p = s.doc.para(StoryRef::Body, &path).filter(|p| is_caption(&s.doc, p))?;
            let text = p.plain_text();
            let label = caption_label(p, &text);
            Some((text, Pos { story: StoryRef::Body, path, off: 0 }, label))
        })
        .collect()
}

/// The labels the body's captions use, first seen first: Figure, Table and Equation, and any the
/// person made with Insert Caption › New Label. Each is a cross-reference type, `label:<Name>` (#2940).
pub fn caption_labels(s: &Session) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for l in captions(s).into_iter().filter_map(|(_, _, l)| l) {
        if !labels.contains(&l) {
            labels.push(l);
        }
    }
    // The document's own labels (New Label, kept in `w:captions`), captioned yet or not.
    for l in &s.doc.settings.captions.labels {
        if !labels.iter().any(|x| x.eq_ignore_ascii_case(&l.name))
            && !wordcraft_doc::BUILTIN_CAPTION_LABELS.iter().any(|b| b.eq_ignore_ascii_case(&l.name))
        {
            labels.push(l.name.clone());
        }
    }
    labels
}

/// Cross-reference targets of a kind: (shown text or bookmark name, position).
pub fn ref_targets(s: &Session, to: &str) -> Vec<(String, Pos)> {
    let mut targets: Vec<(String, Pos)> = Vec::new();
    match to {
        "bookmark" => targets = s.doc.bookmarks(),
        "footnote" | "endnote" => {
            let kind = if to == "footnote" { NoteKind::Footnote } else { NoteKind::Endnote };
            targets = note_targets(s, kind).into_iter().map(|t| (t.listing, t.pos)).collect();
        }
        // A caption label of the person's own: the captions that use exactly that label.
        t if t.starts_with("label:") => {
            let want = t.get("label:".len()..).unwrap_or_default();
            targets = captions(s)
                .into_iter()
                .filter(|(_, _, label)| !want.is_empty() && label.as_deref() == Some(want))
                .map(|(text, pos, _)| (text, pos))
                .collect();
        }
        "figure" | "table" | "equation" => {
            let label = match to {
                "table" => "Table",
                "equation" => "Equation",
                _ => "Figure",
            };
            for path in s.doc.para_paths(StoryRef::Body) {
                if let Some(p) = s.doc.para(StoryRef::Body, &path)
                    && is_caption(&s.doc, p)
                    && p.plain_text().starts_with(label)
                {
                    targets.push((p.plain_text(), Pos { story: StoryRef::Body, path: path.clone(), off: 0 }));
                }
            }
        }
        _ => {
            for path in s.doc.para_paths(StoryRef::Body) {
                if let Some(p) = s.doc.para(StoryRef::Body, &path)
                    && s.doc.styles.resolve_para(&p.props).outline_level.is_some()
                    && !p.plain_text().trim().is_empty()
                {
                    targets.push((p.plain_text().trim().to_string(), Pos { story: StoryRef::Body, path: path.clone(), off: 0 }));
                }
            }
        }
    }
    targets
}

/// The offset just past the page breaks at the start of the paragraph at `pos`, skipping bookmark
/// marks among them (a `_Ref` bookmark may start before the breaks): its text starts there.
pub(crate) fn leading_breaks(doc: &Document, pos: &Pos) -> usize {
    let Some(p) = doc.para(pos.story, &pos.path) else { return 0 };
    let mut after = 0;
    for (i, c) in p.text.char_indices() {
        if c == '\u{0C}' {
            after = i.saturating_add(c.len_utf8());
        } else if c != wordcraft_doc::para::OBJ
            || !matches!(p.object_at(i), Some(InlineObject::BookmarkStart { .. } | InlineObject::BookmarkEnd { .. }))
        {
            break;
        }
    }
    after
}

/// The hidden `_Ref` bookmark around the target paragraph at `pos` (its label and number only
/// when `number_only`), reusing one already there. Returns its name and `at` moved past the
/// bookmark marks inserted before it.
fn ref_bookmark(s: &mut Session, pos: &Pos, number_only: bool, at: Pos) -> Result<(String, Pos), CmdError> {
    let para = s.doc.para(pos.story, &pos.path).ok_or_else(|| CmdError::Params("cross-reference target is gone".into()))?;
    let end = if number_only { para.text.find(':').unwrap_or(para.len()) } else { para.len() };
    // The bookmark starts after the leading page breaks, so `PAGEREF` reads the page the text is on.
    let start = leading_breaks(&s.doc, pos).min(end);
    // Reuse one that starts there or at the paragraph's start (before the breaks): the field's
    // result still reads the page the text is on.
    for from in [start, 0] {
        if let Some(InlineObject::BookmarkStart { name }) = para.object_at(from)
            && name.starts_with("_Ref")
        {
            // Reuse it when it covers the same extent: up to the colon, or the whole paragraph.
            let same = para.object_offsets().into_iter().any(|o| {
                matches!(para.object_at(o), Some(InlineObject::BookmarkEnd { name: n }) if n == name)
                    && para.text.get(o.saturating_add(MARK)..).is_some_and(|rest| if number_only { rest.starts_with(':') } else { rest.is_empty() })
            });
            if same {
                return Ok((name.clone(), at));
            }
        }
    }
    add_ref_bookmark(s, pos, start, end, at)
}
