//! LINK ProgID "FileName" [Bookmark] (ECMA-376 §17.16.5.32), the field Paste link inserts: on an
//! explicit update (F9) a link to a saved Word document reads the source file again and its result
//! becomes the bookmarked range (or the whole main text) as it is now. `\t` and `\u` give the text in
//! the result's own formatting, `\r` the range with its character formatting.
//!
//! Not built, so such links keep their stored result: other programs' links, `\h`, `\p` and `\b`
//! (HTML, picture, bitmap), a link without a format switch. A link that can't be followed (the file
//! is missing, unreadable, outside the automation read root, on the network or a URL, or lacks the
//! bookmark) also keeps its stored result, as Word keeps it when the source is missing. Fields in
//! the source are inserted as they are, and the source file is never written.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use wordcraft_doc::edit::Fragment;
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Block, Comment, Document, InlineObject, PartKind, Pos, StoryRef};

use super::FieldCode;
use crate::{CmdError, Session};

/// How a link shows its source.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Format {
    /// `\t` or `\u`: the text, in the result's formatting.
    Text,
    /// `\r`: the range with its character formatting.
    Formatted,
}

/// The source documents read during one update, by path: each file is read once even when several
/// fields link to it. `None` records a file that could not be read.
#[derive(Default)]
pub(super) struct Sources {
    read: HashMap<PathBuf, Option<Document>>,
    /// How many times a file was opened (for tests of the read-once rule).
    #[cfg(test)]
    opened: usize,
}

impl Sources {
    fn get(&mut self, path: PathBuf) -> Option<&Document> {
        #[cfg(test)]
        if !self.read.contains_key(&path) {
            self.opened = self.opened.saturating_add(1);
        }
        self.read.entry(path).or_insert_with_key(|p| crate::io::open_path(p).ok()).as_ref()
    }
}

/// A LINK field to a Word document that F9 can update.
struct Link {
    path: PathBuf,
    mark: Option<String>,
    format: Format,
}

/// The link `instr` describes, when it is one F9 updates; `None` (nothing looked at on disk) for
/// anything else.
fn parse(s: &Session, instr: &str) -> Option<Link> {
    let f = FieldCode::parse(instr);
    if f.name != "LINK" {
        return None;
    }
    let prog = f.args.first()?.trim().to_ascii_lowercase();
    if !prog.starts_with("word.document") {
        return None;
    }
    // The first format switch decides; HTML, picture and bitmap links are not built.
    let format = f.switches.iter().find_map(|(c, _)| match c {
        't' | 'u' => Some(Some(Format::Text)),
        'r' => Some(Some(Format::Formatted)),
        'h' | 'p' | 'b' => Some(None),
        _ => None,
    })??;
    let path = super::includetext::local_path(s, f.args.get(1)?)?;
    let mark = f.args.get(2).map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    Some(Link { path, mark, format })
}

/// The linked range of `doc`: the bookmark `mark` (found ignoring ASCII case), or the whole main
/// text. `None` when there is no such bookmark.
fn linked_range(doc: &Document, mark: Option<&str>) -> Option<Fragment> {
    let (from, to) = match mark {
        Some(name) => {
            let (found, start) = doc.bookmarks().into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
            crate::cmd::insert::bookmark_span(doc, &found, &start)
        }
        None => {
            let paths = doc.para_paths(StoryRef::Body);
            let first = paths.first()?.clone();
            let last = paths.last()?.clone();
            let len = doc.para(StoryRef::Body, &last).map(|p| p.len()).unwrap_or(0);
            (Pos { story: StoryRef::Body, path: first, off: 0 }, Pos { story: StoryRef::Body, path: last, off: len })
        }
    };
    Some(doc.copy_range(&from, &to))
}

/// A fragment's text, its paragraphs separated by `\r` (a break in a field result).
fn text_of(frag: &Fragment) -> String {
    let doc = frag.to_document();
    let paras: Vec<String> = doc.para_paths(StoryRef::Body).iter().filter_map(|p| doc.para(StoryRef::Body, p)).map(|p| p.plain_text()).collect();
    paras.join("\r")
}

/// The source's linked range for `link`, read through `sources`.
fn source_range(sources: &mut Sources, link: &Link) -> Option<Fragment> {
    let doc = sources.get(link.path.clone())?;
    linked_range(doc, link.mark.as_deref())
}

/// The source's linked range for `link` as a fragment of `s`'s document, as a paste brings it in:
/// the source's media, lists, styles, notes, comments and tracked changes it refers to come along
/// under ids of their own here, so they don't name the host's. `None` (nothing added to the host)
/// when the link can't be followed.
fn adopted_range(s: &mut Session, sources: &mut Sources, link: &Link) -> Option<Fragment> {
    let src = sources.get(link.path.clone())?;
    let frag = linked_range(src, link.mark.as_deref())?;
    // The source with just the range in its body: its tables stay for the range to refer to,
    // and the cached source stays whole for other fields.
    let mut only = src.clone();
    only.body = frag.blocks.into_iter().map(Arc::new).collect();
    keep_only_referred(&mut only);
    Some(crate::cmd::paste::adopt(s, only))
}

/// The comment ids (`CommentStart`/`CommentEnd`) and note story ids (`NoteRef`) the paragraphs of
/// `blocks` refer to, tables and the text boxes their shapes show (each story once) included.
fn references<'a>(doc: &'a Document, blocks: impl Iterator<Item = &'a Block>) -> (BTreeSet<u32>, BTreeSet<u32>) {
    let (mut comments, mut notes) = (BTreeSet::new(), BTreeSet::new());
    let mut boxes = BTreeSet::new();
    let mut todo: Vec<&Block> = blocks.collect();
    while let Some(b) = todo.pop() {
        match b {
            Block::Para(p) => {
                for o in &p.objects {
                    match o {
                        InlineObject::CommentStart { id } | InlineObject::CommentEnd { id } => {
                            comments.insert(*id);
                        }
                        InlineObject::NoteRef { id, .. } => {
                            notes.insert(*id);
                        }
                        _ => {}
                    }
                    for id in o.text_boxes() {
                        // Each story once, so a box shown from itself can't loop.
                        if boxes.insert(id)
                            && let Some(part) = doc.parts.get(&id).filter(|p| p.kind == PartKind::TextBox)
                        {
                            todo.extend(part.blocks.iter().map(|b| &**b));
                        }
                    }
                }
            }
            Block::Table(t) => todo.extend(t.rows.iter().flat_map(|r| r.cells.iter()).flat_map(|c| c.blocks.iter()).map(|b| &**b)),
        }
    }
    (comments, notes)
}

/// The comments in `marked` and every reply whose thread reaches one of them.
fn threads(comments: &BTreeMap<u32, Comment>, marked: &BTreeSet<u32>) -> BTreeSet<u32> {
    comments
        .iter()
        .filter(|(id, c)| {
            let mut at = (**id, c.parent);
            // A parent chain is at most as long as there are comments (a cycle stops there).
            for _ in 0..=comments.len() {
                if marked.contains(&at.0) {
                    return true;
                }
                let Some(parent) = at.1 else { return false };
                at = (parent, comments.get(&parent).and_then(|c| c.parent));
            }
            false
        })
        .map(|(id, _)| *id)
        .collect()
}

/// `doc` (a source whose body is just the linked range) without the comments and notes the range
/// doesn't refer to, so they don't come into the host with it.
fn keep_only_referred(doc: &mut Document) {
    let (marked, notes) = references(doc, doc.body.iter().map(|b| &**b));
    let kept = threads(&doc.comments, &marked);
    let dropped: Vec<u32> = doc.comments.keys().filter(|id| !kept.contains(id)).copied().collect();
    for id in dropped {
        if let Some(c) = doc.comments.remove(&id)
            && doc.parts.get(&c.part).is_some_and(|p| p.kind == PartKind::Comment)
        {
            doc.parts.remove(&c.part);
        }
    }
    doc.parts.retain(|id, p| !matches!(p.kind, PartKind::Footnote | PartKind::Endnote) || notes.contains(id));
}

/// After a result that referred to the comments `comments` and notes `notes` was replaced: those
/// (and their replies) that nothing in `doc` refers to any more go, with their stories.
fn drop_unreferred(doc: &mut Document, comments: &BTreeSet<u32>, notes: &BTreeSet<u32>) {
    if comments.is_empty() && notes.is_empty() {
        return;
    }
    let (mut marked, mut referred) = references(doc, doc.body.iter().map(|b| &**b));
    // Every story but text boxes (their shapes lead to them); a note's own story holds its mark,
    // which doesn't keep it.
    for (id, part) in doc.parts.iter().filter(|(_, p)| p.kind != PartKind::TextBox) {
        let (c, mut n) = references(doc, part.blocks.iter().map(|b| &**b));
        n.remove(id);
        marked.extend(c);
        referred.extend(n);
    }
    let kept = threads(&doc.comments, &marked);
    let gone = threads(&doc.comments, comments);
    for id in gone.difference(&kept) {
        if let Some(c) = doc.comments.remove(id)
            && doc.parts.get(&c.part).is_some_and(|p| p.kind == PartKind::Comment)
        {
            doc.parts.remove(&c.part);
        }
    }
    for id in notes.difference(&referred) {
        if doc.parts.get(id).is_some_and(|p| matches!(p.kind, PartKind::Footnote | PartKind::Endnote)) {
            doc.parts.remove(id);
        }
    }
}

/// The new result of the simple LINK field `instr`: the linked text (paragraphs joined by `\r`),
/// or `None` when the link is not one F9 updates or can't be followed, so the stored result stays.
pub(super) fn link_text(s: &Session, sources: &mut Sources, instr: &str) -> Option<String> {
    let link = parse(s, instr)?;
    source_range(sources, &link).map(|frag| text_of(&frag))
}

/// The new result of the simple LINK field `o`: as [`link_text`], and `None` too when the stored
/// result already is the linked text, so the field is not touched.
pub(super) fn new_simple_result(s: &Session, sources: &mut Sources, o: &wordcraft_doc::InlineObject) -> Option<String> {
    let wordcraft_doc::InlineObject::Field { instr, result, .. } = o else { return None };
    link_text(s, sources, instr).filter(|text| text != result)
}

/// F9 on LINK range fields (`FieldStart` … `FieldEnd`): the content between the markers becomes
/// the linked range as the source holds it now. Unlocked fields only; a link that can't be
/// followed keeps its result.
pub(super) fn update_link_ranges(s: &mut Session, sources: &mut Sources, stories: &[StoryRef]) -> Result<(), CmdError> {
    let width = OBJ.len_utf8();
    for &story in stories {
        // Last first, so replacing one result never moves a field still to do; re-read after each
        // edit, as a replaced result may have held other fields.
        let mut i = s.doc.field_ranges(story).len();
        while i > 0 {
            i -= 1;
            let Some(r) = s.doc.field_ranges(story).into_iter().nth(i) else { continue };
            if r.locked || wordcraft_layout::fields::field_name(&r.instr) != "LINK" {
                continue;
            }
            let Some(link) = parse(s, &r.instr) else { continue };
            let Some(inner) = r.start.off.checked_add(width) else { continue };
            let start = Pos { off: inner, ..r.start.clone() };
            match link.format {
                Format::Text => {
                    let Some(frag) = source_range(sources, &link) else { continue };
                    let text = text_of(&frag);
                    if text_of(&s.doc.copy_range(&start, &r.end)) == text {
                        continue;
                    }
                    let props = s.doc.para_at(&start).map(|p| p.props_of_char(start.off).clone()).unwrap_or_default();
                    let mut at = s.doc.delete_range(&start, &r.end)?;
                    for (k, part) in text.split('\r').enumerate() {
                        if k > 0 {
                            at = s.doc.split_paragraph(&at)?;
                        }
                        at = s.doc.insert_text(&at, part, &props)?;
                    }
                }
                Format::Formatted => {
                    let Some(frag) = adopted_range(s, sources, &link) else { continue };
                    let old = s.doc.copy_range(&start, &r.end);
                    let (comments, notes) = references(&s.doc, old.blocks.iter());
                    let at = s.doc.delete_range(&start, &r.end)?;
                    s.doc.insert_fragment(&at, &frag)?;
                    drop_unreferred(&mut s.doc, &comments, &notes);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::{CharProps, InlineObject};

    /// A source document saved at `path`: "Intro", then "Alpha" and "Beta" inside the bookmark
    /// `OLE_LINK1`, which spans the two paragraphs.
    fn save_source(path: &std::path::Path) {
        let mut doc = Document::from_text("Intro\nAlpha\nBeta");
        let props = CharProps::default();
        let paths = doc.para_paths(StoryRef::Body);
        let end = Pos { story: StoryRef::Body, path: paths[2].clone(), off: 4 };
        doc.insert_object(&end, InlineObject::BookmarkEnd { name: "OLE_LINK1".into() }, &props).unwrap();
        let start = Pos { story: StoryRef::Body, path: paths[1].clone(), off: 0 };
        doc.insert_object(&start, InlineObject::BookmarkStart { name: "OLE_LINK1".into() }, &props).unwrap();
        crate::io::save_path(path, &doc).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wc-link-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn code(path: &std::path::Path, rest: &str) -> String {
        format!("LINK Word.Document.12 \"{}\" {rest}", path.display().to_string().replace('\\', "\\\\"))
    }

    /// "Here: " then a range field `instr` with the result `old`, then " tail".
    fn host(instr: &str, old: &str) -> Session {
        let mut doc = Document::from_text("Here: ");
        let props = CharProps::default();
        let at = doc.insert_object(&Pos::body(0, 6), InlineObject::FieldStart { instr: instr.into(), locked: false, dirty: false }, &props).unwrap();
        let at = doc.insert_text(&at, old, &props).unwrap();
        let at = doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
        doc.insert_text(&at, " tail", &props).unwrap();
        Session::new(doc)
    }

    fn range_result(s: &Session) -> String {
        let r = s.doc.field_ranges(StoryRef::Body).into_iter().next().unwrap();
        text_of(&s.doc.copy_range(&Pos { off: r.start.off + OBJ.len_utf8(), ..r.start.clone() }, &r.end))
    }

    #[test]
    fn no_bookmark_links_the_whole_main_text_and_names_ignore_case() {
        let dir = scratch("whole");
        let src = dir.join("source.docx");
        save_source(&src);
        let s = Session::new(Document::from_text("x"));
        let mut sources = Sources::default();
        assert_eq!(link_text(&s, &mut sources, &code(&src, r"\a \t")).as_deref(), Some("Intro\rAlpha\rBeta"));
        assert_eq!(link_text(&s, &mut sources, &code(&src, r#""ole_link1" \a \t"#)).as_deref(), Some("Alpha\rBeta"));
        assert_eq!(sources.opened, 1, "the source is read once per update");
    }

    #[test]
    fn a_two_paragraph_range_fills_two_paragraphs() {
        let dir = scratch("paras");
        let src = dir.join("source.docx");
        save_source(&src);
        let mut s = host(&code(&src, r#""OLE_LINK1" \a \t"#), "OLD");
        let mut sources = Sources::default();
        update_link_ranges(&mut s, &mut sources, &[StoryRef::Body]).unwrap();
        assert_eq!(range_result(&s), "Alpha\rBeta");
        assert_eq!(s.doc.para_paths(StoryRef::Body).len(), 2);
    }

    #[test]
    fn an_equal_result_is_not_edited_and_several_links_read_the_source_once() {
        let dir = scratch("equal");
        let src = dir.join("source.docx");
        save_source(&src);
        let instr = code(&src, r#""OLE_LINK1" \a \t"#);
        let mut s = host(&instr, "Alpha");
        // Make the stored result already the source's text.
        let mut sources = Sources::default();
        update_link_ranges(&mut s, &mut sources, &[StoryRef::Body]).unwrap();
        let before = s.doc.clone();
        update_link_ranges(&mut s, &mut sources, &[StoryRef::Body]).unwrap();
        assert_eq!(s.doc, before, "no edit when the result already matches");
        assert_eq!(sources.opened, 1);
    }

    fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |_, _| image::Rgba([shade, 30, 30, 255]));
        let mut b = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
        b
    }

    /// F9 on every field of `s`, as `references.updateFields {"all": true}` does.
    fn update(s: &mut Session) {
        s.run("references.updateFields", &serde_json::json!({"all": true})).unwrap();
    }

    /// The objects in the host's field result (between its markers), in every paragraph.
    fn result_objects(s: &Session) -> Vec<InlineObject> {
        let r = s.doc.field_ranges(StoryRef::Body).into_iter().next().unwrap();
        let start = Pos { off: r.start.off + OBJ.len_utf8(), ..r.start.clone() };
        let frag = s.doc.copy_range(&start, &r.end);
        let mut out = Vec::new();
        for b in &frag.blocks {
            if let wordcraft_doc::Block::Para(p) = b {
                out.extend(p.objects.iter().cloned());
            }
        }
        out
    }

    #[test]
    fn a_formatted_list_link_brings_the_sources_list_not_the_hosts() {
        let dir = scratch("list");
        let src = dir.join("source.docx");
        let mut source = Session::new(Document::from_text("One\nTwo\nThree"));
        source.run("select.range", &serde_json::json!({"anchor": Pos::body(0, 0), "focus": Pos::body(2, 5)})).unwrap();
        source.run("para.numbering", &serde_json::json!({})).unwrap();
        let src_num = source.doc.para_at(&Pos::body(1, 0)).unwrap().props.numbering.unwrap().num;
        let src_format = source.doc.numbering.level(src_num, 0).unwrap().format;
        crate::io::save_path(&src, &source.doc).unwrap();

        let mut s = host(&code(&src, r"\a \r"), "OLD");
        let host_num = Arc::make_mut(&mut s.doc.numbering).add_list(wordcraft_doc::numbering::ListKind::Bullet);
        assert_eq!(host_num, src_num, "the test needs the same list id in both");
        let host_level = s.doc.numbering.level(host_num, 0).unwrap().clone();
        assert_ne!(host_level.format, src_format);
        update(&mut s);
        let two = s.doc.para_paths(StoryRef::Body).into_iter().find(|p| s.doc.para(StoryRef::Body, p).unwrap().plain_text() == "Two").unwrap();
        let num = s.doc.para(StoryRef::Body, &two).unwrap().props.numbering.unwrap().num;
        assert_ne!(num, host_num, "the source's list doesn't become the host's list");
        assert_eq!(s.doc.numbering.level(num, 0).unwrap().format, src_format, "the list keeps the source's format");
        assert_eq!(s.doc.numbering.level(host_num, 0), Some(&host_level), "the host's own list is unchanged");
    }

    #[test]
    fn a_formatted_picture_link_brings_the_sources_bytes() {
        let dir = scratch("pic");
        let src = dir.join("source.docx");
        let mut source = Session::new(Document::from_text("A"));
        source.run("caret.set", &serde_json::json!({"pos": Pos::body(0, 1)})).unwrap();
        let theirs = png(4, 4, 200);
        source.run("insert.picture", &serde_json::json!({"data": crate::cmd::insert::base64_encode(&theirs)})).unwrap();
        crate::io::save_path(&src, &source.doc).unwrap();
        let src_bytes = crate::io::open_path(&src).unwrap().media.values().next().unwrap().to_vec();

        let mut s = host(&code(&src, r"\a \r"), "OLD");
        let ours = s.doc.add_media(png(4, 4, 10), "png");
        assert!(crate::io::open_path(&src).unwrap().media.contains_key(&ours), "the test needs the same media key in both");
        update(&mut s);
        let key = result_objects(&s)
            .into_iter()
            .find_map(|o| match o {
                InlineObject::Image { media, .. } => Some(media),
                _ => None,
            })
            .unwrap();
        assert_eq!(s.doc.media.get(&key).map(|b| b.to_vec()), Some(src_bytes));
        assert_ne!(s.doc.media.get(&ours).map(|b| b.to_vec()), s.doc.media.get(&key).map(|b| b.to_vec()));
    }

    /// The text of the note story `id`.
    fn note_text(doc: &Document, id: u32) -> String {
        doc.para_paths(StoryRef::Part(id)).iter().filter_map(|p| doc.para(StoryRef::Part(id), p)).map(|p| p.plain_text()).collect()
    }

    #[test]
    fn a_formatted_footnote_link_brings_the_sources_note() {
        let dir = scratch("note");
        let src = dir.join("source.docx");
        let mut source = Session::new(Document::from_text("Noted"));
        source.run("caret.set", &serde_json::json!({"pos": Pos::body(0, 5)})).unwrap();
        source.run("references.footnote", &serde_json::json!({})).unwrap();
        source.run("text.insert", &serde_json::json!({"text": "source note"})).unwrap();
        crate::io::save_path(&src, &source.doc).unwrap();

        let mut s = host(&code(&src, r"\a \r"), "OLD");
        s.run("caret.set", &serde_json::json!({"pos": Pos::body(0, 0)})).unwrap();
        s.run("references.footnote", &serde_json::json!({})).unwrap();
        s.run("text.insert", &serde_json::json!({"text": "host note"})).unwrap();
        let host_note = s.doc.parts.iter().find(|(_, p)| p.kind == wordcraft_doc::PartKind::Footnote).map(|(k, _)| *k).unwrap();
        update(&mut s);
        let id = result_objects(&s)
            .into_iter()
            .find_map(|o| match o {
                InlineObject::NoteRef { id, .. } => Some(id),
                _ => None,
            })
            .unwrap();
        assert!(note_text(&s.doc, id).contains("source note"), "{:?}", note_text(&s.doc, id));
        assert!(note_text(&s.doc, host_note).contains("host note"));
    }

    #[test]
    fn updating_a_formatted_link_twice_gives_the_same_result() {
        let dir = scratch("twice");
        let src = dir.join("source.docx");
        save_source(&src);
        let mut s = host(&code(&src, r#""OLE_LINK1" \a \r"#), "OLD");
        update(&mut s);
        let (text, paras) = (range_result(&s), s.doc.para_paths(StoryRef::Body).len());
        assert_eq!(text, "Alpha\rBeta");
        update(&mut s);
        assert_eq!((range_result(&s), s.doc.para_paths(StoryRef::Body).len()), (text, paras));
    }

    #[test]
    fn a_link_that_cant_be_followed_adds_nothing_to_the_host() {
        let dir = scratch("nofollow");
        let src = dir.join("source.docx");
        let mut source = Session::new(Document::from_text("A"));
        source.run("para.numbering", &serde_json::json!({})).unwrap();
        crate::io::save_path(&src, &source.doc).unwrap();
        let mut s = host(&code(&src, r#""NoSuchMark" \a \r"#), "OLD");
        let (lists, media, parts) = (s.doc.numbering.nums.len(), s.doc.media.len(), s.doc.parts.len());
        update(&mut s);
        assert_eq!((s.doc.numbering.nums.len(), s.doc.media.len(), s.doc.parts.len()), (lists, media, parts));
        assert_eq!(range_result(&s), "OLD");
    }

    /// A source saved at `path`: "Outside", then "Linked" in the bookmark `OLE_LINK1`; with
    /// `comments` a comment on each ("secret remark", "kept remark"), else a footnote in each
    /// ("outside note", "inside note").
    fn save_marked_source(path: &std::path::Path, comments: bool) {
        let mut src = Session::new(Document::from_text("Outside\nLinked"));
        src.run("select.text", &serde_json::json!({"text": "Linked"})).unwrap();
        src.run("insert.bookmark", &serde_json::json!({"name": "OLE_LINK1"})).unwrap();
        if comments {
            for (word, remark) in [("Outside", "secret remark"), ("Linked", "kept remark")] {
                src.run("select.text", &serde_json::json!({"text": word})).unwrap();
                src.run("review.newComment", &serde_json::json!({"text": remark})).unwrap();
            }
        } else {
            for (pos, note) in [(Pos::body(0, 3), "outside note"), (Pos::body(1, OBJ.len_utf8() + 3), "inside note")] {
                src.run("caret.set", &serde_json::json!({"pos": pos})).unwrap();
                src.run("references.footnote", &serde_json::json!({})).unwrap();
                src.run("text.insert", &serde_json::json!({"text": note})).unwrap();
            }
        }
        crate::io::save_path(path, &src.doc).unwrap();
    }

    /// The text of every story of `doc`.
    fn all_text(doc: &Document) -> String {
        let mut out = doc.plain_text(StoryRef::Body);
        for id in doc.parts.keys() {
            out.push_str(&note_text(doc, *id));
        }
        out
    }

    #[test]
    fn a_formatted_link_brings_only_the_comments_in_its_range() {
        let dir = scratch("comments");
        let src = dir.join("source.docx");
        save_marked_source(&src, true);
        assert_eq!(crate::io::open_path(&src).unwrap().comments.len(), 2, "the source keeps both comments");
        let mut s = host(&code(&src, r#""OLE_LINK1" \a \r"#), "OLD");
        update(&mut s);
        assert_eq!(range_result(&s), "Linked");
        assert_eq!(s.doc.comments.len(), 1);
        let (id, c) = s.doc.comments.iter().next().unwrap();
        assert_eq!(note_text(&s.doc, c.part), "kept remark");
        let objs = result_objects(&s);
        assert!(objs.contains(&InlineObject::CommentStart { id: *id }) && objs.contains(&InlineObject::CommentEnd { id: *id }), "{objs:?}");
        assert!(!all_text(&s.doc).contains("secret remark"));

        // A second update replaces the first one's comment instead of adding another.
        update(&mut s);
        assert!(!all_text(&s.doc).contains("secret remark"));
        assert_eq!(s.doc.comments.len(), 1);
        let c = s.doc.comments.values().next().unwrap();
        assert_eq!(note_text(&s.doc, c.part), "kept remark");
        assert_eq!(s.doc.parts.values().filter(|p| p.kind == PartKind::Comment).count(), 1);
    }

    #[test]
    fn a_formatted_link_brings_only_the_notes_in_its_range() {
        let dir = scratch("notes");
        let src = dir.join("source.docx");
        save_marked_source(&src, false);
        let mut s = host(&code(&src, r#""OLE_LINK1" \a \r"#), "OLD");
        let footnotes = |d: &Document| d.parts.values().filter(|p| p.kind == PartKind::Footnote).count();
        let before = footnotes(&s.doc);
        update(&mut s);
        assert_eq!(footnotes(&s.doc), before + 1);
        let id = result_objects(&s)
            .into_iter()
            .find_map(|o| match o {
                InlineObject::NoteRef { id, .. } => Some(id),
                _ => None,
            })
            .unwrap();
        assert!(note_text(&s.doc, id).contains("inside note"));
        assert!(!all_text(&s.doc).contains("outside note"));
        update(&mut s);
        assert_eq!(footnotes(&s.doc), before + 1, "a second update replaces the note");
    }

    #[test]
    fn a_whole_document_link_brings_its_comment_once() {
        let dir = scratch("whole-comment");
        let src = dir.join("source.docx");
        let mut source = Session::new(Document::from_text("Only words"));
        source.run("select.text", &serde_json::json!({"text": "words"})).unwrap();
        source.run("review.newComment", &serde_json::json!({"text": "a remark"})).unwrap();
        crate::io::save_path(&src, &source.doc).unwrap();
        let mut s = host(&code(&src, r"\a \r"), "OLD");
        update(&mut s);
        assert_eq!(s.doc.comments.len(), 1);
        assert_eq!(note_text(&s.doc, s.doc.comments.values().next().unwrap().part), "a remark");
        update(&mut s);
        assert_eq!(s.doc.comments.len(), 1);
    }
}
