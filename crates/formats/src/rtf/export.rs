use super::date::{iso_to_dttm, rtf_date};
use super::state::{MARK_CLOSE, MARK_OPEN, MAX_NOTE_BLOCKS};
use super::styles::{style_fmt, styled};
use super::{
    Align, Arc, Block, Blocks, Document, FBlock, FTable, Fmt, HashMap, HashSet, Inline, InlineObject, Kind, Meta, Para, Paragraph, RevMark, Rgb,
    VAlign, model,
};
use wordcraft_doc::props::EmphasisMark;

// ---------------------------------------------------------------------------------------------
// Export

/// RTF-escape text (non-ASCII as `\uN?`).
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '\t' => o.push_str("\\tab "),
            '\n' => o.push_str("\\line "),
            '\u{000C}' => o.push_str("\\page "),
            '\u{000E}' => o.push_str("\\column "),
            '\u{00A0}' => o.push_str("\\~"),
            '\u{00AD}' => o.push_str("\\-"),
            '\u{2011}' => o.push_str("\\_"),
            c if c.is_ascii_control() => {}
            c if c.is_ascii() => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    o.push_str(&format!("\\u{}?", *u as i16));
                }
            }
        }
    }
    o
}

/// A note reference: the automatic number (`\chftn`), or the custom mark's text (RTF 1.9.1).
fn note_mark(custom: &str) -> String {
    if custom.is_empty() { String::from("{\\super\\chftn}") } else { format!("{{\\super {}}}", esc(custom)) }
}

/// [`esc`] for a `\fonttbl` name: `;` would end the entry, so it is written `\'3b`, and control
/// characters, which `esc` writes as `\line` or `\tab`, are dropped.
fn esc_font(name: &str) -> String {
    esc(&name.replace(char::is_control, "")).replace(';', "\\'3b")
}

pub(super) struct Tables {
    pub(super) fonts: Vec<String>,
    pub(super) colors: Vec<Rgb>,
}

impl Tables {
    pub(super) fn font(&mut self, name: &str) -> usize {
        match self.fonts.iter().position(|f| f.eq_ignore_ascii_case(name)) {
            Some(i) => i,
            None => {
                self.fonts.push(name.to_string());
                self.fonts.len() - 1
            }
        }
    }
    /// Colour table index (1-based; 0 is "auto").
    fn color(&mut self, c: Rgb) -> usize {
        match self.colors.iter().position(|x| *x == c) {
            Some(i) => i + 1,
            None => {
                self.colors.push(c);
                self.colors.len()
            }
        }
    }
}

/// What a place marker in the exported text stands for (see [`MARK_OPEN`]).
pub(super) enum Mark {
    Note {
        endnote: bool,
        /// The custom reference mark (empty: the automatic number).
        custom: String,
        blocks: Vec<FBlock>,
    },
    /// A custom-marked note's own mark, at the start of its text.
    Own(String),
    RangeStart(u32),
    /// The end of a comment's range; the first end of each comment also carries its annotation.
    RangeEnd {
        id: u32,
        ann: Option<Annotation>,
    },
    Done,
}

pub(super) struct Annotation {
    author: String,
    initials: String,
    date: String,
    blocks: Vec<FBlock>,
}

pub(super) struct Writer {
    pub(super) marks: Vec<Mark>,
    pub(super) t: Tables,
    /// List definitions: per list, the ordered flag per level.
    pub(super) lists: Vec<[bool; 9]>,
    /// Authors of tracked changes (`\revtbl` entries after "Unknown"), when the text has any.
    pub(super) authors: Option<Vec<String>>,
    /// The character formatting the current paragraph's style gives its runs (heading bold and
    /// size, code font…): what a run's formatting without a direct value resolves to.
    pub(super) base: Fmt,
    /// The body's sections in order (the last is the document's); empty when it has just one.
    pub(super) sections: Vec<wordcraft_doc::SectionProps>,
    /// The section being written.
    pub(super) sect_i: usize,
}

pub(super) fn has_marks(bl: &Blocks, depth: usize) -> bool {
    bl.iter().any(|b| match &**b {
        Block::Para(p) => {
            p.objects.iter().any(|o| matches!(o, InlineObject::NoteRef { .. } | InlineObject::CommentStart { .. } | InlineObject::CommentEnd { .. }))
        }
        Block::Table(t) => depth < 8 && t.rows.iter().any(|r| r.cells.iter().any(|c| has_marks(&c.blocks, depth + 1))),
    })
}

/// A copy of `doc` whose note references and comment range ends are place markers in the text
/// (see [`MARK_OPEN`]), with what each stands for; none when the body has neither.
fn mark_doc(doc: &Document) -> Option<(Document, Vec<Mark>)> {
    if !has_marks(&doc.body, 0) {
        return None;
    }
    let mut m = Marker { doc, marks: Vec::new(), annotated: HashSet::new() };
    let body = m.blocks(&doc.body, None, 0);
    let mut d = doc.clone();
    d.body = body;
    Some((d, m.marks))
}

/// The story id standing for "inside a note or comment": no nested notes, no comment ranges.
const IN_STORY: u32 = u32::MAX;

struct Marker<'a> {
    doc: &'a Document,
    marks: Vec<Mark>,
    annotated: HashSet<u32>,
}

impl Marker<'_> {
    fn token(&mut self, m: Mark) -> String {
        self.marks.push(m);
        self.marks.len().saturating_sub(1).to_string()
    }

    /// The flow of a story (a note's or comment's text); tables are flattened to their paragraphs.
    fn story(&mut self, bl: &Blocks, note: u32, depth: usize, out: &mut Vec<FBlock>) {
        for b in bl.iter().take(MAX_NOTE_BLOCKS) {
            match &**b {
                Block::Para(p) => {
                    let q = self.para(p, Some(note));
                    out.extend(model::flow_paras(self.doc, &q).into_iter().map(FBlock::Para));
                }
                Block::Table(t) if depth < 4 => {
                    for c in t.rows.iter().flat_map(|r| r.cells.iter()) {
                        self.story(&c.blocks, note, depth + 1, out);
                    }
                }
                Block::Table(_) => {}
            }
        }
    }

    fn blocks(&mut self, bl: &Blocks, note: Option<u32>, depth: usize) -> Blocks {
        bl.iter()
            .map(|b| match &**b {
                Block::Para(p) if p.objects.is_empty() => b.clone(),
                Block::Para(p) => Arc::new(Block::Para(self.para(p, note))),
                Block::Table(t) if depth < 8 => {
                    let mut t = t.clone();
                    for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                        c.blocks = self.blocks(&c.blocks, note, depth + 1);
                    }
                    Arc::new(Block::Table(t))
                }
                Block::Table(_) => b.clone(),
            })
            .collect()
    }

    /// `p` with its note references and comment range ends replaced by place markers. `note` is
    /// the note being written, when `p` is part of one.
    fn para(&mut self, p: &Paragraph, note: Option<u32>) -> Paragraph {
        let mut q = p.clone();
        for off in p.object_offsets().into_iter().rev() {
            let Some(obj) = q.object_at(off).cloned() else { continue };
            let tok = match obj {
                InlineObject::NoteRef { id, custom, .. } if note == Some(id) => {
                    Some(if custom.is_empty() { String::from("S") } else { self.token(Mark::Own(custom)) })
                }
                InlineObject::NoteRef { .. } if note.is_some() => Some(String::new()),
                InlineObject::NoteRef { id, kind, custom } => self.doc.parts.get(&id).map(|part| {
                    let mut blocks = Vec::new();
                    self.story(&part.blocks, id, 0, &mut blocks);
                    self.token(Mark::Note { endnote: kind == wordcraft_doc::para::NoteKind::Endnote, custom, blocks })
                }),
                InlineObject::CommentStart { id } if note.is_none() => Some(self.token(Mark::RangeStart(id))),
                InlineObject::CommentEnd { id } if note.is_none() => {
                    let ann = if self.annotated.insert(id) { self.annotation(id) } else { None };
                    Some(self.token(Mark::RangeEnd { id, ann }))
                }
                _ => None,
            };
            let Some(tok) = tok else { continue };
            let end = off + wordcraft_doc::para::OBJ.len_utf8();
            let text = if tok.is_empty() { tok } else { format!("{MARK_OPEN}{tok}{MARK_CLOSE}") };
            if q.delete(off, end).is_err() {
                continue;
            }
            if !text.is_empty() && q.insert_text(off, &text, &Default::default()).is_err() {
                continue;
            }
        }
        q
    }

    fn annotation(&mut self, id: u32) -> Option<Annotation> {
        let c = self.doc.comments.get(&id)?;
        let part = self.doc.parts.get(&c.part)?;
        let mut blocks = Vec::new();
        self.story(&part.blocks, IN_STORY, 0, &mut blocks);
        Some(Annotation { author: c.author.clone(), initials: c.initials.clone(), date: c.date.clone(), blocks })
    }
}

/// The document formatting words for the settings that differ from the defaults: default tab
/// stop, facing pages, mirror margins and the automatic hyphenation options. Word writes
/// `\widowctrl` in every document, so a paragraph that doesn't say has widow control on, as
/// `None` means here.
fn doc_opts_words(s: &wordcraft_doc::Settings) -> String {
    let d = wordcraft_doc::Settings::default();
    let tw = |v: f32| if v.is_finite() { (v * 20.0).round().clamp(0.0, 31_680.0) as i64 } else { 0 };
    let mut o = String::from("\\widowctrl");
    if s.default_tab != d.default_tab && tw(s.default_tab) >= 1 {
        o.push_str(&format!("\\deftab{}", tw(s.default_tab)));
    }
    if s.even_odd_headers {
        o.push_str("\\facingp");
    }
    if s.mirror_margins {
        o.push_str("\\margmirror");
    }
    if s.auto_hyphenation {
        o.push_str("\\hyphauto");
    }
    if s.hyphenation_zone != d.hyphenation_zone {
        o.push_str(&format!("\\hyphhotz{}", tw(s.hyphenation_zone)));
    }
    if !s.hyphenate_caps {
        o.push_str("\\hyphcaps0");
    }
    if s.consecutive_hyphen_limit != d.consecutive_hyphen_limit {
        o.push_str(&format!("\\hyphconsec{}", s.consecutive_hyphen_limit.min(i32::MAX as u32)));
    }
    if s.wrap_trail_spaces {
        o.push_str("\\wraptrsp");
    }
    o
}

/// `\sectd` and a section's formatting, in the words the reader reads back, then its formatting
/// before a tracked change (`{\*\oldsprops …}`) when it has one.
/// `doc_gutter` is the document's `\gutterN` (points), which `\sectd` resets a section to.
fn sect_words(s: &wordcraft_doc::SectionProps, doc_gutter: f32, t: &mut Tables) -> String {
    let mut o = sect_props_words(s, doc_gutter, t);
    if let Some(c) = &s.fmt_change {
        o.push_str(&format!("{{\\*\\oldsprops {}}}", sect_props_words(&c.old, doc_gutter, t)));
    }
    o.push('\n');
    o
}

/// `\sectd` and a section's own formatting words; page border colours go into `t`.
fn sect_props_words(s: &wordcraft_doc::SectionProps, doc_gutter: f32, t: &mut Tables) -> String {
    use wordcraft_doc::section::{ChapterSep, LineNumberRestart, NotePos, NumFormat, PageVAlign, SectionStart};
    let tw = |v: f32| if v.is_finite() { ((v * 20.0).round() as i64).max(0) } else { 0 };
    let mut o = String::from("\\sectd");
    o.push_str(match s.start {
        SectionStart::Continuous => "\\sbknone",
        SectionStart::NextColumn => "\\sbkcol",
        SectionStart::NextPage => "\\sbkpage",
        SectionStart::EvenPage => "\\sbkeven",
        SectionStart::OddPage => "\\sbkodd",
    });
    o.push_str(&format!("\\pgwsxn{}\\pghsxn{}", tw(s.page_w), tw(s.page_h)));
    if s.landscape {
        o.push_str("\\lndscpsxn");
    }
    o.push_str(match s.valign {
        PageVAlign::Top => "",
        PageVAlign::Center => "\\vertalc",
        PageVAlign::Justify => "\\vertalj",
        PageVAlign::Bottom => "\\vertalb",
    });
    if s.no_endnote == Some(false) {
        o.push_str("\\endnhere");
    }
    if let Some(n) = s.paper_src_first.filter(|&n| n != 0) {
        o.push_str(&format!("\\binfsxn{n}"));
    }
    if let Some(n) = s.paper_src_other.filter(|&n| n != 0) {
        o.push_str(&format!("\\binsxn{n}"));
    }
    if let Some(b) = &s.page_borders {
        for (side, opt) in [('t', b.top), ('l', b.left), ('b', b.bottom), ('r', b.right)] {
            if let Some(border) = opt {
                border_words("pgbrdr", side, border, true, t, &mut o);
            }
        }
    }
    o.push_str(&format!(
        "\\marglsxn{}\\margrsxn{}\\margtsxn{}\\margbsxn{}\\headery{}\\footery{}",
        tw(s.margin_left),
        tw(s.margin_right),
        tw(s.margin_top),
        tw(s.margin_bottom),
        tw(s.header),
        tw(s.footer)
    ));
    o.push_str(&section_gutter_word(s.gutter, doc_gutter));
    if s.columns.count > 1 {
        o.push_str(&format!("\\cols{}\\colsx{}", s.columns.count, tw(s.columns.space)));
        if s.columns.separator {
            o.push_str("\\linebetcol");
        }
        if s.columns.widths.len() == s.columns.count as usize {
            for (i, &(w, sr)) in s.columns.widths.iter().enumerate() {
                o.push_str(&format!("\\colno{}", i + 1));
                if tw(sr) > 0 {
                    o.push_str(&format!("\\colsr{}", tw(sr)));
                }
                o.push_str(&format!("\\colw{}", tw(w)));
            }
        }
    }
    if let Some(n) = s.page_num_start {
        o.push_str(&format!("\\pgnrestart\\pgnstarts{n}"));
    }
    match s.page_num_format {
        NumFormat::UpperRoman => o.push_str("\\pgnucrm"),
        NumFormat::LowerRoman => o.push_str("\\pgnlcrm"),
        NumFormat::UpperLetter => o.push_str("\\pgnucltr"),
        NumFormat::LowerLetter => o.push_str("\\pgnlcltr"),
        _ => {}
    }
    if s.title_page {
        o.push_str("\\titlepg");
    }
    if let Some(l) = &s.line_numbers {
        o.push_str(&format!("\\linemod{}\\linex{}\\linestarts{}", l.count_by.clamp(1, 100), tw(l.distance), l.start.clamp(1, 32_768)));
        o.push_str(match l.restart {
            LineNumberRestart::Page => "\\lineppage",
            LineNumberRestart::Section => "\\linerestart",
            LineNumberRestart::Continuous => "\\linecont",
        });
    }
    if let Some(g) = s.doc_grid {
        let word = match g.kind {
            wordcraft_doc::section::DocGridType::Lines => "\\sectspecifyl",
            wordcraft_doc::section::DocGridType::LinesAndChars | wordcraft_doc::section::DocGridType::SnapToChars => "\\sectspecifycl",
            wordcraft_doc::section::DocGridType::Default => "",
        };
        if !word.is_empty() {
            let pitch = if g.line_pitch.is_finite() { (g.line_pitch * 20.0).round() as i64 } else { 0 };
            o.push_str(&format!("{word}\\sectlinegrid{}", pitch.clamp(1, 31_680)));
        }
    }
    if s.rtl {
        o.push_str("\\rtlsect");
    }
    match s.text_direction {
        wordcraft_doc::props::TextDirection::Down => o.push_str("\\stextflow1"),
        wordcraft_doc::props::TextDirection::Up => o.push_str("\\stextflow2"),
        wordcraft_doc::props::TextDirection::Horizontal => {}
    }
    if let Some(n) = s.chapter_style {
        o.push_str(&format!("\\pgnhn{n}"));
    }
    if let Some(sep) = s.chapter_sep {
        o.push_str(match sep {
            ChapterSep::Hyphen => "\\pgnhnsh",
            ChapterSep::Period => "\\pgnhnsp",
            ChapterSep::Colon => "\\pgnhnsc",
            ChapterSep::EmDash => "\\pgnhnsm",
            ChapterSep::EnDash => "\\pgnhnsn",
        });
    }
    o.push_str(match s.footnote_pr.pos {
        Some(NotePos::PageBottom) => "\\sftnbj",
        Some(NotePos::BeneathText) => "\\sftntj",
        _ => "",
    });
    o.push_str(&note_words("\\sftn", &s.footnote_pr, s.footnote_pr.num_fmt));
    o.push_str(&note_words("\\saftn", &s.endnote_pr, s.endnote_pr.num_fmt));
    o
}

/// The `\guttersxn` word of a section whose gutter (points) differs from the document's `\gutter`,
/// else nothing: a section with the document's gutter needs no word of its own.
fn section_gutter_word(gutter: f32, doc_gutter: f32) -> String {
    let tw = |v: f32| if v.is_finite() { ((v * 20.0).round() as i64).max(0) } else { 0 };
    if tw(gutter) == tw(doc_gutter) { String::new() } else { format!("\\guttersxn{}", tw(gutter)) }
}

/// The start, restart and number format words of footnote or endnote options `n` (`prefix`:
/// `\ftn`, `\aftn`, `\sftn` or `\saftn`), each only when set; endnotes have no per-page restart.
fn note_words(prefix: &str, n: &wordcraft_doc::section::NoteProps, fmt: Option<wordcraft_doc::section::NumFormat>) -> String {
    use wordcraft_doc::section::NoteRestart;
    let mut o = String::new();
    if let Some(v) = n.num_start {
        o.push_str(&format!("{prefix}start{}", v.clamp(1, 32_767)));
    }
    let endnote = prefix.ends_with("aftn");
    match n.num_restart {
        Some(NoteRestart::EachPage) if !endnote => o.push_str(&format!("{prefix}rstpg")),
        Some(NoteRestart::EachSect) => o.push_str(&format!("{prefix}restart")),
        Some(NoteRestart::Continuous) => o.push_str(&format!("{prefix}rstcont")),
        _ => {}
    }
    if let Some(w) = fmt.and_then(super::state::note_format_word) {
        o.push_str(&format!("{prefix}n{w}"));
    }
    o
}

/// Whether `bl` (and the tables in it) holds a footnote reference and an endnote reference.
fn note_kinds(bl: &Blocks, depth: usize, found: &mut (bool, bool)) {
    use wordcraft_doc::para::NoteKind;
    for b in bl.iter() {
        match &**b {
            Block::Para(p) => {
                for o in &p.objects {
                    match o {
                        InlineObject::NoteRef { kind: NoteKind::Endnote, .. } => found.1 = true,
                        InlineObject::NoteRef { .. } => found.0 = true,
                        _ => {}
                    }
                }
            }
            Block::Table(t) if depth < 8 => {
                for c in t.rows.iter().flat_map(|r| r.cells.iter()) {
                    note_kinds(&c.blocks, depth + 1, found);
                }
            }
            Block::Table(_) => {}
        }
    }
}

/// The document's footnote and endnote options: each word only when the option is set, the
/// number formats only when they aren't the defaults (decimal footnotes, lower Roman endnotes).
/// A document with notes always states where footnotes and endnotes go and which it has
/// (`\fet`): a reader takes footnotes in a file that never says where they go for endnotes.
fn doc_note_words(doc: &Document) -> String {
    use wordcraft_doc::section::{NotePos, NumFormat};
    let set = &doc.settings;
    let mut found = (false, false);
    note_kinds(&doc.body, 0, &mut found);
    for part in doc.parts.values() {
        note_kinds(&part.blocks, 0, &mut found);
    }
    let (has_notes, has_endnotes) = (found.0 || found.1, found.1);
    let mut o = String::new();
    o.push_str(match set.footnote_pr.pos {
        Some(NotePos::PageBottom) => "\\ftnbj",
        Some(NotePos::BeneathText) => "\\ftntj",
        _ if has_notes => "\\ftnbj",
        _ => "",
    });
    o.push_str(match set.endnote_pr.pos {
        Some(NotePos::SectEnd) => "\\aendnotes",
        Some(NotePos::DocEnd) => "\\aenddoc",
        _ if has_notes => "\\aenddoc",
        _ => "",
    });
    o.push_str(&note_words("\\ftn", &set.footnote_pr, Some(set.footnote_format).filter(|&f| f != NumFormat::Decimal)));
    o.push_str(&note_words("\\aftn", &set.endnote_pr, Some(set.endnote_format).filter(|&f| f != NumFormat::LowerRoman)));
    if has_notes {
        o.push_str(if has_endnotes { "\\fet2" } else { "\\fet0" });
    }
    o
}

/// The document settings that differ from a new document's: view zoom, drawing grid, Restrict
/// Editing (never a password hash), linked styles, privacy options, Shift+Return expansion,
/// Track Changes on and print only the data from a form.
fn doc_setting_words(set: &wordcraft_doc::Settings) -> String {
    use wordcraft_doc::ZoomFit;
    let def = wordcraft_doc::Settings::default();
    let mut o = String::new();
    if let Some(z) = set.zoom {
        o.push_str(&format!("\\viewscale{}", z.percent));
        // Text Width has no RTF form: the percentage alone.
        o.push_str(match z.fit {
            Some(ZoomFit::OnePage) => "\\viewzk1",
            Some(ZoomFit::PageWidth) => "\\viewzk2",
            _ => "",
        });
    }
    let grid =
        |s: &wordcraft_doc::Settings| (s.grid_h, s.grid_v, s.grid_every_h, s.grid_every_v, s.grid_use_margins, s.grid_origin_x, s.grid_origin_y);
    if grid(set) != grid(&def) {
        let tw = |v: f32| (v * 20.0).round() as i64;
        o.push_str(&format!("\\dghspace{}\\dgvspace{}", tw(set.grid_h), tw(set.grid_v)));
        if set.grid_use_margins {
            o.push_str("\\dgmargin");
        } else {
            o.push_str(&format!("\\dghorigin{}\\dgvorigin{}", tw(set.grid_origin_x), tw(set.grid_origin_y)));
        }
        o.push_str(&format!("\\dghshow{}\\dgvshow{}", set.grid_every_h, set.grid_every_v));
    }
    let level = match set.protection.as_deref() {
        Some("trackedChanges") => Some(0),
        Some("comments") => Some(1),
        Some("forms") => Some(2),
        Some("readOnly") => Some(3),
        _ => None,
    };
    if let Some(n) = level {
        o.push_str(&format!("\\enforceprot1\\protlevel{n}"));
    }
    if !set.track_moves {
        o.push_str("\\trackmoves0");
    }
    if let Some(t) = set.attached_template.as_deref().filter(|t| !t.trim().is_empty()) {
        o.push_str(&format!("{{\\*\\template {}}}", esc(t)));
    }
    for (on, w) in [
        (!set.sum_paragraph_spacing, "\\htmautsp"),
        (set.link_styles, "\\linkstyles"),
        (set.remove_personal_info, "\\rempersonalinfo"),
        (set.remove_date_time, "\\remdttm"),
        (set.do_not_expand_shift_return, "\\expshrtn"),
        (set.track_changes, "\\revisions"),
        (set.print_forms_data, "\\printdata"),
    ] {
        if on {
            o.push_str(w);
        }
    }
    o
}

pub(super) const HEAD_SIZES: [f32; 6] = [20.0, 16.0, 14.0, 12.0, 12.0, 11.0];

thread_local! {
    /// The `\stylesheet` number of each of the document's own styles, by id, while [`export`]
    /// writes the document.
    static OWN_STYLES: std::cell::RefCell<HashMap<String, u32>> = std::cell::RefCell::new(HashMap::new());
}

/// The `\stylesheet` number of the document's own style `id`, while [`export`] writes it.
fn own_style_num(id: &str) -> Option<u32> {
    OWN_STYLES.with(|m| m.try_borrow().ok().and_then(|m| m.get(id).copied()))
}

/// Set the numbers [`own_style_num`] gives, returning the ones it gave before.
fn set_own_styles(nums: HashMap<String, u32>) -> HashMap<String, u32> {
    OWN_STYLES.with(|m| m.try_borrow_mut().map(|mut m| std::mem::replace(&mut *m, nums)).unwrap_or_default())
}

thread_local! {
    /// The bookmarks of the story being written whose end follows their start
    /// ([`model::ranged_anchors`]): their `\bkmkend` goes where the range ends, while every other
    /// bookmark is written as a point, start and end together.
    static RANGED: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
}

/// Whether bookmark `name` covers text in the story being written.
fn is_ranged(name: &str) -> bool {
    RANGED.with(|r| r.try_borrow().is_ok_and(|r| r.contains(name)))
}

/// Set the bookmarks [`is_ranged`] knows, returning the ones it knew before.
fn set_ranged(names: HashSet<String>) -> HashSet<String> {
    RANGED.with(|r| r.try_borrow_mut().map(|mut r| std::mem::replace(&mut *r, names)).unwrap_or_default())
}

/// The first `\stylesheet` number of the document's own styles: 0-9 are the fixed entries.
const FIRST_OWN_STYLE: u32 = 10;

/// The `\stylesheet` number of style `id`: a fixed entry's, or the document's own style's.
fn style_ref_num(id: &str) -> Option<u32> {
    match id {
        "Normal" => Some(0),
        "Title" => Some(7),
        "Quote" => Some(8),
        model::CODE_STYLE => Some(9),
        _ => id.strip_prefix("Heading").and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=6).contains(n)).or_else(|| own_style_num(id)),
    }
}

/// The row words of a table style's stylesheet entry (RTF 1.9.1, "Table styles"): its borders,
/// shading and cell padding (in twips).
fn table_style_words(parts: &wordcraft_doc::styles::TableStyleParts, t: &mut Tables, out: &mut String) {
    if let Some(b) = &parts.borders {
        for (side, opt) in [('t', b.top), ('l', b.left), ('b', b.bottom), ('r', b.right), ('h', b.between), ('v', b.inside_v)] {
            if let Some(border) = opt.filter(|x| x.is_visible() && x.width.is_finite()) {
                border_words("trbrdr", side, border, false, t, out);
            }
        }
    }
    if let Some(c) = parts.fill {
        out.push_str(&format!("\\trcbpat{}", t.color(c)));
    }
    if let Some([top, left, bottom, right]) = parts.cell_margins {
        for (side, v) in [('l', left), ('t', top), ('b', bottom), ('r', right)] {
            let tw = if v.is_finite() { (v.clamp(0.0, 1584.0) * 20.0).round() as i32 } else { 0 };
            out.push_str(&format!("\\trpadd{side}{tw}\\trpaddf{side}3"));
        }
    }
}

/// The table styles `doc`'s tables use, in the order they are first used, then the styles those
/// are based on: the ones its stylesheet writes.
fn used_table_styles(doc: &Document) -> Vec<String> {
    use wordcraft_doc::styles::StyleKind;
    fn walk(blocks: &Blocks, depth: usize, out: &mut Vec<String>) {
        for b in blocks.iter() {
            if let Block::Table(t) = &**b {
                if let Some(id) = &t.props.style
                    && !out.contains(id)
                    && out.len() < 256
                {
                    out.push(id.clone());
                }
                if depth < model::MAX_DEPTH {
                    for c in t.rows.iter().flat_map(|r| &r.cells) {
                        walk(&c.blocks, depth + 1, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&doc.body, 0, &mut out);
    for part in doc.parts.values() {
        walk(&part.blocks, 0, &mut out);
    }
    let is_table = |id: &str| doc.styles.get(id).is_some_and(|s| s.kind == StyleKind::Table);
    out.retain(|id| is_table(id));
    let mut bases: Vec<String> = Vec::new();
    for id in &out {
        let mut cur = doc.styles.get(id).and_then(|s| s.based_on.clone());
        for _ in 0..16 {
            let Some(b) = cur.filter(|b| is_table(b) && !out.contains(b) && !bases.contains(b)) else { break };
            cur = doc.styles.get(&b).and_then(|s| s.based_on.clone());
            bases.push(b);
        }
    }
    out.extend(bases);
    out
}

/// The `\stylesheet` entries of `doc`'s own paragraph and character styles, numbered as
/// [`own_style_num`] gives them; their fonts and colours go into `t`.
fn own_style_entries(doc: &Document, t: &mut Tables) -> String {
    use wordcraft_doc::styles::StyleKind;
    let tw = |v: f32| (v.clamp(-1584.0, 1584.0) * 20.0).round() as i32;
    let mut out = String::new();
    // In their numbers' order: a table's own style before the styles it is based on.
    let mut styles: Vec<(u32, &wordcraft_doc::styles::Style)> = doc.styles.styles.iter().filter_map(|s| Some((own_style_num(&s.id)?, s))).collect();
    styles.sort_by_key(|(n, _)| *n);
    for (n, s) in styles {
        let char = s.kind == StyleKind::Character;
        let table = s.kind == StyleKind::Table;
        if char {
            out.push_str(&format!("{{\\*\\cs{n}\\additive"));
        } else if table {
            out.push_str(&format!("{{\\*\\ts{n}\\tsrowd"));
            if let Some(parts) = &s.table {
                table_style_words(parts, t, &mut out);
            }
        } else {
            out.push_str(&format!("{{\\s{n}"));
        }
        if let Some(b) = s.based_on.as_deref().and_then(style_ref_num) {
            out.push_str(&format!("\\sbasedon{b}"));
        }
        if !char
            && !table
            && let Some(k) = s.next.as_deref().and_then(style_ref_num)
        {
            out.push_str(&format!("\\snext{k}"));
        }
        if let Some(l) = s.linked.as_deref().and_then(own_style_num) {
            out.push_str(&format!("\\slink{l}"));
        }
        if s.quick {
            out.push_str("\\sqformat");
        }
        if let Some(p) = s.priority {
            out.push_str(&format!("\\spriority{p}"));
        }
        if s.hidden {
            out.push_str("\\ssemihidden");
        }
        if s.locked {
            out.push_str("\\slocked");
        }
        if !char {
            let p = &s.para;
            match p.align {
                Some(Align::Left) => out.push_str("\\ql"),
                Some(Align::Center) => out.push_str("\\qc"),
                Some(Align::Right) => out.push_str("\\qr"),
                Some(Align::Justify) => out.push_str("\\qj"),
                Some(Align::Distribute) => out.push_str("\\qd"),
                None => {}
            }
            for (word, v) in [("li", p.indent_left), ("ri", p.indent_right), ("fi", p.indent_first), ("sb", p.space_before), ("sa", p.space_after)] {
                if let Some(v) = v.filter(|v| v.is_finite()) {
                    out.push_str(&format!("\\{word}{}", tw(v)));
                }
            }
            use wordcraft_doc::props::LineSpacing;
            match p.line_spacing {
                Some(LineSpacing::Multiple(m)) if m.is_finite() => {
                    out.push_str(&format!("\\sl{}\\slmult1", (m.clamp(0.06, 132.0) * 240.0).round() as i32));
                }
                Some(LineSpacing::AtLeast(v)) if v.is_finite() => out.push_str(&format!("\\sl{}\\slmult0", tw(v.max(0.0)))),
                Some(LineSpacing::Exactly(v)) if v.is_finite() => out.push_str(&format!("\\sl-{}\\slmult0", tw(v.max(1.0)))),
                _ => {}
            }
            for (word, v) in [("keepn", p.keep_next), ("keep", p.keep_lines)] {
                match v {
                    Some(true) => out.push_str(&format!("\\{word}")),
                    Some(false) => out.push_str(&format!("\\{word}0")),
                    None => {}
                }
            }
            style_tab_and_char_indent_words(p, &mut out);
        }
        let c = &s.chr;
        for (word, v) in [("b", c.bold), ("i", c.italic)] {
            match v {
                Some(true) => out.push_str(&format!("\\{word}")),
                Some(false) => out.push_str(&format!("\\{word}0")),
                None => {}
            }
        }
        if let Some(f) = c.font.as_deref().filter(|f| !f.is_empty()) {
            out.push_str(&format!("\\f{}", t.font(f)));
        }
        if let Some(sz) = c.size.filter(|v| v.is_finite()) {
            out.push_str(&format!("\\fs{}", (sz * 2.0).round().clamp(2.0, 3276.0) as i32));
        }
        match c.color {
            Some(wordcraft_doc::TextColor::Rgb(rgb)) => out.push_str(&format!("\\cf{}", t.color(rgb))),
            Some(wordcraft_doc::TextColor::Auto) => out.push_str("\\cf0"),
            None => {}
        }
        style_char_spacing_words(c.spacing, &mut out);
        out.push_str(&format!(" {};}}", esc_font(&s.name)));
    }
    out
}

/// A style entry's tab stops in position order, then its indents in hundredths of a character
/// (`\\cufi`, `\\culi`, `\\curi`).
fn style_tab_and_char_indent_words(p: &wordcraft_doc::props::ParaProps, out: &mut String) {
    let mut tabs: Vec<_> = p.tabs.iter().flatten().filter(|t| t.pos.is_finite()).collect();
    tabs.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    for t in tabs {
        out.push_str(&tab_words(t));
    }
    for (word, v) in [("\\cufi", p.indent_first_chars), ("\\culi", p.indent_left_chars), ("\\curi", p.indent_right_chars)] {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            out.push_str(&format!("{word}{}", (v.clamp(-1584.0, 1584.0) * 100.0).round() as i64));
        }
    }
}

/// A style entry's expanded or condensed spacing, as the text writes it: quarter points, then
/// the exact twips.
fn style_char_spacing_words(spacing: Option<f32>, out: &mut String) {
    if let Some(pt) = spacing.filter(|v| v.is_finite() && *v != 0.0) {
        let pt = pt.clamp(-1584.0, 1584.0);
        out.push_str(&format!("\\expnd{}\\expndtw{}", (pt * 4.0).round() as i32, (pt * 20.0).round() as i32));
    }
}

fn style_num(k: Kind) -> Option<u32> {
    match k {
        Kind::Heading(n) => Some(u32::from(n.clamp(1, 6))),
        Kind::Title => Some(7),
        Kind::Quote => Some(8),
        Kind::Code => Some(9),
        _ => None,
    }
}

/// The control word for the underline of `f` (RTF 1.9.1), `None` when it has none; a link's
/// text is underlined.
fn underline_word(f: &Fmt) -> Option<&'static str> {
    use wordcraft_doc::props::Underline;
    if !f.underline {
        return f.link.is_some().then_some("\\ul");
    }
    Some(match f.extra.as_ref().and_then(|x| x.underline) {
        Some(Underline::Words) => "\\ulw",
        Some(Underline::Double) => "\\uldb",
        Some(Underline::Thick) => "\\ulth",
        Some(Underline::Dotted) => "\\uld",
        Some(Underline::Dash) => "\\uldash",
        Some(Underline::DotDash) => "\\uldashd",
        Some(Underline::DotDotDash) => "\\uldashdd",
        Some(Underline::Wave) => "\\ulwave",
        Some(Underline::DoubleWave) => "\\ululdbwave",
        Some(Underline::None | Underline::Single) | None => "\\ul",
    })
}

/// The highlight colour (`\highlight`) and the character shading (`\chcbpat`) of `f`: with a
/// highlight its background is the highlight colour, and the shading under it is kept apart.
fn highlight_and_shading(f: &Fmt) -> (Option<Rgb>, Option<Rgb>) {
    match f.extra.as_deref().and_then(|x| x.highlight.and_then(|h| h.rgb()).map(|c| (c, x.shading))) {
        Some((c, shading)) => (Some(c), shading),
        None => (None, f.background),
    }
}

/// The underline colour of `f` (`\ulc`).
fn underline_color(f: &Fmt) -> Option<Rgb> {
    f.extra.as_ref().and_then(|x| x.underline_color)
}

fn rtf_border_style(s: wordcraft_doc::props::BorderStyle) -> &'static str {
    use wordcraft_doc::props::BorderStyle;
    match s {
        BorderStyle::None => "\\brdrnone",
        BorderStyle::Single => "\\brdrs",
        BorderStyle::Double => "\\brdrdb",
        BorderStyle::Dotted => "\\brdrdot",
        BorderStyle::Dashed => "\\brdrdash",
        BorderStyle::Thick => "\\brdrth",
        BorderStyle::Triple => "\\brdrtriple",
        BorderStyle::DotDash => "\\brdrdashd",
        BorderStyle::Wave => "\\brdrwavy",
        BorderStyle::DotDotDash => "\\brdrdashdd",
        BorderStyle::DoubleWave => "\\brdrwavydb",
        BorderStyle::DashDotStroked => "\\brdrdashdotstr",
        BorderStyle::ThinThickSmallGap => "\\brdrtnthsg",
        BorderStyle::ThickThinSmallGap => "\\brdrthtnsg",
        BorderStyle::ThinThickThinSmallGap => "\\brdrtnthtnsg",
        BorderStyle::ThinThickMediumGap => "\\brdrtnthmg",
        BorderStyle::ThickThinMediumGap => "\\brdrthtnmg",
        BorderStyle::ThinThickThinMediumGap => "\\brdrtnthtnmg",
        BorderStyle::ThinThickLargeGap => "\\brdrtnthlg",
        BorderStyle::ThickThinLargeGap => "\\brdrthtnlg",
        BorderStyle::ThinThickThinLargeGap => "\\brdrtnthtnlg",
        BorderStyle::ThreeDEmboss => "\\brdremboss",
        BorderStyle::ThreeDEngrave => "\\brdrengrave",
        BorderStyle::Outset => "\\brdroutset",
        BorderStyle::Inset => "\\brdrinset",
    }
}

/// One border side: `\{prefix}{side}`, its style, width, space (`\brsp`, when `space` and
/// non-zero) and colour, which goes into `t`.
fn border_words(prefix: &str, side: char, b: wordcraft_doc::props::Border, space: bool, t: &mut Tables, out: &mut String) {
    out.push_str(&format!("\\{prefix}{side}"));
    out.push_str(rtf_border_style(b.style));
    let w = ((b.width * 20.0).round() as i64).clamp(1, 150);
    out.push_str(&format!("\\brdrw{w}"));
    if space {
        let sp = (b.space * 20.0).round() as i64;
        if sp > 0 {
            out.push_str(&format!("\\brsp{sp}"));
        }
    }
    if let Some(c) = b.color {
        let i = t.color(c);
        out.push_str(&format!("\\brdrcf{i}"));
    }
}

impl Writer {
    fn write_border(&mut self, prefix: &str, side: char, b: wordcraft_doc::props::Border, out: &mut String) {
        border_words(prefix, side, b, prefix.starts_with("brdr"), &mut self.t, out);
    }

    fn cell_borders(&mut self, borders: Option<&wordcraft_doc::props::Borders>, borderless: bool, out: &mut String) {
        if let Some(b) = borders {
            for (side, opt) in [('t', &b.top), ('l', &b.left), ('b', &b.bottom), ('r', &b.right)] {
                if let Some(border) = opt.filter(|x| x.is_visible()) {
                    self.write_border("clbrdr", side, border, out);
                }
            }
        } else if !borderless {
            out.push_str("\\clbrdrt\\brdrs\\brdrw10\\clbrdrl\\brdrs\\brdrw10\\clbrdrb\\brdrs\\brdrw10\\clbrdrr\\brdrs\\brdrw10");
        }
    }

    /// `\revtbl` index of a tracked change's author (0 = "Unknown").
    fn author(&mut self, a: &str) -> usize {
        let authors = self.authors.get_or_insert_with(Vec::new);
        let a = a.trim();
        if a.is_empty() {
            return 0;
        }
        match authors.iter().position(|x| x == a) {
            Some(i) => i + 1,
            None => {
                authors.push(a.to_string());
                authors.len()
            }
        }
    }

    /// Control words of a tracked insertion and deletion (`\revised`, `\deleted`).
    fn rev_ctl(&mut self, ins: Option<&RevMark>, del: Option<&RevMark>) -> String {
        let mut ctl = String::new();
        if let Some(m) = ins {
            ctl.push_str(&self.rev_words("\\revised\\revauth", "\\revdttm", m));
        }
        if let Some(m) = del {
            ctl.push_str(&self.rev_words("\\deleted\\revauthdel", "\\revdttmdel", m));
        }
        ctl
    }

    /// `auth` with the `\revtbl` index of `m`'s author, then `date` with its DTTM when it has one.
    fn rev_words(&mut self, auth: &str, date: &str, m: &RevMark) -> String {
        let mut s = format!("{auth}{}", self.author(&m.author));
        if let Some(d) = iso_to_dttm(&m.date) {
            s.push_str(&format!("{date}{}", d as i32));
        }
        s
    }

    /// A run of text; place markers in it become notes and comment ranges.
    fn run(&mut self, t: &str, f: &Fmt, out: &mut String) {
        if !t.contains(MARK_OPEN) {
            return self.run_plain(t, f, out);
        }
        let mut rest = t;
        while !rest.is_empty() {
            let Some((pre, after)) = rest.split_once(MARK_OPEN) else {
                self.run_plain(rest, f, out);
                break;
            };
            if !pre.is_empty() {
                self.run_plain(pre, f, out);
            }
            let Some((tok, tail)) = after.split_once(MARK_CLOSE) else {
                self.run_plain(after, f, out);
                break;
            };
            self.mark(tok, out);
            rest = tail;
        }
    }

    /// The RTF for one place marker: `S` is a note's own number, a number indexes `marks`.
    fn mark(&mut self, tok: &str, out: &mut String) {
        if tok == "S" {
            out.push_str("{\\super\\chftn}");
            return;
        }
        let Some(m) = tok.parse::<usize>().ok().and_then(|i| self.marks.get_mut(i)) else { return };
        match std::mem::replace(m, Mark::Done) {
            Mark::Note { endnote, custom, blocks } => {
                out.push_str(&note_mark(&custom));
                out.push_str("{\\footnote");
                if endnote {
                    out.push_str("\\ftnalt");
                }
                out.push(' ');
                self.blocks(&blocks, 0, "\\par", out, 0);
                out.push('}');
            }
            Mark::Own(custom) => out.push_str(&note_mark(&custom)),
            Mark::RangeStart(id) => out.push_str(&format!("{{\\*\\atrfstart {id}}}")),
            Mark::RangeEnd { id, ann } => {
                out.push_str(&format!("{{\\*\\atrfend {id}}}"));
                if let Some(a) = ann {
                    out.push_str(&format!(
                        "{{\\*\\atnid {}}}{{\\*\\atnauthor {}}}\\chatn{{\\*\\annotation{{\\*\\atnref {id}}}",
                        esc(&a.initials),
                        esc(&a.author)
                    ));
                    if let Some(d) = rtf_date(&a.date) {
                        out.push_str(&format!("{{\\*\\atndate {d}}}"));
                    }
                    out.push('\n');
                    self.blocks(&a.blocks, 0, "\\par", out, 0);
                    out.push('}');
                }
            }
            Mark::Done => {}
        }
    }

    fn run_plain(&mut self, t: &str, f: &Fmt, out: &mut String) {
        let mut ctl = String::new();
        if let Some(c) = &f.fmt_change {
            // RTF 1.9.1, as Word writes it: the properties the change altered, as they were,
            // then who changed it and when, all in the group. The group goes first so a control
            // word, not the group, ends the run's properties.
            let old = self.char_diff(&c.old, f);
            let words = self.rev_words("\\crauth", "\\crdate", &c.mark);
            ctl.push_str(&format!("{{\\*\\oldcprops {old}{words}}}"));
        }
        ctl.push_str(&self.rev_ctl(f.ins.as_ref(), f.del.as_ref()));
        ctl.push_str(&self.char_ctl(f));
        let body = if ctl.is_empty() { esc(t) } else { format!("{{{ctl} {}}}", esc(t)) };
        // A script, `file:`, UNC or other non-web target is written as plain text, as the HTML and
        // Markdown exporters do: Word follows a HYPERLINK field's target on click, and a UNC one
        // leaks the reader's NTLM credentials.
        match f.link.as_deref().filter(|l| wordcraft_doc::link::is_safe_href(l)) {
            Some(l) => {
                // In a field's quotes `\\` is one backslash; a quote is stored as `%22`.
                let target = l.replace('\\', "\\\\").replace('"', "%22");
                // A ScreenTip is the `\o` switch; in its quotes a backslash escapes the next
                // character, and the RTF escaping keeps it text.
                let tip = f
                    .link_tip
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map_or_else(String::new, |t| format!(" \\\\o \"{}\"", esc(&t.replace('\\', "\\\\").replace('"', "\\\""))));
                out.push_str(&format!("{{\\field{{\\*\\fldinst{{HYPERLINK \"{}\"{}}}}}{{\\fldrslt{{{}}}}}}}", esc(&target), tip, body));
            }
            None => out.push_str(&body),
        }
    }

    /// The character formatting control words that turn `cur` back into `old`: only the
    /// properties that differ, with explicit "off" values (`\b0`, `\ulnone`, `\cf0`…).
    fn char_diff(&mut self, old: &Fmt, cur: &Fmt) -> String {
        // A link's underline and colour are the link's, before the change and after; what a run
        // doesn't set directly is its paragraph style's, as Word writes resolved values.
        let old = &styled(&self.base, &Fmt { link: cur.link.clone(), ..old.clone() });
        let cur = &styled(&self.base, cur);
        let mut ctl = String::new();
        let flags = [(old.bold, cur.bold, "\\b", "\\b0"), (old.italic, cur.italic, "\\i", "\\i0"), (old.strike, cur.strike, "\\strike", "\\strike0")];
        for (o, c, on, off) in flags {
            if o != c {
                ctl.push_str(if o { on } else { off });
            }
        }
        let old_ul = underline_word(old);
        if old_ul != underline_word(cur) {
            ctl.push_str(old_ul.unwrap_or("\\ulnone"));
        }
        let old_ulc = underline_color(old);
        if old_ulc != underline_color(cur) {
            let i = old_ulc.map_or(0, |c| self.t.color(c));
            ctl.push_str(&format!("\\ulc{i}"));
        }
        if (old.sup, old.sub) != (cur.sup, cur.sub) {
            ctl.push_str(if old.sup {
                "\\super"
            } else if old.sub {
                "\\sub"
            } else {
                "\\nosupersub"
            });
        }
        fn font(f: &Fmt) -> Option<&str> {
            if f.code { Some(model::MONO_FONT) } else { f.font.as_deref() }
        }
        let old_font = font(old);
        if old_font != font(cur) {
            // No font is the document's default font, font 0 (Normal's).
            let i = old_font.map_or(0, |n| self.t.font(n));
            ctl.push_str(&format!("\\f{i}"));
        }
        let half_points = |s: Option<f32>| s.map(|s| (s * 2.0).round().clamp(2.0, 3276.0) as i32);
        let old_size = half_points(old.size);
        if old_size != half_points(cur.size) {
            // No size is Normal's, RTF's default 12 pt.
            ctl.push_str(&format!("\\fs{}", old_size.unwrap_or(24)));
        }
        let color = |f: &Fmt| f.color.or(if f.link.is_some() { Some(Rgb(0x05, 0x63, 0xC1)) } else { None });
        let old_color = color(old);
        if old_color != color(cur) {
            let i = old_color.map_or(0, |c| self.t.color(c));
            ctl.push_str(&format!("\\cf{i}"));
        }
        let (old_hl, old_shd) = highlight_and_shading(old);
        let (cur_hl, cur_shd) = highlight_and_shading(cur);
        if old_hl != cur_hl {
            let i = old_hl.map_or(0, |c| self.t.color(c));
            ctl.push_str(&format!("\\highlight{i}"));
        }
        if old_shd != cur_shd {
            let i = old_shd.map_or(0, |c| self.t.color(c));
            ctl.push_str(&format!("\\chcbpat{i}"));
        }
        let border = |f: &Fmt| f.extra.as_ref().and_then(|x| x.border).filter(|b| b.is_visible());
        let old_border = border(old);
        if old_border != border(cur) {
            match old_border {
                Some(b) => border_words("chbrd", 'r', b, true, &mut self.t, &mut ctl),
                None => ctl.push_str("\\chbrdr\\brdrnone"),
            }
        }
        if old.hidden != cur.hidden {
            ctl.push_str(if old.hidden { "\\v" } else { "\\v0" });
        }
        let old_pos = position_word(old);
        if old_pos != position_word(cur) {
            ctl.push_str(old_pos.as_deref().unwrap_or("\\up0"));
        }
        ctl
    }

    /// Character formatting control words of `f` (no revision marks).
    fn char_ctl(&mut self, f: &Fmt) -> String {
        let mut ctl = String::new();
        if let Some(n) = f.extra.as_ref().and_then(|x| x.style.as_deref()).and_then(own_style_num) {
            ctl.push_str(&format!("\\cs{n}"));
        }
        if f.bold {
            ctl.push_str("\\b");
        }
        if f.italic {
            ctl.push_str("\\i");
        }
        if let Some(w) = underline_word(f) {
            ctl.push_str(w);
        }
        if let Some(c) = underline_color(f) {
            let i = self.t.color(c);
            ctl.push_str(&format!("\\ulc{i}"));
        }
        if f.strike {
            ctl.push_str("\\strike");
        }
        if f.sup {
            ctl.push_str("\\super");
        } else if f.sub {
            ctl.push_str("\\sub");
        }
        if f.code {
            let i = self.t.font(model::MONO_FONT);
            ctl.push_str(&format!("\\f{i}"));
        } else if let Some(fam) = &f.font {
            let i = self.t.font(fam);
            ctl.push_str(&format!("\\f{i}"));
        }
        if let Some(s) = f.size {
            ctl.push_str(&format!("\\fs{}", (s * 2.0).round().clamp(2.0, 3276.0) as i32));
        }
        let color = f.color.or(if f.link.is_some() { Some(Rgb(0x05, 0x63, 0xC1)) } else { None });
        if let Some(c) = color {
            let i = self.t.color(c);
            ctl.push_str(&format!("\\cf{i}"));
        }
        let (highlight, shading) = highlight_and_shading(f);
        if let Some(c) = highlight {
            let i = self.t.color(c);
            ctl.push_str(&format!("\\highlight{i}"));
        }
        if let Some(c) = shading {
            let i = self.t.color(c);
            ctl.push_str(&format!("\\chcbpat{i}"));
        }
        // Expanded or condensed: quarter points for older readers, then the exact twips.
        if let Some(pt) = f.extra.as_ref().and_then(|x| x.spacing).filter(|v| v.is_finite() && *v != 0.0) {
            let pt = pt.clamp(-1584.0, 1584.0);
            ctl.push_str(&format!("\\expnd{}\\expndtw{}", (pt * 4.0).round() as i32, (pt * 20.0).round() as i32));
        }
        if let Some(x) = f.extra.as_deref() {
            let effects = [
                (x.caps, "\\caps"),
                (x.small_caps, "\\scaps"),
                (x.outline, "\\outl"),
                (x.shadow, "\\shad"),
                (x.emboss, "\\embo"),
                (x.engrave, "\\impr"),
                (x.double_strike, "\\striked1"),
                (x.no_proof, "\\noproof"),
            ];
            for (v, w) in effects {
                if v == Some(true) {
                    ctl.push_str(w);
                }
            }
            // Horizontal scale in percent, 1-600; kerning from a size in half-points.
            if let Some(s) = x.scale.filter(|v| v.is_finite()).map(|v| v.round().clamp(1.0, 600.0) as i32).filter(|v| *v != 100) {
                ctl.push_str(&format!("\\charscalex{s}"));
            }
            if let Some(k) = x.kern.filter(|v| v.is_finite() && *v > 0.0).map(|v| (v * 2.0).round().clamp(1.0, 3276.0) as i32) {
                ctl.push_str(&format!("\\kerning{k}"));
            }
            let mark = match x.emphasis {
                Some(EmphasisMark::Dot) => "\\accdot",
                Some(EmphasisMark::Comma) => "\\acccomma",
                Some(EmphasisMark::Circle) => "\\acccircle",
                Some(EmphasisMark::UnderDot) => "\\accunderdot",
                Some(EmphasisMark::None) | None => "",
            };
            ctl.push_str(mark);
            // The border around the text (Borders and Shading, Apply to: Text).
            if let Some(b) = x.border.filter(|b| b.is_visible()) {
                border_words("chbrd", 'r', b, true, &mut self.t, &mut ctl);
            }
        }
        if let Some(w) = position_word(f) {
            ctl.push_str(&w);
        }
        if f.hidden {
            ctl.push_str("\\v");
        }
        ctl
    }

    fn picture(&mut self, img: &model::Img, out: &mut String) {
        // A metafile's `\picw`/`\pich` are its size in 0.01 mm (#4259).
        let hundredths = ((img.w * 2540.0 / 72.0).round() as i64, (img.h * 2540.0 / 72.0).round() as i64);
        let (data, blip) = match img.ext.as_str() {
            "png" => (img.data.to_vec(), "\\pngblip"),
            "jpeg" | "jpg" => (img.data.to_vec(), "\\jpegblip"),
            // The bytes decide, as on reading: an EMF starts with its header record, type 1 (a
            // DOCX may store one under `.wmf`).
            "emf" | "wmf" if img.data.starts_with(&[1, 0, 0, 0]) => (img.data.to_vec(), "\\emfblip"),
            // RTF holds a WMF without its 22-byte placeable header.
            "emf" | "wmf" => {
                let data = if img.data.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) { img.data.get(22..).unwrap_or_default() } else { img.data.as_slice() };
                (data.to_vec(), "\\wmetafile8")
            }
            _ => {
                // Re-encode other formats as PNG, bounding the decode so a tiny file declaring
                // huge dimensions can't exhaust memory on export.
                let Some(dec) = model::decode_bounded(&img.data) else { return };
                let mut buf = Vec::new();
                if dec.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).is_err() {
                    return;
                }
                (buf, "\\pngblip")
            }
        };
        // A placeable header with nothing after it leaves no picture to write.
        if data.is_empty() {
            return;
        }
        let (pw, ph) = match blip {
            "\\emfblip" | "\\wmetafile8" => hundredths,
            _ => model::image_px(&data).map(|(a, b)| (i64::from(a), i64::from(b))).unwrap_or((1, 1)),
        };
        out.push_str(&format!("{{\\pict{blip}\\picw{pw}\\pich{ph}{}\n", pict_goal(img)));
        for chunk in data.chunks(64) {
            for b in chunk {
                out.push_str(&format!("{b:02x}"));
            }
            out.push('\n');
        }
        out.push('}');
    }

    fn inlines(&mut self, inl: &[Inline], out: &mut String) {
        let inl = model::equations_as_text(inl);
        for i in inl.iter() {
            match i {
                Inline::Text(t, f) => self.run(t, f, out),
                Inline::Image(img) => self.picture(img, out),
                Inline::Shape { kind, w, h, float, blocks, look, .. } => {
                    use wordcraft_doc::para::{Anchor, ShapeKind, Wrap};
                    let f = float.as_deref().cloned().unwrap_or_default();
                    let tw = |v: f32| (v * 20.0).round() as i32;
                    let wrap = match f.wrap {
                        Wrap::TopAndBottom => 1,
                        Wrap::Square => 2,
                        Wrap::Tight => 4,
                        Wrap::Through => 5,
                        _ => 3,
                    };
                    let bx = match f.h_rel {
                        Anchor::Page => "page",
                        Anchor::Margin => "margin",
                        _ => "column",
                    };
                    let by = match f.v_rel {
                        Anchor::Page => "page",
                        Anchor::Margin => "margin",
                        _ => "para",
                    };
                    out.push_str(&format!(
                        "{{\\shp{{\\*\\shpinst\\shpleft{}\\shptop{}\\shpright{}\\shpbottom{}\\shpbx{bx}\\shpby{by}\\shpwr{wrap}\\shpfblwtxt{}",
                        tw(f.x),
                        tw(f.y),
                        tw(f.x + w),
                        tw(f.y + h),
                        u8::from(f.wrap == Wrap::BehindText)
                    ));
                    out.push_str(&format!("\\shpz{}", f.z.saturating_sub(1)));
                    for (name, value) in [
                        ("shapeType", if *kind == ShapeKind::TextBox { 202 } else { 1 }),
                        ("fFilled", i32::from(look.fill.is_some())),
                        ("fLine", i32::from(look.line.is_some())),
                    ] {
                        out.push_str(&format!("{{\\sp{{\\sn {name}}}{{\\sv {value}}}}}"));
                    }
                    if let Some(inset) = look.inset {
                        for (name, v) in ["dxTextLeft", "dyTextTop", "dxTextRight", "dyTextBottom"].iter().zip(inset) {
                            out.push_str(&format!("{{\\sp{{\\sn {name}}}{{\\sv {}}}}}", (v * 12700.0).round() as i32));
                        }
                    }
                    out.push_str("{\\shptxt ");
                    let base = self.base.clone();
                    self.blocks(blocks, 0, "\\par", out, 1);
                    self.base = base;
                    out.push_str("}}}");
                }
                // RTF 1.9.1 "Bookmarks": a range's end is written where it ends; a point bookmark
                // gets both marks together, as Word needs an end for each start.
                Inline::Anchor(a) if is_ranged(a) => out.push_str(&format!("{{\\*\\bkmkstart {}}}", esc(a))),
                Inline::Anchor(a) => {
                    let a = esc(a);
                    out.push_str(&format!("{{\\*\\bkmkstart {a}}}{{\\*\\bkmkend {a}}}"));
                }
                Inline::AnchorEnd(a) if is_ranged(a) => out.push_str(&format!("{{\\*\\bkmkend {}}}", esc(a))),
                // RTF keeps no ruby yet: its base text, in the run's formatting.
                Inline::Ruby { base, fmt, .. } => self.run(base, fmt, out),
                // ASK and SET show nothing (see `model::shows_nothing`).
                Inline::Field { instr, .. } if model::shows_nothing(instr) => {}
                Inline::Field { instr, result, fmt, form, locked } => {
                    // A locked field keeps its result (RTF 1.9.1 "Fields": `\fldlock`).
                    let lock = if *locked { "\\fldlock" } else { "" };
                    // A legacy form field's definition goes in its instruction, as Word writes it.
                    match super::form_field::rtf_group(form) {
                        Some(group) => out.push_str(&format!("{{\\field{lock}{{\\*\\fldinst {{{} {group}}}}}{{\\fldrslt ", esc(instr.trim()))),
                        None => out.push_str(&format!("{{\\field{lock}{{\\*\\fldinst {}}}{{\\fldrslt ", esc(instr))),
                    }
                    self.run(result, fmt, out);
                    out.push_str("}}");
                }
                Inline::Control { text: result, fmt, .. } => self.run(result, fmt, out),
                Inline::Figure(_)
                | Inline::Chart(_)
                | Inline::Equation { .. }
                | Inline::Note { .. }
                | Inline::Comment { .. }
                | Inline::CommentEnd(_)
                | Inline::Group { .. }
                | Inline::FieldStart(_)
                | Inline::FieldEnd
                | Inline::AnchorEnd(_)
                | Inline::ObjRev(_) => {}
            }
        }
    }

    pub(super) fn para(&mut self, p: &Para, itap: impl Into<usize>, ls: Option<(usize, String)>, end: &str, out: &mut String) {
        let itap = itap.into();
        let ctl = self.para_ctl(p, itap);
        out.push_str(&ctl);
        if let Some(c) = &p.fmt_change {
            // RTF 1.9.1: the paragraph formatting before the change, then who changed it and when.
            let old = self.para_ctl(&c.old, itap);
            // The old list is the same list instance (only its level may differ).
            let list = match (c.old.list, &ls) {
                (Some(li), Some((n, _))) => format!("\\ls{n}\\ilvl{}", li.level.min(8)),
                _ => String::new(),
            };
            out.push_str(&format!("{{\\*\\oldpprops {old}{list}}}"));
            let words = self.rev_words("\\prauth", "\\prdate", &c.mark);
            out.push_str(&words);
        }
        if p.kind != Kind::Rule
            && let Some(b) = &p.borders
        {
            for (side, opt) in [('t', &b.top), ('l', &b.left), ('b', &b.bottom), ('r', &b.right)] {
                if let Some(border) = opt.filter(|x| x.is_visible()) {
                    self.write_border("brdr", side, border, out);
                }
            }
            if let Some(border) = b.between.filter(|x| x.is_visible()) {
                // `\brdrbtw`: the border between paragraphs with the same borders.
                self.write_border("brdrbt", 'w', border, out);
            }
        }
        if let Some(c) = p.extra.as_ref().and_then(|x| x.shading) {
            let i = self.t.color(c);
            out.push_str(&format!("\\cbpat{i}"));
        }
        if let (Some(li), Some((n, label))) = (p.list, &ls) {
            let lv = li.level.min(8);
            out.push_str(&format!("\\ls{n}\\ilvl{lv}"));
            // The level's indents, unless the paragraph states its own (written by `para_ctl`).
            if p.spacing.left.is_none() && p.spacing.first.is_none() {
                out.push_str(&format!("\\fi-360\\li{}", 720 * (u32::from(lv) + 1)));
            }
            // The closing brace ends the label; a space after it would be text.
            out.push_str(&format!(" {{\\listtext\\pard\\plain {}\\tab}}", esc(label)));
        } else {
            out.push(' ');
        }
        self.base = style_fmt(p.kind);
        self.inlines(&p.inlines, out);
        self.base = Fmt::default();
        // A tracked paragraph mark is the `\par` inside the revision's group.
        let mut marks = self.rev_ctl(p.mark_ins.as_deref(), p.mark_del.as_deref());
        // An empty paragraph's mark formatting: the character formatting at its `\par`.
        if let Some(m) = p.mark.as_deref().filter(|_| p.textless()) {
            let f = mark_fmt(m);
            marks.push_str(&self.char_ctl(&f));
        }
        if end == "\\par" && !marks.is_empty() {
            out.push_str(&format!("{{{marks} {end}}}"));
        } else if !marks.is_empty() {
            out.push_str(&format!("{marks} {end}"));
        } else {
            out.push_str(end);
        }
        out.push('\n');
    }

    /// The paragraph formatting control words of `p`, from `\pard\plain` (its list left out),
    /// at table depth `itap` (0 the body).
    fn para_ctl(&mut self, p: &Para, itap: usize) -> String {
        let mut out = String::from("\\pard\\plain");
        out.push_str(&in_table(itap));
        if let Some(s) = p.extra.as_ref().and_then(|x| x.style.as_deref()).and_then(own_style_num).or_else(|| style_num(p.kind)) {
            out.push_str(&format!("\\s{s}"));
        }
        if let Kind::Heading(n) = p.kind {
            out.push_str(&format!("\\outlinelevel{}", n.clamp(1, 6) - 1));
        } else if let Some(l) = p.extra.as_ref().and_then(|x| x.outline_level).filter(|l| *l < 9) {
            // An outline level apart from a heading style: the paragraph keeps its own style.
            out.push_str(&format!("\\outlinelevel{l}"));
        }
        match p.align {
            Some(Align::Center) => out.push_str("\\qc"),
            Some(Align::Right) => out.push_str("\\qr"),
            Some(Align::Justify) => out.push_str("\\qj"),
            Some(Align::Distribute) => out.push_str("\\qd"),
            _ => out.push_str("\\ql"),
        }
        if p.page_break {
            out.push_str("\\pagebb");
        }
        for t in p.extra.as_ref().and_then(|x| x.tabs.as_deref()).unwrap_or_default() {
            out.push_str(&tab_words(t));
        }
        match p.extra.as_ref().and_then(|x| x.widow_control) {
            Some(true) => out.push_str("\\widctlpar"),
            Some(false) => out.push_str("\\nowidctlpar"),
            None => {}
        }
        // Automatic space between Asian and Latin text and numbers: off in RTF unless written
        // (the flow model holds the effective value, unset when on).
        let x = p.extra.as_deref();
        if x.and_then(|x| x.auto_space_de) != Some(false) {
            out.push_str("\\aspalpha");
        }
        if x.and_then(|x| x.auto_space_dn) != Some(false) {
            out.push_str("\\aspnum");
        }
        if let Some(x) = p.extra.as_deref() {
            if x.contextual_spacing == Some(true) {
                out.push_str("\\contextualspace");
            }
            if x.mirror_indents == Some(true) {
                out.push_str("\\mirrorindents");
            }
            if let Some(v) = x.space_before_auto {
                out.push_str(if v { "\\sbauto1" } else { "\\sbauto0" });
            }
            if let Some(v) = x.space_after_auto {
                out.push_str(if v { "\\saauto1" } else { "\\saauto0" });
            }
            if let Some(v) = x.suppress_hyphens {
                out.push_str(if v { "\\hyphpar0" } else { "\\hyphpar1" });
            }
            if x.snap_to_grid == Some(false) {
                out.push_str("\\nosnaplinegrid");
            }
            // Indents in hundredths of a character, beside the twips ones.
            for (word, v) in [("\\culi", x.indent_left_chars), ("\\curi", x.indent_right_chars), ("\\cufi", x.indent_first_chars)] {
                if let Some(v) = v {
                    out.push_str(&format!("{word}{}", (v.clamp(-1584.0, 1584.0) * 100.0).round() as i64));
                }
            }
            out.push_str(&super::frames::frame_words(x));
        }
        match p.kind {
            Kind::Quote => out.push_str("\\li720\\ri720\\i"),
            Kind::Code => {
                let f = self.t.font(model::MONO_FONT);
                out.push_str(&format!("\\sa0\\f{f}\\fs20"));
            }
            Kind::Rule => out.push_str("\\brdrb\\brdrs\\brdrw15\\brsp20"),
            Kind::Heading(n) => {
                let sz = HEAD_SIZES.get(usize::from(n.clamp(1, 6)) - 1).copied().unwrap_or(12.0);
                out.push_str(&format!("\\sb240\\sa80\\keepn\\b\\fs{}", (sz * 2.0) as i32));
            }
            Kind::Title => out.push_str("\\sa80\\fs56"),
            Kind::Normal => {}
        }
        // The paragraph's own spacing, indents and keeps, after its kind's (the later word wins).
        out.push_str(&spacing_words(&p.spacing));
        if p.keep_next && !matches!(p.kind, Kind::Heading(_)) {
            out.push_str("\\keepn");
        }
        if let Some(x) = p.extra.as_deref() {
            if x.keep_lines == Some(true) {
                out.push_str("\\keep");
            }
            if x.suppress_line_numbers == Some(true) {
                out.push_str("\\noline");
            }
        }
        out
    }

    /// Writes `blocks` at table depth `itap` (0 the body, 1 a table's cells, 2 a nested table's…);
    /// `depth` counts the stories and tables they sit in.
    fn blocks(&mut self, blocks: &[FBlock], itap: usize, last_end: &str, out: &mut String, depth: usize) {
        let mut counter = model::ListCounter::default();
        let mut cur_list: Option<usize> = None;
        let mut top_ordered = false;
        let n = blocks.len();
        for (i, b) in blocks.iter().enumerate() {
            let mut end = if i + 1 == n { last_end.to_string() } else { "\\par".to_string() };
            match b {
                FBlock::Para(p) => {
                    // A paragraph that ends a section ends it with `\sect`; the next one begins.
                    if p.section.is_some() && itap == 0 && depth == 0 && self.sect_i + 1 < self.sections.len() {
                        self.sect_i += 1;
                        let doc_gutter = self.sections.first().map_or(0.0, |f| f.gutter);
                        end =
                            format!("\\sect\n{}", self.sections.get(self.sect_i).map(|s| sect_words(s, doc_gutter, &mut self.t)).unwrap_or_default());
                        // A body that ends on the break keeps its last section: an empty paragraph holds it.
                        if i + 1 == n {
                            end.push_str("\\pard\\plain \\par");
                        }
                    }
                    let ls = match p.list {
                        Some(li) => {
                            // A top-level item of the other kind starts a new list.
                            if li.level == 0 && cur_list.is_some() && top_ordered != li.ordered {
                                cur_list = None;
                            }
                            if li.level == 0 {
                                top_ordered = li.ordered;
                            }
                            let id = match cur_list {
                                Some(id) => id,
                                None => {
                                    // Level kinds from the whole run of list paragraphs.
                                    let mut kinds = [li.ordered; 9];
                                    let mut seen = [false; 9];
                                    for q in blocks.iter().skip(i) {
                                        let FBlock::Para(q) = q else { break };
                                        let Some(ql) = q.list else { break };
                                        let l = ql.level.min(8) as usize;
                                        if l == 0 && seen[0] && kinds[0] != ql.ordered {
                                            break;
                                        }
                                        if let (Some(s), Some(k)) = (seen.get_mut(l), kinds.get_mut(l))
                                            && !*s
                                        {
                                            *s = true;
                                            *k = ql.ordered;
                                        }
                                    }
                                    self.lists.push(kinds);
                                    counter.reset();
                                    self.lists.len()
                                }
                            };
                            cur_list = Some(id);
                            Some((id, counter.label(li)))
                        }
                        None => {
                            cur_list = None;
                            None
                        }
                    };
                    self.para(p, itap, ls, &end, out);
                }
                FBlock::Table(t) => {
                    cur_list = None;
                    if itap > 0 && itap < model::MAX_DEPTH {
                        // A table in a cell: Word's nested table, one level deeper. The cell's
                        // end mark follows in a paragraph of its own.
                        self.table(t, out, itap + 1, depth + 1);
                        if i + 1 == n && !last_end.is_empty() {
                            out.push_str(&format!("\\pard\\plain{} {last_end}\n", in_table(itap)));
                        }
                    } else if itap > 0 || depth > 0 {
                        // Too deep, or in a story with no tables: its cells' paragraphs flow
                        // into this cell.
                        let inner: Vec<FBlock> = t.rows.iter().flatten().flat_map(|c| c.blocks.clone()).collect();
                        if depth < model::MAX_DEPTH {
                            self.blocks(&inner, itap, &end, out, depth + 1);
                        }
                    } else {
                        self.table(t, out, 1, depth);
                    }
                }
            }
        }
        if blocks.is_empty() && !last_end.is_empty() {
            out.push_str(&format!("\\pard\\plain{} {last_end}\n", in_table(itap)));
        }
    }

    /// The row definition of row `ri` of `t` (`\trowd` … each cell's `\cellxN`), with the
    /// table's column `widths` in twips.
    fn row_props(&mut self, t: &FTable, ri: usize, row: &[model::Cell], widths: &[f32]) -> String {
        let mut props = String::new();
        let out = &mut props;
        let extra = t.extra.as_deref();
        // Its alignment, else its left edge (`\trleft`, which each `\cellx` is measured with).
        out.push_str("\\trowd");
        let align = extra.and_then(|x| x.align);
        match align {
            Some(Align::Center) => out.push_str("\\trqc"),
            Some(Align::Right) => out.push_str("\\trqr"),
            _ => {}
        }
        let left = match align {
            Some(Align::Center | Align::Right) => 0,
            _ => extra.and_then(|x| x.indent).filter(|i| i.is_finite()).map_or(0, |i| ((i * 20.0).round() as i64).clamp(-31_680, 31_680)),
        };
        // Its default cell padding: the gap is the left padding, each side stated in twips.
        let pad = extra.and_then(|x| x.cell_margins).filter(|m| m.iter().all(|v| v.is_finite()));
        match pad {
            Some(m) => {
                let [t, l, b, r] = m.map(|v| ((v * 20.0).round() as i64).clamp(0, 31_680));
                out.push_str(&format!(
                    "\\trgaph{l}\\trleft{left}\\trpaddl{l}\\trpaddt{t}\\trpaddb{b}\\trpaddr{r}\\trpaddfl3\\trpaddft3\\trpaddfb3\\trpaddfr3"
                ));
            }
            None => out.push_str(&format!("\\trgaph108\\trleft{left}")),
        }
        // The table's style and its look.
        if let Some(n) = extra.and_then(|x| x.style.as_deref()).and_then(own_style_num) {
            out.push_str(&format!("\\ts{n}"));
        }
        if let Some(l) = extra.and_then(|x| x.look) {
            for (on, word) in [
                (l.header_row, "\\tbllkhdrrows"),
                (l.total_row, "\\tbllklastrow"),
                (l.first_column, "\\tbllkhdrcols"),
                (l.last_column, "\\tbllklastcol"),
                (!l.banded_rows, "\\tbllknorowband"),
                (!l.banded_columns, "\\tbllknocolband"),
            ] {
                if on {
                    out.push_str(word);
                }
            }
        }
        if !row.is_empty() && row.iter().all(|c| c.header) {
            out.push_str("\\trhdr");
        }
        // Row height in twips: negative for exactly, positive for at least.
        if let Some(&Some((h, exact))) = extra.and_then(|x| x.heights.get(ri))
            && h.is_finite()
            && h > 0.0
        {
            let tw = (h.min(1584.0) * 20.0).round() as i64;
            out.push_str(&format!("\\trrh{}", if exact { -tw } else { tw }));
        }
        if extra.is_some_and(|x| x.keep_rows.get(ri) == Some(&true)) {
            out.push_str("\\trkeep");
        }
        let mut g = 0usize;
        let mut x = 0.0f32;
        for c in row {
            let span = c.colspan.clamp(1, 63) as usize;
            for k in g..(g + span).min(widths.len()) {
                x += widths.get(k).copied().unwrap_or(0.0);
            }
            g += span;
            if c.covered {
                out.push_str("\\clvmrg");
            } else if c.rowspan > 1 {
                out.push_str("\\clvmgf");
            }
            match c.valign {
                Some(VAlign::Top) => out.push_str("\\clvertalt"),
                Some(VAlign::Center) => out.push_str("\\clvertalc"),
                Some(VAlign::Bottom) => out.push_str("\\clvertalb"),
                None => {}
            }
            // The padding sides the cell sets itself; Word writes the top as `\clpadl` and the
            // left as `\clpadt`.
            for (side, v) in ["l", "t", "b", "r"].iter().zip(c.own_margins) {
                if let Some(v) = v.filter(|v| v.is_finite()) {
                    let tw = (v * 20.0).round().clamp(0.0, 31_680.0) as i32;
                    out.push_str(&format!("\\clpad{side}{tw}\\clpadf{side}3"));
                }
            }
            if c.no_wrap {
                out.push_str("\\clNoWrap");
            }
            match c.direction {
                wordcraft_doc::props::TextDirection::Down => out.push_str("\\cltxtbrl"),
                wordcraft_doc::props::TextDirection::Up => out.push_str("\\cltxbtlr"),
                _ => {}
            }
            self.cell_borders(c.borders.as_ref(), t.borderless, out);
            if let Some(s) = c.shading {
                let i = self.t.color(s);
                out.push_str(&format!("\\clcbpat{i}"));
            }
            out.push_str(&format!("\\cellx{}", (x.round() as i64).saturating_add(left)));
        }
        props
    }

    /// Writes table `t` at table depth `itap` (1 a top-level table; 2 and deeper a table in a
    /// cell, whose cells end with `\nestcell` and whose row properties follow each row in
    /// `{\*\nesttableprops … \nestrow}`, then an empty `{\nonesttables …}` copy).
    fn table(&mut self, t: &FTable, out: &mut String, itap: usize, depth: usize) {
        let nested = itap > 1;
        let cell_end = if nested { "\\nestcell" } else { "\\cell" };
        let cols = t.cols().max(1);
        let total = 9360.0f32; // 6.5" in twips
        let widths: Vec<f32> = if t.widths.len() == cols && t.widths.iter().all(|w| w.is_finite() && *w > 0.0) {
            t.widths.iter().map(|w| w * 20.0).collect()
        } else {
            vec![total / cols as f32; cols]
        };
        for (ri, row) in t.rows.iter().enumerate() {
            let props = self.row_props(t, ri, row, &widths);
            if !nested {
                out.push_str(&props);
                out.push('\n');
            }
            for c in row {
                if c.covered {
                    out.push_str(&format!("\\pard\\plain{} {cell_end}\n", in_table(itap)));
                } else {
                    let blocks: Vec<FBlock> = c
                        .blocks
                        .iter()
                        .cloned()
                        .map(|b| match b {
                            FBlock::Para(mut p) if c.header => {
                                for i in &mut p.inlines {
                                    if let Inline::Text(_, f) = i {
                                        f.bold = true;
                                    }
                                }
                                FBlock::Para(p)
                            }
                            other => other,
                        })
                        .collect();
                    self.blocks(&blocks, itap, cell_end, out, depth);
                }
            }
            if nested {
                out.push_str(&format!("{{\\*\\nesttableprops{props}\\nestrow}}{{\\nonesttables\\par}}\n"));
            } else {
                out.push_str("\\row\n");
            }
        }
    }
}

/// The paragraph spacing and indent words of `s` (RTF 1.9.1), in twips: only what it states.
fn spacing_words(s: &model::Spacing) -> String {
    use wordcraft_doc::props::LineSpacing;
    let tw = |pt: f32, lo: f32| (pt.clamp(lo, 1584.0) * 20.0).round() as i32;
    let mut out = String::new();
    for (word, v, lo) in [("sb", s.before, 0.0), ("sa", s.after, 0.0), ("li", s.left, -1584.0), ("ri", s.right, -1584.0), ("fi", s.first, -1584.0)] {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            out.push_str(&format!("\\{word}{}", tw(v, lo)));
        }
    }
    match s.line {
        // 240 is single spacing.
        Some(LineSpacing::Multiple(m)) if m.is_finite() => {
            out.push_str(&format!("\\sl{}\\slmult1", (m.clamp(0.06, 132.0) * 240.0).round() as i32));
        }
        // Twips: positive at least, negative exactly.
        Some(LineSpacing::AtLeast(pt)) if pt.is_finite() && pt > 0.0 => out.push_str(&format!("\\sl{}\\slmult0", tw(pt, 0.0))),
        Some(LineSpacing::Exactly(pt)) if pt.is_finite() && pt > 0.0 => out.push_str(&format!("\\sl-{}\\slmult0", tw(pt, 0.0))),
        _ => {}
    }
    out
}

/// The raised or lowered position of `f` in half-points: `\upN` or `\dnN`; `None` at the baseline.
fn position_word(f: &Fmt) -> Option<String> {
    let pt = f.extra.as_ref().and_then(|x| x.position).filter(|v| v.is_finite())?;
    let half = (pt.clamp(-1584.0, 1584.0) * 2.0).round() as i32;
    match half {
        0 => None,
        h if h > 0 => Some(format!("\\up{h}")),
        h => Some(format!("\\dn{}", h.unsigned_abs())),
    }
}

/// The table words of a paragraph at table depth `itap`: `\intbl`, with `\itapN` in a nested
/// table.
fn in_table(itap: usize) -> String {
    match itap {
        0 => String::new(),
        1 => "\\intbl".to_string(),
        n => format!("\\intbl\\itap{n}"),
    }
}

fn level_def(ordered: bool, l: usize) -> String {
    let indent = 720 * (l + 1);
    if ordered {
        let nfc = [0, 4, 2].get(l % 3).copied().unwrap_or(0);
        format!(
            "{{\\listlevel\\levelnfc{nfc}\\levelnfcn{nfc}\\leveljc0\\leveljcn0\\levelfollow0\\levelstartat1\\levelspace0\\levelindent0{{\\leveltext\\'02\\'{l:02x}.;}}{{\\levelnumbers\\'01;}}\\fi-360\\li{indent}\\lin{indent}}}"
        )
    } else {
        let ch = ['\u{2022}', '\u{25E6}', '\u{25AA}'].get(l % 3).copied().unwrap_or('\u{2022}');
        format!(
            "{{\\listlevel\\levelnfc23\\levelnfcn23\\leveljc0\\leveljcn0\\levelfollow0\\levelstartat1\\levelspace0\\levelindent0{{\\leveltext\\'01\\u{}?;}}{{\\levelnumbers;}}\\fi-360\\li{indent}\\lin{indent}}}",
            ch as u32 as i16
        )
    }
}

/// The control words of tab stop `t`: alignment, leader, then `\txN` (`\tbN` for a bar tab).
fn tab_words(t: &wordcraft_doc::props::TabStop) -> String {
    use wordcraft_doc::props::{TabAlign, TabLeader};
    let at = (t.pos.clamp(-1584.0, 1584.0) * 20.0).round() as i32;
    let align = match t.align {
        TabAlign::Right => "\\tqr",
        TabAlign::Center => "\\tqc",
        TabAlign::Decimal => "\\tqdec",
        TabAlign::Bar => return format!("\\tb{at}"),
        // A clearing stop is left out: the paragraph's own stops are all it has.
        TabAlign::Clear => return String::new(),
        TabAlign::Left => "",
    };
    let leader = match t.leader {
        TabLeader::None => "",
        TabLeader::Dot => "\\tldot",
        TabLeader::MiddleDot => "\\tlmdot",
        TabLeader::Hyphen => "\\tlhyph",
        TabLeader::Underscore => "\\tlul",
    };
    format!("{align}{leader}\\tx{at}")
}

/// The character formatting RTF writes for a paragraph mark `m` (its font by name, monospaced
/// or not).
fn mark_fmt(m: &wordcraft_doc::props::CharProps) -> Fmt {
    Fmt {
        bold: m.bold == Some(true),
        italic: m.italic == Some(true),
        underline: m.underline.is_some_and(|u| u != wordcraft_doc::props::Underline::None),
        strike: m.strike == Some(true),
        sup: m.vert_align == Some(wordcraft_doc::props::VertAlign::Superscript),
        sub: m.vert_align == Some(wordcraft_doc::props::VertAlign::Subscript),
        color: match m.color {
            Some(wordcraft_doc::props::TextColor::Rgb(c)) => Some(c),
            _ => None,
        },
        size: m.size,
        font: m.font.as_deref().map(Arc::from),
        ..Fmt::default()
    }
}

/// RTF text of a document.
pub fn export(doc: &Document) -> String {
    // A drop cap is written as Word stores it: its letter in a paragraph of its own.
    let split = super::frames::split_drop_caps(doc);
    let doc = split.as_ref().unwrap_or(doc);
    let (marked, marks) = match mark_doc(doc) {
        Some((d, m)) => (Some(d), m),
        None => (None, Vec::new()),
    };
    let doc = marked.as_ref().unwrap_or(doc);
    // The document's own styles are numbered after the fixed entries.
    // Then the table styles its tables use.
    let tables = used_table_styles(doc);
    let own: HashMap<String, u32> = doc
        .styles
        .styles
        .iter()
        .filter(|s| model::is_custom_style(doc, &s.id) || model::is_custom_char_style(doc, &s.id))
        .chain(tables.iter().filter_map(|id| doc.styles.get(id)))
        .take(4096)
        .zip(FIRST_OWN_STYLE..)
        .map(|(s, n)| (s.id.clone(), n))
        .collect();
    let before = set_own_styles(own);
    let ranged_before = set_ranged(HashSet::new());
    // An empty paragraph's mark formatting sets its height.
    let opts = model::FlowOpts { marks: true, outline_apart: true, auto_space: true, ..model::FlowOpts::tracked() };
    let flow = model::from_doc_with(doc, opts);
    let base_font = doc.styles.default_chr.font.clone().unwrap_or_else(|| wordcraft_doc::styles::BODY_FONT.to_string());
    let mut w = Writer {
        marks,
        t: Tables { fonts: vec![base_font], colors: Vec::new() },
        lists: Vec::new(),
        authors: None,
        base: Fmt::default(),
        sections: Vec::new(),
        sect_i: 0,
    };
    let mut body = String::new();
    // Every section starts with its own `\sectd`; `\sect` breaks only where there are several.
    // With endnotes at each section's end, a section that doesn't suppress them includes them
    // (`\endnhere`, written for `no_endnote == Some(false)`).
    let sect_end = doc.settings.endnote_pr.pos == Some(wordcraft_doc::section::NotePos::SectEnd);
    let secs: Vec<wordcraft_doc::SectionProps> = doc
        .sections()
        .into_iter()
        .map(|(_, s)| {
            let mut s = s.clone();
            if sect_end && s.no_endnote != Some(true) {
                s.no_endnote = Some(false);
            }
            // The same for the formatting before a tracked change (`{\*\oldsprops …}`).
            if sect_end
                && let Some(c) = s.fmt_change.as_deref_mut()
                && c.old.no_endnote != Some(true)
            {
                c.old.no_endnote = Some(false);
            }
            s
        })
        .collect();
    if let Some(first) = secs.first() {
        body.push_str(&sect_words(first, first.gutter, &mut w.t));
        for (set, prefix) in [(&first.headers, "header"), (&first.footers, "footer")] {
            for (id, suffix) in [(set.default, ""), (set.first, "f"), (set.even, "l")] {
                if let Some(part) = id.and_then(|id| doc.parts.get(&id)) {
                    body.push_str(&format!("{{\\{prefix}{suffix} "));
                    let blocks = model::part_flow(doc, &part.blocks, opts);
                    set_ranged(model::ranged_anchors(&blocks));
                    w.blocks(&blocks, 0, "\\par", &mut body, 1);
                    body.push('}');
                }
            }
        }
    }
    if secs.len() > 1 {
        w.sections = secs;
    }
    set_ranged(model::ranged_anchors(&flow.blocks));
    w.blocks(&flow.blocks, 0, "\\par", &mut body, 0);
    let own_entries = own_style_entries(doc, &mut w.t);
    set_own_styles(before);
    set_ranged(ranged_before);
    export_with(&flow.meta, doc, &w, &own_entries, &body)
}

/// `{\*\userprops …}` (RTF 1.9.1 "User Properties") for the custom properties RTF has a type
/// for, in document order; empty when there are none.
fn user_props(props: &[wordcraft_doc::CustomProp]) -> String {
    let mut out = String::new();
    for p in props {
        let (ptype, value) = match p.kind.as_str() {
            "lpwstr" => (30, p.value.as_str()),
            "i4" => (3, p.value.as_str()),
            "r8" => (5, p.value.as_str()),
            "bool" => (11, if p.value.eq_ignore_ascii_case("true") || p.value == "1" { "1" } else { "0" }),
            "filetime" => (64, p.value.as_str()),
            // `raw` vectors and blobs, and kinds RTF has no type for.
            _ => continue,
        };
        out.push_str(&format!("{{\\propname {}}}\\proptype{ptype}{{\\staticval {}}}", esc(&p.name), esc(value)));
        if let Some(link) = &p.link_target {
            out.push_str(&format!("{{\\linkval {}}}", esc(link)));
        }
    }
    if out.is_empty() { out } else { format!("{{\\*\\userprops {out}}}\n") }
}

/// The RTF file: its header (`own_styles`: the document's own stylesheet entries) and `body`.
fn export_with(m: &Meta, doc: &Document, w: &Writer, own_styles: &str, body: &str) -> String {
    let (sec, settings, core) = (&doc.last_section, &doc.settings, &doc.core);
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0\\deflang1033\\uc1\n{\\fonttbl");
    for (i, f) in w.t.fonts.iter().enumerate() {
        let fam = if model::is_mono(f) { "\\fmodern" } else { "\\fswiss" };
        out.push_str(&format!("{{\\f{i}{fam}\\fcharset0 {};}}", esc_font(f)));
    }
    out.push_str("}\n{\\colortbl;");
    for c in &w.t.colors {
        out.push_str(&format!("\\red{}\\green{}\\blue{};", c.0, c.1, c.2));
    }
    out.push_str("}\n{\\*\\generator WordCraft;}\n{\\stylesheet{\\s0\\snext0 Normal;}");
    for n in 1..=6u32 {
        let sz = HEAD_SIZES.get(n as usize - 1).copied().unwrap_or(12.0);
        out.push_str(&format!("{{\\s{n}\\sbasedon0\\snext0\\outlinelevel{}\\keepn\\b\\fs{} heading {n};}}", n - 1, (sz * 2.0) as i32));
    }
    out.push_str("{\\s7\\sbasedon0\\snext0\\fs56 Title;}{\\s8\\sbasedon0\\snext0\\li720\\ri720\\i Quote;}{\\s9\\sbasedon0\\snext9\\sa0\\fs20 Code;}");
    out.push_str(own_styles);
    out.push_str("}\n");
    if let Some(xml) = doc.styles.latent_styles.as_deref() {
        out.push_str(&super::latent_styles::group(xml));
    }
    if !w.lists.is_empty() {
        out.push_str("{\\*\\listtable");
        for (i, kinds) in w.lists.iter().enumerate() {
            let id = i + 1;
            out.push_str(&format!("\n{{\\list\\listtemplateid{id}\\listhybrid"));
            for (l, k) in kinds.iter().enumerate() {
                out.push_str(&level_def(*k, l));
            }
            out.push_str(&format!("{{\\listname ;}}\\listid{id}}}"));
        }
        out.push_str("}\n{\\*\\listoverridetable");
        for i in 1..=w.lists.len() {
            out.push_str(&format!("{{\\listoverride\\listid{i}\\listoverridecount0\\ls{i}}}"));
        }
        out.push_str("}\n");
    }
    if let Some(authors) = &w.authors {
        out.push_str("{\\*\\revtbl {Unknown;}");
        for a in authors {
            out.push_str(&format!("{{{};}}", esc(&a.replace(';', ","))));
        }
        out.push_str("}\n");
    }
    out.push_str("{\\info");
    for (k, v) in [("title", &m.title), ("author", &m.author), ("subject", &m.subject), ("keywords", &m.keywords), ("doccomm", &m.description)] {
        if !v.is_empty() {
            out.push_str(&format!("{{\\{k} {}}}", esc(v)));
        }
    }
    for (k, v) in [("operator", &core.last_modified_by), ("category", &core.category)] {
        if !v.is_empty() {
            out.push_str(&format!("{{\\{k} {}}}", esc(v)));
        }
    }
    out.push_str(&app_kept_info(doc));
    if core.revision > 0 {
        out.push_str(&format!("{{\\version{}}}", core.revision));
    }
    for (k, v) in [("creatim", &m.created), ("revtim", &core.modified), ("printim", &core.last_printed)] {
        if let Some(d) = rtf_date(v) {
            out.push_str(&format!("{{\\{k}{d}}}"));
        }
    }
    out.push_str("}\n");
    out.push_str(&user_props(&doc.custom_props));
    let tw = |v: f32| (v * 20.0).round() as i64;
    let landscape = if sec.landscape { "\\landscape" } else { "" };
    let notes = doc_note_words(doc);
    let opts = doc_opts_words(settings) + &doc_setting_words(settings);
    // The gutter and its side, from the first section (sections that differ say `\guttersxn`).
    let mut gutter = String::new();
    if let Some((_, first)) = doc.sections().first() {
        let g = if first.gutter.is_finite() { ((first.gutter * 20.0).round() as i64).max(0) } else { 0 };
        if g != 0 {
            gutter.push_str(&format!("\\gutter{g}"));
        }
        if first.rtl_gutter {
            gutter.push_str("\\rtlgutter");
        }
    }
    out.push_str(&format!(
        "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}{gutter}{landscape}{notes}{opts}\\viewkind4\n",
        tw(sec.page_w),
        tw(sec.page_h),
        tw(sec.margin_left),
        tw(sec.margin_right),
        tw(sec.margin_top),
        tw(sec.margin_bottom)
    ));
    out.push_str(body);
    out.push_str("}\n");
    out
}

/// A picture's `\picwgoal`/`\pichgoal` and, when it is cropped, its `\piccrop*` words: the goal is
/// the uncropped size and each crop the twips cut off that side (only the non-zero ones). A crop
/// leaving under 1 % of an axis, or an uncropped goal over 31,680 twips, writes no crop.
fn pict_goal(img: &model::Img) -> String {
    let plain = format!("\\picwgoal{}\\pichgoal{}", (img.w * 20.0).round() as i64, (img.h * 20.0).round() as i64);
    let [l, t, r, b] = img.crop.map(|c| if c.is_finite() && c > 0.0 { c } else { 0.0 });
    if l + t + r + b <= 0.0 || l + r >= 0.99 || t + b >= 0.99 {
        return plain;
    }
    let gw = (img.w * 20.0 / (1.0 - l - r)).round();
    let gh = (img.h * 20.0 / (1.0 - t - b)).round();
    if !(gw.is_finite() && gh.is_finite()) || gw > 31_680.0 || gh > 31_680.0 {
        return plain;
    }
    let mut s = format!("\\picwgoal{}\\pichgoal{}", gw as i64, gh as i64);
    for (word, frac, goal) in [("l", l, gw), ("t", t, gh), ("r", r, gw), ("b", b, gh)] {
        let n = (frac * goal).round() as i64;
        if n != 0 {
            s.push_str(&format!("\\piccrop{word}{n}"));
        }
    }
    s
}

/// Manager, Company and Hyperlink base from the kept extended properties (`wordcraft:app.kept`,
/// `name\tvalue` lines, as the DOCX reader stores `docProps/app.xml`) as `{\manager …}`,
/// `{\company …}` and `{\hlinkbase …}` (RTF 1.9.1, information group). Remove personal
/// information leaves out Manager and Company, as the DOCX writer does.
fn app_kept_info(doc: &Document) -> String {
    let mut out = String::new();
    let Some(kept) = doc.passthrough.get(wordcraft_doc::APP_KEPT) else { return out };
    for line in String::from_utf8_lossy(kept).lines().take(64) {
        let Some((name, value)) = line.split_once('\t') else { continue };
        let word = match name {
            "Manager" if !doc.settings.remove_personal_info => "manager",
            "Company" if !doc.settings.remove_personal_info => "company",
            "HyperlinkBase" => "hlinkbase",
            _ => continue,
        };
        if !value.is_empty() {
            out.push_str(&format!("{{\\{word} {}}}", esc(value)));
        }
    }
    out
}
