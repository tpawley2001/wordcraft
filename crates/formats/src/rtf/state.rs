use super::{Align, Arc, Cell, FBlock, Fmt, Para, Rgb, VAlign};
use wordcraft_doc::props::{TabAlign, TabLeader, TabStop};

/// Deepest group nesting we track; deeper groups share the innermost state.
pub(super) const MAX_GROUPS: usize = 512;

/// Largest picture we decode (bytes).
pub(super) const MAX_PICT: usize = 64 << 20;
/// Most notes, and most comments, one file may bring in.
pub(super) const MAX_NOTES: usize = 5_000;
/// Most blocks kept of one note's or comment's text.
pub(super) const MAX_NOTE_BLOCKS: usize = 500;
/// Brackets of a place marker in text: a note reference or a comment range end travels through the
/// flow model as `OPEN tag number CLOSE` (private-use characters, which both the reader and the
/// writer keep out of real text) and becomes an inline object once the document exists.
pub(super) const MARK_OPEN: char = '\u{F8F0}';
pub(super) const MARK_CLOSE: char = '\u{F8F1}';

/// Most sections we read (`\sect`); later breaks end paragraphs only.
pub(super) const MAX_SECTIONS: usize = 10_000;

// ---------------------------------------------------------------------------------------------
// Import

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Dest {
    Body,
    ShapeInst,
    ShapeName,
    ShapeValue,
    Skip,
    FontTbl,
    ColorTbl,
    StyleSheet,
    Info(u8),
    ListTable,
    ListOverride,
    FldInst,
    Pict,
    /// An embedded object (`\object`): its control data is skipped, its `{\result …}` group is
    /// read as body content, which is what Word shows for the object.
    Object,
    /// A bookmark's name: its start (`\bkmkstart`, `false`) or its end (`\bkmkend`, `true`).
    Bookmark(bool),
    /// A comment's text-valued word: 1 `\atrfstart`, 2 `\atrfend`, 3 `\atnid`, 4 `\atnauthor`,
    /// 5 `\atndate`/`\atntime`, 6 `\atnref`.
    Atn(u8),
    /// The author table of tracked changes (`\revtbl`).
    RevTbl,
    /// Formatting before a tracked change (`\oldpprops` when true, else `\oldcprops`): read
    /// into the group's own state, which the group's end keeps as the change's old formatting.
    OldProps(bool),
    /// Section formatting before a tracked change (`\oldsprops`): read into the reader's
    /// scratch section, which the group's end keeps as the current section's old formatting.
    OldSect,
    /// Word 6.0/95 paragraph numbering (`{\*\pn …}`): its words are read into the reader's
    /// scratch numbering, which the group's end keeps as the paragraph's.
    Pn,
    /// The text before the number of a `{\*\pn …}` group (`{\pntxtb …}`): kept as the bullet of
    /// a bulleted level, or the prefix of a numbered one.
    PnTxtB,
    /// The text after the number of a `{\*\pn …}` group (`{\pntxta …}`): the label's suffix.
    PnTxtA,
    /// The program that wrote the file (`{\*\generator …}`): its text is kept.
    Generator,
    /// A font-table entry's PANOSE (`{\*\panose …}`): its hex digits are kept for the entry.
    Panose,
    /// A legacy form field's definition inside a field instruction (`{\*\formfield …}`): its
    /// words are read into the field's [`super::form_field::FormField`].
    FormField,
    /// A text-valued part of a `{\*\formfield …}` group (`{\*\ffname …}`, `{\*\ffl …}`…).
    FfText(super::form_field::FfSlot),
    /// User properties (`{\*\userprops …}`, RTF 1.9.1 "User Properties"): `\proptypeN` is read
    /// here, `\propname`, `\staticval` and `\linkval` are text groups (`Info(8..=10)`).
    UserProps,
    /// A document variable (`{\*\docvar {name}{value}}`, RTF 1.9.1 "Document Variables"): its
    /// first nested group is the name, the second the value, both text groups (`Info(14)`).
    DocVar,
    /// The attached template (`{\*\template …}`, RTF 1.9.1): its text is the template's path.
    Template,
    /// The latent-style table (`{\*\latentstyles …}`): its words are read into
    /// [`super::latent_styles::Latent`].
    LatentStyles,
    /// The exceptions of the latent-style table (`{\lsdlockedexcept …}`): words and names.
    LsdExcept,
}

/// Longest attached template path kept (characters).
pub(super) const MAX_TEMPLATE_CHARS: usize = 4096;

/// Longest text kept of one information-group field or user property name or value (bytes).
pub(super) const MAX_INFO_TEXT: usize = 64 * 1024;
/// Most characters of a `{\*\panose …}` group kept: one past its 20 hex digits, so a longer
/// one is rejected rather than cut to fit.
pub(super) const MAX_PANOSE_TEXT: usize = 21;
/// Most user properties one file may bring in.
pub(super) const MAX_USER_PROPS: usize = 1_000;
/// Most latent-style exceptions kept (`{\lsdlockedexcept …}`); Word writes about 380.
pub(super) const MAX_LATENT_EXCEPTIONS: usize = 2_000;

/// Most document variables one file may bring in.
pub(super) const MAX_DOC_VARS: usize = 4096;
/// Longest document variable name and value kept (characters); longer ones are dropped.
pub(super) const MAX_DOC_VAR_NAME: usize = 255;
pub(super) const MAX_DOC_VAR_VALUE: usize = 65_280;

/// Cut `s` to at most `max` bytes, at a character boundary.
pub(super) fn truncate_chars(s: &mut String, max: usize) {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
}

/// What the information group says beyond the flow model's metadata: `\operator`, `\category`
/// and `\version` (the dates are read with `\creatim`), and the user properties.
#[derive(Debug, Default)]
pub(super) struct InfoMore {
    pub(super) last_modified_by: String,
    pub(super) category: String,
    /// `{\manager …}`, `{\company …}` and `{\hlinkbase …}`: kept as the DOCX code keeps
    /// `docProps/app.xml`'s Manager, Company and HyperlinkBase.
    pub(super) manager: String,
    pub(super) company: String,
    pub(super) hyperlink_base: String,
    pub(super) revision: u32,
    pub(super) props: Vec<wordcraft_doc::CustomProp>,
    /// The user property being read: its name, `\proptypeN`, value and linked bookmark.
    pub(super) cur: Option<UserProp>,
    pub(super) doc_vars: Vec<wordcraft_doc::DocVar>,
    /// The name and value groups read so far of the `{\*\docvar …}` group being read; `None`
    /// for a part longer than the information-group text cap.
    pub(super) var_parts: Vec<Option<String>>,
}

#[derive(Debug, Default)]
pub(super) struct UserProp {
    pub(super) name: String,
    pub(super) ptype: i32,
    pub(super) value: String,
    pub(super) link: Option<String>,
}

impl InfoMore {
    /// The text of a `\propname` (8), `\staticval` (9) or `\linkval` (10) group. A name starts
    /// a new property, committing the one before it.
    pub(super) fn user_prop_text(&mut self, k: u8, t: String) {
        if k == 8 {
            self.commit_user_prop();
            self.cur = Some(UserProp { name: t, ..Default::default() });
            return;
        }
        if let Some(p) = self.cur.as_mut() {
            if k == 9 {
                p.value = t;
            } else {
                p.link = Some(t);
            }
        }
    }

    /// Keep the document variable just read: it needs a name; a missing value is empty; a name
    /// or value over the limits drops it.
    pub(super) fn commit_doc_var(&mut self) {
        let mut parts = std::mem::take(&mut self.var_parts).into_iter();
        let Some(Some(name)) = parts.next() else { return };
        let Some(value) = parts.next().unwrap_or(Some(String::new())) else { return };
        if name.is_empty()
            || name.chars().count() > MAX_DOC_VAR_NAME
            || value.chars().count() > MAX_DOC_VAR_VALUE
            || self.doc_vars.len() >= MAX_DOC_VARS
        {
            return;
        }
        self.doc_vars.push(wordcraft_doc::DocVar { name, value });
    }

    pub(super) fn set_prop_type(&mut self, n: i32) {
        if let Some(p) = self.cur.as_mut() {
            p.ptype = n;
        }
    }

    /// Keep the property being read, if it has a name: a later one of the same name (in any
    /// case) replaces it.
    pub(super) fn commit_user_prop(&mut self) {
        let Some(p) = self.cur.take() else { return };
        if p.name.is_empty() {
            return;
        }
        let (kind, value) = user_prop_value(p.ptype, p.value);
        let link_target = p.link;
        let room = self.props.len() < MAX_USER_PROPS;
        match self.props.iter_mut().find(|q| q.name.eq_ignore_ascii_case(&p.name)) {
            Some(q) => (q.kind, q.value, q.link_target) = (kind.into(), value, link_target),
            None if room => self.props.push(wordcraft_doc::CustomProp { name: p.name, kind: kind.into(), value, link_target, ..Default::default() }),
            None => {}
        }
    }
}

/// A user property's variant type and value from its `\proptypeN` and `\staticval` text; a value
/// that doesn't read as its type stays text.
fn user_prop_value(ptype: i32, v: String) -> (&'static str, String) {
    match ptype {
        3 => match v.parse::<i32>() {
            Ok(n) => ("i4", n.to_string()),
            Err(_) => ("lpwstr", v),
        },
        5 if v.parse::<f64>().is_ok_and(f64::is_finite) => ("r8", v),
        11 => match v.to_ascii_lowercase().as_str() {
            "1" | "true" => ("bool", "true".into()),
            "0" | "false" => ("bool", "false".into()),
            _ => ("lpwstr", v),
        },
        64 => match prop_date(&v) {
            Some(d) => ("filetime", d),
            None => ("lpwstr", v),
        },
        _ => ("lpwstr", v),
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` from a date property's value that starts with a valid `YYYY-MM-DD`; the
/// time after it is read when given.
fn prop_date(v: &str) -> Option<String> {
    let date = chrono::NaiveDate::parse_from_str(v.get(..10)?, "%Y-%m-%d").ok()?;
    let part = |a: usize, b: usize, max: u32| v.get(a..b).and_then(|x| x.parse::<u32>().ok()).filter(|n| *n <= max).unwrap_or(0);
    let (h, mi, sec) = (part(11, 13, 23), part(14, 16, 59), part(17, 19, 59));
    Some(format!("{}T{h:02}:{mi:02}:{sec:02}Z", date.format("%Y-%m-%d")))
}

/// Word 6.0/95 paragraph numbering (`{\*\pn …}`, RTF 1.9.1 "Bullets and Numbering"): what one
/// group says about the paragraph's label. A paragraph's own group describes its level only.
#[derive(Clone, Debug, Default)]
pub(super) struct Pn {
    /// `\pnlvlblt` (bulleted), `\pnlvlbody` (numbered, one level), `\pnlvlN` (outline level N,
    /// 1-based as written) or `\pnlvlcont` (no label, the count doesn't advance).
    pub(super) kind: PnKind,
    /// The number format (`\pndec`, `\pnucrm`…); `None` until the group names one.
    pub(super) format: Option<wordcraft_doc::section::NumFormat>,
    /// The first item's number (`\pnstartN`).
    pub(super) start: Option<u32>,
    /// The text before the number (`{\pntxtb …}`): a bulleted level's bullet.
    pub(super) before: String,
    /// The text after the number (`{\pntxta …}`): "1." has a ".".
    pub(super) after: String,
    /// The font of the label (`\pnfN`, a `\fonttbl` index).
    pub(super) font: Option<i32>,
}

/// Which kind of Word 6.0/95 numbering a `{\*\pn …}` group describes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum PnKind {
    #[default]
    None,
    /// `\pnlvlblt`: bulleted.
    Bullet,
    /// `\pnlvlbody`: numbered, a simple one-level list.
    Body,
    /// `\pnlvlcont`: the paragraph continues the list without a label.
    Cont,
    /// `\pnlvlN`: outline numbering at level N (1-based as written).
    Outline(u8),
}

/// A footnote, endnote or comment group being read into an output of its own.
pub(super) struct Sub {
    pub(super) depth: usize,
    pub(super) comment: bool,
    pub(super) endnote: bool,
    /// The note's custom reference mark (empty: numbered).
    pub(super) custom: String,
    /// A group with text has been read in the note (only the first can be its own mark).
    pub(super) own_mark_done: bool,
    /// A note separator story instead (`\ftnsep`…): its slot in [`SEP_WORDS`].
    pub(super) sep: Option<usize>,
    pub(super) saved: Box<Output>,
}

/// The note separator and continuation story words (RTF 1.9.1 "Footnotes"), in the order of
/// the document's footnote separator, continuation separator and continuation notice, then the
/// endnote ones.
pub(super) const SEP_WORDS: [&str; 6] = ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"];

/// A note read: its kind and text.
pub(super) struct NoteData {
    pub(super) endnote: bool,
    /// The custom reference mark (empty: numbered).
    pub(super) custom: String,
    pub(super) blocks: Vec<FBlock>,
}

/// A comment read: the id its range markers carry, who wrote it and when, and its text.
pub(super) struct CommentData {
    pub(super) rtf_id: i32,
    pub(super) author: String,
    pub(super) initials: String,
    pub(super) date: String,
    pub(super) blocks: Vec<FBlock>,
}

/// A tracked formatting change being read (`\crauth`/`\crdate` with `\oldcprops`,
/// `\prauth`/`\prdate` with `\oldpprops`): author (`\revtbl` index), date (DTTM) and the old
/// formatting, once its group has been read.
#[derive(Clone, Debug, Default)]
pub(super) struct FmtRevSt<T> {
    pub(super) auth: i32,
    pub(super) date: u32,
    pub(super) old: Option<Box<T>>,
}

/// Character formatting words of an `\oldcprops` group, with their parameters.
type OldWords = Vec<(String, Option<i32>)>;

/// Most words kept from one `\oldcprops` group (hostile input).
pub(super) const MAX_OLD_WORDS: usize = 64;

/// Most tab stops kept for one paragraph (hostile input).
pub(super) const MAX_TABS: usize = 64;

/// Most authors read from a `\revtbl` (hostile input).
pub(super) const MAX_REV_AUTHORS: usize = 10_000;

/// Tracked-change character properties: `\revised` / `\deleted`, their authors (`\revtbl`
/// indices) and dates (DTTM).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RevSt {
    pub(super) ins: bool,
    pub(super) del: bool,
    pub(super) auth: i32,
    pub(super) authdel: i32,
    pub(super) dttm: u32,
    pub(super) dttmdel: u32,
}

/// Which header or footer a `\header…`/`\footer…` group is: footer or header, and the pages
/// (`\headerf` first, `\headerl` left = even, `\header`/`\headerr` the others).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HeaderFooter {
    pub(super) footer: bool,
    pub(super) pages: HfPages,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum HfPages {
    Default,
    First,
    Even,
}

/// The reader's output so far, set aside while a header or footer is read.
#[derive(Default)]
pub(super) struct Output {
    pub(super) body: Vec<FBlock>,
    pub(super) para: Para,
    pub(super) para_has: bool,
    pub(super) table: Vec<Vec<Cell>>,
    pub(super) row: Vec<Cell>,
    pub(super) cell: Vec<FBlock>,
    pub(super) defs: Vec<CellDef>,
    pub(super) def_cur: CellDef,
    pub(super) row_header: bool,
    pub(super) row_height: Option<(f32, bool)>,
    pub(super) row_keep: bool,
    pub(super) rule_pending: bool,
    pub(super) table_rights: Vec<Vec<i32>>,
    pub(super) table_lefts: Vec<i32>,
    pub(super) table_heights: Vec<Option<(f32, bool)>>,
    pub(super) table_keeps: Vec<bool>,
    pub(super) row_style: RowStyle,
    pub(super) table_style: RowStyle,
    pub(super) nest: Vec<TableLevel>,
}

/// The state of one open table that encloses the one being read (RTF nested tables, `\itapN`):
/// its rows so far, the row and cell being read, and the row's definitions.
#[derive(Default)]
pub(super) struct TableLevel {
    pub(super) table: Vec<Vec<Cell>>,
    pub(super) row: Vec<Cell>,
    pub(super) cell: Vec<FBlock>,
    pub(super) defs: Vec<CellDef>,
    pub(super) def_cur: CellDef,
    pub(super) row_header: bool,
    pub(super) row_height: Option<(f32, bool)>,
    pub(super) row_keep: bool,
    pub(super) table_rights: Vec<Vec<i32>>,
    pub(super) table_lefts: Vec<i32>,
    pub(super) table_heights: Vec<Option<(f32, bool)>>,
    pub(super) table_keeps: Vec<bool>,
    pub(super) row_style: RowStyle,
    pub(super) table_style: RowStyle,
}

/// A row's table style (`\tsN`, as its document style id) and look (`\tbllk*`), reset by
/// `\trowd`; a table's are its first row's.
#[derive(Clone, Debug, Default)]
pub(super) struct RowStyle {
    pub(super) style: Option<String>,
    pub(super) look: Option<wordcraft_doc::props::TableLook>,
    /// The row's left edge from the left margin (`\trleftN`, twips), the half gap between its
    /// cells (`\trgaphN`), its cell text indent (`\tblindN`, with `\tblindtypeN`) and its
    /// alignment (`\trqc`, `\trqr`; `\trql` none).
    pub(super) left: i32,
    pub(super) gap: i32,
    pub(super) ind: Option<i32>,
    pub(super) ind_type: i32,
    pub(super) align: Option<Align>,
    /// Whether the row states a gap at all (`\trgaphN`, N > 0).
    pub(super) has_gap: bool,
    /// The row's default cell padding (`\trpaddlN`, `\trpaddtN`, `\trpaddbN`, `\trpaddrN`,
    /// twips) and each side's unit (`\trpaddflN`… : 3 twips, else the value is ignored), in
    /// the order top, left, bottom, right.
    pub(super) pad: [i32; 4],
    pub(super) pad_unit: [i32; 4],
}

impl RowStyle {
    /// `\trgaphN`: the gap, twips, clamped to 22 inches.
    pub(super) fn gap_word(&mut self, pv: i32) {
        self.gap = pv.clamp(0, 31_680);
        self.has_gap = self.gap > 0;
    }

    /// A `\trpadd?N` value or `\trpaddf?N` unit word, by its side (top, left, bottom, right).
    pub(super) fn pad_word(&mut self, w: &str, pv: i32) {
        let i = match w.as_bytes().last() {
            Some(b't') => 0,
            Some(b'l') => 1,
            Some(b'b') => 2,
            _ => 3,
        };
        let slot = if w.starts_with("trpaddf") { self.pad_unit.get_mut(i) } else { self.pad.get_mut(i) };
        if let Some(v) = slot {
            *v = pv.clamp(-31_680, 31_680);
        }
    }

    /// Whether the row keeps its table style's cell padding: it has a table style and states
    /// neither a gap nor a side in twips.
    pub(super) fn keeps_style_padding(&self) -> bool {
        self.style.is_some() && !self.has_gap && !self.pad_unit.contains(&3)
    }

    /// The row's cell padding (top, left, bottom, right), points: a side in twips (unit 3) as
    /// stated (a negative value is 0), top and bottom else none, and left and right else the
    /// gap while the row states neither its left nor its right side in twips (once it states
    /// one, the other is 0).
    pub(super) fn padding(&self) -> [f32; 4] {
        let twips = |i: usize| self.pad_unit.get(i) == Some(&3);
        let gap = if twips(1) || twips(3) { 0.0 } else { self.gap.clamp(0, 31_680) as f32 / 20.0 };
        let side = |i: usize, default: f32| {
            if twips(i) { self.pad.get(i).copied().unwrap_or(0).clamp(0, 31_680) as f32 / 20.0 } else { default }
        };
        [side(0, 0.0), side(1, gap), side(2, 0.0), side(3, gap)]
    }

    /// The row's default cell padding (top, left, bottom, right), points, when it is not Word's
    /// default (see [`RowStyle::padding`]). A row that keeps its table style's padding has none.
    pub(super) fn cell_margins(&self) -> Option<[f32; 4]> {
        if self.keeps_style_padding() {
            return None;
        }
        let m = self.padding();
        let word = [0.0, 5.4, 0.0, 5.4];
        if m.iter().zip(word).all(|(v, w)| (v - w).abs() < 0.01) { None } else { Some(m) }
    }

    /// The table's left edge from the left margin in twips, when the row states one: `\tblind`
    /// in twips (`\tblindtype3`) less the gap, else `\trleft`. A centred or right-aligned row
    /// has none.
    pub(super) fn edge(&self) -> Option<i32> {
        if self.align.is_some() {
            return None;
        }
        let e = match self.ind {
            Some(n) if self.ind_type == 3 => n.saturating_sub(self.gap),
            _ => self.left,
        };
        (e != 0).then_some(e)
    }
}

#[derive(Clone, Debug)]
pub(super) struct St {
    pub(super) dest: Dest,
    pub(super) fmt: Fmt,
    /// The name of the monospaced font `\f` selected, which [`Fmt::code`] stands for in the
    /// text: a paragraph mark keeps it by name.
    pub(super) mono: Option<Arc<str>>,
    pub(super) rev: RevSt,
    /// Tracked change of the character formatting (reset by `\plain`). Its old formatting is
    /// the `\oldcprops` words, applied on top of the run's formatting when text is read: Word
    /// writes only the properties that changed there.
    pub(super) chg_c: FmtRevSt<OldWords>,
    /// Character shading pattern (`\chcbpat`/`\cb`, `\chcfpat`, `\chshdng`); reset by `\plain`.
    pub(super) ch_pattern: Pattern,
    /// While an `\oldcprops` group is read: its character formatting words, and the author and
    /// date of the change (`\crauth`/`\crdate`, which Word writes inside the group).
    pub(super) old_words: OldWords,
    pub(super) old_rev: (Option<i32>, Option<u32>),
    /// Tracked change of the paragraph formatting (reset by `\pard`).
    pub(super) chg_p: FmtRevSt<Para>,
    pub(super) uc: usize,
    // Paragraph properties.
    pub(super) align: Option<Align>,
    pub(super) style: i32,
    pub(super) intbl: bool,
    /// The paragraph's table depth (`\itapN`): 0 the body, 1 a table, 2 a table in its cell…
    /// `None` until the paragraph says, when `\intbl` means 1.
    pub(super) itap: Option<u8>,
    pub(super) ls: Option<i32>,
    pub(super) ilvl: u8,
    pub(super) outline: Option<u8>,
    pub(super) pagebb: bool,
    /// Keep with next (`\keepn`), keep lines together (`\keep`) and suppress line numbers
    /// (`\noline`); cleared by `\pard`.
    pub(super) keepn: bool,
    pub(super) keep: bool,
    pub(super) noline: bool,
    /// Widow/orphan control (`\widctlpar`, `\nowidctlpar`); `None` until the paragraph says.
    pub(super) widow: Option<bool>,
    /// Automatic space between Asian and Latin text (`\aspalpha`) and between Asian text and
    /// numbers (`\aspnum`); off unless the paragraph says, cleared by `\pard`.
    pub(super) asp_alpha: bool,
    pub(super) asp_num: bool,
    /// Contextual spacing (`\contextualspace`), mirror indents (`\mirrorindents`), automatic
    /// space before/after (`\sbautoN`, `\saautoN`), hyphenation suppressed (`\hyphpar0`) and
    /// line grid snapping (`\nosnaplinegrid`); `None` until the paragraph says.
    pub(super) contextual: Option<bool>,
    pub(super) mirror: Option<bool>,
    pub(super) sb_auto: Option<bool>,
    pub(super) sa_auto: Option<bool>,
    pub(super) no_hyph: Option<bool>,
    pub(super) snap_grid: Option<bool>,
    pub(super) field: Option<usize>,
    /// The code page of the current font's charset (`\fcharset`), when it names one.
    pub(super) cp: Option<u16>,
    /// Space before/after (`\sb`, `\sa`), line spacing (`\sl`, `\slmult`) and indents
    /// (`\li`, `\ri`, `\fi`), twips.
    /// The paragraph's borders (`\brdrt`, `\brdrb`, `\brdrl`, `\brdrr`) and the side being
    /// described.
    pub(super) borders: wordcraft_doc::props::Borders,
    pub(super) border_side: Option<char>,
    /// Paragraph shading: the background (`\cbpat`) and pattern (`\cfpat`) colour table
    /// indexes and the pattern percentage in hundredths (`\shading`).
    pub(super) cbpat: Option<i32>,
    pub(super) cfpat: Option<i32>,
    pub(super) shading: i32,
    pub(super) sb: Option<i32>,
    pub(super) sa: Option<i32>,
    pub(super) sl: i32,
    pub(super) slmult: bool,
    pub(super) li: Option<i32>,
    pub(super) ri: Option<i32>,
    pub(super) fi: Option<i32>,
    /// The same indents in hundredths of a character (`\culi`, `\curi`, `\cufi`).
    pub(super) culi: Option<i32>,
    pub(super) curi: Option<i32>,
    pub(super) cufi: Option<i32>,
    /// The paragraph's tab stops (`\tx`, `\tb`), and the alignment (`\tq*`) and leader (`\tl*`)
    /// words read for the next one.
    pub(super) tabs: Vec<TabStop>,
    pub(super) tab_align: TabAlign,
    pub(super) tab_leader: TabLeader,
    /// Word 6.0/95 paragraph numbering (`{\*\pn …}`), which the group's end keeps here; `\pard`
    /// clears it. A paragraph with `\lsN` (Word 97+) ignores it.
    pub(super) pn: Option<Box<Pn>>,
    /// The paragraph's frame and drop cap words (`\absw`, `\posx`, `\dropcapli`…).
    pub(super) frame: Option<Box<super::frames::FrameSt>>,
    /// The last run direction word in the group chain (`\rtlch` true, `\ltrch` false): the
    /// font, size, bold and italic words after `\rtlch` are the complex-script formatting.
    pub(super) dir: Option<bool>,
    /// The font slot (`\loch`, `\hich`, `\dbch`) the next font word fills.
    pub(super) slot: Option<FontSlot>,
    /// The character properties the text's own words turned off (`\b0`, `\ulnone`, `\cf0`…);
    /// part of the group's state, forgotten by `\plain`.
    pub(super) off: StatedOff,
}

/// Which character properties the text's own words turned off: over a style that turns one on,
/// the text has it off (RTF 1.9.1, "Style Sheet": a style's formatting applies only where the
/// text repeats it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct StatedOff {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
    pub(super) strike: bool,
    pub(super) double_strike: bool,
    pub(super) caps: bool,
    pub(super) small_caps: bool,
    pub(super) hidden: bool,
    /// Superscript and subscript (`\nosupersub`, `\super0`, `\sub0`).
    pub(super) script: bool,
    pub(super) highlight: bool,
    /// The automatic colour (`\cf0`).
    pub(super) color: bool,
}

/// The font a font word right after `\loch`, `\hich` or `\dbch` names (RTF 1.9.1, "Associated
/// Character Properties").
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FontSlot {
    /// The ordinary (low-ANSI) font.
    Low,
    /// The high-ANSI font, which the model has no place for.
    High,
    /// The East Asian (double-byte) font.
    DoubleByte,
}

impl Default for St {
    fn default() -> Self {
        St {
            dest: Dest::Body,
            fmt: Fmt::default(),
            mono: None,
            rev: RevSt::default(),
            chg_c: FmtRevSt::default(),
            ch_pattern: Pattern::default(),
            chg_p: FmtRevSt::default(),
            old_words: Vec::new(),
            old_rev: (None, None),
            uc: 1,
            off: StatedOff::default(),
            align: None,
            style: 0,
            intbl: false,
            itap: None,
            ls: None,
            ilvl: 0,
            outline: None,
            pagebb: false,
            keepn: false,
            keep: false,
            noline: false,
            widow: None,
            asp_alpha: false,
            asp_num: false,
            contextual: None,
            mirror: None,
            sb_auto: None,
            sa_auto: None,
            no_hyph: None,
            snap_grid: None,
            field: None,
            cp: None,
            borders: Default::default(),
            border_side: None,
            cbpat: None,
            cfpat: None,
            shading: 0,
            sb: None,
            sa: None,
            sl: 0,
            slmult: false,
            li: None,
            ri: None,
            fi: None,
            culi: None,
            curi: None,
            cufi: None,
            tabs: Vec::new(),
            tab_align: TabAlign::Left,
            tab_leader: TabLeader::None,
            pn: None,
            frame: None,
            dir: None,
            slot: None,
        }
    }
}

/// A shading pattern as read: the pattern (ink) and background (fill) colours, `None` for
/// automatic, and the pattern's coverage in hundredths of a percent.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Pattern {
    pub(super) ink: Option<Rgb>,
    pub(super) fill: Option<Rgb>,
    pub(super) pct: i32,
}

impl Pattern {
    /// The one colour the pattern paints: see [`pattern_shading`].
    pub(super) fn colour(&self) -> Option<Rgb> {
        pattern_shading(self.pct, self.ink, self.fill)
    }
}

/// The colour a percent shading pattern averages to: `pct` hundredths of a percent of the pattern
/// colour `ink` (automatic = black) over the background `fill` (automatic = white), exact halves
/// rounding down. With no pattern (0) it is the background colour, or none when that is automatic.
pub(super) fn pattern_shading(pct: i32, ink: Option<Rgb>, fill: Option<Rgb>) -> Option<Rgb> {
    let n = u32::try_from(pct.clamp(0, 10_000)).unwrap_or(0);
    if n == 0 {
        return fill;
    }
    let ink = ink.unwrap_or(Rgb(0, 0, 0));
    let fill = fill.unwrap_or(Rgb(255, 255, 255));
    let mix = |i: u8, f: u8| u8::try_from((u32::from(i) * n + u32::from(f) * (10_000 - n) + 4_999) / 10_000).unwrap_or(u8::MAX);
    Some(Rgb(mix(ink.0, fill.0), mix(ink.1, fill.1), mix(ink.2, fill.2)))
}

#[derive(Default, Clone)]
pub(super) struct CellDef {
    pub(super) vmerge: u8,
    pub(super) hmerge: u8,
    pub(super) shading: Option<Rgb>,
    /// The cell's shading pattern (`\clcbpat`, `\clcfpat`, `\clshdng`), `shading` resolved.
    pub(super) pattern: Pattern,
    pub(super) right: i32,
    /// The borders the cell names (`\clbrdrt` … `\clbrdrr`, each with its line style, `\brdrw`
    /// width and `\brdrcf` colour).
    pub(super) borders: wordcraft_doc::props::Borders,
    pub(super) valign: Option<VAlign>,
    /// Turned text (`\cltxtbrl`, `\cltxbtlr`…).
    pub(super) direction: wordcraft_doc::props::TextDirection,
    /// The cell's own padding, twips, per side (top, left, bottom, right): the value word
    /// (`\clpadl` top, `\clpadt` left, `\clpadb`, `\clpadr`; Word reads the first two swapped)
    /// and its unit word (`\clpadfl` … `\clpadfr`).
    pub(super) pad: [Option<i32>; 4],
    pub(super) pad_unit: [Option<i32>; 4],
    /// Wrap text off (`\clNoWrap`).
    pub(super) no_wrap: bool,
}

impl CellDef {
    /// The sides of its padding the cell sets, points: a side is set only with unit 3 (twips),
    /// and then a negative, missing or beyond-22-inch value is 0, as Word reads it.
    pub(super) fn own_margins(&self) -> [Option<f32>; 4] {
        let mut out = [None; 4];
        for ((o, v), u) in out.iter_mut().zip(self.pad).zip(self.pad_unit) {
            if u == Some(3) {
                let tw = v.filter(|v| (0..=31_680).contains(v)).unwrap_or(0);
                *o = Some(tw as f32 / 20.0);
            }
        }
        out
    }
}

/// Page size and margins in twips (`\paperw`, `\margl`… and their `…sxn` section forms).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PageGeom {
    pub(super) w: Option<i32>,
    pub(super) h: Option<i32>,
    pub(super) left: Option<i32>,
    pub(super) right: Option<i32>,
    pub(super) top: Option<i32>,
    pub(super) bottom: Option<i32>,
    pub(super) header: Option<i32>,
    pub(super) footer: Option<i32>,
    /// The gutter (`\gutterN`, `\guttersxnN`).
    pub(super) gutter: Option<i32>,
    /// `\rtlgutter`: the gutter is on the right (document-wide).
    pub(super) rtl_gutter: bool,
    pub(super) landscape: bool,
}

impl PageGeom {
    /// The section properties these give, Word's defaults where the file names none.
    fn section(&self) -> wordcraft_doc::SectionProps {
        let mut s = wordcraft_doc::SectionProps::default();
        // Twips to points, ignoring nonsense (zero, negative, or beyond Word's 22").
        let pt = |v: Option<i32>, lo: f32, hi: f32| v.map(|t| t as f32 / 20.0).filter(|v| v.is_finite() && (lo..=hi).contains(v));
        if let Some(w) = pt(self.w, 72.0, 1584.0) {
            s.page_w = w;
        }
        if let Some(h) = pt(self.h, 72.0, 1584.0) {
            s.page_h = h;
        }
        if self.landscape && s.page_w < s.page_h {
            std::mem::swap(&mut s.page_w, &mut s.page_h);
        }
        s.landscape = s.page_w > s.page_h;
        for (v, f) in [
            (self.left, &mut s.margin_left),
            (self.right, &mut s.margin_right),
            (self.top, &mut s.margin_top),
            (self.bottom, &mut s.margin_bottom),
            (self.header, &mut s.header),
            (self.footer, &mut s.footer),
        ] {
            if let Some(v) = pt(v, 0.0, 720.0) {
                *f = v;
            }
        }
        // The gutter: negative counts as none; capped like the margins.
        if let Some(g) = self.gutter {
            let g = g.max(0) as f32 / 20.0;
            s.gutter = if g.is_finite() { g.min(720.0) } else { 0.0 };
        }
        s.rtl_gutter = self.rtl_gutter;
        // Margins that leave no room for text fall back to Word's.
        let d = wordcraft_doc::SectionProps::default();
        if s.page_w - s.margin_left - s.margin_right < 72.0 {
            (s.margin_left, s.margin_right) = (d.margin_left, d.margin_right);
        }
        if s.page_h - s.margin_top - s.margin_bottom < 72.0 {
            (s.margin_top, s.margin_bottom) = (d.margin_top, d.margin_bottom);
        }
        s
    }
}

/// Document formatting words that go into the document's settings, as read: `None` where the
/// file doesn't give one (the setting keeps its default). `\widowctrl` is the document's widow
/// control default for paragraphs that don't set their own.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DocOpts {
    /// `\deftabN`, twips (1..=31680).
    pub(super) default_tab: Option<i32>,
    /// `\facingp`.
    pub(super) facing_pages: Option<bool>,
    /// `\margmirror`.
    pub(super) mirror_margins: Option<bool>,
    /// `\hyphauto`.
    pub(super) auto_hyphenation: Option<bool>,
    /// `\hyphhotzN`, twips (>= 0).
    pub(super) hyphenation_zone: Option<i32>,
    /// `\hyphcaps`.
    pub(super) hyphenate_caps: Option<bool>,
    /// `\hyphconsecN` (>= 0; 0 is no limit).
    pub(super) consecutive_hyphen_limit: Option<i32>,
    /// `\widowctrl`: widow and orphan control on for the paragraphs that don't say
    /// (`\widctlpar`/`\nowidctlpar`). Without it Word has it off for them.
    pub(super) widow_control: bool,
    /// `\wraptrsp`: Wrap trailing spaces onto the next line.
    pub(super) wrap_trail_spaces: Option<bool>,
    /// `\jexpand` (`Some(true)`) / `\jcompress` (`Some(false)`), the last one read: with
    /// expanding justification Word does not squeeze a line's spaces to keep its overrunning
    /// last word.
    pub(super) expand_justification: Option<bool>,
}

impl DocOpts {
    /// Reads document formatting word `w` (parameter `p`). False when `w` isn't one.
    pub(super) fn word(&mut self, w: &str, p: Option<i32>) -> bool {
        let on = p != Some(0);
        let pv = p.unwrap_or(0);
        match w {
            "deftab" => {
                if (1..=31_680).contains(&pv) {
                    self.default_tab = Some(pv);
                }
            }
            "facingp" => self.facing_pages = Some(on),
            "margmirror" => self.mirror_margins = Some(on),
            "hyphauto" => self.auto_hyphenation = Some(on),
            "hyphhotz" => {
                if pv >= 0 {
                    self.hyphenation_zone = Some(pv);
                }
            }
            "hyphcaps" => self.hyphenate_caps = Some(on),
            "hyphconsec" => {
                if pv >= 0 {
                    self.consecutive_hyphen_limit = Some(pv);
                }
            }
            "widowctrl" => self.widow_control = true,
            "wraptrsp" => self.wrap_trail_spaces = Some(on),
            "jexpand" => self.expand_justification = Some(true),
            "jcompress" => self.expand_justification = Some(false),
            _ => return false,
        }
        true
    }

    /// Puts what the file gave into the document's settings.
    pub(super) fn apply(&self, s: &mut wordcraft_doc::Settings) {
        if let Some(v) = self.default_tab {
            s.default_tab = v as f32 / 20.0;
        }
        if let Some(v) = self.facing_pages {
            s.even_odd_headers = v;
        }
        if let Some(v) = self.mirror_margins {
            s.mirror_margins = v;
        }
        if let Some(v) = self.auto_hyphenation {
            s.auto_hyphenation = v;
        }
        if let Some(v) = self.hyphenation_zone {
            s.hyphenation_zone = v as f32 / 20.0;
        }
        if let Some(v) = self.hyphenate_caps {
            s.hyphenate_caps = v;
        }
        if let Some(v) = self.consecutive_hyphen_limit {
            s.consecutive_hyphen_limit = u32::try_from(v).unwrap_or(0);
        }
        if let Some(v) = self.wrap_trail_spaces {
            s.wrap_trail_spaces = v;
        }
        // An RTF document squeezes spaces unless it says `\jexpand`.
        s.rtf_justification = self.expand_justification != Some(true);
    }
}

/// Section formatting (`\sectd` resets it): the section's page words (`\pgwsxn`…, over the
/// document's `\paperw`…), its break, columns, page numbering and title page.
#[derive(Clone, Debug, Default)]
pub(super) struct SecFmt {
    pub(super) page: PageGeom,
    pub(super) start: wordcraft_doc::section::SectionStart,
    pub(super) cols: Option<i32>,
    pub(super) colsx: Option<i32>,
    pub(super) linebetcol: bool,
    /// `\colnoN` (1..=45) selects the column that `\colwN` / `\colsrN` describe; `col_dims`
    /// holds (width, space right) in twips per column, index 0 = column 1.
    pub(super) col_sel: Option<usize>,
    pub(super) col_dims: Vec<(Option<i32>, Option<i32>)>,
    pub(super) pgn_restart: bool,
    pub(super) pgn_start: Option<i32>,
    pub(super) pgn_fmt: wordcraft_doc::section::NumFormat,
    pub(super) title_page: bool,
    /// `\linemodN`: line numbering on (N >= 1), every Nth line; `\linex`, `\linestarts` and the
    /// restart word only count with it.
    pub(super) line_mod: Option<i32>,
    pub(super) line_x: Option<i32>,
    pub(super) line_starts: Option<i32>,
    pub(super) line_restart: Option<wordcraft_doc::section::LineNumberRestart>,
    /// `\rtlsect` / `\ltrsect`.
    pub(super) rtl: bool,
    /// `\stextflowN`: which way the section's text runs, as Word draws it.
    pub(super) text_flow: wordcraft_doc::props::TextDirection,
    /// `\pgnhnN` (heading level 1..=9) and its separator `\pgnhns…`.
    pub(super) chapter_style: Option<u8>,
    pub(super) chapter_sep: Option<wordcraft_doc::section::ChapterSep>,
    /// `\binfsxn` / `\binsxn`: printer bins for the first page and the other pages.
    pub(super) bin_first: Option<u16>,
    pub(super) bin_other: Option<u16>,
    /// The section's own footnote and endnote options (`\sftn…`, `\saftn…`).
    pub(super) footnote_pr: wordcraft_doc::section::NoteProps,
    pub(super) endnote_pr: wordcraft_doc::section::NoteProps,
    /// `\vertalt|c|j|b`: where the text sits between the top and bottom margins.
    pub(super) valign: wordcraft_doc::section::PageVAlign,
    /// `\sectspecifyl` / `\sectspecifycl`: the document grid kind; `\sectlinegridN` its pitch in
    /// twips. A pitch alone turns no grid on, so the kind decides whether there is one.
    pub(super) grid_kind: Option<wordcraft_doc::section::DocGridType>,
    pub(super) line_grid: Option<i32>,
    /// `\pgbrdrt` … `\pgbrdrr` with their border words: the page borders.
    pub(super) page_borders: wordcraft_doc::props::Borders,
    /// `\endnhere`: the section includes its endnotes (when they go at each section's end).
    pub(super) endnhere: bool,
}

impl SecFmt {
    /// The section's properties over the document's page (`doc`); headers and footers unset.
    pub(super) fn props(&self, doc: &PageGeom) -> wordcraft_doc::SectionProps {
        let p = &self.page;
        let page = PageGeom {
            w: p.w.or(doc.w),
            h: p.h.or(doc.h),
            left: p.left.or(doc.left),
            right: p.right.or(doc.right),
            top: p.top.or(doc.top),
            bottom: p.bottom.or(doc.bottom),
            header: p.header.or(doc.header),
            footer: p.footer.or(doc.footer),
            gutter: p.gutter.or(doc.gutter),
            rtl_gutter: doc.rtl_gutter,
            // A section that gives its own page size or orientation has its own orientation;
            // the document's `\landscape` only applies to sections that give neither.
            landscape: if p.w.is_some() || p.h.is_some() || p.landscape { p.landscape } else { doc.landscape },
        };
        let mut s = page.section();
        s.start = self.start;
        if let Some(n) = self.cols {
            s.columns.count = n.clamp(1, 45) as u32;
        }
        if let Some(x) = self.colsx {
            s.columns.space = (x as f32 / 20.0).clamp(0.0, 1584.0);
        }
        s.columns.separator = self.linebetcol;
        let count = s.columns.count as usize;
        if count > 1 {
            let widths: Option<Vec<(f32, f32)>> = (0..count)
                .map(|i| {
                    let (w, sr) = self.col_dims.get(i).copied().unwrap_or_default();
                    let pt = |t: i32| t.min(31_680) as f32 / 20.0;
                    w.map(|w| (pt(w), sr.map_or(0.0, pt)))
                })
                .collect();
            s.columns.widths = widths.unwrap_or_default();
        }
        if self.pgn_restart {
            s.page_num_start = Some(self.pgn_start.unwrap_or(1).clamp(0, 32_767) as u32);
        }
        s.page_num_format = self.pgn_fmt;
        s.title_page = self.title_page;
        if let Some(n) = self.line_mod.filter(|&n| n >= 1) {
            s.line_numbers = Some(wordcraft_doc::section::LineNumbering {
                count_by: n.clamp(1, 100) as u32,
                start: self.line_starts.unwrap_or(1).clamp(1, 32_768) as u32,
                distance: self.line_x.unwrap_or(0).max(0) as f32 / 20.0,
                restart: self.line_restart.unwrap_or_default(),
            });
        }
        s.rtl = self.rtl;
        s.text_direction = self.text_flow;
        s.chapter_style = self.chapter_style;
        s.chapter_sep = self.chapter_sep;
        s.paper_src_first = self.bin_first;
        s.paper_src_other = self.bin_other;
        s.footnote_pr = self.footnote_pr;
        s.endnote_pr = self.endnote_pr;
        s.valign = self.valign;
        if let Some(kind) = self.grid_kind {
            let line_pitch = match self.line_grid.filter(|&n| n > 0) {
                Some(n) => n as f32 / 20.0,
                None => wordcraft_doc::section::DocGrid::default().line_pitch,
            };
            s.doc_grid = Some(wordcraft_doc::section::DocGrid { kind, line_pitch, char_space: 0 });
        }
        let b = &self.page_borders;
        if [b.top, b.left, b.bottom, b.right].iter().any(Option::is_some) {
            s.page_borders = Some(*b);
        }
        s
    }
}

/// The note number formats RTF names (`\ftnn…`, `\aftnn…`, `\sftnn…`, `\saftnn…`), by the
/// word's ending.
const NOTE_FORMATS: [(&str, wordcraft_doc::section::NumFormat); 6] = {
    use wordcraft_doc::section::NumFormat;
    [
        ("ar", NumFormat::Decimal),
        ("alc", NumFormat::LowerLetter),
        ("auc", NumFormat::UpperLetter),
        ("rlc", NumFormat::LowerRoman),
        ("ruc", NumFormat::UpperRoman),
        ("chi", NumFormat::Chicago),
    ]
};

/// The ending of the note number format word for `f`; none for the formats RTF has no word for.
pub(super) fn note_format_word(f: wordcraft_doc::section::NumFormat) -> Option<&'static str> {
    NOTE_FORMATS.iter().find(|(_, g)| *g == f).map(|(w, _)| *w)
}

/// Applies footnote (`ftn…`) or endnote (`aftn…`) option word `w` (parameter `p`), without a
/// section word's leading `s`: placement (footnotes), start, restart and number format. A start
/// outside 1..=32767 and the number formats WordCraft has no RTF word for are ignored. False
/// when `w` isn't one of these words.
pub(super) fn apply_note_word(
    w: &str,
    p: Option<i32>,
    foot: &mut wordcraft_doc::section::NoteProps,
    end: &mut wordcraft_doc::section::NoteProps,
) -> bool {
    use wordcraft_doc::section::{NotePos, NoteRestart};
    let (props, rest, endnote) = match (w.strip_prefix("aftn"), w.strip_prefix("ftn")) {
        (Some(rest), _) => (end, rest, true),
        (None, Some(rest)) => (foot, rest, false),
        (None, None) => return false,
    };
    match rest {
        "bj" if !endnote => props.pos = Some(NotePos::PageBottom),
        "tj" if !endnote => props.pos = Some(NotePos::BeneathText),
        "start" => {
            if let Some(n) = p.filter(|n| (1..=32_767).contains(n)).and_then(|n| u32::try_from(n).ok()) {
                props.num_start = Some(n);
            }
        }
        "rstpg" if !endnote => props.num_restart = Some(NoteRestart::EachPage),
        "restart" => props.num_restart = Some(NoteRestart::EachSect),
        "rstcont" => props.num_restart = Some(NoteRestart::Continuous),
        _ => {
            let Some(fmt) = rest.strip_prefix('n') else {
                return false;
            };
            if let Some((_, f)) = NOTE_FORMATS.iter().find(|(w, _)| *w == fmt) {
                props.num_fmt = Some(*f);
            }
        }
    }
    true
}
