use super::date::{dttm_to_iso, iso_date};
use super::parse::column_grid;
use super::state::{
    CellDef, CommentData, Dest, DocOpts, HeaderFooter, MARK_CLOSE, MARK_OPEN, MAX_GROUPS, MAX_NOTE_BLOCKS, MAX_NOTES, MAX_REV_AUTHORS, MAX_SECTIONS,
    NoteData, Output, PageGeom, Pn, SecFmt, St, Sub, TableLevel, apply_note_word,
};
use super::styles::{StyleDef, drop_style_formatting, style_fmt};
use super::{Arc, Cell, FBlock, FTable, Fmt, HashMap, HashSet, Inline, Kind, Meta, Para, Rgb, model};

#[derive(Default)]
pub(super) struct Reader {
    pub(super) st: St,
    pub(super) shapes: Vec<super::shapes::Shape>,
    stack: Vec<St>,
    overflow: usize,
    pub(super) fonts: HashMap<i32, Arc<str>>,
    pub(super) font_cur: i32,
    pub(super) font_name: String,
    /// Code pages of the fonts whose charset names one (`\fcharset204` → 1251).
    pub(super) font_cps: HashMap<i32, u16>,
    pub(super) font_cp_cur: Option<u16>,
    /// The document's code page (`\ansicpg`); 0 until it names one (Windows-1252 then).
    pub(super) ansicpg: u16,
    /// The default font (`\deff`), whose charset applies before any `\f`.
    pub(super) deff: i32,
    /// The document's page (`\paperw`, `\margl`…) and the current section's formatting.
    pub(super) page: PageGeom,
    /// The document formatting words that go into the document's settings (`\deftab`, `\facingp`…).
    pub(super) doc_opts: DocOpts,
    pub(super) sec: SecFmt,
    /// The document's footnote and endnote options (`\ftnbj`, `\aftnstart`…); their `num_fmt`
    /// is the document's note number format.
    pub(super) doc_notes: (wordcraft_doc::section::NoteProps, wordcraft_doc::section::NoteProps),
    /// The document's `\fetN` (0 footnotes only, 1 endnotes only, 2 both), when it names one.
    pub(super) fet: Option<i32>,
    /// The document settings the header names (`\viewscale`, `\dghspace`, `\protlevel`…).
    pub(super) doc_set: DocSettingWords,
    /// The current section's formatting before a tracked change (`\oldsprops`), and the one
    /// an `\oldsprops` group being read builds.
    pub(super) sec_old: Option<SecFmt>,
    pub(super) old_sec: SecFmt,
    /// The headers and footers of each section ended so far (`\sect`); `hf_parts` holds the
    /// current section's.
    pub(super) section_parts: Vec<Vec<(HeaderFooter, Vec<FBlock>)>>,
    /// The header or footer being read: the group depth it started at, which one, and the
    /// body's output, put back when the group closes.
    pub(super) hf: Option<(usize, HeaderFooter, Box<Output>)>,
    /// The current section's headers and footers, in order.
    pub(super) hf_parts: Vec<(HeaderFooter, Vec<FBlock>)>,
    /// The cell border side being described (`\clbrdrt`, `l`, `b`, `r`): its line style, width
    /// and colour follow.
    pub(super) cell_side: Option<char>,
    /// The page border side being described (`\pgbrdrt`, `l`, `b`, `r`), a section property:
    /// the border words that follow describe it until a paragraph or cell border side,
    /// `\pard`, `\sectd` or `\sect`.
    pub(super) page_side: Option<char>,
    /// Each table row's cell right edges (`\cellx`, twips), for the table's column grid.
    table_rights: Vec<Vec<i32>>,
    /// Each table row's left edge (`\trleft`, twips), which its `\cellx` edges are measured with.
    table_lefts: Vec<i32>,
    /// Each table row's height (points, exact) and whether it keeps from breaking across pages.
    table_heights: Vec<Option<(f32, bool)>>,
    table_keeps: Vec<bool>,
    pub(super) colors: Vec<Option<Rgb>>,
    pub(super) color_cur: (u8, u8, u8, bool),
    pub(super) styles: HashMap<i32, String>,
    /// Each style's own character formatting, as its `\stylesheet` entry writes it.
    pub(super) style_defs: HashMap<i32, StyleDef>,
    pub(super) style_cur: i32,
    pub(super) style_name: String,
    pub(super) style_def_cur: StyleDef,
    /// The stylesheet's entries in the order the file gives them.
    pub(super) style_order: Vec<i32>,
    /// The document style id each paragraph and character entry stands for, made when first
    /// needed (a new entry clears it).
    pub(super) style_ids: std::cell::OnceCell<super::styles::StyleIds>,
    /// The text of `{\*\generator …}`, when the file names the program that wrote it.
    pub(super) generator: Option<String>,
    pub(super) meta: Meta,
    pub(super) info_text: String,
    // Lists: list id → ordered per level; override ls → list id.
    pub(super) lists: HashMap<i32, Vec<bool>>,
    pub(super) list_cur: Vec<bool>,
    pub(super) list_cur_id: i32,
    pub(super) overrides: HashMap<i32, i32>,
    pub(super) ov_list: i32,
    pub(super) ov_ls: i32,
    pub(super) fields: Vec<String>,
    /// The legacy form field definitions read, by index in `fields`.
    pub(super) forms: HashMap<usize, super::form_field::FormField>,
    /// Where each field's result started in the paragraph being read, by index in `fields`.
    pub(super) field_at: HashMap<usize, usize>,
    /// Fields whose result runs over a paragraph mark (a table of contents, an index): their
    /// result is read as plain text, so one field never turns into one field per paragraph.
    pub(super) plain_fields: HashSet<usize>,
    /// Locked fields (`\fldlock`), by index in `fields`: their stored result is kept.
    pub(super) locked_fields: HashSet<usize>,
    // Picture being read.
    pub(super) pict_depth: Option<usize>,
    pub(super) pict_hex: Vec<u8>,
    pub(super) pict_half: Option<u8>,
    pub(super) pict_kind: u8,
    pub(super) pict_w: Option<f32>,
    pub(super) pict_h: Option<f32>,
    pub(super) pict_sx: f32,
    pub(super) pict_sy: f32,
    /// `\piccropl`/`t`/`r`/`b` as written: twips cut off the left, top, right and bottom,
    /// measured against the goal size (0 when absent).
    pub(super) pict_crop: [i32; 4],
    /// `\picw`/`\pich` as written, read for metafiles (0 when absent).
    pub(super) pict_nat: (i32, i32),
    /// The mapping mode `N` of `\wmetafileN`.
    pub(super) pict_mm: i32,
    /// The bookmark name being read and the depth its group started at.
    pub(super) bkmk: Option<(usize, String)>,
    /// Whether that group is a bookmark's end (`\bkmkend`) rather than its start.
    pub(super) bkmk_end: bool,
    /// Word 6.0/95 paragraph numbering (`{\*\pn …}`) being read, and the depth its group
    /// started at; the group's end keeps it as the paragraph's.
    pub(super) pn_cur: Pn,
    pub(super) pn_depth: Option<usize>,
    /// The text of the `{\pntxtb …}` or `{\pntxta …}` group being read.
    pub(super) pn_txt: String,
    /// Authors of tracked changes (`\revtbl`), and the entry being read.
    rev_authors: Vec<String>,
    pub(super) rev_name: String,
    pub(super) skip_chars: usize,
    /// A high surrogate from `\uN` waiting for its low half, with the formatting it was read in.
    pub(super) pending_high: Option<(u16, Fmt)>,
    /// A streaming decoder for the current multibyte code page, and the code page it was made for.
    /// A lead byte and its trail byte decode together even when a text word, a group boundary or the
    /// end of a chunk falls between them, so a run of any length never splits a character.
    pub(super) mb_decoder: Option<encoding_rs::Decoder>,
    pub(super) mb_cp: u16,
    // Output.
    pub(super) body: Vec<FBlock>,
    pub(super) para: Para,
    pub(super) para_has: bool,
    table: Vec<Vec<Cell>>,
    row: Vec<Cell>,
    cell: Vec<FBlock>,
    pub(super) defs: Vec<CellDef>,
    pub(super) def_cur: CellDef,
    pub(super) row_header: bool,
    /// The row's height (`\trrh`: points, exact) and `\trkeep`, reset by `\trowd`.
    pub(super) row_height: Option<(f32, bool)>,
    pub(super) row_keep: bool,
    pub(super) row_style: super::state::RowStyle,
    /// The table's style and look: its first row's.
    table_style: super::state::RowStyle,
    /// The tables enclosing the one being read, outermost first (nested tables: the current
    /// table's fields above are the innermost open table's).
    nest: Vec<TableLevel>,
    /// The information group's dates as read (`\yr \mo \dy \hr \min`), 0 `\creatim`, 1 `\revtim`, 2 `\printim`,
    /// and the one whose group is open with the depth that group started at.
    pub(super) dates: [Option<(u32, u32, u32, u32, u32)>; 3],
    pub(super) in_date: Option<(usize, usize)>,
    /// The information group's fields the flow model has no place for, and `{\*\userprops …}`.
    pub(super) info_more: super::state::InfoMore,
    /// The latent-style table (`{\*\latentstyles …}`).
    pub(super) latent: super::latent_styles::Latent,
    pub(super) rule_pending: bool,
    pub(super) info_depth: Option<usize>,
    // Notes and comments (only when the caller wants them).
    pub(super) marks_on: bool,
    pub(super) sub: Option<Sub>,
    pub(super) notes: Vec<NoteData>,
    pub(super) comments: Vec<CommentData>,
    pub(super) atn_text: String,
    pub(super) atn_initials: String,
    pub(super) atn_author: String,
    pub(super) atn_date: String,
    pub(super) atn_parts: Option<(u32, u32, u32, u32, u32)>,
    pub(super) atn_ref: Option<i32>,
    pub(super) last_end: Option<i32>,
    pub(super) range_starts: HashSet<i32>,
    pub(super) range_ends: HashSet<i32>,
    pub(super) claimed: HashSet<i32>,
    pub(super) synth: i32,
    /// Custom note marks: the groups read in the body.
    pub(super) note_mark: super::note_marks::MarkTrack,
}

impl Reader {
    pub(super) fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Set the output aside (a header or footer starts) or put it back (it ended), returning the
    /// other one.
    pub(super) fn swap_output(&mut self, mut o: Output) -> Output {
        self.note_mark.serial = self.note_mark.serial.wrapping_add(1);
        std::mem::swap(&mut o.body, &mut self.body);
        std::mem::swap(&mut o.para, &mut self.para);
        std::mem::swap(&mut o.para_has, &mut self.para_has);
        std::mem::swap(&mut o.table, &mut self.table);
        std::mem::swap(&mut o.row, &mut self.row);
        std::mem::swap(&mut o.cell, &mut self.cell);
        std::mem::swap(&mut o.defs, &mut self.defs);
        std::mem::swap(&mut o.def_cur, &mut self.def_cur);
        std::mem::swap(&mut o.row_header, &mut self.row_header);
        std::mem::swap(&mut o.rule_pending, &mut self.rule_pending);
        std::mem::swap(&mut o.table_rights, &mut self.table_rights);
        std::mem::swap(&mut o.table_lefts, &mut self.table_lefts);
        std::mem::swap(&mut o.row_height, &mut self.row_height);
        std::mem::swap(&mut o.row_keep, &mut self.row_keep);
        std::mem::swap(&mut o.table_heights, &mut self.table_heights);
        std::mem::swap(&mut o.table_keeps, &mut self.table_keeps);
        std::mem::swap(&mut o.row_style, &mut self.row_style);
        std::mem::swap(&mut o.table_style, &mut self.table_style);
        std::mem::swap(&mut o.nest, &mut self.nest);
        o
    }

    /// Swaps the current table's state with `l`'s.
    fn swap_level(&mut self, l: &mut TableLevel) {
        std::mem::swap(&mut l.table, &mut self.table);
        std::mem::swap(&mut l.row, &mut self.row);
        std::mem::swap(&mut l.cell, &mut self.cell);
        std::mem::swap(&mut l.defs, &mut self.defs);
        std::mem::swap(&mut l.def_cur, &mut self.def_cur);
        std::mem::swap(&mut l.row_header, &mut self.row_header);
        std::mem::swap(&mut l.row_height, &mut self.row_height);
        std::mem::swap(&mut l.row_keep, &mut self.row_keep);
        std::mem::swap(&mut l.table_rights, &mut self.table_rights);
        std::mem::swap(&mut l.table_lefts, &mut self.table_lefts);
        std::mem::swap(&mut l.table_heights, &mut self.table_heights);
        std::mem::swap(&mut l.table_keeps, &mut self.table_keeps);
        std::mem::swap(&mut l.row_style, &mut self.row_style);
        std::mem::swap(&mut l.table_style, &mut self.table_style);
    }

    /// The table depth of the table being read: 1 the outermost, 2 a table in its cell…
    pub(super) fn table_level(&self) -> usize {
        1 + self.nest.len()
    }

    /// The current paragraph's table depth (`\itapN`, else 1 for `\intbl`), at most
    /// [`model::MAX_DEPTH`]: deeper paragraphs go into the deepest table kept.
    pub(super) fn para_depth(&self) -> usize {
        let d = match self.st.itap {
            Some(n) => usize::from(n),
            None => usize::from(self.st.intbl),
        };
        d.min(model::MAX_DEPTH)
    }

    /// Makes the table at depth `d` (>= 1) the current one: deeper tables end and go into the
    /// cell of the one enclosing them, after its paragraphs so far; missing levels open.
    fn enter_depth(&mut self, d: usize) {
        let d = d.clamp(1, model::MAX_DEPTH);
        while self.table_level() > d {
            let t = self.take_table();
            let Some(mut parent) = self.nest.pop() else { break };
            self.swap_level(&mut parent);
            if let Some(t) = t {
                self.cell.push(FBlock::Table(t));
            }
        }
        while self.table_level() < d {
            let mut l = TableLevel::default();
            self.swap_level(&mut l);
            self.nest.push(l);
        }
    }

    /// Whether a page, cell or paragraph border side is being described.
    pub(super) fn border_open(&self) -> bool {
        self.page_side.is_some() || self.cell_side.is_some() || self.st.border_side.is_some()
    }

    /// A page border word (section formatting): `\pgbrdrt` … `\pgbrdrr` starts a side, `\brspN`
    /// gives its space. `\pgbrdropt`, `\pgbrdrhead`, `\pgbrdrfoot`, `\pgbrdrsnap` and `\brdrart`
    /// have no model value and are skipped. False when `w` isn't one.
    pub(super) fn page_border_word(&mut self, w: &str, pv: i32) -> bool {
        match w {
            "pgbrdrt" | "pgbrdrl" | "pgbrdrb" | "pgbrdrr" => {
                self.st.border_side = None;
                self.page_side = w.chars().last();
            }
            // A page side's, else a paragraph side's; nothing while a cell side is described.
            "brsp" if self.page_side.is_some() || (self.cell_side.is_none() && self.st.border_side.is_some()) => {
                let space = (pv as f32 / 20.0).clamp(0.0, 31.0);
                self.each_border_slot(|slot| {
                    if let Some(b) = slot {
                        b.space = space;
                    }
                });
            }
            _ => return false,
        }
        true
    }

    /// Applies `f` to the border of the page side being described, else of the cell side, else
    /// of the paragraph side: `h` is the border between paragraphs (`\brdrbtw`), `x` all four
    /// sides (`\box`), `c` the run's character border (`\chbrdr`); any other side (the bar
    /// border's `v`) has no slot.
    pub(super) fn each_border_slot(&mut self, mut f: impl FnMut(&mut Option<wordcraft_doc::props::Border>)) {
        // The character border (`\chbrdr`) is the current run's.
        if (self.page_side, self.cell_side, self.st.border_side) == (None, None, Some('c')) {
            let mut b = self.st.fmt.extra.as_ref().and_then(|x| x.border);
            f(&mut b);
            self.set_extra(|x| x.border = b);
            return;
        }
        let (b, side) = match (self.page_side, self.cell_side) {
            // A header or footer doesn't set the body's section.
            (Some(_), _) if self.hf.is_some() || self.sub.is_some() => return,
            (Some(c), _) => (&mut self.sec.page_borders, c),
            (None, Some(c)) => (&mut self.def_cur.borders, c),
            (None, None) => match self.st.border_side {
                Some(s) => (&mut self.st.borders, s),
                None => return,
            },
        };
        match side {
            't' => f(&mut b.top),
            'l' => f(&mut b.left),
            'b' => f(&mut b.bottom),
            'r' => f(&mut b.right),
            'h' => f(&mut b.between),
            'x' => {
                for slot in [&mut b.top, &mut b.left, &mut b.bottom, &mut b.right] {
                    f(slot);
                }
            }
            _ => {}
        }
    }

    /// A header or footer group starts: read it into an output of its own.
    pub(super) fn start_header_footer(&mut self, which: HeaderFooter) {
        let body = self.swap_output(Output::default());
        self.hf = Some((self.depth(), which, Box::new(body)));
        self.st.intbl = false;
    }

    /// The header or footer group closed: keep what it held and go back to the body.
    pub(super) fn end_header_footer(&mut self) {
        if self.hf.is_none() {
            return;
        }
        // A high surrogate left at the end stays in the header or footer: it never pairs across.
        self.flush_high_surrogate();
        let Some((_, which, body)) = self.hf.take() else { return };
        if self.para_has || !self.para.inlines.is_empty() {
            self.end_para(false);
        }
        self.close_table();
        let part = self.swap_output(*body);
        // A section names each one once; a repeat replaces it.
        self.hf_parts.retain(|(w, _)| *w != which);
        if self.hf_parts.len() < 16 {
            self.hf_parts.push((which, part.body));
        }
    }

    /// A footnote, endnote or comment group starts: read it into an output of its own.
    pub(super) fn start_sub(&mut self, comment: bool) {
        if self.overflow > 0 || self.notes.len() >= MAX_NOTES || self.comments.len() >= MAX_NOTES {
            self.st.dest = Dest::Skip;
            return;
        }
        let custom = if comment { String::new() } else { self.take_custom_mark() };
        let saved = self.swap_output(Output::default());
        self.sub = Some(Sub { depth: self.depth(), comment, endnote: false, custom, own_mark_done: false, saved: Box::new(saved) });
        self.st.intbl = false;
    }

    /// A place marker as text in the current paragraph.
    pub(super) fn push_mark(&mut self, tag: char, n: i64, fmt: Fmt) {
        self.para.push_text(&format!("{MARK_OPEN}{tag}{n}{MARK_CLOSE}"), &fmt);
        self.para_has = true;
    }

    /// The footnote or comment group closed: keep its text and leave a marker in the body.
    pub(super) fn end_sub(&mut self) {
        if self.sub.is_none() {
            return;
        }
        // A high surrogate left at the end stays in the note or comment: it never pairs across.
        self.flush_high_surrogate();
        let Some(sub) = self.sub.take() else { return };
        if self.para_has || !self.para.inlines.is_empty() {
            self.end_para(false);
        }
        self.close_table();
        let fmt = self.stack.last().map(|s| s.fmt.clone()).unwrap_or_default();
        let mut inner = self.swap_output(*sub.saved);
        inner.body.truncate(MAX_NOTE_BLOCKS);
        if !sub.comment {
            self.push_mark('N', self.notes.len() as i64, fmt);
            self.notes.push(NoteData { endnote: sub.endnote, custom: sub.custom, blocks: inner.body });
            return;
        }
        let want = self.atn_ref.take().or(self.last_end.take());
        let id = match want {
            Some(i) if self.range_starts.contains(&i) && self.range_ends.contains(&i) && self.claimed.insert(i) => i,
            _ => {
                // No range of its own: the comment sits at this point.
                self.synth = self.synth.saturating_sub(1);
                let id = self.synth;
                self.push_mark('B', i64::from(id), fmt.clone());
                self.push_mark('E', i64::from(id), fmt);
                id
            }
        };
        let date =
            if !self.atn_date.is_empty() { std::mem::take(&mut self.atn_date) } else { self.atn_parts.take().map(iso_date).unwrap_or_default() };
        self.atn_parts = None;
        self.comments.push(CommentData {
            rtf_id: id,
            author: std::mem::take(&mut self.atn_author),
            initials: std::mem::take(&mut self.atn_initials),
            date,
            blocks: inner.body,
        });
    }

    /// A comment word's text group closed (`\atrfstart 3`, `\atnauthor Ann`…).
    pub(super) fn commit_atn(&mut self, kind: u8) {
        let text = std::mem::take(&mut self.atn_text);
        let text = text.trim();
        let id = text.parse::<i32>().ok();
        let fmt = self.stack.last().map(|s| s.fmt.clone()).unwrap_or_default();
        match kind {
            1 => {
                if let Some(id) = id {
                    self.range_starts.insert(id);
                    self.push_mark('B', i64::from(id), fmt);
                }
            }
            2 => {
                if let Some(id) = id {
                    self.range_ends.insert(id);
                    self.last_end = Some(id);
                    self.push_mark('E', i64::from(id), fmt);
                }
            }
            3 => self.atn_initials = text.to_string(),
            4 => self.atn_author = text.to_string(),
            5 => {
                // Either `\yr\mo\dy\hr\min` words (read as they came) or a packed date-time number.
                if let Some(parts) = self.atn_parts.take() {
                    self.atn_date = iso_date(parts);
                } else if let Some(n) = text.parse::<u32>().ok().filter(|n| *n != 0) {
                    self.atn_date = iso_date((1900 + ((n >> 20) & 0x1FF), (n >> 16) & 0xF, (n >> 11) & 0x1F, (n >> 6) & 0x1F, n & 0x3F));
                }
            }
            6 => self.atn_ref = id,
            _ => {}
        }
    }

    /// `\sect`: end the paragraph, and with it the section, which takes its formatting and the
    /// headers and footers read since the last break. Inside a header, footer or table, and past
    /// [`MAX_SECTIONS`], it only ends the paragraph.
    pub(super) fn end_section(&mut self) {
        self.page_side = None;
        self.end_para_mark();
        if self.hf.is_some() || self.sub.is_some() || self.st.intbl || self.section_parts.len() + 1 >= MAX_SECTIONS {
            return;
        }
        let mut props = self.sec.props(&self.page);
        props.no_endnote = self.no_endnote();
        props.fmt_change = self.section_change();
        if let Some(FBlock::Para(p)) = self.body.last_mut() {
            p.section = Some(Box::new(props));
            self.section_parts.push(std::mem::take(&mut self.hf_parts));
        }
    }

    /// The current section's Suppress endnotes: only with endnotes at each section's end
    /// (`\aendnotes`), where a section without `\endnhere` suppresses them.
    pub(super) fn no_endnote(&self) -> Option<bool> {
        (self.doc_notes.1.pos == Some(wordcraft_doc::section::NotePos::SectEnd)).then_some(!self.sec.endnhere)
    }

    /// The current section's tracked formatting change (`\oldsprops`), taken: its revision is
    /// set when the document is built ([`super::parse`]), as RTF names no author or date for it.
    pub(super) fn section_change(&mut self) -> Option<Box<wordcraft_doc::props::PropChange<wordcraft_doc::SectionProps>>> {
        let old = self.sec_old.take()?;
        let mut props = old.props(&self.page);
        // Suppress endnotes before the change: the same rule as the current section's.
        props.no_endnote = (self.doc_notes.1.pos == Some(wordcraft_doc::section::NotePos::SectEnd)).then_some(!old.endnhere);
        wordcraft_doc::props::PropChange::boxed(0, props)
    }

    /// A section formatting word: applied to the current section, unless it's inside a header
    /// or footer (whose `\sectd` isn't the body's). False when `w` isn't one.
    pub(super) fn section_word(&mut self, w: &str, p: Option<i32>) -> bool {
        // Inside a header or footer the word is read into a scratch copy and dropped.
        if self.hf.is_some() || self.sub.is_some() {
            return apply_section_word(&mut self.sec.clone(), w, p);
        }
        if w == "sectd" {
            self.sec_old = None;
            self.page_side = None;
        }
        apply_section_word(&mut self.sec, w, p)
    }

    /// Applies paragraph word `w` when it names a border side (`\brdrt`, `\brdrl`, `\brdrb`,
    /// `\brdrr`, the between border `\brdrbtw`, all four sides `\box`) or the shading (`\cbpat`,
    /// `\cfpat`, `\shading`). False when it isn't one.
    pub(super) fn para_border_word(&mut self, w: &str, pv: i32) -> bool {
        match w {
            "brdrbtw" => {
                self.page_side = None;
                self.st.border_side = Some('h');
            }
            "box" => {
                self.page_side = None;
                self.st.border_side = Some('x');
            }
            // A bar border has no model value: its side is described and dropped, leaving the
            // other sides as they are.
            "brdrbar" => {
                self.page_side = None;
                self.st.border_side = Some('v');
            }
            "cbpat" => self.st.cbpat = Some(pv),
            "cfpat" => self.st.cfpat = Some(pv),
            "shading" => self.st.shading = pv,
            "brdrb" | "brdrt" | "brdrl" | "brdrr" => {
                self.page_side = None;
                if w == "brdrb" && self.st.dest == Dest::Body {
                    self.rule_pending = true;
                }
                self.st.border_side = w.chars().last();
            }
            _ => return false,
        }
        true
    }

    /// Applies document formatting word `w` when it is a footnote or endnote option. False when
    /// it isn't one.
    pub(super) fn doc_note_word(&mut self, w: &str, p: Option<i32>) -> bool {
        use wordcraft_doc::section::NotePos;
        let (foot, end) = &mut self.doc_notes;
        match w {
            "aendnotes" => end.pos = Some(NotePos::SectEnd),
            "aenddoc" => end.pos = Some(NotePos::DocEnd),
            "fet" => self.fet = Some(p.unwrap_or(0)),
            // Footnotes at the end of the section or document: no setting of their own; whether
            // footnotes become endnotes is decided from `\ftnbj`/`\ftntj` and `\fet` alone.
            "endnotes" | "enddoc" => {}
            _ => return apply_note_word(w, p, foot, end),
        }
        true
    }

    /// Records document setting word `w` (view zoom, drawing grid, Restrict Editing, privacy
    /// options, linked styles, Shift+Return expansion, HTML paragraph auto spacing, Track Changes on,
    /// print form data only). False when `w` isn't one.
    pub(super) fn doc_setting_word(&mut self, w: &str, p: Option<i32>) -> bool {
        use wordcraft_doc::ZoomFit;
        let on = p != Some(0);
        let pv = p.unwrap_or(0);
        let d = &mut self.doc_set;
        match w {
            "annotprot" => d.old_prot = on.then_some("comments"),
            "dghorigin" => d.origin_x = (pv >= 0).then(|| pv as f32 / 20.0).or(d.origin_x),
            "dghshow" => d.every_h = u8::try_from(pv).ok().filter(|&n| n <= 127).or(d.every_h),
            "dghspace" => d.grid_h = (pv >= 1).then(|| pv as f32 / 20.0).or(d.grid_h),
            "dgmargin" => d.margins = Some(on),
            "dgvorigin" => d.origin_y = (pv >= 0).then(|| pv as f32 / 20.0).or(d.origin_y),
            "dgvshow" => d.every_v = u8::try_from(pv).ok().filter(|&n| n <= 127).or(d.every_v),
            "dgvspace" => d.grid_v = (pv >= 1).then(|| pv as f32 / 20.0).or(d.grid_v),
            "enforceprot" => d.enforce = Some(on),
            "expshrtn" => d.expand_shift_return = Some(on),
            "formprot" => d.old_prot = on.then_some("forms"),
            "htmautsp" => d.html_auto_spacing = on,
            "linkstyles" => d.link_styles = Some(on),
            "printdata" => d.print_forms_data = Some(on),
            "protlevel" => d.level = u8::try_from(pv).ok().filter(|&n| n <= 3).or(d.level),
            "readprot" => d.old_prot = on.then_some("readOnly"),
            "remdttm" => d.remove_date_time = Some(on),
            "rempersonalinfo" => d.remove_personal_info = Some(on),
            "revisions" => d.track_changes = Some(on),
            "revprot" => d.old_prot = on.then_some("trackedChanges"),
            "viewscale" => d.percent = u32::try_from(pv).ok().filter(|n| (10..=500).contains(n)).or(d.percent),
            "viewzk" => match pv {
                0 => d.fit = Some(None),
                1 => d.fit = Some(Some(ZoomFit::OnePage)),
                2 => d.fit = Some(Some(ZoomFit::PageWidth)),
                _ => {}
            },
            _ => return false,
        }
        true
    }

    pub(super) fn open(&mut self) {
        self.skip_chars = 0;
        if self.stack.len() >= MAX_GROUPS {
            self.overflow += 1;
            return;
        }
        self.stack.push(self.st.clone());
        // A `{\*\docvar …}` group's first two nested groups are its name and value; any more are skipped.
        if self.st.dest == Dest::DocVar {
            self.st.dest = if self.info_more.var_parts.len() < 2 { Dest::Info(14) } else { Dest::Skip };
        }
        self.mark_group_open();
    }

    pub(super) fn close(&mut self) {
        // A group end decodes the multibyte bytes read in it, before the state is restored.
        self.finish_multibyte();
        self.skip_chars = 0;
        if self.overflow > 0 {
            self.overflow -= 1;
            return;
        }
        self.mark_group_close();
        self.close_shape_group();
        if self.sub.as_ref().is_some_and(|s| s.depth == self.depth()) {
            self.end_sub();
        }
        let closing = self.st.dest;
        // Destination-level commits.
        match closing {
            Dest::FontTbl if !self.font_name.is_empty() => self.commit_font(),
            Dest::StyleSheet if !self.style_name.is_empty() => self.commit_style(),
            Dest::RevTbl if !self.rev_name.trim().is_empty() => self.commit_author(),
            // A document variable's name or value: kept as written, not trimmed.
            Dest::Info(14) => {
                let t = std::mem::take(&mut self.info_text);
                if self.stack.last().is_some_and(|p| p.dest == Dest::DocVar) {
                    let part = Some(t).filter(|t| t.len() <= super::state::MAX_INFO_TEXT);
                    self.info_more.var_parts.push(part);
                } else {
                    // A nested group inside the name or value: its text belongs to that part.
                    self.info_text = t;
                }
            }
            Dest::Info(k) => {
                let mut t = std::mem::take(&mut self.info_text).trim().to_string();
                super::state::truncate_chars(&mut t, super::state::MAX_INFO_TEXT);
                match k {
                    1 => self.meta.title = t,
                    2 => self.meta.author = t,
                    3 => self.meta.subject = t,
                    4 => self.meta.keywords = t,
                    5 => self.meta.description = t,
                    6 => self.info_more.last_modified_by = t,
                    7 => self.info_more.category = t,
                    8..=10 => self.info_more.user_prop_text(k, t),
                    11 => self.info_more.manager = t,
                    12 => self.info_more.company = t,
                    13 => self.info_more.hyperlink_base = t,
                    _ => {}
                }
            }
            Dest::UserProps => self.info_more.commit_user_prop(),
            Dest::DocVar => self.info_more.commit_doc_var(),
            Dest::LatentStyles => self.latent.end(),
            Dest::LsdExcept => self.latent.end_exceptions(),
            Dest::Atn(k) => self.commit_atn(k),
            _ => {}
        }
        let Some(mut prev) = self.stack.pop() else { return };
        if self.in_date.is_some_and(|(_, d)| self.depth() < d) {
            self.in_date = None;
        }
        // The end of old formatting read in the body: the formatting it holds is the change's
        // old formatting (nested old-formatting groups only change their parent's copy).
        if closing == Dest::OldSect && prev.dest == Dest::Body && self.hf.is_none() {
            self.sec_old = Some(std::mem::take(&mut self.old_sec));
        }
        if let Dest::OldProps(para) = closing
            && prev.dest == Dest::Body
        {
            if para {
                prev.chg_p.old = Some(Box::new(self.para_props(&self.st)));
            } else {
                prev.chg_c.old = Some(Box::new(std::mem::take(&mut self.st.old_words)));
                if let Some(a) = self.st.old_rev.0 {
                    prev.chg_c.auth = a;
                }
                if let Some(d) = self.st.old_rev.1 {
                    prev.chg_c.date = d;
                }
            }
        }
        if self.pict_depth.is_some_and(|d| self.depth() < d) {
            self.pict_depth = None;
            self.finish_pict();
        }
        // The end of a `{\*\pn …}` group: the numbering it describes is the paragraph's, unless
        // the paragraph names a Word 97+ list (`\ls`), which wins.
        if self.pn_depth.is_some_and(|d| self.depth() < d) {
            if prev.dest == Dest::Body && self.st.ls.is_none() {
                prev.pn = Some(Box::new(std::mem::take(&mut self.pn_cur)));
            }
            self.pn_depth = None;
        }
        // The end of a `{\pntxtb …}` or `{\pntxta …}` group: its text is the label's bullet or
        // the text around the number.
        match closing {
            Dest::PnTxtB if prev.dest != closing => self.pn_cur.before = std::mem::take(&mut self.pn_txt),
            Dest::PnTxtA if prev.dest != closing => self.pn_cur.after = std::mem::take(&mut self.pn_txt),
            _ => {}
        }
        if let Some((d, _)) = &self.bkmk
            && self.depth() < *d
            && let Some((_, name)) = self.bkmk.take()
        {
            let name = name.trim().to_string();
            if !name.is_empty() {
                // RTF 1.9.1 "Bookmarks": the text between `\bkmkstart` and `\bkmkend` is the
                // bookmark's range; the model pairs them by name.
                self.para.inlines.push(if self.bkmk_end { Inline::AnchorEnd(name) } else { Inline::Anchor(name) });
                self.para_has = true;
            }
        }
        self.st = prev;
        if self.hf.as_ref().is_some_and(|(d, _, _)| self.depth() < *d) {
            self.end_header_footer();
        }
    }

    pub(super) fn commit_author(&mut self) {
        let name = std::mem::take(&mut self.rev_name).trim().to_string();
        if self.rev_authors.len() < MAX_REV_AUTHORS {
            self.rev_authors.push(name);
        }
    }

    /// The tracked-change mark for `\revtbl` author index `i` and DTTM `dttm`; an index outside
    /// the table is "Unknown".
    pub(super) fn rev_mark(&self, i: i32, dttm: u32) -> model::RevMark {
        let author = usize::try_from(i).ok().and_then(|i| self.rev_authors.get(i)).cloned().unwrap_or_else(|| "Unknown".to_string());
        model::RevMark { author, date: dttm_to_iso(dttm).unwrap_or_default(), id: None }
    }

    /// `\par` (or `\sect`): end the paragraph, whose mark is tracked as inserted or deleted when the text
    /// at it is.
    pub(super) fn end_para_mark(&mut self) {
        let r = self.st.rev;
        self.para.mark_ins = r.ins.then(|| Box::new(self.rev_mark(r.auth, r.dttm)));
        self.para.mark_del = r.del.then(|| Box::new(self.rev_mark(r.authdel, r.dttmdel)));
        self.end_para(false);
    }

    /// Whether text in the current field's result is read as an [`Inline::Field`]: always beside
    /// shapes and for legacy form fields, otherwise while the result stays in one paragraph.
    pub(super) fn field_inline(&self) -> bool {
        !self.shapes.is_empty() || self.st.field.is_some_and(|i| self.forms.contains_key(&i) || !self.plain_fields.contains(&i))
    }

    /// A paragraph mark inside a field's result (other than beside shapes or in a legacy form
    /// field): the result read so far in this paragraph goes back to plain text, and so does the
    /// rest of it.
    fn plain_field_result(&mut self) {
        let Some(i) = self.st.field else { return };
        if !self.shapes.is_empty() || self.forms.contains_key(&i) || !self.plain_fields.insert(i) {
            return;
        }
        let (Some(at), Some(instr)) = (self.field_at.remove(&i), self.fields.get(i)) else { return };
        let Some(tail) = self.para.inlines.get(at..).map(<[Inline]>::to_vec) else { return };
        self.para.inlines.truncate(at);
        for inline in tail {
            match inline {
                Inline::Field { instr: x, result, fmt, .. } if &x == instr => self.para.push_text(&result, &fmt),
                other => self.para.inlines.push(other),
            }
        }
    }

    /// End the current paragraph (`\par`, `\cell`).
    pub(super) fn end_para(&mut self, in_cell: bool) {
        self.flush_high_surrogate();
        self.plain_field_result();
        let read = std::mem::take(&mut self.para);
        self.note_mark.serial = self.note_mark.serial.wrapping_add(1);
        let props = self.para_props(&self.st);
        let mut p = Para { inlines: read.inlines, mark_ins: read.mark_ins, mark_del: read.mark_del, ..props };
        p.mark = self.empty_mark(&p);
        if p.kind == Kind::Normal && p.inlines.is_empty() && self.rule_pending {
            p.kind = Kind::Rule;
        }
        self.rule_pending = false;
        let c = &self.st.chg_p;
        if let Some(old) = &c.old {
            p.fmt_change = Some(Box::new(model::FmtChange { mark: self.rev_mark(c.auth, c.date), old: (**old).clone() }));
        }
        // Headings, code, titles and quotes keep their style formatting implicit, in the text's
        // formatting and in the formatting before a tracked change alike. The style is the
        // file's when its name gave the kind (not an outline level on another style).
        let base = if self.style_kind(&self.st) == Some(p.kind) { self.style_base(p.kind, self.st.style) } else { style_fmt(p.kind) };
        for i in &mut p.inlines {
            if let Inline::Text(_, f) = i {
                drop_style_formatting(p.kind, &base, f);
                if let Some(c) = f.fmt_change.as_deref_mut() {
                    drop_style_formatting(p.kind, &base, &mut c.old);
                }
            }
        }
        self.para_has = false;
        let d = self.para_depth();
        let d = if in_cell { d.max(1) } else { d };
        self.place_para(p, d);
    }

    /// Puts paragraph `p` at table depth `d`: the body (0) or the cell being read of the table
    /// at that depth.
    fn place_para(&mut self, p: Para, d: usize) {
        if d == 0 {
            self.close_table();
            self.body.push(FBlock::Para(p));
        } else {
            self.enter_depth(d);
            self.cell.push(FBlock::Para(p));
        }
    }

    /// The paragraph mark's formatting of `p` when it holds no text: the character formatting in
    /// effect at its end (RTF 1.9.1, Paragraph Text), which sets an empty line's height. A
    /// monospaced font keeps its name.
    fn empty_mark(&self, p: &Para) -> Option<Box<wordcraft_doc::props::CharProps>> {
        if !p.textless() {
            return None;
        }
        let f = &self.st.fmt;
        let plain = Fmt { link: None, link_tip: None, ins: None, del: None, fmt_change: None, ..f.clone() };
        let mut m = model::char_props(&plain);
        if f.code
            && let Some(n) = &self.st.mono
        {
            m.font = Some(n.to_string());
        }
        (!m.is_empty()).then(|| Box::new(m))
    }

    /// `\cell`: the end of a cell of the outermost table.
    pub(super) fn end_cell(&mut self) {
        self.end_cell_at(1);
    }

    /// `\nestcell`: the end of a cell of the table at the paragraph's depth; outside a nested
    /// table, the end of the paragraph.
    pub(super) fn end_nest_cell(&mut self) {
        let d = self.para_depth();
        if d >= 2 {
            self.end_cell_at(d);
        } else {
            self.end_para(false);
        }
    }

    /// The end of the cell of the table at depth `d`, with the paragraph being read.
    fn end_cell_at(&mut self, d: usize) {
        // The cell ends at its own depth, whatever the paragraph's words say.
        let saved = self.st.itap;
        self.st.itap = u8::try_from(d.clamp(1, model::MAX_DEPTH)).ok();
        self.end_para(true);
        self.st.itap = saved;
        let blocks = std::mem::take(&mut self.cell);
        if self.row.len() < 63 {
            self.row.push(Cell { blocks, ..Default::default() });
        }
    }

    /// `\row`: the end of a row of the outermost table; tables nested in it end first.
    pub(super) fn end_outer_row(&mut self) {
        if self.para_has {
            self.end_cell_at(1);
        }
        self.enter_depth(1);
        self.end_row();
    }

    /// `\nestrow`: the end of a row of the innermost nested table; nothing outside one.
    pub(super) fn end_nest_row(&mut self) {
        if !self.nest.is_empty() {
            self.end_row();
        }
    }

    /// The end of a row of the current table.
    fn end_row(&mut self) {
        if self.para_has || !self.para.inlines.is_empty() {
            self.flush_high_surrogate();
        } else {
            // Nothing since the last `\cell`: the surrogate sits on the row's end mark, which shows
            // nothing, rather than in a cell of its own.
            self.pending_high = None;
        }
        if self.para_has {
            self.end_cell_at(self.table_level());
        }
        let mut cells = std::mem::take(&mut self.row);
        let defs = self.defs.clone();
        let mut out: Vec<Cell> = Vec::new();
        let mut rights: Vec<i32> = Vec::new();
        for (i, mut c) in cells.drain(..).enumerate() {
            let d = defs.get(i).cloned().unwrap_or_default();
            // A merged-in cell widens the cell it joins.
            if d.hmerge == 2
                && let Some(last) = rights.last_mut()
            {
                *last = d.right;
            } else {
                rights.push(d.right);
            }
            c.shading = d.shading;
            c.borders = d.borders.any_visible().then_some(d.borders);
            c.header = self.row_header;
            c.valign = d.valign;
            c.direction = d.direction;
            c.own_margins = d.own_margins();
            c.no_wrap = d.no_wrap;
            match d.vmerge {
                1 => c.rowspan = 2,
                2 => c.covered = true,
                _ => {}
            }
            if d.hmerge == 2
                && let Some(prev) = out.last_mut()
            {
                prev.colspan = (prev.colspan + 1).min(63);
                continue;
            }
            out.push(c);
        }
        if !out.is_empty() && self.table.len() < wordcraft_doc::table::MAX_ROWS {
            rights.truncate(out.len());
            if self.table.is_empty() {
                self.table_style = self.row_style.clone();
            }
            self.table.push(out);
            self.table_rights.push(rights);
            self.table_lefts.push(self.row_style.left);
            self.table_heights.push(self.row_height);
            self.table_keeps.push(self.row_keep);
        }
    }

    /// Ends every open table; the outermost goes into the body.
    pub(super) fn close_table(&mut self) {
        self.enter_depth(1);
        if let Some(t) = self.take_table() {
            self.body.push(FBlock::Table(t));
        }
    }

    /// Ends the current table, with its row and cell being read.
    fn take_table(&mut self) -> Option<FTable> {
        if !self.row.is_empty() || !self.cell.is_empty() {
            if !self.cell.is_empty() {
                let blocks = std::mem::take(&mut self.cell);
                self.row.push(Cell { blocks, ..Default::default() });
            }
            self.end_row();
        }
        if self.table.is_empty() {
            return None;
        }
        let mut rows = std::mem::take(&mut self.table);
        let rights = std::mem::take(&mut self.table_rights);
        let lefts = std::mem::take(&mut self.table_lefts);
        let rights = from_left_edge(rights, &lefts);
        let widths = column_grid(&mut rows, &rights);
        let heights = std::mem::take(&mut self.table_heights);
        let keep_rows = std::mem::take(&mut self.table_keeps);
        // Only a table whose rows give a height or keep gets them, so others stay as before.
        let mut extra = (heights.iter().any(Option::is_some) || keep_rows.contains(&true))
            .then(|| Box::new(model::TableExtra { heights, keep_rows, ..Default::default() }));
        // Its first row's table style and look.
        let ts = std::mem::take(&mut self.table_style);
        // Its first row's position: a zero edge with no alignment is the default.
        let indent = ts.edge().map(|e| e as f32 / 20.0);
        if ts.align.is_some() || indent.is_some() {
            let x = extra.get_or_insert_with(Default::default);
            (x.align, x.indent) = (ts.align, indent);
        }
        // Its first row's default cell padding, when not Word's.
        if let Some(m) = ts.cell_margins() {
            extra.get_or_insert_with(Default::default).cell_margins = Some(m);
        }
        if ts.style.is_some() || ts.look.is_some() {
            let x = extra.get_or_insert_with(Default::default);
            (x.style, x.look) = (ts.style, ts.look);
        }
        // Word draws only the borders the cells name: no grid style, each cell its own.
        Some(FTable { rows, widths, borderless: true, extra, kept: None, wrap: None })
    }
}

/// The rows' cell right edges measured from the table's left edge rather than the margin: less
/// the first row's `\trleft`, or each row's own when the first row's would put a row's first edge
/// at or left of the table's; as read when neither leaves a cell.
fn from_left_edge(rights: Vec<Vec<i32>>, lefts: &[i32]) -> Vec<Vec<i32>> {
    let shift = |own: bool| -> Vec<Vec<i32>> {
        let first = lefts.first().copied().unwrap_or(0);
        rights
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let left = if own { lefts.get(i).copied().unwrap_or(0) } else { first };
                r.iter().map(|&x| x.saturating_sub(left)).collect()
            })
            .collect()
    };
    let starts = |rs: &[Vec<i32>]| rs.iter().all(|r| r.first().is_none_or(|&x| x > 0));
    if lefts.iter().all(|&l| l == 0) {
        return rights;
    }
    let shared = shift(false);
    if starts(&shared) {
        return shared;
    }
    let own = shift(true);
    if starts(&own) { own } else { rights }
}

/// Applies section formatting word `w` (parameter `p`) to `sec`. False when `w` isn't one.
pub(super) fn apply_section_word(sec: &mut SecFmt, w: &str, p: Option<i32>) -> bool {
    use wordcraft_doc::section::{ChapterSep, LineNumberRestart, NumFormat, PageVAlign, SectionStart};
    let on = p != Some(0);
    let pv = p.unwrap_or(0);
    match w {
        "sectd" => *sec = SecFmt::default(),
        "titlepg" => sec.title_page = on,
        "pgwsxn" => sec.page.w = Some(pv),
        "pghsxn" => sec.page.h = Some(pv),
        "marglsxn" => sec.page.left = Some(pv),
        "margrsxn" => sec.page.right = Some(pv),
        "margtsxn" => sec.page.top = Some(pv),
        "margbsxn" => sec.page.bottom = Some(pv),
        "guttersxn" => sec.page.gutter = Some(pv),
        "headery" => sec.page.header = Some(pv),
        "footery" => sec.page.footer = Some(pv),
        "lndscpsxn" => sec.page.landscape = on,
        "endnhere" => sec.endnhere = on,
        _ if w.strip_prefix('s').is_some_and(|w| apply_note_word(w, p, &mut sec.footnote_pr, &mut sec.endnote_pr)) => {}
        // Printer bin codes; 0 (the default) and values outside u16 leave the bin unset.
        "binfsxn" => sec.bin_first = u16::try_from(pv).ok().filter(|&v| v != 0),
        "binsxn" => sec.bin_other = u16::try_from(pv).ok().filter(|&v| v != 0),
        "vertalt" => sec.valign = PageVAlign::Top,
        "vertalc" => sec.valign = PageVAlign::Center,
        "vertalj" => sec.valign = PageVAlign::Justify,
        "vertalb" => sec.valign = PageVAlign::Bottom,
        "sbknone" => sec.start = SectionStart::Continuous,
        "sbkcol" => sec.start = SectionStart::NextColumn,
        "sbkpage" => sec.start = SectionStart::NextPage,
        "sbkeven" => sec.start = SectionStart::EvenPage,
        "sbkodd" => sec.start = SectionStart::OddPage,
        "cols" => sec.cols = Some(pv),
        "colsx" => sec.colsx = Some(pv),
        "linebetcol" => sec.linebetcol = on,
        "colno" => sec.col_sel = usize::try_from(pv).ok().filter(|n| (1..=45).contains(n)).map(|n| n - 1),
        "colw" | "colsr" => {
            if let (Some(i), true) = (sec.col_sel, pv >= 0) {
                if sec.col_dims.len() <= i {
                    sec.col_dims.resize(i + 1, (None, None));
                }
                if let Some(d) = sec.col_dims.get_mut(i) {
                    if w == "colw" {
                        d.0 = Some(pv);
                    } else {
                        d.1 = Some(pv);
                    }
                }
            }
        }
        "pgnrestart" => sec.pgn_restart = true,
        "pgncont" => sec.pgn_restart = false,
        "pgnstarts" => sec.pgn_start = Some(pv),
        "pgndec" => sec.pgn_fmt = NumFormat::Decimal,
        "pgnucrm" => sec.pgn_fmt = NumFormat::UpperRoman,
        "pgnlcrm" => sec.pgn_fmt = NumFormat::LowerRoman,
        "pgnucltr" => sec.pgn_fmt = NumFormat::UpperLetter,
        "pgnlcltr" => sec.pgn_fmt = NumFormat::LowerLetter,
        "linemod" => sec.line_mod = Some(pv),
        "linex" => sec.line_x = Some(pv),
        "linestarts" => sec.line_starts = Some(pv),
        "lineppage" => sec.line_restart = Some(LineNumberRestart::Page),
        "linerestart" => sec.line_restart = Some(LineNumberRestart::Section),
        "linecont" => sec.line_restart = Some(LineNumberRestart::Continuous),
        "sectspecifyl" => sec.grid_kind = Some(wordcraft_doc::section::DocGridType::Lines),
        "sectspecifycl" => sec.grid_kind = Some(wordcraft_doc::section::DocGridType::LinesAndChars),
        "sectlinegrid" => sec.line_grid = Some(pv),
        // Read and ignored: their meaning is not confirmed, so they set no grid and no char space.
        "sectspecifygen" | "sectexpand" => {}
        "rtlsect" => sec.rtl = true,
        "ltrsect" => sec.rtl = false,
        // Word draws 1, 3 and 5 top to bottom (5 stacking from the left, which the model has no
        // direction for; `Down` is nearest) and 2 bottom to top; anything else is horizontal.
        "stextflow" => {
            sec.text_flow = match pv {
                1 | 3 | 5 => wordcraft_doc::props::TextDirection::Down,
                2 => wordcraft_doc::props::TextDirection::Up,
                _ => wordcraft_doc::props::TextDirection::Horizontal,
            }
        }
        "pgnhn" => {
            if let Ok(n @ 1..=9) = u8::try_from(pv) {
                sec.chapter_style = Some(n);
            }
        }
        "pgnhnsh" => sec.chapter_sep = Some(ChapterSep::Hyphen),
        "pgnhnsp" => sec.chapter_sep = Some(ChapterSep::Period),
        "pgnhnsc" => sec.chapter_sep = Some(ChapterSep::Colon),
        "pgnhnsm" => sec.chapter_sep = Some(ChapterSep::EmDash),
        "pgnhnsn" => sec.chapter_sep = Some(ChapterSep::EnDash),
        _ => return false,
    }
    true
}

/// The document setting words read from the header, applied to `Document.settings` once the
/// document is built. `None` is a word the header doesn't name.
#[derive(Default)]
pub(super) struct DocSettingWords {
    /// `\viewscale` (10–500) and `\viewzk` (`Some(None)` for `\viewzk0`).
    percent: Option<u32>,
    fit: Option<Option<wordcraft_doc::ZoomFit>>,
    /// The drawing grid, in points.
    grid_h: Option<f32>,
    grid_v: Option<f32>,
    origin_x: Option<f32>,
    origin_y: Option<f32>,
    every_h: Option<u8>,
    every_v: Option<u8>,
    margins: Option<bool>,
    /// `\protlevel` (0–3), `\enforceprot`, and the level an older word (`\formprot`…) names.
    level: Option<u8>,
    enforce: Option<bool>,
    old_prot: Option<&'static str>,
    link_styles: Option<bool>,
    remove_personal_info: Option<bool>,
    remove_date_time: Option<bool>,
    expand_shift_return: Option<bool>,
    /// `\htmautsp` (Use HTML paragraph auto spacing): the space between two paragraphs is the
    /// larger of space after and space before. Without it Word adds the two.
    html_auto_spacing: bool,
    /// `\revisions` (Track Changes on) and `\printdata` (print only the data from a form).
    track_changes: Option<bool>,
    print_forms_data: Option<bool>,
}

impl DocSettingWords {
    pub(super) fn apply(&self, s: &mut wordcraft_doc::Settings) {
        s.sum_paragraph_spacing = !self.html_auto_spacing;
        if self.percent.is_some() || self.fit.is_some() {
            let (percent, fit) = (self.percent.unwrap_or(100), self.fit.flatten());
            // 100% with no fit mode is the model's "no saved zoom".
            s.zoom = (percent != 100 || fit.is_some()).then_some(wordcraft_doc::Zoom { percent, fit });
        }
        if let Some(v) = self.grid_h {
            s.grid_h = v;
        }
        if let Some(v) = self.grid_v {
            s.grid_v = v;
        }
        if let Some(v) = self.origin_x {
            s.grid_origin_x = v;
        }
        if let Some(v) = self.origin_y {
            s.grid_origin_y = v;
        }
        if let Some(v) = self.every_h {
            s.grid_every_h = v;
        }
        if let Some(v) = self.every_v {
            s.grid_every_v = v;
        }
        // An origin with no `\dgmargin` is measured from the page edge.
        match self.margins {
            Some(m) => s.grid_use_margins = m,
            None if self.origin_x.is_some() || self.origin_y.is_some() => s.grid_use_margins = false,
            None => {}
        }
        let level = self.level.and_then(|n| ["trackedChanges", "comments", "forms", "readOnly"].get(usize::from(n)).copied());
        let prot = match self.enforce {
            Some(true) => level.or(self.old_prot),
            Some(false) => None,
            None => self.old_prot,
        };
        if let Some(p) = prot {
            s.protection = Some(p.to_string());
        }
        for (v, field) in [
            (self.link_styles, &mut s.link_styles),
            (self.remove_personal_info, &mut s.remove_personal_info),
            (self.remove_date_time, &mut s.remove_date_time),
            (self.expand_shift_return, &mut s.do_not_expand_shift_return),
            (self.track_changes, &mut s.track_changes),
            (self.print_forms_data, &mut s.print_forms_data),
        ] {
            if let Some(v) = v {
                *field = v;
            }
        }
    }
}

#[cfg(test)]
mod bookmark_end_tests {
    use super::super::{FBlock, Inline, export, import, parse};

    fn inlines(rtf: &str) -> Vec<Inline> {
        let flow = parse(rtf.as_bytes()).unwrap();
        flow.blocks
            .into_iter()
            .filter_map(|b| match b {
                FBlock::Para(p) => Some(p.inlines),
                FBlock::Table(_) => None,
            })
            .flatten()
            .collect()
    }

    #[test]
    fn many_stray_ends_are_read_one_each_without_a_bookmark() {
        let body: String = (0..5000).map(|i| format!("{{\\*\\bkmkend n{}}}x", i % 7)).collect();
        let rtf = format!("{{\\rtf1 {body}\\par}}");
        let ends = inlines(&rtf).iter().filter(|i| matches!(i, Inline::AnchorEnd(_))).count();
        assert_eq!(ends, 5000);
        let d = import(rtf.as_bytes()).unwrap();
        assert!(!export(&d).contains("bkmk"));
    }

    #[test]
    fn an_end_name_is_capped_and_an_empty_one_pushes_nothing() {
        let long = "n".repeat(10_000);
        let got = inlines(&format!("{{\\rtf1 a{{\\*\\bkmkend {long}}}{{\\*\\bkmkend   }}b\\par}}"));
        let names: Vec<usize> = got
            .iter()
            .filter_map(|i| match i {
                Inline::AnchorEnd(n) => Some(n.len()),
                _ => None,
            })
            .collect();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], 256);
    }

    #[test]
    fn a_nested_field_does_not_take_the_outer_lock() {
        let got = inlines(r"{\rtf1 {\field\fldlock{\*\fldinst IF 1 = 1 x}{\fldrslt {\field{\*\fldinst AUTHOR}{\fldrslt Ann}}}}\par}");
        let locks: Vec<(String, bool)> = got
            .iter()
            .filter_map(|i| match i {
                Inline::Field { instr, locked, .. } => Some((instr.trim().to_string(), *locked)),
                _ => None,
            })
            .collect();
        assert!(locks.iter().any(|(i, l)| i == "AUTHOR" && !l), "{locks:?}");
    }

    #[test]
    fn a_header_bookmark_keeps_its_range_on_export() {
        let rtf = r"{\rtf1{\header \pard {\*\bkmkstart h}HEAD{\*\bkmkend h} tail\par}\pard body\par}";
        let out = export(&import(rtf.as_bytes()).unwrap());
        let (s, t, e) = (out.find(r"{\*\bkmkstart h}").unwrap(), out.find("HEAD").unwrap(), out.find(r"{\*\bkmkend h}").unwrap());
        assert!(s < t && t < e, "{out}");
    }
}
