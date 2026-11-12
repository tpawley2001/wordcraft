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

use std::collections::HashMap;
use std::path::PathBuf;

use wordcraft_doc::edit::Fragment;
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Document, Pos, StoryRef};

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
            let Some(frag) = source_range(sources, &link) else { continue };
            let Some(inner) = r.start.off.checked_add(width) else { continue };
            let start = Pos { off: inner, ..r.start.clone() };
            match link.format {
                Format::Text => {
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
                    let at = s.doc.delete_range(&start, &r.end)?;
                    s.doc.insert_fragment(&at, &frag)?;
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
}
