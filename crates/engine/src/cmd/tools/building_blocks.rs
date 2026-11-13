//! Insert › Quick Parts › Building Blocks Organizer: the document's building blocks
//! (`Document::glossary`, ECMA-376 Part 1 §17.12 `w:docPart`), their insert behaviours
//! (§17.12.1 `w:behavior`: content, own paragraph, own page) and F3, which replaces the word
//! before the caret with the AutoText entry of that name.

use wordcraft_doc::BuildingBlock;
use wordcraft_doc::edit::Fragment;
use wordcraft_doc::para::PAGE_BREAK;

use super::{Arc, Block, CharProps, CmdError, CmdResult, Paragraph, Pos, Selection, Session, StoryRef, Value, delete_selection, json, p};

/// The galleries a building block can be in (§17.18.32 ST_DocPartGallery).
const GALLERIES: &[&str] = &[
    "placeholder",
    "any",
    "default",
    "docParts",
    "coverPg",
    "eq",
    "ftrs",
    "hdrs",
    "pgNum",
    "tbls",
    "watermarks",
    "autoTxt",
    "txtBox",
    "pgNumT",
    "pgNumB",
    "pgNumMargins",
    "tblOfContents",
    "bib",
    "custQuickParts",
    "custCoverPg",
    "custEq",
    "custFtrs",
    "custHdrs",
    "custPgNum",
    "custTbls",
    "custWatermarks",
    "custAutoTxt",
    "custTxtBox",
    "custPgNumT",
    "custPgNumB",
    "custPgNumMargins",
    "custTblOfContents",
    "custBib",
    "custom1",
    "custom2",
    "custom3",
    "custom4",
    "custom5",
];

/// How a building block is inserted (§17.18.30 ST_DocPartBehavior).
const BEHAVIORS: &[&str] = &["content", "p", "pg"];

/// The longest name, category or description kept (Word's own fields are short).
const MAX_FIELD: usize = 255;

fn gallery_param(v: &Value) -> Result<Option<String>, CmdError> {
    match p::str(v, "gallery") {
        None => Ok(None),
        Some(g) if GALLERIES.contains(&g) => Ok(Some(g.to_string())),
        Some(g) => Err(CmdError::Params(format!("unknown gallery `{g}`"))),
    }
}

fn behavior_param(v: &Value) -> Result<Option<String>, CmdError> {
    match p::str(v, "behavior") {
        None => Ok(None),
        Some(b) if BEHAVIORS.contains(&b) => Ok(Some(b.to_string())),
        Some(b) => Err(CmdError::Params(format!("unknown behavior `{b}` (content, p or pg)"))),
    }
}

fn text_param(v: &Value, k: &str) -> Option<String> {
    p::str(v, k).map(|t| t.chars().take(MAX_FIELD).collect())
}

fn name_param(v: &Value) -> Result<Option<String>, CmdError> {
    match text_param(v, "name") {
        Some(n) if n.trim().is_empty() => Err(CmdError::Params("a building block needs a name".into())),
        n => Ok(n),
    }
}

fn index_param(s: &Session, v: &Value, k: &str) -> Result<Option<usize>, CmdError> {
    let Some(x) = v.get(k) else { return Ok(None) };
    let i = x.as_u64().and_then(|i| usize::try_from(i).ok()).filter(|&i| i < s.doc.glossary.len());
    i.map(Some).ok_or_else(|| CmdError::Params(format!("`{k}`: no building block {x}")))
}

/// The behaviour a building block is inserted with: its first, else "content".
fn behavior_of(e: &BuildingBlock) -> &str {
    e.behaviors.first().map(String::as_str).unwrap_or("content")
}

/// `insert.buildingBlocks`: list the document's building blocks, and create, modify, delete or
/// insert one.
pub(super) fn organizer(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(c) = v.get("create") {
        create(s, c)?;
    } else if let Some(i) = index_param(s, v, "modify")? {
        let (name, gallery, behavior) = (name_param(v)?, gallery_param(v)?, behavior_param(v)?);
        let (category, description) = (text_param(v, "category"), text_param(v, "description"));
        let e = s.doc.glossary.get_mut(i).ok_or_else(|| CmdError::Params(format!("no building block {i}")))?;
        if let Some(n) = name {
            e.name = n;
        }
        if let Some(g) = gallery {
            e.gallery = g;
        }
        if let Some(c) = category {
            e.category = c;
        }
        if let Some(d) = description {
            e.description = d;
        }
        if let Some(b) = behavior {
            e.behaviors = vec![b];
        }
    } else if let Some(i) = index_param(s, v, "delete")? {
        s.doc.glossary.remove(i);
    } else if let Some(i) = index_param(s, v, "insert")? {
        let e = s.doc.glossary.get(i).ok_or_else(|| CmdError::Params(format!("no building block {i}")))?;
        let behavior = behavior_of(e).to_string();
        let blocks: Vec<_> = e.blocks.iter().map(|b| (**b).clone()).collect();
        let media = s.doc.media_of_blocks(&blocks);
        let frag = Fragment { blocks, parts: Default::default(), media };
        insert_with_behavior(s, &frag, &behavior)?;
    }
    Ok(json!({ "entries": entries(s) }))
}

fn create(s: &mut Session, c: &Value) -> Result<(), CmdError> {
    let name = name_param(c)?.ok_or_else(|| CmdError::Params("a building block needs a name".into()))?;
    let gallery = gallery_param(c)?.unwrap_or_else(|| wordcraft_doc::QUICK_PARTS_GALLERY.to_string());
    let behavior = behavior_param(c)?.unwrap_or_else(|| "content".to_string());
    let category = text_param(c, "category").filter(|c| !c.trim().is_empty()).unwrap_or_else(|| "General".to_string());
    let description = text_param(c, "description").unwrap_or_default();
    let (a, b) = s.sel.ordered();
    if a == b {
        return Err(CmdError::Params("select the content to save".into()));
    }
    let blocks: wordcraft_doc::Blocks = s.doc.copy_range(&a, &b).blocks.into_iter().map(Arc::new).collect();
    let entry = BuildingBlock { name, gallery, category, description, behaviors: vec![behavior], blocks, ..Default::default() };
    // Same name in the same gallery: Word redefines the entry.
    match s.doc.glossary.iter_mut().find(|e| e.name == entry.name && e.gallery == entry.gallery) {
        Some(e) => {
            let BuildingBlock { category, description, behaviors, blocks, .. } = entry;
            (e.category, e.description, e.behaviors, e.blocks) = (category, description, behaviors, blocks);
        }
        None => s.doc.glossary.push(entry),
    }
    Ok(())
}

/// The organizer's list: every entry but content-control placeholders, by gallery, category and
/// name as Word lists them; `index` is the entry's place in `Document::glossary`.
fn entries(s: &Session) -> Vec<Value> {
    let mut list: Vec<(usize, &BuildingBlock)> =
        s.doc.glossary.iter().enumerate().filter(|(_, e)| !e.types.iter().any(|t| t == "bbPlcHdr")).collect();
    list.sort_by_cached_key(|(_, e)| (e.gallery.to_lowercase(), e.category.to_lowercase(), e.name.to_lowercase()));
    list.into_iter()
        .map(|(i, e)| {
            json!({"index": i, "name": e.name, "gallery": e.gallery, "category": e.category, "description": e.description, "behavior": behavior_of(e)})
        })
        .collect()
}

/// Insert `frag` in place of the selection the way a building block's `behavior` asks: inline
/// (`content`), as paragraphs of its own (`p`), or on a page of its own (`pg`). The caret ends
/// after it.
pub(super) fn insert_with_behavior(s: &mut Session, frag: &Fragment, behavior: &str) -> Result<(), CmdError> {
    let at = delete_selection(s)?;
    if !matches!(behavior, "p" | "pg") || frag.blocks.is_empty() {
        let end = s.doc.insert_fragment(&at, frag)?;
        s.sel = Selection::caret(end);
        return Ok(());
    }
    let len = s.doc.para_at(&at).map(Paragraph::len).ok_or_else(|| CmdError::Failed("no paragraph at caret".into()))?;
    let count = s.doc.container(at.story, &at.path).map(|c| c.len()).unwrap_or(0);
    let top = at.story == StoryRef::Body && at.path.0.len() == 1;
    // The block index the content goes at: before the paragraph at its start, after it at its
    // end, between its halves in the middle.
    let idx = if at.off == 0 {
        at.path.last()
    } else if at.off >= len {
        at.path.last().saturating_add(1)
    } else {
        s.doc.split_paragraph(&at)?;
        at.path.last().saturating_add(1)
    };
    let mut blocks = frag.blocks.clone();
    if behavior == "pg" {
        // A page break before it unless it starts the document, after it unless it ends it.
        let at_start = top && idx == 0;
        let after_split = at.off > 0 && at.off < len;
        let at_end = top && !after_split && idx >= count;
        if !at_start {
            match blocks.first_mut() {
                Some(Block::Para(p)) => {
                    p.insert_text(0, &PAGE_BREAK.to_string(), &CharProps::default())?;
                }
                _ => blocks.insert(0, Block::Para(Paragraph::with_text(&PAGE_BREAK.to_string(), CharProps::default()))),
            }
        }
        if !at_end {
            match blocks.last_mut() {
                Some(Block::Para(p)) => {
                    let end = p.len();
                    p.insert_text(end, &PAGE_BREAK.to_string(), &CharProps::default())?;
                }
                _ => blocks.push(Block::Para(Paragraph::with_text(&PAGE_BREAK.to_string(), CharProps::default()))),
            }
        }
    }
    // An empty paragraph to paste into, with the content's first paragraph formatting (a single
    // paragraph is pasted inline and keeps the target's).
    let mut host = Paragraph::new();
    if let Some(Block::Para(first)) = blocks.first() {
        host.props = first.props.clone();
    }
    let here = Pos { story: at.story, path: at.path.with_last(idx), off: 0 };
    s.doc.insert_block(at.story, &here.path, Block::Para(host))?;
    let end = s.doc.insert_fragment(&here, &Fragment { blocks, parts: frag.parts.clone(), media: frag.media.clone() })?;
    s.sel = Selection::caret(end);
    Ok(())
}

/// F3 (`insert.autoTextByName`): the word before the caret becomes the AutoText entry with that
/// name, or the only one whose name starts with it.
pub(super) fn auto_text_by_name(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    if a != b {
        return Err(CmdError::Disabled("no AutoText entry for a selection".into()));
    }
    let text = s.doc.para_at(&b).map(|p| p.text.clone()).unwrap_or_default();
    let before = text.get(..b.off).unwrap_or("");
    let start = before.char_indices().rev().find(|(_, c)| c.is_whitespace()).map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
    let word = before.get(start..).unwrap_or("").to_string();
    let none = || CmdError::Disabled(format!("no AutoText entry named {word}"));
    if word.is_empty() || word.contains(wordcraft_doc::para::OBJ) {
        return Err(none());
    }
    // AutoText: the document's own entries and this session's.
    let mut found: Vec<(String, Fragment, String)> = s
        .doc
        .glossary
        .iter()
        .filter(|e| e.gallery == wordcraft_doc::AUTO_TEXT_GALLERY)
        .map(|e| {
            (
                e.name.clone(),
                {
                    let blocks: Vec<_> = e.blocks.iter().map(|b| (**b).clone()).collect();
                    let media = s.doc.media_of_blocks(&blocks);
                    Fragment { blocks, parts: Default::default(), media }
                },
                behavior_of(e).to_string(),
            )
        })
        .collect();
    found.extend(s.building_blocks.iter().map(|(n, f)| (n.clone(), f.clone(), "content".to_string())));
    let lower = word.to_lowercase();
    let pick = match found.iter().position(|(n, ..)| n.to_lowercase() == lower) {
        Some(i) => Some(i),
        None => {
            let mut starts = found.iter().enumerate().filter(|(_, (n, ..))| n.to_lowercase().starts_with(&lower)).map(|(i, _)| i);
            match (starts.next(), starts.next()) {
                (Some(i), None) => Some(i),
                _ => None,
            }
        }
    };
    let (_, frag, behavior) = pick.and_then(|i| found.get(i)).cloned().ok_or_else(none)?;
    s.sel = Selection { anchor: Pos { off: start, ..b.clone() }, focus: b.clone() };
    insert_with_behavior(s, &frag, &behavior)?;
    Ok(json!({ "word": word }))
}
