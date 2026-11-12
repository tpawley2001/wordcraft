use super::numbers::{body_bookmark_paths, para_numbers, ref_number};
use super::properties::{
    BodyCounts, bookmark_ends_in_its_paragraph, bookmark_text, bookmark_text_at, count_text, date_picture, doc_property_arg,
    doc_property_names_pages, doc_property_result, new_property_value, set_summary_property, summary_property, text_result,
};
use super::toc::{heading_style_level, update_toc, update_toc_pages};
use super::{
    Block, CharProps, CmdError, CmdResult, FieldCode, InlineObject, NumFormat, Paragraph, Path, Pos, Selection, Session, StoryRef, Value, p,
    sel_result,
};

/// Core date fields keep their cached result when the property cannot identify an instant.
/// `default_picture` is used when the field has no `\@` picture.
fn document_date_result(instr: &str, value: &str, default_picture: &str) -> Option<String> {
    let date = chrono::DateTime::parse_from_rfc3339(value).ok()?;
    let unix = u64::try_from(date.timestamp()).ok()?;
    let picture = date_picture(instr).unwrap_or_else(|| default_picture.into());
    Some(text_result(instr, crate::cmd::insert::format_date_at(&picture, unix, crate::cmd::local_offset_secs(unix))))
}

/// The result Update Field gives a CREATEDATE, NUMWORDS, NUMCHARS, REVNUM or DOCPROPERTY field
/// (DOCPROPERTY <name>: a custom property first, then a built-in one, #1798), reading `core` for
/// the properties and `counts` for DOCPROPERTY's body counts. `None` keeps the stored result.
fn document_info_result(
    doc: &wordcraft_doc::Document,
    core: &wordcraft_doc::CoreProps,
    counts: &BodyCounts,
    name: &str,
    instr: &str,
) -> Option<String> {
    match name {
        "CREATEDATE" => document_date_result(instr, &core.created, "M/d/yyyy"),
        "NUMWORDS" => Some(count_text(instr, doc.word_count())),
        "NUMCHARS" => Some(count_text(instr, doc.plain_text(StoryRef::Body).chars().filter(|c| !c.is_whitespace()).count())),
        "REVNUM" => Some(count_text(instr, core.revision as usize)),
        "DOCPROPERTY" => doc_property_result(doc, core, counts, |value: &str, pic: &str| document_date_result(instr, value, pic), instr),
        _ => None,
    }
}

/// The body counts DOCPROPERTY fields show, when a field among `paths` is a DOCPROPERTY field;
/// the layout is taken only when one names `Pages`.
fn doc_property_counts(s: &mut Session, paths: &[(StoryRef, Path)]) -> BodyCounts {
    let instrs = || {
        paths.iter().filter_map(|(st, p)| s.doc.para(*st, p)).flat_map(|p| p.objects.iter()).filter_map(|o| match o {
            InlineObject::Field { instr, locked: false, .. } => Some(instr.as_str()),
            _ => None,
        })
    };
    if !instrs().any(|i| wordcraft_layout::fields::field_name(i) == "DOCPROPERTY") {
        return BodyCounts::default();
    }
    let needs_pages = instrs().any(doc_property_names_pages);
    let pages = needs_pages.then(|| s.layout().pages.len());
    BodyCounts::of(&s.doc, pages)
}

/// The `\*` format switch value for a caption's number format (OOXML names work too).
pub fn seq_format(name: &str) -> &'static str {
    match name {
        "alphabetic" | "lowerLetter" => "alphabetic",
        "ALPHABETIC" | "upperLetter" => "ALPHABETIC",
        "roman" | "lowerRoman" => "roman",
        "ROMAN" | "upperRoman" => "ROMAN",
        _ => "ARABIC",
    }
}

/// Separators Word offers between a chapter number and a caption number.
pub const CHAPTER_SEPARATORS: [&str; 5] = ["-", ".", ":", "\u{2014}", "\u{2013}"];

/// Insert a caption paragraph below (or above) the caret's paragraph, table or object: the label,
/// a chapter number (`STYLEREF n \s` and a separator) and a `SEQ label \* format \s n` number
/// (ECMA-376 §17.16.5.56, §17.16.5.59).
pub(super) fn caption(s: &mut Session, v: &Value) -> CmdResult {
    let label: String = p::str(v, "label").unwrap_or("Figure").chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).take(40).collect();
    let label = if label.trim().is_empty() { "Figure".to_string() } else { label.trim().to_string() };
    // SEQ identifiers are one word: Word joins a multi-word label with underscores.
    let ident = label.split_whitespace().collect::<Vec<_>>().join("_");
    let text = p::str(v, "text").unwrap_or("").to_string();
    // Without a position, the label's own (Insert Caption › Position, AutoCaption), else below.
    let above = match p::str(v, "position") {
        Some(pos) => pos == "above",
        None => s.doc.settings.captions.label(&label).is_some_and(|l| l.above),
    };
    let format = seq_format(p::str(v, "format").unwrap_or("ARABIC"));
    let chapter = p::u64(v, "chapter").filter(|l| (1..=9).contains(l));
    let sep = p::str(v, "separator").and_then(|x| CHAPTER_SEPARATORS.iter().find(|s| **s == x)).copied().unwrap_or("-");
    let mut para = Paragraph::new().styled("Caption");
    let field = |instr: String| InlineObject::Field {
        instr,
        result: String::new(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    };
    if !p::bool(v, "excludeLabel").unwrap_or(false) {
        para.insert_text(0, &format!("{label} "), &CharProps::default())?;
    }
    let mut seq = format!("SEQ {ident} \\* {format}");
    if let Some(l) = chapter {
        para.insert_object(para.len(), field(format!("STYLEREF {l} \\s")), &CharProps::default())?;
        para.insert_text(para.len(), sep, &CharProps::default())?;
        seq.push_str(&format!(" \\s {l}"));
    }
    para.insert_object(para.len(), field(seq), &CharProps::default())?;
    if !text.is_empty() {
        para.insert_text(para.len(), &format!(": {text}"), &CharProps::default())?;
    }
    // A selected picture, shape or chart: next to the paragraph holding it, in the same cell or
    // text box (#1230). Otherwise next to the caret's top-level block: a paragraph, or the whole
    // table it is in.
    let f = s.sel.focus.clone();
    let at = match crate::cmd::objects::object_selection(s) {
        Some((pos, _)) => {
            let k = pos.path.last();
            pos.path.with_last(if above { k } else { k.saturating_add(1) })
        }
        None => {
            let top = f.path.0.first().copied().unwrap_or(0) as usize;
            Path::top(if above { top } else { top.saturating_add(1) })
        }
    };
    s.doc.insert_block(f.story, &at, Block::Para(para))?;
    update_seq(s)?;
    let off = s.doc.para(f.story, &at).map_or(0, |p| p.len());
    s.sel = Selection::caret(Pos { story: f.story, path: at, off });
    sel_result(s)
}

/// Renumber the body's LISTNUM fields and SEQ fields in document order. SEQ honours
/// `\* format`, `\s level` (restart after each heading of that level), `\r n` (reset to n),
/// `\c` (repeat) and `\h` (hide). LISTNUM uses shared level counters and `\l` / `\s`. Also
/// fill body STYLEREF fields ([`wordcraft_layout::styleref`]): chapter numbers (`STYLEREF n \s`,
/// the last level-n heading's number, or its count when the heading isn't numbered) and the
/// text or number of the nearest paragraph of a style.
pub fn update_seq(s: &mut Session) -> Result<(), CmdError> {
    // Numbers first: a STYLEREF to a heading that holds a SEQ field shows its new number.
    let numbers = seq_results(&s.doc);
    apply_results(&mut s.doc, numbers)?;
    let refs = styleref_results(&s.doc);
    apply_results(&mut s.doc, refs)
}

/// A field's fresh result: its story and paragraph, its object index there and the text.
type FieldResult = (StoryRef, Path, usize, String);

/// LISTNUM's shared multilevel counters, independent of SEQ identifiers and restarts.
#[derive(Default)]
struct ListNumCount {
    levels: [u32; 9],
    last_heading: u8,
}

impl ListNumCount {
    fn result(&mut self, field: &FieldCode, heading: Option<u8>) -> String {
        let level = field
            .arg('l')
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|l| (1..=9).contains(l))
            .unwrap_or_else(|| heading.unwrap_or_else(|| self.last_heading.saturating_add(1).min(9)));
        let index = usize::from(level.saturating_sub(1));
        let start = field.arg('s').and_then(|v| v.parse::<u32>().ok());
        for (i, n) in self.levels.iter_mut().enumerate() {
            if i < index {
                if *n == 0 {
                    *n = 1;
                }
            } else if i == index {
                *n = start.unwrap_or_else(|| n.saturating_add(1));
            } else {
                *n = 0;
            }
        }
        let name = field.args.first().map(String::as_str).unwrap_or("NumberDefault");
        if name.eq_ignore_ascii_case("LegalDefault") {
            let parts: Vec<String> = self.levels.iter().take(usize::from(level)).map(u32::to_string).collect();
            return format!("{}.", parts.join("."));
        }
        let (fmt, prefix, suffix) = if name.eq_ignore_ascii_case("OutlineDefault") {
            outline_format(level)
        } else {
            let fmt = match level {
                2 | 5 | 8 => NumFormat::LowerLetter,
                3 | 6 | 9 => NumFormat::LowerRoman,
                _ => NumFormat::Decimal,
            };
            let (prefix, suffix) = match level {
                4..=6 => ("(", ")"),
                7..=9 => ("", "."),
                _ => ("", ")"),
            };
            (fmt, prefix, suffix)
        };
        let n = self.levels.get(index).copied().unwrap_or(0);
        format!("{prefix}{}{suffix}", fmt.format(n))
    }
}

/// The number format, prefix and suffix of a level in the outline style (`OutlineDefault`).
fn outline_format(level: u8) -> (NumFormat, &'static str, &'static str) {
    match level {
        1 => (NumFormat::UpperRoman, "", "."),
        2 => (NumFormat::UpperLetter, "", "."),
        3 => (NumFormat::Decimal, "", "."),
        4 => (NumFormat::LowerLetter, "", ")"),
        5 => (NumFormat::Decimal, "(", ")"),
        6 | 8 => (NumFormat::LowerLetter, "(", ")"),
        _ => (NumFormat::LowerRoman, "(", ")"),
    }
}

/// AUTONUM, AUTONUMLGL and AUTONUMOUT's shared level counters (ECMA-376 §17.16.5.4-6), separate
/// from LISTNUM's and SEQ's.
#[derive(Default)]
struct AutoNumCount {
    levels: [u32; 9],
    last_heading: u8,
}

impl AutoNumCount {
    /// Count a field of `name` in a paragraph of heading level `heading` and give its result.
    fn result(&mut self, name: &str, field: &FieldCode, heading: Option<u8>) -> String {
        let level = heading.unwrap_or_else(|| self.last_heading.saturating_add(1).min(9));
        let index = usize::from(level.saturating_sub(1));
        for (i, n) in self.levels.iter_mut().enumerate() {
            if i == index {
                *n = n.saturating_add(1);
            } else if i > index {
                *n = 0;
            }
        }
        let n = self.levels.get(index).copied().unwrap_or(0);
        match name {
            "AUTONUMLGL" => {
                let parts: Vec<String> = self.levels.iter().take(usize::from(level)).map(|n| n.max(&1).to_string()).collect();
                let sep = if field.has('e') { "" } else { "." };
                format!("{}{sep}", parts.join("."))
            }
            "AUTONUMOUT" => {
                let (fmt, prefix, suffix) = outline_format(level);
                format!("{prefix}{}{suffix}", fmt.format(n))
            }
            _ => {
                let sep = field.arg('s').and_then(|s| s.chars().next()).unwrap_or('.');
                format!("{n}{sep}")
            }
        }
    }
}

/// The SEQ numbering so far while the document is walked in order.
#[derive(Default)]
struct SeqCount {
    listnum: ListNumCount,
    autonum: AutoNumCount,
    /// How many headings of each level there have been (for `\s`).
    headings: [u32; 9],
    /// Per sequence: its number, and the heading count it was last restarted at.
    seq: std::collections::HashMap<String, (u32, u32)>,
    /// Text boxes already counted: a story shown twice, or inside itself, counts once.
    boxes: std::collections::HashSet<u32>,
    out: Vec<FieldResult>,
}

/// The fresh results of the unlocked LISTNUM and SEQ fields in the body and in the text boxes anchored there
/// ([`update_seq`]). A text box's fields count where the box is anchored, before the fields after
/// the anchor in its paragraph and after those before it (measured in Word for Windows). A locked
/// SEQ keeps its result but still counts, `\r` included; a SEQ in a tracked deletion keeps its
/// result and doesn't count, with markup shown or not (measured in Word for Windows, exporting).
fn seq_results(doc: &wordcraft_doc::Document) -> Vec<FieldResult> {
    let mut count = SeqCount::default();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        if let Some(l) = heading_style_level(p) {
            count.listnum.last_heading = l;
            count.autonum.last_heading = l;
        }
        if let Some(l) = heading_style_level(p)
            && let Some(k) = (l as usize).checked_sub(1)
            && let Some(n) = count.headings.get_mut(k)
        {
            *n = n.saturating_add(1);
        }
        count_seq(doc, StoryRef::Body, &path, p, &mut count);
    }
    count.out
}

/// Number the SEQ fields of paragraph `p` (at `path` in `story`) and of the text boxes anchored in it.
fn count_seq(doc: &wordcraft_doc::Document, story: StoryRef, path: &Path, p: &Paragraph, count: &mut SeqCount) {
    // The runs come in order and the objects by offset, so one cursor walks the deleted ranges.
    let mut deleted = p.deleted_ranges().into_iter().peekable();
    for (k, (off, o)) in p.objects_with_offsets().enumerate() {
        while deleted.next_if(|r| r.end <= off).is_some() {}
        let in_deletion = deleted.peek().is_some_and(|r| r.contains(&off));
        for id in o.text_boxes() {
            if count.boxes.insert(id) {
                let part = StoryRef::Part(id);
                for bp in doc.para_paths(part) {
                    if let Some(boxed) = doc.para(part, &bp) {
                        count_seq(doc, part, &bp, boxed, count);
                    }
                }
            }
        }
        let InlineObject::Field { instr, locked, .. } = o else { continue };
        let f = FieldCode::parse(instr);
        // A locked LISTNUM or AUTONUM keeps its result and doesn't count; only SEQ counts locked.
        if *locked && f.name != "SEQ" {
            continue;
        }
        if f.name == "LISTNUM" {
            // A LISTNUM in a tracked deletion keeps its result and doesn't count, as SEQ does.
            if in_deletion {
                continue;
            }
            let result = count.listnum.result(&f, heading_style_level(p));
            count.out.push((story, path.clone(), k, result));
            continue;
        }
        if matches!(f.name.as_str(), "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT") {
            let result = count.autonum.result(&f.name, &f, heading_style_level(p));
            count.out.push((story, path.clone(), k, result));
            continue;
        }
        if f.name != "SEQ" || in_deletion {
            continue;
        }
        let Some(id) = f.args.first() else { continue };
        let restart = f.arg('s').and_then(|l| l.parse::<usize>().ok()).and_then(|l| l.checked_sub(1)).and_then(|l| count.headings.get(l)).copied();
        let n = count.seq.entry(id.clone()).or_insert((0, 0));
        if let Some(epoch) = restart
            && n.1 != epoch
        {
            *n = (0, epoch);
        }
        if let Some(r) = f.arg('r').and_then(|r| r.parse::<u32>().ok()) {
            n.0 = r;
        } else if !f.has('c') {
            n.0 = n.0.saturating_add(1);
        }
        if *locked {
            continue;
        }
        let fmt = wordcraft_layout::fields::format_switch(instr).unwrap_or_default();
        let r = if f.has('h') { String::new() } else { wordcraft_layout::fields::field_number(instr, fmt, n.0) };
        count.out.push((story, path.clone(), k, r));
    }
}

/// The fresh results of the body's unlocked STYLEREF fields ([`update_seq`]), from the text
/// `doc` holds: the SEQ fields in the headings they find should already show their numbers.
fn styleref_results(doc: &wordcraft_doc::Document) -> Vec<FieldResult> {
    let mut out = Vec::new();
    // Built on the first STYLEREF.
    let mut styles: Option<wordcraft_layout::styleref::StyleIndex> = None;
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        for (k, o) in p.objects.iter().enumerate() {
            let InlineObject::Field { instr, locked: false, .. } = o else { continue };
            if FieldCode::parse(instr).name != "STYLEREF" {
                continue;
            }
            let ix = styles.get_or_insert_with(|| wordcraft_layout::styleref::StyleIndex::build(doc));
            if let Some(i) = ix.index_of(&path) {
                out.push((StoryRef::Body, path.clone(), k, ix.eval(instr, wordcraft_layout::styleref::At::Body(i))));
            }
        }
    }
    out
}

/// What Word prints for a `REF` to a bookmark the document doesn't have.
pub const REFERENCE_NOT_FOUND: &str = "Error! Reference source not found.";

/// What Word shows for a `REF` that sits inside the bookmark it names.
pub const BOOKMARK_SELF_REFERENCE: &str = "Error! Not a valid bookmark self-reference.";

/// What Word shows for a `REF`, `PAGEREF` or `NOTEREF` that names no bookmark.
pub const NO_BOOKMARK_NAME: &str = "Error! No bookmark name given.";

/// What Word prints for a `DOCVARIABLE` field without a variable name.
pub const DOC_VARIABLE_NOT_DEFINED: &str = "Error! Document Variable not defined.";

/// What Word prints for a `DOCVARIABLE` field whose variable is missing or empty.
pub const NO_DOC_VARIABLE: &str = "Error! No document variable supplied.";

/// What Word shows for a `PAGEREF` or `NOTEREF` to a bookmark the document doesn't have.
pub const BOOKMARK_NOT_DEFINED: &str = "Error! Bookmark not defined.";

/// The fresh results of the body's unlocked REF fields as output shows them: the bookmark's
/// text, or Word's error when the document lacks the bookmark (measured in Word for Windows);
/// `\p` ("above" or "below") and the paragraph number switches `\r`, `\n` and `\w` keep the
/// stored result, and so does a REF to a bookmark that ends in a later paragraph: Word shows such
/// a bookmark's paragraphs as paragraphs of the result, which a field's one-paragraph result
/// can't hold (measured in Word for Windows). A bookmark in a header, footer, footnote, endnote,
/// comment or text box is found too, and a REF to it shows that text as `stored` holds it, its
/// own fields not yet refreshed, as Word's output does (measured in Word for Windows). The
/// document's bookmarks are looked up once, and each bookmark's text read once, however many
/// fields refer to it. A REF inside the bookmark it names shows Word's self-reference error, and
/// one that names no bookmark Word's error for that, whatever the switches.
fn ref_results(doc: &wordcraft_doc::Document, stored: &wordcraft_doc::Document) -> Vec<FieldResult> {
    let mut out = Vec::new();
    let mut starts: std::collections::HashMap<String, Pos> = std::collections::HashMap::new();
    for (name, start) in doc.bookmarks().into_iter().chain(part_bookmarks(doc)) {
        // The first of two bookmarks with one name, as `bookmark_text` takes.
        starts.entry(name).or_insert(start);
    }
    // A bookmark's text, `None` when it ends in a later paragraph.
    let mut texts: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let mut ranges = None;
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let offsets: Vec<usize> = p.objects_with_offsets().map(|(off, _)| off).collect();
        for (k, o) in p.objects.iter().enumerate() {
            let InlineObject::Field { instr, locked: false, .. } = o else { continue };
            if wordcraft_layout::fields::field_name(instr) != "REF" {
                continue;
            }
            let f = FieldCode::parse(instr);
            let target = f.args.first().map(|s| s.split('\\').next().unwrap_or("").trim_matches('"')).unwrap_or("");
            if target.is_empty() {
                out.push((StoryRef::Body, path.clone(), k, NO_BOOKMARK_NAME.to_string()));
                continue;
            }
            if let Some(off) = offsets.get(k) {
                let ranges = ranges.get_or_insert_with(|| BookmarkRanges::build(doc));
                if let Some(err) = self_reference_error("REF", instr, target, (StoryRef::Body, &path, *off), ranges) {
                    out.push((StoryRef::Body, path.clone(), k, err.to_string()));
                    continue;
                }
            }
            // `\r`, `\n` and `\w` show the bookmarked paragraph's number, which isn't computed
            // here: those keep their stored result rather than print the paragraph's text.
            if ['p', 'r', 'n', 'w'].iter().any(|&sw| has_switch(instr, sw)) {
                continue;
            }
            let Some(start) = starts.get(target) else {
                out.push((StoryRef::Body, path.clone(), k, REFERENCE_NOT_FOUND.to_string()));
                continue;
            };
            let text = texts.entry(target.to_string()).or_insert_with(|| {
                let source = if start.story == StoryRef::Body { doc } else { stored };
                bookmark_ends_in_its_paragraph(source, target, start).then(|| bookmark_text_at(source, target, start)).flatten()
            });
            if let Some(r) = text.clone() {
                out.push((StoryRef::Body, path.clone(), k, text_result(instr, r)));
            }
        }
    }
    out
}

/// The bookmarks outside the body (headers, footers, notes, comments and text boxes), in story
/// order.
fn part_bookmarks(doc: &wordcraft_doc::Document) -> Vec<(String, Pos)> {
    let mut out = Vec::new();
    for id in doc.parts.keys() {
        let story = StoryRef::Part(*id);
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            for (off, o) in p.objects_with_offsets() {
                if let InlineObject::BookmarkStart { name } = o {
                    out.push((name.clone(), Pos { story, path: path.clone(), off }));
                }
            }
        }
    }
    out
}

/// Put `results` into their fields, clearing `w:dirty` (a computed result is no longer stale:
/// Word's F9 clears it too).
fn apply_results(doc: &mut wordcraft_doc::Document, results: Vec<FieldResult>) -> Result<(), CmdError> {
    for (story, path, k, r) in results {
        let para = doc.para_mut(story, &path)?;
        if let Some(InlineObject::Field { result, dirty, .. }) = para.objects.get_mut(k) {
            *result = r;
            *dirty = false;
        }
        para.touch();
    }
    Ok(())
}

/// `results` less those its fields already show.
fn changed(doc: &wordcraft_doc::Document, mut results: Vec<FieldResult>) -> Vec<FieldResult> {
    results.retain(|(story, path, k, r)| {
        !matches!(doc.para(*story, path).and_then(|p| p.objects.get(*k)), Some(InlineObject::Field { result, .. }) if result == r)
    });
    results
}

/// The document as Word prints and exports it (PDF, print jobs, page images). Laying a document
/// out for output, Word fills in the body's unlocked LISTNUM, SEQ, STYLEREF and REF fields, even with
/// "Update fields before printing" off, while NUMWORDS, FILENAME, the summary properties and
/// formulas keep their stored results (measured with Word for Windows, printing and exporting
/// fields whose stored results were wrong). In headers and footers, the summary properties
/// (TITLE, AUTHOR ...), LASTSAVEDBY, DATE, TIME, a QUOTE of one quoted text, NUMWORDS, NUMCHARS,
/// REVNUM, CREATEDATE and DOCPROPERTY show their current
/// values too, while the same fields in the body and in notes keep theirs (measured with
/// Microsoft 365 Word's PDF export). The document itself keeps its stored results.
/// `None` when every such field already shows its fresh result, so nothing is copied then.
pub fn output_fields(doc: &wordcraft_doc::Document) -> Option<wordcraft_doc::Document> {
    // Numbers first, then the fields that read them: a STYLEREF to a heading holding a SEQ field,
    // and a REF to a bookmarked caption, show the new number.
    let seq = changed(doc, seq_results(doc));
    let mut out = (!seq.is_empty()).then(|| doc.clone());
    if let Some(d) = out.as_mut() {
        apply_results(d, seq).ok()?;
    }
    let styles = changed(out.as_ref().unwrap_or(doc), styleref_results(out.as_ref().unwrap_or(doc)));
    if !styles.is_empty() {
        apply_results(out.get_or_insert_with(|| doc.clone()), styles).ok()?;
    }
    let refs = changed(out.as_ref().unwrap_or(doc), ref_results(out.as_ref().unwrap_or(doc), doc));
    if !refs.is_empty() {
        apply_results(out.get_or_insert_with(|| doc.clone()), refs).ok()?;
    }
    let parts = changed(out.as_ref().unwrap_or(doc), header_footer_results(out.as_ref().unwrap_or(doc)));
    if !parts.is_empty() {
        apply_results(out.get_or_insert_with(|| doc.clone()), parts).ok()?;
    }
    out
}

/// The fresh results output shows for the unlocked fields in every header and footer (each
/// section's default, first-page and even-page ones, table cells included): the summary
/// properties and LASTSAVEDBY, DATE and TIME, a QUOTE of one quoted text, and NUMWORDS,
/// NUMCHARS, REVNUM, CREATEDATE and DOCPROPERTY (but `Pages`), each as Update
/// Field in that header would store it (Word's PDF export shows them current, measured with
/// Microsoft 365 Word). A summary property field that gives the property a new value keeps its
/// result: output never changes a property. Every other field keeps what it shows.
fn header_footer_results(doc: &wordcraft_doc::Document) -> Vec<FieldResult> {
    use wordcraft_doc::PartKind;
    let mut out = Vec::new();
    // DOCPROPERTY's body counts, taken once when a field needs them.
    let mut counts = None;
    for (id, part) in &doc.parts {
        if !matches!(part.kind, PartKind::Header | PartKind::Footer) {
            continue;
        }
        let story = StoryRef::Part(*id);
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            for (k, o) in p.objects.iter().enumerate() {
                let InlineObject::Field { instr, locked: false, .. } = o else { continue };
                let name = wordcraft_layout::fields::field_name(instr);
                let fresh = match name.as_str() {
                    "TITLE" | "SUBJECT" | "AUTHOR" | "KEYWORDS" | "COMMENTS" if new_property_value(instr).is_none() => {
                        Some(text_result(instr, summary_property(&doc.core, &name).to_string()))
                    }
                    "LASTSAVEDBY" => Some(text_result(instr, doc.core.last_modified_by.clone())),
                    "DATE" | "TIME" => {
                        let lang = field_lang(doc, p, k);
                        let pic = date_picture(instr).unwrap_or_else(|| default_picture(&name, lang.as_deref()));
                        Some(text_result(instr, crate::cmd::insert::format_date_in(&pic, lang.as_deref())))
                    }
                    "QUOTE" => quote_literal(instr).map(|text| text_result(instr, text)),
                    // Output has no layout to count pages from: DOCPROPERTY Pages keeps its result.
                    "DOCPROPERTY" if doc_property_names_pages(instr) => None,
                    "CREATEDATE" | "NUMWORDS" | "NUMCHARS" | "REVNUM" | "DOCPROPERTY" => {
                        let counts = counts.get_or_insert_with(|| BodyCounts::of(doc, None));
                        document_info_result(doc, &doc.core, counts, &name, instr)
                    }
                    _ => None,
                };
                out.extend(fresh.map(|r| (story, path.clone(), k, r)));
            }
        }
    }
    out
}

/// The text of a `QUOTE "text"` field: exactly one quoted argument and no switch but `\*`.
/// `None` for any other form (unquoted words, several arguments, picture switches).
fn quote_literal(instr: &str) -> Option<String> {
    let f = FieldCode::parse(instr);
    if f.name != "QUOTE" || f.switches.iter().any(|(c, _)| *c != '*') {
        return None;
    }
    let [text] = f.args.as_slice() else { return None };
    let words = super::field_words(instr);
    match words.get(1) {
        Some((w, true)) if w == text => Some(text.clone()),
        _ => None,
    }
}

/// Whether `instr` has the one-letter switch `\sw`, also run together with another (`\p\h`).
pub(super) fn has_switch(instr: &str, sw: char) -> bool {
    instr.split('\\').skip(1).any(|s| {
        let mut chars = s.chars();
        chars.next().is_some_and(|c| c.eq_ignore_ascii_case(&sw)) && chars.next().is_none_or(|c| !c.is_alphanumeric())
    })
}

/// Update fields of the story being edited, as Word's F9 does: dates, REF/PAGEREF, the document
/// information fields (TITLE, SUBJECT, AUTHOR, KEYWORDS, COMMENTS, LASTSAVEDBY, REVNUM, NUMWORDS,
/// NUMCHARS, FILENAME, DOCPROPERTY, DOCVARIABLE) in the body, or in the header, footer, note or text box the
/// caret is in (the body's fields stay as they are then). SEQ numbering, citations and TOC are
/// body-only, so they run only when the caret is in the body.
pub fn update_fields(s: &mut Session) -> Result<(), CmdError> {
    update_fields_in(s, false, false)
}

/// Each section's page number format (`w:pgNumType w:fmt`), by section index.
pub(crate) fn page_formats(s: &Session) -> Vec<NumFormat> {
    s.doc.sections().into_iter().map(|(_, sec)| sec.page_num_format).collect()
}

/// The page holding `pos` in `l`: its index in the layout, its number, and its section's page
/// number format (`formats` from [`page_formats`]). `None` when no page holds it.
pub(crate) fn page_at<'a>(l: &'a wordcraft_layout::DocLayout, formats: &[NumFormat], pos: &Pos) -> Option<(usize, u32, NumFormat, &'a str)> {
    let c = l.caret(pos)?;
    let page = l.pages.get(c.page)?;
    Some((c.page, page.number, formats.get(page.section).copied().unwrap_or_default(), &page.chapter))
}

/// A page number as the page's footer shows it (`ii` in a lowerRoman section), as the `PAGE`
/// and `PAGEREF` fields write it without a `\*` switch.
pub(crate) fn page_text(number: u32, fmt: NumFormat, chapter: &str) -> String {
    wordcraft_layout::fields::page_label("", fmt, number, chapter)
}

/// The page number of the page holding `pos`, in its section's number format: what generated
/// lists (TOC, index, tables of figures and authorities) and page cross-references write. Page
/// "1" when no page holds it.
pub(crate) fn page_label(s: &mut Session, pos: &Pos) -> String {
    let formats = page_formats(s);
    let l = s.layout();
    page_at(&l, &formats, pos).map_or_else(|| "1".into(), |(_, n, fmt, chapter)| page_text(n, fmt, chapter))
}

/// Where `PAGEREF` reads bookmark `name` (starting at `pos`): a hidden `_Ref` bookmark from the
/// start of a heading that starts with page breaks reads the page its text is on, as when its
/// cross-reference was inserted.
pub(super) fn bookmark_page_pos(doc: &wordcraft_doc::Document, name: &str, pos: Pos) -> Pos {
    if pos.off == 0 && name.starts_with("_Ref") {
        let off = crate::cmd::citations::leading_breaks(doc, &pos);
        return Pos { off, ..pos };
    }
    pos
}

/// Bookmark name → where it starts (the first of a name).
fn bookmark_starts(marks: &[(String, Pos)]) -> std::collections::HashMap<String, Pos> {
    let mut at: std::collections::HashMap<String, Pos> = Default::default();
    for (name, pos) in marks {
        at.entry(name.clone()).or_insert_with(|| pos.clone());
    }
    at
}

/// The result of a REF or NOTEREF with `\\p` (where the bookmark is from the field `here`,
/// ECMA-376 §17.16.5.40, §17.16.5.51; body fields only) or of a NOTEREF (the note's mark):
/// `None` when the field is neither.
fn position_or_note(
    doc: &wordcraft_doc::Document,
    (name, instr, target): (&str, &str, &str),
    here: Option<Pos>,
    bookmark_at: &std::collections::HashMap<String, Pos>,
    note_marks: &mut Option<std::collections::HashMap<u32, String>>,
) -> Option<Option<String>> {
    // `\p` with a number switch (`\r \p`: "1 above") keeps its result.
    if name != "PAGEREF" && has_switch(instr, 'p') {
        if ['w', 'r', 'n'].iter().any(|&c| has_switch(instr, c)) {
            return Some(None);
        }
        let at = bookmark_at.get(target);
        return Some(match (at, here) {
            (Some(at), Some(here)) if here.story == StoryRef::Body => Some(if *at < here { "above" } else { "below" }.to_string()),
            _ => None,
        });
    }
    if name == "NOTEREF" {
        let marks = note_marks.get_or_insert_with(|| wordcraft_layout::note_marks(doc));
        return Some(bookmark_at.get(target).and_then(|at| noteref_mark(doc, at, marks)));
    }
    None
}

/// Word's error text for a REF, PAGEREF or NOTEREF whose bookmark `target` is in no story of the
/// document (`names`), or that names no bookmark, measured in Word: `None` when the bookmark
/// exists, or when a SET field gave the REF's name a value (`set`, `last_set`, keyed by
/// the lowercased name).
fn missing_bookmark_error(
    name: &str,
    target: &str,
    names: &std::collections::HashSet<String>,
    (set, last_set): (&std::collections::HashMap<String, String>, &std::collections::HashMap<String, String>),
) -> Option<&'static str> {
    if target.is_empty() {
        return Some(NO_BOOKMARK_NAME);
    }
    if names.contains(target) {
        return None;
    }
    if name != "REF" {
        return Some(BOOKMARK_NOT_DEFINED);
    }
    let key = target.to_lowercase();
    (!set.contains_key(&key) && !last_set.contains_key(&key)).then_some(REFERENCE_NOT_FOUND)
}

/// Where each bookmark lies, for [`self_reference_error`]: built once per update, however many
/// fields there are.
struct BookmarkRanges {
    /// Each story's paragraphs → their place in the story, in document order.
    order: std::collections::HashMap<StoryRef, std::collections::HashMap<Path, usize>>,
    /// Bookmark name (the first of a name, body first) → its story and the (paragraph, offset) of
    /// its `BookmarkStart` and of its matching `BookmarkEnd` (the end of the start paragraph when
    /// it has none).
    ranges: std::collections::HashMap<String, (StoryRef, (usize, usize), (usize, usize))>,
}

impl BookmarkRanges {
    fn build(doc: &wordcraft_doc::Document) -> BookmarkRanges {
        let mut order: std::collections::HashMap<StoryRef, std::collections::HashMap<Path, usize>> = Default::default();
        let mut ranges: std::collections::HashMap<String, (StoryRef, (usize, usize), (usize, usize))> = Default::default();
        let stories = std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|id| StoryRef::Part(*id)));
        for story in stories {
            // Bookmarks started in this story whose end hasn't been seen yet.
            let mut open: std::collections::HashSet<String> = Default::default();
            let paras = order.entry(story).or_default();
            for (i, path) in doc.para_paths(story).into_iter().enumerate() {
                let Some(p) = doc.para(story, &path) else { continue };
                for (off, o) in p.objects_with_offsets() {
                    match o {
                        InlineObject::BookmarkStart { name } if !ranges.contains_key(name) => {
                            ranges.insert(name.clone(), (story, (i, off), (i, p.len())));
                            open.insert(name.clone());
                        }
                        InlineObject::BookmarkEnd { name } if open.remove(name) => {
                            if let Some(r) = ranges.get_mut(name) {
                                r.2 = (i, off);
                            }
                        }
                        _ => {}
                    }
                }
                paras.insert(path, i);
            }
        }
        BookmarkRanges { order, ranges }
    }
}

/// Word's error text for a `REF` at `here` (story, paragraph, offset) that sits inside the
/// bookmark `target` it names: after the bookmark's start and before its end, in the same story.
/// The paragraph number switches (`\n`, `\r`, `\w`) show the number instead, and PAGEREF and
/// NOTEREF are never self-references (measured in Word).
fn self_reference_error(name: &str, instr: &str, target: &str, here: (StoryRef, &Path, usize), ranges: &BookmarkRanges) -> Option<&'static str> {
    if name != "REF" || ['n', 'r', 'w'].iter().any(|&sw| has_switch(instr, sw)) {
        return None;
    }
    let (story, start, end) = ranges.ranges.get(target)?;
    if *story != here.0 {
        return None;
    }
    let at = (*ranges.order.get(&here.0)?.get(here.1)?, here.2);
    (*start < at && at < *end).then_some(BOOKMARK_SELF_REFERENCE)
}

/// Puts Word's self-reference error into every unlocked REF of `paths` that sits inside the
/// bookmark it names, before any other field is updated: a REF to that bookmark elsewhere then
/// shows the bookmark's text with the error in it, wherever it stands. Gives the bookmark ranges
/// when it needed them.
fn mark_self_references(doc: &mut wordcraft_doc::Document, paths: &[(StoryRef, Path)]) -> Result<Option<BookmarkRanges>, CmdError> {
    let mut ranges = None;
    let mut self_refs: Vec<FieldResult> = Vec::new();
    for (story, path) in paths {
        let Some(p) = doc.para(*story, path) else { continue };
        for ((k, o), (off, _)) in p.objects.iter().enumerate().zip(p.objects_with_offsets()) {
            let InlineObject::Field { instr, locked: false, .. } = o else { continue };
            let name = wordcraft_layout::fields::field_name(instr);
            if name != "REF" {
                continue;
            }
            let f = FieldCode::parse(instr);
            let target = f.args.first().map(|s| s.split('\\').next().unwrap_or("").trim_matches('"')).unwrap_or("");
            if target.is_empty() {
                continue;
            }
            let ranges = ranges.get_or_insert_with(|| BookmarkRanges::build(doc));
            if let Some(err) = self_reference_error(&name, instr, target, (*story, path, off), ranges) {
                self_refs.push((*story, path.clone(), k, err.to_string()));
            }
        }
    }
    apply_results(doc, self_refs)?;
    Ok(ranges)
}

/// The mark of the footnote or endnote whose reference starts the bookmark at `at` (NOTEREF's
/// result): its custom mark, or its number as shown.
fn noteref_mark(doc: &wordcraft_doc::Document, at: &Pos, marks: &std::collections::HashMap<u32, String>) -> Option<String> {
    let p = doc.para(at.story, &at.path)?;
    for (off, o) in p.objects_with_offsets() {
        if off < at.off {
            continue;
        }
        match o {
            InlineObject::NoteRef { id, custom, .. } => return if custom.is_empty() { marks.get(id).cloned() } else { Some(custom.clone()) },
            InlineObject::BookmarkStart { .. } | InlineObject::BookmarkEnd { .. } => {}
            _ => return None,
        }
    }
    None
}

/// [`update_fields`], or with `all` the whole document: the body (with SEQ, citations and TOC) and
/// every part story, wherever the caret is (what printing needs). With `toc_pages_only` the table
/// of contents keeps its entries and only its page numbers are updated ([`update_toc_pages`]).
pub fn update_fields_in(s: &mut Session, all: bool, toc_pages_only: bool) -> Result<(), CmdError> {
    update_fields_with(s, all, toc_pages_only, true)
}

/// A `DOCVARIABLE` field's result: the variable its first argument names (quoted, or the first
/// word), with the field's case switch, or Word's error text, stored as is.
fn doc_variable_result(doc: &wordcraft_doc::Document, instr: &str) -> String {
    match doc_property_arg(instr) {
        None => DOC_VARIABLE_NOT_DEFINED.to_string(),
        Some(n) => match doc.doc_var(&n) {
            Some(v) => text_result(instr, v.to_string()),
            None => NO_DOC_VARIABLE.to_string(),
        },
    }
}

/// The refresh after Insert › Field: as [`update_fields`], except that `DOCVARIABLE` fields keep
/// their stored result. Only Update Field recomputes a document variable's result (Word keeps it
/// on open and on export too).
pub fn update_fields_after_insert(s: &mut Session) -> Result<(), CmdError> {
    update_fields_with(s, false, false, false)
}

fn update_fields_with(s: &mut Session, all: bool, toc_pages_only: bool, doc_vars: bool) -> Result<(), CmdError> {
    let caret_story = s.sel.focus.story;
    let in_body = all || caret_story == StoryRef::Body;
    if in_body {
        update_seq(s)?;
    }
    let mut pages: std::collections::HashMap<String, (u32, NumFormat, String)> = Default::default();
    let formats = page_formats(s);
    let marks = s.doc.bookmarks();
    // Where each body bookmark starts, for `\p` (above/below) and NOTEREF.
    let bookmark_at = bookmark_starts(&marks);
    // Every bookmark name in any story, for the error of a reference whose bookmark is gone.
    let bookmark_names: std::collections::HashSet<String> =
        marks.iter().map(|(n, _)| n.clone()).chain(part_bookmarks(&s.doc).into_iter().map(|(n, _)| n)).collect();
    let mut note_marks = None;
    for (name, pos) in marks {
        let pos = bookmark_page_pos(&s.doc, &name, pos);
        let l = s.layout();
        let page_info =
            page_at(&l, &formats, &pos).map_or((1, NumFormat::Decimal, String::new()), |(_, n, fmt, chapter)| (n, fmt, chapter.to_string()));
        pages.entry(name).or_insert(page_info);
    }
    let stories: Vec<StoryRef> =
        if all { std::iter::once(StoryRef::Body).chain(s.doc.parts.keys().map(|k| StoryRef::Part(*k))).collect() } else { vec![caret_story] };
    let range_stories = stories.clone();
    // The document properties as the fields read and set them (`SUBJECT "New subject"`): a field
    // after one that sets a property shows the new value.
    let mut core = s.doc.core.clone();
    // Build each body lookup once, only when a supported numeric REF needs it.
    let mut numbers = None;
    let mut bookmark_paths = None;
    let paths: Vec<(StoryRef, Path)> = stories.into_iter().flat_map(|st| s.doc.para_paths(st).into_iter().map(move |p| (st, p))).collect();
    // SET <name> "<text>" (ECMA-376 §17.16.5.58): the values it gives, which REF <name> shows; a
    // REF before the SET shows the last value set, as Word's bookmark does after an update.
    let last_set = set_values(s, &paths);
    // DOCPROPERTY's counts (Words, Pages ...) of the body as it is before this pass changes it.
    let counts = doc_property_counts(s, &paths);
    let mut set: std::collections::HashMap<String, String> = Default::default();
    // A REF inside its own bookmark gets its error first, so a REF to that bookmark elsewhere
    // shows the bookmark's text with the error in it, wherever it stands.
    let mut ranges = mark_self_references(&mut s.doc, &paths)?;
    // LINK sources read during this update, each file once.
    let mut sources = super::link::Sources::default();
    for (story, path) in paths {
        let Some(p) = s.doc.para(story, &path) else { continue };
        if !p.objects.iter().any(|o| matches!(o, InlineObject::Field { .. })) {
            continue;
        }
        let mut updates: Vec<(usize, String)> = Vec::new();
        let offsets: Vec<usize> = p.objects_with_offsets().map(|(off, _)| off).collect();
        for (k, o) in p.objects.iter().enumerate() {
            let InlineObject::Field { instr, locked: false, .. } = o else { continue };
            // A table formula reads the cells around its own cell again; a formula anywhere
            // else reads numbers and bookmarks only.
            if instr.trim_start().starts_with('=') {
                let default_lang = crate::cmd::default_lang(s);
                let result = match path.cell().and_then(|(tp, r, c)| s.doc.table(story, &tp).map(|t| (t, r, c))) {
                    Some((t, r, c)) => crate::cmd::table_formula::evaluate_field_with(t, Some(&s.doc), default_lang.as_deref(), r, c, instr),
                    None => crate::cmd::table_formula::evaluate_text_field(&s.doc, default_lang.as_deref(), instr),
                };
                let text = match result {
                    Ok(text) => text,
                    Err(e) => format!("!{e}"),
                };
                updates.push((k, text));
                continue;
            }
            let name = wordcraft_layout::fields::field_name(instr);
            match name.as_str() {
                "SYMBOL" => updates.push((k, wordcraft_layout::fields::symbol_text(instr))),
                "DATE" | "TIME" | "SAVEDATE" => {
                    let lang = field_lang(&s.doc, p, k);
                    let pic = date_picture(instr).unwrap_or_else(|| default_picture(&name, lang.as_deref()));
                    updates.push((k, text_result(instr, crate::cmd::insert::format_date_in(&pic, lang.as_deref()))));
                }
                "CREATEDATE" | "NUMWORDS" | "NUMCHARS" | "REVNUM" | "DOCPROPERTY" => {
                    updates.extend(document_info_result(&s.doc, &core, &counts, &name, instr).map(|text| (k, text)));
                }
                "PRINTDATE" => updates.extend(document_date_result(instr, &core.last_printed, "M/d/yyyy").map(|text| (k, text))),
                "REF" | "PAGEREF" | "NOTEREF" => {
                    let f = FieldCode::parse(instr);
                    let target = f.args.first().map(|s| s.split('\\').next().unwrap_or("").trim_matches('"')).unwrap_or("");
                    // A bookmark that is gone, or no name, gives Word's error text whatever the switches, stored
                    // as is (no `\*` format applied).
                    if let Some(err) = missing_bookmark_error(&name, target, &bookmark_names, (&set, &last_set)) {
                        updates.push((k, err.to_string()));
                        continue;
                    }
                    // A REF inside the bookmark it names: Word's error, stored as is.
                    let ranges = ranges.get_or_insert_with(|| BookmarkRanges::build(&s.doc));
                    if let Some(err) = offsets.get(k).and_then(|off| self_reference_error(&name, instr, target, (story, &path, *off), ranges)) {
                        updates.push((k, err.to_string()));
                        continue;
                    }
                    let here = offsets.get(k).map(|off| Pos { story, path: path.clone(), off: *off });
                    let r = if let Some(r) = position_or_note(&s.doc, (&name, instr, target), here, &bookmark_at, &mut note_marks) {
                        r
                    } else if name == "REF" {
                        if let Some(sw) = ['w', 'r', 'n'].into_iter().find(|c| has_switch(instr, *c)) {
                            if has_switch(instr, 'd') || has_switch(instr, 't') {
                                continue; // Preserve cached results for unsupported delimiter/text switches.
                            }
                            let numbers = numbers.get_or_insert_with(|| para_numbers(&s.doc));
                            let from = (story == StoryRef::Body).then_some(&path);
                            let paths = bookmark_paths.get_or_insert_with(|| body_bookmark_paths(&s.doc));
                            paths.get(target).and_then(|tp| ref_number(numbers, tp, from, sw))
                        } else {
                            bookmark_text(&s.doc, target).or_else(|| {
                                let key = target.to_lowercase();
                                set.get(&key).or_else(|| last_set.get(&key)).cloned()
                            })
                        }
                    } else if !has_switch(instr, 'p') {
                        pages.get(target).map(|(num, page_fmt, chapter)| {
                            let fmt = wordcraft_layout::fields::format_switch(instr).unwrap_or(*page_fmt);
                            wordcraft_layout::fields::page_label(instr, fmt, *num, chapter)
                        })
                    } else {
                        None
                    };
                    updates.extend(r.map(|r| (k, if name == "PAGEREF" { r } else { text_result(instr, r) })));
                }
                // A value given a name (the layout draws nothing for it; its result stays).
                "SET" => {
                    if let Some((n, v)) = set_args(instr)
                        && set.len() < MAX_SET
                    {
                        set.insert(n, v);
                    }
                }
                // The summary properties; a quoted text after the name replaces the property.
                "TITLE" | "SUBJECT" | "AUTHOR" | "KEYWORDS" | "COMMENTS" => {
                    if let Some(v) = new_property_value(instr) {
                        set_summary_property(&mut core, &name, v);
                    }
                    updates.push((k, text_result(instr, summary_property(&core, &name).to_string())));
                }
                "LASTSAVEDBY" => updates.push((k, text_result(instr, core.last_modified_by.clone()))),
                // The user's name and initials (File › Options › General); a quoted text after
                // the name is shown instead.
                "USERNAME" | "USERINITIALS" => {
                    let v = match new_property_value(instr) {
                        Some(v) => v.to_string(),
                        None if name == "USERNAME" => s.author.clone(),
                        None => s.user_initials(),
                    };
                    updates.push((k, text_result(instr, v)));
                }
                // DOCVARIABLE <name>: the document variable (names ignore case), or Word's error text.
                "DOCVARIABLE" => updates.extend(doc_vars.then(|| (k, doc_variable_result(&s.doc, instr)))),
                "FILENAME" => {
                    let (fn_name, fn_path) = s.resolved_filenames();
                    let text = if has_switch(instr, 'p') { fn_path } else { fn_name };
                    updates.push((k, text_result(instr, text)));
                }
                // The text of another local file, read only here, on an explicit update.
                "INCLUDETEXT" => updates.push((k, super::includetext::include_text(s, instr))),
                // A link to a saved Word document: its source as it is now. A link that can't be
                // followed keeps its stored result; an equal result is not touched.
                "LINK" => updates.extend(super::link::new_simple_result(s, &mut sources, o).map(|r| (k, r))),
                // GOTOBUTTON and MACROBUTTON store the text their code shows.
                "GOTOBUTTON" | "MACROBUTTON" => {
                    if let Some(text) = wordcraft_layout::fields::button_display_text(instr) {
                        updates.push((k, text));
                    }
                }
                _ => {}
            }
        }
        if core != s.doc.core {
            s.doc.core.clone_from(&core);
        }
        if updates.is_empty() {
            continue;
        }
        let para = s.doc.para_mut(story, &path)?;
        for (k, r) in updates {
            if let Some(InlineObject::Field { result, dirty, .. }) = para.objects.get_mut(k) {
                // A computed result is no longer stale: Word's F9 clears `w:dirty` too.
                *result = r;
                *dirty = false;
            }
        }
        para.touch();
    }
    update_button_ranges(s, &range_stories)?;
    super::link::update_link_ranges(s, &mut sources, &range_stories)?;
    if !in_body {
        return Ok(());
    }
    crate::cmd::citations::update_citations(s)?;
    update_body_toc(s, toc_pages_only)
}

/// The table of contents at the end of a body update: its page numbers only, or rebuilt.
fn update_body_toc(s: &mut Session, pages_only: bool) -> Result<(), CmdError> {
    if pages_only { update_toc_pages(s) } else { update_toc(s) }
}

/// Refresh every unlocked `DATE` and `TIME` field to now, as Word does whenever it shows,
/// prints or exports a document: a letter written from a template carries today's date, not the
/// date the template was last saved (#1672).
///
/// Only those two. `CREATEDATE`, `SAVEDATE` and `PRINTDATE` record when something happened and
/// are not "now"; `REF`, `PAGEREF`, `TOC` and citations are refreshed on an explicit update
/// (`fields.updateAll`) and not on open, again as Word does. A field carrying `w:fldLock` keeps
/// its stored result, which is what the lock is for.
pub fn refresh_date_fields(doc: &mut wordcraft_doc::Document) {
    let stories: Vec<StoryRef> = std::iter::once(StoryRef::Body).chain(doc.parts.keys().copied().map(StoryRef::Part)).collect();
    for story in stories {
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            let mut updates: Vec<(usize, String)> = Vec::new();
            for (k, o) in p.objects.iter().enumerate() {
                let InlineObject::Field { instr, locked: false, .. } = o else { continue };
                let name = wordcraft_layout::fields::field_name(instr);
                if !matches!(name.as_str(), "DATE" | "TIME") {
                    continue;
                }
                // Word's default picture when the code carries no `\@` switch.
                let lang = field_lang(doc, p, k);
                let pic = date_picture(instr).unwrap_or_else(|| default_picture(&name, lang.as_deref()));
                updates.push((k, text_result(instr, crate::cmd::insert::format_date_in(&pic, lang.as_deref()))));
            }
            if updates.is_empty() {
                continue;
            }
            let Ok(para) = doc.para_mut(story, &path) else { continue };
            for (k, r) in updates {
                if let Some(InlineObject::Field { result, .. }) = para.objects.get_mut(k) {
                    *result = r;
                }
            }
            para.touch();
        }
    }
}

/// Resolve a field object's language through the paragraph and document defaults.
fn field_lang(doc: &wordcraft_doc::Document, p: &wordcraft_doc::Paragraph, k: usize) -> Option<String> {
    let off = p.object_offsets().get(k).copied()?;
    doc.styles.resolve_char(p.props.style.as_deref(), p.props_of_char(off)).lang
}

/// Word's default date or time picture when a field instruction carries no `\@` picture switch.
fn default_picture(name: &str, lang: Option<&str>) -> String {
    if name == "TIME" { "h:mm am/pm".into() } else { crate::cmd::insert::default_date_picture(lang).into() }
}
/// Most names SET fields give values.
const MAX_SET: usize = 10_000;

/// A SET field's name (lower case: names are a bookmark's, which ignore case) and value.
fn set_args(instr: &str) -> Option<(String, String)> {
    let f = FieldCode::parse(instr);
    let name = f.args.first().map(|n| n.trim()).filter(|n| !n.is_empty() && n.chars().count() <= 40)?;
    let value: String = f.args.get(1..).map(|v| v.join(" ")).unwrap_or_default().chars().take(MAX_SET).collect();
    Some((name.to_lowercase(), value))
}

/// The value each name has after the last SET giving it one, in the stories at `paths`.
fn set_values(s: &Session, paths: &[(StoryRef, Path)]) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for (story, path) in paths {
        let Some(p) = s.doc.para(*story, path) else { continue };
        for o in &p.objects {
            if let InlineObject::Field { instr, .. } = o
                && wordcraft_layout::fields::field_name(instr) == "SET"
                && let Some((n, v)) = set_args(instr)
                && out.len() < MAX_SET
            {
                out.insert(n, v);
            }
        }
    }
    out
}

/// F9 on GOTOBUTTON and MACROBUTTON range fields (`FieldStart` … `FieldEnd`, as read from .docx):
/// the content between the markers becomes the text their code shows. Unlocked fields only; the
/// text takes the formatting of the field's first character.
fn update_button_ranges(s: &mut Session, stories: &[StoryRef]) -> Result<(), CmdError> {
    let width = wordcraft_doc::para::OBJ.len_utf8();
    for &story in stories {
        // Last first, so replacing one result never moves a field still to do; re-read after each
        // edit, as a replaced result may have held other fields.
        let mut i = s.doc.field_ranges(story).len();
        while i > 0 {
            i -= 1;
            let Some(r) = s.doc.field_ranges(story).into_iter().nth(i) else { continue };
            let name = wordcraft_layout::fields::field_name(&r.instr);
            if r.locked || (name != "GOTOBUTTON" && name != "MACROBUTTON") {
                continue;
            }
            let Some(text) = wordcraft_layout::fields::button_display_text(&r.instr) else { continue };
            let Some(inner) = r.start.off.checked_add(width) else { continue };
            let start = Pos { off: inner, ..r.start.clone() };
            if s.doc.copy_range(&start, &r.end).plain_text() == text {
                continue;
            }
            let props = s.doc.para_at(&r.start).map(|p| p.props_of_char(r.start.off).clone()).unwrap_or_default();
            let at = s.doc.delete_range(&start, &r.end)?;
            s.doc.insert_text(&at, &text, &props)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod listnum_tests {
    use super::*;

    #[test]
    fn output_refreshes_listnum_without_changing_stored_results() {
        let mut doc = wordcraft_doc::Document::from_text("Numbers");
        let path = Path::top(0);
        let para = doc.para_mut(StoryRef::Body, &path).unwrap();
        for locked in [false, true, false] {
            para.insert_object(
                para.len(),
                InlineObject::Field {
                    instr: "LISTNUM".into(),
                    result: "stale".into(),
                    locked,
                    dirty: true,
                    code: false,
                    form: String::new(),
                    code_paras: None,
                },
                &CharProps::default(),
            )
            .unwrap();
        }
        let fresh = output_fields(&doc).unwrap();
        let results = |doc: &wordcraft_doc::Document| {
            doc.para(StoryRef::Body, &path)
                .unwrap()
                .objects
                .iter()
                .filter_map(|o| match o {
                    InlineObject::Field { result, dirty, .. } => Some((result.clone(), *dirty)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(results(&fresh), [("1)".into(), false), ("stale".into(), true), ("2)".into(), false)]);
        assert_eq!(results(&doc), vec![("stale".into(), true); 3]);
        assert!(output_fields(&fresh).is_none());
    }

    #[test]
    fn listnum_default_formats_and_bounded_switches() {
        for (name, want) in [
            ("NumberDefault", ["1)", "a)", "i)", "(1)", "(a)", "(i)", "1.", "a.", "i."]),
            ("OutlineDefault", ["I.", "A.", "1.", "a)", "(1)", "(a)", "(i)", "(a)", "(i)"]),
        ] {
            for (index, expected) in want.iter().enumerate() {
                let mut count = ListNumCount::default();
                let code = FieldCode::parse(&format!("LISTNUM {name} \\l {}", index + 1));
                assert_eq!(&count.result(&code, None), expected);
            }
        }
        let mut count = ListNumCount { last_heading: 9, ..Default::default() };
        assert_eq!(count.result(&FieldCode::parse(r"LISTNUM \l 99999999999999999999"), None), "i.");
        assert_eq!(count.result(&FieldCode::parse(r"LISTNUM \l 0 \s 4294967295"), None), NumFormat::LowerRoman.format(u32::MAX) + ".");
        assert_eq!(count.result(&FieldCode::parse("LISTNUM"), None), NumFormat::LowerRoman.format(u32::MAX) + ".");
    }

    /// PR #5881 review: switch values out of range or not numbers fall back, each on a fresh
    /// counter set: a bad `\l` takes level 1 (no heading before it), `\s 0` starts at 0, and a
    /// `\s` that isn't a number is ignored.
    #[test]
    fn listnum_switch_fallbacks() {
        for (instr, want) in [
            (r"LISTNUM \l 10", "1)"),
            (r"LISTNUM \l 0", "1)"),
            (r"LISTNUM \l -1", "1)"),
            (r"LISTNUM \l x", "1)"),
            (r"LISTNUM \s 0", "0)"),
            (r"LISTNUM \s -3", "1)"),
        ] {
            let mut count = ListNumCount::default();
            assert_eq!(count.result(&FieldCode::parse(instr), None), want, "{instr}");
        }
    }
}

#[cfg(test)]
mod header_footer_output_tests {
    use super::*;
    use std::sync::Arc;
    use wordcraft_doc::{Document, PartKind};

    fn field(instr: &str, result: &str) -> InlineObject {
        InlineObject::Field {
            instr: instr.into(),
            result: result.into(),
            locked: false,
            dirty: false,
            code: false,
            form: String::new(),
            code_paras: None,
        }
    }

    fn para_with(fields: &[(&str, &str)]) -> Paragraph {
        let mut p = Paragraph::new();
        for (instr, result) in fields {
            let off = p.len();
            p.insert_object(off, field(instr, result), &CharProps::default()).unwrap();
        }
        p
    }

    fn doc() -> Document {
        let mut doc = Document::from_text("Body");
        doc.core.title = "Ada Notes".into();
        doc.core.creator = "Grace".into();
        doc.core.last_modified_by = "Linus".into();
        doc
    }

    /// The results output shows for `fields`, each stored as `stale`, in one header paragraph.
    fn shown(doc: &mut Document, fields: &[&str]) -> Vec<String> {
        let pairs: Vec<(&str, &str)> = fields.iter().map(|f| (*f, "stale")).collect();
        let id = doc.add_part(PartKind::Header, vec![Arc::new(Block::Para(para_with(&pairs)))]);
        let story = StoryRef::Part(id);
        let out = output_fields(doc).unwrap_or_else(|| doc.clone());
        out.para_paths(story)
            .iter()
            .flat_map(|path| out.para(story, path).unwrap().objects.clone())
            .filter_map(|o| match o {
                InlineObject::Field { result, .. } => Some(result),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn lastsavedby_and_summary_properties_with_case_switches() {
        let mut d = doc();
        assert_eq!(
            shown(&mut d, &["LASTSAVEDBY", r"TITLE \* Upper", r"AUTHOR \* Lower", r"title \* FirstCap", "SUBJECT"]),
            ["Linus", "ADA NOTES", "grace", "Ada Notes", ""]
        );
    }

    /// A summary property field that gives a new value keeps its result, and the property stays.
    #[test]
    fn property_with_a_new_value_keeps_its_result() {
        let mut d = doc();
        assert_eq!(shown(&mut d, &[r#"TITLE "Other""#, "TITLE"]), ["stale", "Ada Notes"]);
        assert_eq!(d.core.title, "Ada Notes");
    }

    #[test]
    fn quote_forms() {
        let mut d = doc();
        assert_eq!(
            shown(
                &mut d,
                &[
                    r#"QUOTE "fresh text""#,
                    r#"quote "Mixed" \* Upper"#,
                    r#"QUOTE """#,
                    "QUOTE fresh",
                    r#"QUOTE "a" "b""#,
                    r#"QUOTE "2024" \# "0""#,
                    r#"QUOTE "1/2/2024" \@ "yyyy""#,
                    "QUOTE",
                ]
            ),
            ["fresh text", "MIXED", "", "stale", "stale", "stale", "stale", "stale"]
        );
    }

    #[test]
    fn other_fields_keep_their_results() {
        let mut d = doc();
        assert!(output_fields(&d).is_none());
        let id = d.add_part(PartKind::Footer, vec![Arc::new(Block::Para(para_with(&[("FILENAME", "old.docx"), ("FILESIZE", "9")])))]);
        assert!(output_fields(&d).is_none(), "FILENAME and FILESIZE keep their results in part {id}");
    }

    /// A header CREATEDATE shows what Update Field in that header stores: the created date in
    /// local time, in its picture or the default one; the body's keeps its result.
    #[test]
    fn header_createdate_matches_update_field() {
        let mut d = doc();
        d.core.created = "2024-03-05T14:07:00Z".into();
        let fields = [("CREATEDATE", "stale"), (r#"CREATEDATE \@ "yyyy-MM-dd""#, "stale")];
        let id = d.add_part(PartKind::Header, vec![Arc::new(Block::Para(para_with(&fields)))]);
        let story = StoryRef::Part(id);
        let results = |doc: &Document| -> Vec<String> {
            doc.para_paths(story)
                .iter()
                .flat_map(|p| doc.para(story, p).unwrap().objects.clone())
                .filter_map(|o| match o {
                    InlineObject::Field { result, .. } => Some(result),
                    _ => None,
                })
                .collect()
        };
        let mut s = Session::new(d.clone());
        let path = s.doc.para_paths(story).remove(0);
        s.sel = Selection::caret(Pos::new(story, path, 0));
        s.run("references.updateFields", &serde_json::json!({})).unwrap();
        let want = results(&s.doc);
        assert!(want.iter().all(|r| r != "stale"), "{want:?}");
        assert!(want[1].starts_with("2024-03-0"), "{want:?}");
        let out = output_fields(&d).expect("the header dates change");
        assert_eq!(results(&out), want);
    }

    /// DOCPROPERTY in a footer: Update Field's lookup and error text; `Pages` keeps its result
    /// (output has no layout to count them).
    #[test]
    fn footer_doc_property_forms() {
        let mut d = doc();
        assert_eq!(
            shown(&mut d, &[r#"DOCPROPERTY "Title" \* Upper"#, "DOCPROPERTY NoSuchName", "DOCPROPERTY Pages", r#"NUMWORDS \* roman"#]),
            ["ADA NOTES", super::super::properties::UNKNOWN_DOC_PROPERTY, "stale", "i"]
        );
    }

    #[test]
    fn field_in_a_header_table_cell() {
        let mut d = doc();
        let mut t = wordcraft_doc::Table::new(1, 2, 200.0);
        t.rows[0].cells[1].blocks = vec![Arc::new(Block::Para(para_with(&[("TITLE", "stale")])))];
        let id = d.add_part(PartKind::Footer, vec![Arc::new(Block::Table(t))]);
        let story = StoryRef::Part(id);
        let out = output_fields(&d).expect("the cell's field changes");
        let results: Vec<String> = out
            .para_paths(story)
            .iter()
            .flat_map(|p| out.para(story, p).unwrap().objects.clone())
            .filter_map(|o| match o {
                InlineObject::Field { result, .. } => Some(result),
                _ => None,
            })
            .collect();
        assert_eq!(results, ["Ada Notes"]);
    }

    #[test]
    fn quote_literal_parses_only_the_plain_form() {
        assert_eq!(quote_literal(r#"QUOTE "x \"y\"""#).as_deref(), Some(r#"x "y""#));
        assert_eq!(quote_literal(r#"QUOTE "x" \* MERGEFORMAT"#).as_deref(), Some("x"));
        assert_eq!(quote_literal(r#"QUOTE \* Upper "x""#), None);
        assert_eq!(quote_literal(r#"TITLE "x""#), None);
    }
}
