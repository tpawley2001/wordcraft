use super::{Align, Cell, FBlock, Fmt, Para, Rgb, VAlign};
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
    Bookmark,
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
    pub(super) saved: Box<Output>,
}

/// A note read: its kind and text.
pub(super) struct NoteData {
    pub(super) endnote: bool,
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
    pub(super) table_heights: Vec<Option<(f32, bool)>>,
    pub(super) table_keeps: Vec<bool>,
}

#[derive(Clone, Debug)]
pub(super) struct St {
    pub(super) dest: Dest,
    pub(super) fmt: Fmt,
    pub(super) rev: RevSt,
    /// Tracked change of the character formatting (reset by `\plain`). Its old formatting is
    /// the `\oldcprops` words, applied on top of the run's formatting when text is read: Word
    /// writes only the properties that changed there.
    pub(super) chg_c: FmtRevSt<OldWords>,
    /// While an `\oldcprops` group is read: its character formatting words, and the author and
    /// date of the change (`\crauth`/`\crdate`, which Word writes inside the group).
    pub(super) old_words: OldWords,
    pub(super) old_rev: (Option<i32>, Option<u32>),
    /// Tracked change of the paragraph formatting (reset by `\pard`).
    pub(super) chg_p: FmtRevSt<Para>,
    pub(super) uc: usize,
    pub(super) hidden: bool,
    // Paragraph properties.
    pub(super) align: Option<Align>,
    pub(super) style: i32,
    pub(super) intbl: bool,
    pub(super) ls: Option<i32>,
    pub(super) ilvl: u8,
    pub(super) outline: Option<u8>,
    pub(super) pagebb: bool,
    /// Widow/orphan control (`\widctlpar`, `\nowidctlpar`); `None` until the paragraph says.
    pub(super) widow: Option<bool>,
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
}

impl Default for St {
    fn default() -> Self {
        St {
            dest: Dest::Body,
            fmt: Fmt::default(),
            rev: RevSt::default(),
            chg_c: FmtRevSt::default(),
            chg_p: FmtRevSt::default(),
            old_words: Vec::new(),
            old_rev: (None, None),
            uc: 1,
            hidden: false,
            align: None,
            style: 0,
            intbl: false,
            ls: None,
            ilvl: 0,
            outline: None,
            pagebb: false,
            widow: None,
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
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct CellDef {
    pub(super) vmerge: u8,
    pub(super) hmerge: u8,
    pub(super) shading: Option<Rgb>,
    pub(super) right: i32,
    /// The borders the cell names (`\clbrdrt` … `\clbrdrr`, each with its line style, `\brdrw`
    /// width and `\brdrcf` colour).
    pub(super) borders: wordcraft_doc::props::Borders,
    pub(super) valign: Option<VAlign>,
    /// Turned text (`\cltxtbrl`, `\cltxbtlr`…).
    pub(super) direction: wordcraft_doc::props::TextDirection,
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
/// file doesn't give one (the setting keeps its default).
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
