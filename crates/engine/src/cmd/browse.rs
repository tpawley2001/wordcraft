//! Select Browse Object (Word's Alt+Ctrl+Home palette) and Next / Previous by it (Ctrl+PageDown /
//! Ctrl+PageUp): jump to the next or previous heading, table, field, comment… from the caret.
//! Find is the default browse object, so the keys repeat Find Next until another is picked.
//! The choice is session state: not saved, not undoable.

use serde_json::json;
use wordcraft_doc::para::{InlineObject, NoteKind, OBJ};
use wordcraft_doc::{Paragraph, Pos, StoryRef};

use super::pos_json;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// What Next and Previous move to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowseObject {
    GoTo,
    #[default]
    Find,
    Edit,
    Heading,
    Graphic,
    Table,
    Field,
    Endnote,
    Footnote,
    Comment,
    Section,
    Page,
}

impl BrowseObject {
    /// Every object, in the palette's order.
    pub const ALL: [BrowseObject; 12] = [
        BrowseObject::GoTo,
        BrowseObject::Find,
        BrowseObject::Edit,
        BrowseObject::Heading,
        BrowseObject::Graphic,
        BrowseObject::Table,
        BrowseObject::Field,
        BrowseObject::Endnote,
        BrowseObject::Footnote,
        BrowseObject::Comment,
        BrowseObject::Section,
        BrowseObject::Page,
    ];

    /// The name `nav.browseBy` takes.
    pub fn name(self) -> &'static str {
        match self {
            BrowseObject::GoTo => "goto",
            BrowseObject::Find => "find",
            BrowseObject::Edit => "edit",
            BrowseObject::Heading => "heading",
            BrowseObject::Graphic => "graphic",
            BrowseObject::Table => "table",
            BrowseObject::Field => "field",
            BrowseObject::Endnote => "endnote",
            BrowseObject::Footnote => "footnote",
            BrowseObject::Comment => "comment",
            BrowseObject::Section => "section",
            BrowseObject::Page => "page",
        }
    }

    /// The palette button's tooltip.
    pub fn label(self) -> &'static str {
        match self {
            BrowseObject::GoTo => "Go To",
            BrowseObject::Find => "Find",
            BrowseObject::Edit => "Browse by Edits",
            BrowseObject::Heading => "Browse by Heading",
            BrowseObject::Graphic => "Browse by Graphic",
            BrowseObject::Table => "Browse by Table",
            BrowseObject::Field => "Browse by Field",
            BrowseObject::Endnote => "Browse by Endnote",
            BrowseObject::Footnote => "Browse by Footnote",
            BrowseObject::Comment => "Browse by Comment",
            BrowseObject::Section => "Browse by Section",
            BrowseObject::Page => "Browse by Page",
        }
    }

    pub fn from_name(name: &str) -> Option<BrowseObject> {
        BrowseObject::ALL.into_iter().find(|o| o.name() == name)
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("nav.browseBy", "Browse By", "Navigation › Select Browse Object", |s, v| {
            let name = p::str(v, "object").ok_or_else(|| CmdError::Params("`object` (string) is required".into()))?;
            let o = BrowseObject::from_name(name).ok_or_else(|| CmdError::Params(format!("no browse object `{name}`")))?;
            s.find.browse = o;
            Ok(json!({"current": o.name()}))
        })
        .params(r#"{"object": "goto|find|edit|heading|graphic|table|field|endnote|footnote|comment|section|page"}"#)
        .pure(),
        CommandSpec::new("nav.browseNext", "Next", "Navigation › Select Browse Object", |s, _| browse(s, true))
            .key(super::non_mac("Mod+PageDown"))
            .pure(),
        CommandSpec::new("nav.browsePrevious", "Previous", "Navigation › Select Browse Object", |s, _| browse(s, false))
            .key(super::non_mac("Mod+PageUp"))
            .pure(),
        CommandSpec::new("nav.selectBrowseObject", "Select Browse Object", "Navigation", |s, _| {
            Ok(json!({
                "objects": BrowseObject::ALL.iter().map(|o| o.name()).collect::<Vec<_>>(),
                "current": s.find.browse.name(),
            }))
        })
        .key(super::non_mac("Mod+Alt+Home"))
        .pure(),
    ]
}

fn browse(s: &mut Session, forward: bool) -> CmdResult {
    let o = s.find.browse;
    match o {
        BrowseObject::Find => return super::edit::find_step(s, if forward { 1 } else { -1 }),
        BrowseObject::GoTo => return super::edit::goto_command(s, &json!({})),
        BrowseObject::Page => return super::edit::goto_command(s, &json!({"pageBy": if forward { 1 } else { -1 }})),
        _ => {}
    }
    let (from, to) = s.sel.ordered();
    let targets = targets(s, o);
    let hit = if forward { targets.into_iter().find(|t| *t > to) } else { targets.into_iter().rev().find(|t| *t < from) };
    let Some(at) = hit else {
        s.status = format!("No further {} in the document.", o.name());
        return Ok(json!({"found": false, "object": o.name()}));
    };
    s.sel = Selection::caret(at);
    s.goal_x = None;
    s.push_ui_request(json!({"scrollTo": "caret"}));
    Ok(json!({"found": true, "object": o.name(), "focus": pos_json(&s.sel.focus)}))
}

/// What Go To (`edit.goto {"what"}`) and Next / Previous by browse object look for.
pub(super) enum Target<'a> {
    Browse(BrowseObject),
    /// Every section's start, section 1 (the top of the body) included.
    SectionStarts,
    Equation,
    /// Embedded or linked OLE objects.
    Object,
    /// Comments by a reviewer (ASCII case ignored); empty = any reviewer.
    CommentBy(&'a str),
    /// Fields of a type, the first word of the field code (case ignored); empty = any field.
    FieldType(&'a str),
}

fn targets(s: &Session, o: BrowseObject) -> Vec<Pos> {
    goto_targets(s, &Target::Browse(o))
}

/// Where each object `t` names starts in the body, in document order.
pub(super) fn goto_targets(s: &Session, t: &Target) -> Vec<Pos> {
    let paths = s.doc.para_paths(StoryRef::Body);
    let mut out = Vec::new();
    if let (Target::SectionStarts, Some(first)) = (t, paths.first()) {
        out.push(Pos { story: StoryRef::Body, path: first.clone(), off: 0 });
    }
    let mut after_break = false;
    for path in &paths {
        let Some(para) = s.doc.para(StoryRef::Body, path) else { continue };
        let at = |off: usize| Pos { story: StoryRef::Body, path: path.clone(), off };
        let o = match t {
            Target::Browse(o) => *o,
            Target::SectionStarts => BrowseObject::Section,
            _ => {
                out.extend(objects(para).filter(|(_, ob)| goto_matches(s, t, ob)).map(|(off, _)| at(off)));
                continue;
            }
        };
        match o {
            BrowseObject::Heading => {
                if s.doc.styles.resolve_para(&para.props).outline_level.is_some_and(|l| l < 9) {
                    out.push(at(0));
                }
            }
            BrowseObject::Table => {
                // A top-level table's first cell: [table, row 0, cell 0, paragraph 0].
                if let [_, 0, 0, 0] = path.0.as_slice() {
                    out.push(at(0));
                }
            }
            BrowseObject::Section => {
                if path.0.len() == 1 {
                    if after_break {
                        out.push(at(0));
                    }
                    after_break = para.section.is_some();
                }
            }
            BrowseObject::Edit => {
                if let Some(off) = first_edit(para) {
                    out.push(at(off));
                }
            }
            _ => out.extend(objects(para).filter(|(_, ob)| matches(o, ob)).map(|(off, _)| at(off))),
        }
    }
    out
}

/// Each inline object with its byte offset.
fn objects(para: &Paragraph) -> impl Iterator<Item = (usize, &InlineObject)> {
    para.text.char_indices().filter(|(_, c)| *c == OBJ).map(|(i, _)| i).zip(para.objects.iter())
}

fn matches(o: BrowseObject, ob: &InlineObject) -> bool {
    match o {
        BrowseObject::Field => matches!(ob, InlineObject::Field { .. } | InlineObject::FieldStart { .. }),
        BrowseObject::Graphic => {
            matches!(ob, InlineObject::Image { .. } | InlineObject::Graphic { .. } | InlineObject::Shape { .. } | InlineObject::Group { .. })
        }
        BrowseObject::Comment => matches!(ob, InlineObject::CommentStart { .. }),
        BrowseObject::Footnote => matches!(ob, InlineObject::NoteRef { kind: NoteKind::Footnote, .. }),
        BrowseObject::Endnote => matches!(ob, InlineObject::NoteRef { kind: NoteKind::Endnote, .. }),
        _ => false,
    }
}

fn goto_matches(s: &Session, t: &Target, ob: &InlineObject) -> bool {
    match (t, ob) {
        (Target::Equation, InlineObject::Equation { .. }) => true,
        (Target::Object, InlineObject::Image { ole, .. }) => ole.is_some(),
        (Target::CommentBy(who), InlineObject::CommentStart { id }) => {
            who.is_empty() || s.doc.comments.get(id).is_some_and(|c| c.author.eq_ignore_ascii_case(who))
        }
        (Target::FieldType(kind), InlineObject::Field { instr, .. } | InlineObject::FieldStart { instr, .. }) => {
            kind.is_empty() || field_type(instr).eq_ignore_ascii_case(kind)
        }
        _ => false,
    }
}

/// A field's type: the first word of its code (`PAGE` in ` PAGE \* roman`).
fn field_type(instr: &str) -> &str {
    instr.split_whitespace().next().unwrap_or("")
}

/// The first tracked change in a paragraph: an insertion, deletion or formatting change in its
/// text, else a change to the paragraph itself (its formatting or its mark).
fn first_edit(para: &Paragraph) -> Option<usize> {
    let run = para.run_ranges().find(|(r, c)| !r.is_empty() && (c.ins.is_some() || c.del.is_some() || c.fmt_change.is_some()));
    if let Some((r, _)) = run {
        return Some(r.start);
    }
    if para.props.fmt_change.is_some() {
        return Some(0);
    }
    (para.mark.ins.is_some() || para.mark.del.is_some()).then_some(para.text.len())
}
