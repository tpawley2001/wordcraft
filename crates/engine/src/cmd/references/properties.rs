use super::{FieldCode, InlineObject, NumFormat, Pos};

/// The summary property a TITLE, SUBJECT, AUTHOR, KEYWORDS or COMMENTS field shows.
pub(super) fn summary_property<'a>(core: &'a wordcraft_doc::CoreProps, field: &str) -> &'a str {
    match field {
        "TITLE" => &core.title,
        "SUBJECT" => &core.subject,
        "AUTHOR" => &core.creator,
        "KEYWORDS" => &core.keywords,
        "COMMENTS" => &core.description,
        _ => "",
    }
}

pub(super) fn set_summary_property(core: &mut wordcraft_doc::CoreProps, field: &str, value: String) {
    match field {
        "TITLE" => core.title = value,
        "SUBJECT" => core.subject = value,
        "AUTHOR" => core.creator = value,
        "KEYWORDS" => core.keywords = value,
        "COMMENTS" => core.description = value,
        _ => {}
    }
}

/// The new value a summary-property field gives its property: the argument after the field's
/// name (`SUBJECT "Quarterly report"`, or one word without quotes: `SUBJECT Draft`), at most 255
/// characters as Word allows.
pub(super) fn new_property_value(instr: &str) -> Option<String> {
    FieldCode::parse(instr).args.first().map(|a| a.chars().take(255).collect())
}

/// A count (NUMWORDS, NUMCHARS, REVNUM) in the field's `\*` number format, digits without one or
/// when the format does not cover the count (see [`super::table_formula::number_format_covers`]),
/// with the field's capitalization switches.
pub(super) fn count_text(instr: &str, n: usize) -> String {
    let fmt = wordcraft_layout::fields::format_switch(instr)
        .filter(|f| crate::cmd::table_formula::number_format_covers(*f, n as f64))
        .unwrap_or(NumFormat::Decimal);
    wordcraft_layout::fields::field_number(instr, fmt, u32::try_from(n).unwrap_or(u32::MAX))
}

/// A field's computed text with its capitalization switches (`\* Upper`, `\* Lower`, `\* Caps`,
/// `\* FirstCap`) applied in order.
pub(super) fn text_result(instr: &str, text: String) -> String {
    wordcraft_layout::fields::apply_case_switch(instr, text)
}

/// The property name of a `DOCPROPERTY` field: the word (or quoted text) after the field name,
/// before any switch (`DOCPROPERTY "Project Name" \* MERGEFORMAT`).
pub(super) fn doc_property_arg(instr: &str) -> Option<String> {
    let rest = instr.trim().split_once(char::is_whitespace)?.1.trim_start();
    let name = match rest.strip_prefix('"') {
        Some(q) => q.split('"').next().unwrap_or(""),
        None => rest.split([' ', '\\']).next().unwrap_or(""),
    };
    (!name.is_empty()).then(|| name.to_string())
}

/// The value of a document property under the name a `DOCPROPERTY` field uses: a custom property
/// when one has this name, else the built-in property (Word's English names, case-insensitively,
/// as Word's field accepts) from `core`. `None` for a name this function doesn't map (see
/// [`doc_property_result`] for the computed ones).
pub(super) fn doc_property(doc: &wordcraft_doc::Document, core: &wordcraft_doc::CoreProps, name: &str) -> Option<String> {
    if let Some(v) = doc.custom_prop(name) {
        return Some(v.to_string());
    }
    let c = core;
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "title" => c.title.clone(),
        "subject" => c.subject.clone(),
        "author" => c.creator.clone(),
        "keywords" => c.keywords.clone(),
        "comments" => c.description.clone(),
        "category" => c.category.clone(),
        "lastsavedby" => c.last_modified_by.clone(),
        "revisionnumber" => c.revision.to_string(),
        _ => return None,
    })
}

/// Word's text for a DOCPROPERTY field whose name is neither a custom property nor a built-in one.
pub const UNKNOWN_DOC_PROPERTY: &str = "Error! Unknown document property name.";

/// Word's text for a DOCPROPERTY field with no property name (`DOCPROPERTY \* Upper`).
pub const NO_DOC_PROPERTY_NAME: &str = "Error! No property name supplied.";

/// Built-in property names Word's DOCPROPERTY field knows but this application cannot compute
/// (the file size, the editing time, a line count that matches Word's, Word's own template and
/// application name). A field naming one keeps its stored result on Update Field: the name is
/// valid in Word, so showing [`UNKNOWN_DOC_PROPERTY`] for it would be wrong.
const UNCOMPUTED_BUILT_IN_PROPERTIES: [&str; 7] = ["Bytes", "Lines", "NameofApplication", "ODMADocId", "Security", "Template", "TotalEditingTime"];

/// Counts of the document body for the DOCPROPERTY count names, taken once when Update Field
/// starts (before any result changes). Header and footer text is not counted, as in Word.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct BodyCounts {
    pub words: usize,
    /// Characters without white space (NUMCHARS).
    pub characters: usize,
    /// Characters with spaces, paragraph marks not counted.
    pub characters_with_spaces: usize,
    /// Non-empty paragraphs.
    pub paragraphs: usize,
    /// Laid-out pages; `None` when no field asked for them (so layout isn't needed).
    pub pages: Option<usize>,
}

impl BodyCounts {
    /// The counts of `doc`'s body; `pages` is the laid-out page count when the caller needs it.
    /// The results of DOCPROPERTY fields are left out of the character and paragraph counts, so
    /// a count field does not count its own (or another count's) result.
    pub(super) fn of(doc: &wordcraft_doc::Document, pages: Option<usize>) -> Self {
        let mut counts = BodyCounts { words: doc.word_count(), pages, ..Default::default() };
        for path in doc.para_paths(wordcraft_doc::StoryRef::Body) {
            let Some(p) = doc.para(wordcraft_doc::StoryRef::Body, &path) else { continue };
            let text = text_without_doc_properties(p);
            counts.characters = counts.characters.saturating_add(wordcraft_doc::count_chars_no_spaces(&text));
            counts.characters_with_spaces = counts.characters_with_spaces.saturating_add(wordcraft_doc::count_chars(&text));
            if !text.trim().is_empty() {
                counts.paragraphs = counts.paragraphs.saturating_add(1);
            }
        }
        counts
    }

    fn get(&self, key: &str) -> Option<usize> {
        match key {
            "words" => Some(self.words),
            "characters" => Some(self.characters),
            "characterswithspaces" => Some(self.characters_with_spaces),
            "paragraphs" => Some(self.paragraphs),
            "pages" => self.pages,
            _ => None,
        }
    }
}

/// A paragraph's plain text with the results of its DOCPROPERTY fields left out.
fn text_without_doc_properties(p: &wordcraft_doc::Paragraph) -> String {
    let mut objects = p.objects.iter();
    let mut out = String::with_capacity(p.text.len());
    for c in p.text.chars() {
        if c != wordcraft_doc::para::OBJ {
            out.push(c);
            continue;
        }
        match objects.next() {
            Some(InlineObject::Field { instr, .. }) if wordcraft_layout::fields::field_name(instr) == "DOCPROPERTY" => {}
            Some(o) => out.push_str(o.plain_text()),
            None => {}
        }
    }
    out
}

/// Does this DOCPROPERTY field code name the `Pages` property (so Update Field needs the layout)?
pub(super) fn doc_property_names_pages(instr: &str) -> bool {
    wordcraft_layout::fields::field_name(instr) == "DOCPROPERTY" && doc_property_arg(instr).is_some_and(|n| n.trim().eq_ignore_ascii_case("pages"))
}

/// The value of an extended property the file stored and the model keeps as is
/// ([`wordcraft_doc::APP_KEPT`]): `Company`, `Manager`, `HyperlinkBase`. Empty when there is none.
fn kept_app_property(doc: &wordcraft_doc::Document, name: &str) -> String {
    let Some(bytes) = doc.passthrough.get(wordcraft_doc::APP_KEPT) else { return String::new() };
    let text = String::from_utf8_lossy(bytes);
    text.lines().filter_map(|l| l.split_once('\t')).find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.to_string()).unwrap_or_default()
}

/// A `\#` numeric picture applied to a count, then the capitalization switches; else
/// [`count_text`] (the `\*` number formats).
fn doc_property_count(instr: &str, n: usize) -> String {
    let (_, switch) = crate::cmd::table_formula::split_switch(instr);
    match switch.and_then(crate::cmd::table_formula::picture_of) {
        Some(picture) => text_result(instr, crate::cmd::table_formula::format_picture(n as f64, &picture)),
        None => count_text(instr, n),
    }
}

/// The new result of a DOCPROPERTY field on Update Field; `None` keeps the stored result. A field
/// with no name gives [`NO_DOC_PROPERTY_NAME`]; a name that is neither a custom property nor a
/// built-in one gives [`UNKNOWN_DOC_PROPERTY`]. Both are stored as is: Word does not apply the `\*`
/// capitalization switches to an error text. `date` formats a core date (an RFC 3339 instant) with
/// the field's `\@` picture or the given default, `None` when it isn't a date.
pub(super) fn doc_property_result(
    doc: &wordcraft_doc::Document,
    core: &wordcraft_doc::CoreProps,
    counts: &BodyCounts,
    date: impl Fn(&str, &str) -> Option<String>,
    instr: &str,
) -> Option<String> {
    let Some(name) = doc_property_arg(instr) else { return Some(NO_DOC_PROPERTY_NAME.to_string()) };
    if let Some(v) = doc_property(doc, core, &name) {
        return Some(text_result(instr, v));
    }
    let key = name.trim().to_ascii_lowercase();
    const DATE_PICTURE: &str = "M/d/yyyy h:mm AM/PM";
    match key.as_str() {
        "company" | "manager" | "hyperlinkbase" => return Some(text_result(instr, kept_app_property(doc, &key))),
        "createtime" => return date(&core.created, DATE_PICTURE),
        "lastsavedtime" => return date(&core.modified, DATE_PICTURE),
        "lastprinted" if core.last_printed.trim().is_empty() => return Some(String::new()),
        "lastprinted" => return date(&core.last_printed, DATE_PICTURE),
        _ => {}
    }
    if let Some(n) = counts.get(&key) {
        return Some(doc_property_count(instr, n));
    }
    if UNCOMPUTED_BUILT_IN_PROPERTIES.iter().any(|n| n.eq_ignore_ascii_case(&key)) {
        return None;
    }
    Some(UNKNOWN_DOC_PROPERTY.to_string())
}

/// A field's `\@` date picture: the quoted text after it (switches may follow), or one word.
pub fn date_picture(instr: &str) -> Option<String> {
    let rest = instr.split_once("\\@")?.1.trim_start();
    let pic = match rest.strip_prefix('"') {
        Some(q) => q.split('"').next().unwrap_or(""),
        None => rest.split_whitespace().next().unwrap_or(""),
    };
    (!pic.is_empty()).then(|| pic.to_string())
}

/// The text a bookmark marks (field results included), up to its end in the same paragraph or
/// the paragraph's end. `None` when there is no such bookmark.
pub fn bookmark_text(doc: &wordcraft_doc::Document, name: &str) -> Option<String> {
    let (_, start) = doc.bookmarks().into_iter().find(|(n, _)| n == name)?;
    bookmark_text_at(doc, name, &start)
}

/// Does the bookmark `name` that starts at `start` end in the paragraph it starts in?
pub(super) fn bookmark_ends_in_its_paragraph(doc: &wordcraft_doc::Document, name: &str, start: &Pos) -> bool {
    doc.para(start.story, &start.path).is_some_and(|para| {
        para.object_offsets()
            .into_iter()
            .any(|o| o > start.off && matches!(para.object_at(o), Some(InlineObject::BookmarkEnd { name: n }) if n == name))
    })
}

/// [`bookmark_text`] for the bookmark `name` that starts at `start`, for a caller that has
/// looked the document's bookmarks up once.
pub(crate) fn bookmark_text_at(doc: &wordcraft_doc::Document, name: &str, start: &Pos) -> Option<String> {
    let para = doc.para(start.story, &start.path)?;
    let end = para
        .object_offsets()
        .into_iter()
        .find(|o| *o > start.off && matches!(para.object_at(*o), Some(InlineObject::BookmarkEnd { name: n }) if n == name))
        .unwrap_or(para.len());
    Some(doc.copy_range(start, &Pos { off: end, ..start.clone() }).plain_text().trim().to_string())
}
