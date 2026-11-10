//! Sections and headers/footers: `PlcfSed` → `Sed` → `Sepx` (grpprls of section sprms) and
//! the `Plcfhdd` stories of the header subdocument ([MS-DOC] §2.8.25, §2.9.244).

use wordcraft_doc::props::{Border, Borders};
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::section::{DocGrid, DocGridType, LineNumberRestart, LineNumbering, MAX_CHAR_SPACE, PageVAlign, SectionProps, SectionStart};

use crate::fib::{Fib, pair};
use crate::fmt;
use crate::sprm::{self, Prl};

/// Most sections we read.
const MAX_SECTIONS: usize = 1024;

// Section sprms ([MS-DOC] §2.6.4).
const S_F_EVENLY_SPACED: u16 = 0x3005; // sprmSFEvenlySpaced
const S_BKC: u16 = 0x3009;
const S_TITLE_PAGE: u16 = 0x300A; // sprmSFTitlePage
const S_CCOLUMNS: u16 = 0x500B;
const S_DXA_COLUMNS: u16 = 0x900C; // sprmSDxaColumns
const S_NFC_PGN: u16 = 0x300E; // sprmSNfcPgn
const S_PGN_RESTART: u16 = 0x3011; // sprmSFPgnRestart
const S_PGN_START: u16 = 0x501C; // sprmSPgnStart97
const S_L_BETWEEN: u16 = 0x3019; // sprmSLBetween
const S_VJC: u16 = 0x301A; // sprmSVjc
pub(crate) const S_ORIENTATION: u16 = 0x301D; // sprmSBOrientation
const S_DM_BIN_FIRST: u16 = 0x5007; // sprmSDmBinFirst
const S_DM_BIN_OTHER: u16 = 0x5008; // sprmSDmBinOther
const S_HDR_TOP: u16 = 0xB017;
const S_HDR_BOTTOM: u16 = 0xB018;
const S_XA_PAGE: u16 = 0xB01F;
const S_YA_PAGE: u16 = 0xB020;
const S_DXA_LEFT: u16 = 0xB021;
const S_DXA_RIGHT: u16 = 0xB022;
const S_DYA_TOP: u16 = 0x9023;
const S_DYA_BOTTOM: u16 = 0x9024;
const S_DXA_GUTTER: u16 = 0xB025;
const S_DXA_COL_WIDTH: u16 = 0xF203; // sprmSDxaColWidth
const S_DXA_COL_SPACING: u16 = 0xF204; // sprmSDxaColSpacing
const S_LNC: u16 = 0x3013; // sprmSLnc
const S_NLNN_MOD: u16 = 0x5015; // sprmSNLnnMod
const S_DXA_LNN: u16 = 0x9016; // sprmSDxaLnn
const S_LNN_MIN: u16 = 0x501B; // sprmSLnnMin
const S_CLM: u16 = 0x5032; // sprmSClm
const S_DYA_LINE_PITCH: u16 = 0x9031; // sprmSDyaLinePitch
const S_DXT_CHAR_SPACE: u16 = 0x7030; // sprmSDxtCharSpace
// Page borders ([MS-DOC] §2.6.4). The 80 forms carry a Brc80 (palette colour), the newer ones a
// BrcOperand: cb(1) + Brc(8, with a full COLORREF).
const S_BRC_TOP80: u16 = 0x702B; // sprmSBrcTop80
const S_BRC_LEFT80: u16 = 0x702C; // sprmSBrcLeft80
const S_BRC_BOTTOM80: u16 = 0x702D; // sprmSBrcBottom80
const S_BRC_RIGHT80: u16 = 0x702E; // sprmSBrcRight80
const S_BRC_TOP: u16 = 0xD234; // sprmSBrcTop
const S_BRC_LEFT: u16 = 0xD235; // sprmSBrcLeft
const S_BRC_BOTTOM: u16 = 0xD236; // sprmSBrcBottom
const S_BRC_RIGHT: u16 = 0xD237; // sprmSBrcRight

/// Most columns a section has ([MS-DOC] §2.6.4: iCol < 44).
const MAX_COLUMNS: usize = 44;

/// One section: covers paragraphs up to (excluding) `end_cp`, with its properties.
pub(crate) struct Section {
    pub(crate) end_cp: u32,
    pub(crate) props: SectionProps,
}

/// Parse `PlcfSed` and each section's `Sepx` grpprl.
pub(crate) fn parse(word: &[u8], table: &[u8], fib: &Fib) -> Vec<Section> {
    let Some((fc, lcb)) = fib.pair(pair::PLCF_SED) else { return Vec::new() };
    let Some(plc) = table.get(fc as usize..(fc as usize).saturating_add(lcb as usize)) else { return Vec::new() };
    parse_plc(plc, word, |props, pgn, grpprl| {
        let mut explicit_orient = false;
        for prl in sprm::iter(grpprl) {
            // Only a defined operand counts as explicit; anything else leaves the
            // orientation to the page shape.
            if prl.op == S_ORIENTATION && matches!(prl.operand.first(), Some(1 | 2)) {
                explicit_orient = true;
            }
            apply_section(props, pgn, &prl);
        }
        explicit_orient
    })
}

/// A section's page-number restart flag and start value, and its unequal column widths, resolved
/// once its whole grpprl is read (sprm order inside a Sepx is not fixed).
pub(crate) struct PageNumbering {
    restart: bool,
    start: Option<u32>,
    /// sprmSFEvenlySpaced = 0 seen (the default is evenly spaced).
    uneven: bool,
    /// sprmSDxaColWidth / sprmSDxaColSpacing per column, twips.
    col_widths: [Option<u16>; MAX_COLUMNS],
    col_spacing: [u16; MAX_COLUMNS],
    /// Line numbering: sprmSNLnnMod (0 = off), sprmSLnc, sprmSDxaLnn, sprmSLnnMin.
    lnn_mod: u16,
    lnc: Option<u8>,
    dxa_lnn: Option<i16>,
    lnn_min: Option<i16>,
}

impl Default for PageNumbering {
    fn default() -> Self {
        PageNumbering {
            restart: false,
            start: None,
            uneven: false,
            col_widths: [None; MAX_COLUMNS],
            col_spacing: [0; MAX_COLUMNS],
            lnn_mod: 0,
            lnc: None,
            dxa_lnn: None,
            lnn_min: None,
        }
    }
}

impl PageNumbering {
    /// Per-column (width, space after) in points, when the columns are unequal and every one of
    /// the `count` columns has a width; the last column has no space after it. Else empty.
    fn column_widths(&self, count: u32) -> Vec<(f32, f32)> {
        let n = count as usize;
        if !self.uneven || n < 2 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(n.min(MAX_COLUMNS));
        for i in 0..n {
            let Some(w) = self.col_widths.get(i).copied().flatten() else { return Vec::new() };
            let space = if i + 1 == n { 0 } else { self.col_spacing.get(i).copied().unwrap_or(0) };
            out.push((f32::from(w) / 20.0, f32::from(space) / 20.0));
        }
        out
    }

    /// The section's line numbering: on only when the count-by is non-zero, with the same clamps
    /// as DOCX `w:lnNumType` (lnnMin is the start minus one, like `w:start`).
    fn line_numbers(&self) -> Option<LineNumbering> {
        if self.lnn_mod == 0 {
            return None;
        }
        let d = LineNumbering::default();
        Some(LineNumbering {
            count_by: u32::from(self.lnn_mod).clamp(1, 100),
            start: self.lnn_min.map_or(d.start, |v| (v.max(0) as u32).saturating_add(1)),
            distance: self.dxa_lnn.map_or(d.distance, |v| f32::from(v.max(0)) / 20.0),
            restart: match self.lnc {
                Some(1) => LineNumberRestart::Section,
                Some(2) => LineNumberRestart::Continuous,
                Some(_) => LineNumberRestart::Page,
                None => d.restart,
            },
        })
    }
}

/// Walk a `PlcSed` (the same layout in Word 6/95 and Word 97) whose `Sepx`es live in `word`.
/// `apply` applies one section's grpprl and says whether it set the orientation explicitly.
pub(crate) fn parse_plc(plc: &[u8], word: &[u8], mut apply: impl FnMut(&mut SectionProps, &mut PageNumbering, &[u8]) -> bool) -> Vec<Section> {
    // PlcSed: aCp[n+1] u32s + aSed[n] (Sed = fn 2, fcSepx 4, fnMpr 2, fcMpr 4).
    if plc.len() < 4 || !(plc.len() - 4).is_multiple_of(16) {
        return Vec::new();
    }
    let n = (plc.len() - 4) / 16;
    let cp_at = |i: usize| -> Option<u32> { plc.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
    let mut out = Vec::with_capacity(n.min(MAX_SECTIONS));
    for i in 0..n.min(MAX_SECTIONS) {
        let Some(end_cp) = cp_at(i + 1) else { break };
        let sed_at = (n + 1) * 4 + i * 12;
        let fc_sepx = plc.get(sed_at + 2..sed_at + 6).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0xFFFF_FFFF);
        let mut props = SectionProps::default();
        // Sepx: cb u16, then a grpprl of section sprms.
        let mut explicit_orient = false;
        let mut pgn = PageNumbering::default();
        if let Some(grpprl) =
            word.get(fc_sepx as usize..).and_then(|sepx| sepx.get(..2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)).and_then(|cb| {
                let at = (fc_sepx as usize).checked_add(2)?;
                word.get(at..at.checked_add(cb)?)
            })
        {
            explicit_orient = apply(&mut props, &mut pgn, grpprl);
        }
        // Numbering restarts only where fPgnRestart is set (from 1 unless a start is given);
        // elsewhere it continues, whatever the start value. The first section begins numbering
        // anyway, so a start value there counts without the flag.
        if pgn.restart {
            props.page_num_start = Some(pgn.start.unwrap_or(1));
        } else if i == 0 {
            props.page_num_start = pgn.start;
        }
        props.columns.widths = pgn.column_widths(props.columns.count);
        props.line_numbers = pgn.line_numbers();
        // A section whose borders are all "no border" has none, as a document without them.
        props.page_borders = props.page_borders.filter(Borders::any_visible);
        // Without an explicit orientation sprm, infer it from the page shape.
        if !explicit_orient {
            if props.page_w < props.page_h {
                props.landscape = false;
            } else if props.page_w > props.page_h {
                props.landscape = true;
            }
        }
        out.push(Section { end_cp, props });
    }
    out
}

/// Collect a line-numbering sprm; the numbering is resolved once the whole Sepx is read.
fn apply_line_numbering(pgn: &mut PageNumbering, prl: &Prl) {
    let s16 = match prl.operand {
        [b0, b1, ..] => Some(i16::from_le_bytes([*b0, *b1])),
        _ => None,
    };
    match prl.op {
        S_LNC => pgn.lnc = prl.operand.first().copied().or(pgn.lnc),
        S_NLNN_MOD => pgn.lnn_mod = s16.map_or(pgn.lnn_mod, |v| v as u16),
        S_DXA_LNN => pgn.dxa_lnn = s16.or(pgn.dxa_lnn),
        S_LNN_MIN => pgn.lnn_min = s16.or(pgn.lnn_min),
        _ => {}
    }
}

/// Apply a document-grid sprm: any of them turns the grid on; the rest keep `DocGrid`'s defaults.
fn apply_doc_grid(s: &mut SectionProps, prl: &Prl) {
    match (prl.op, prl.operand) {
        (S_CLM, [b0, b1, ..]) => {
            s.doc_grid.get_or_insert_with(DocGrid::default).kind = match u16::from_le_bytes([*b0, *b1]) {
                1 => DocGridType::LinesAndChars,
                2 => DocGridType::Lines,
                3 => DocGridType::SnapToChars,
                _ => DocGridType::Default,
            };
        }
        (S_DYA_LINE_PITCH, [b0, b1, ..]) => {
            s.doc_grid.get_or_insert_with(DocGrid::default).line_pitch = f32::from(i16::from_le_bytes([*b0, *b1])) / 20.0;
        }
        (S_DXT_CHAR_SPACE, [b0, b1, b2, b3, ..]) => {
            s.doc_grid.get_or_insert_with(DocGrid::default).char_space =
                i32::from_le_bytes([*b0, *b1, *b2, *b3]).clamp(-MAX_CHAR_SPACE, MAX_CHAR_SPACE);
        }
        _ => {}
    }
}

/// Put a decoded page border on the side its sprm names; the last sprm for a side wins.
fn set_page_border(s: &mut SectionProps, op: u16, b: Border) {
    let borders = s.page_borders.get_or_insert_with(Borders::default);
    let slot = match op {
        S_BRC_TOP80 | S_BRC_TOP => &mut borders.top,
        S_BRC_LEFT80 | S_BRC_LEFT => &mut borders.left,
        S_BRC_BOTTOM80 | S_BRC_BOTTOM => &mut borders.bottom,
        _ => &mut borders.right,
    };
    *slot = Some(b);
}

/// Apply a section sprm to the model's section properties. All the measurement sprms here
/// carry 16-bit twip operands ([MS-DOC] §2.6.4). The page-number restart flag and start value
/// go to `pgn`, as they only count together.
pub(crate) fn apply_section(s: &mut SectionProps, pgn: &mut PageNumbering, prl: &Prl) {
    let tw = |v: i32| (v as f32 / 20.0).clamp(0.0, 1584.0);
    let s16 = |prl: &Prl| -> Option<i16> {
        match prl.operand {
            [b0, b1, ..] => Some(i16::from_le_bytes([*b0, *b1])),
            _ => None,
        }
    };
    match prl.op {
        S_BKC => {
            s.start = match prl.operand.first().copied().unwrap_or(2) {
                // SBkcOperand ([MS-DOC] sprmSBkc): bkcContinuous, bkcNewColumn, bkcNewPage,
                // bkcEvenPage, bkcOddPage; anything else falls back to the default (new page).
                0 => SectionStart::Continuous,
                1 => SectionStart::NextColumn,
                3 => SectionStart::EvenPage,
                4 => SectionStart::OddPage,
                _ => SectionStart::NextPage,
            };
        }
        S_TITLE_PAGE => s.title_page = prl.operand.first().is_some_and(|&v| v != 0),
        S_CCOLUMNS => {
            if let Some(c) = s16(prl) {
                s.columns.count = (c as u32).saturating_add(1).min(44);
            }
        }
        S_DXA_COLUMNS => {
            if let [b0, b1, ..] = prl.operand {
                s.columns.space = f32::from(u16::from_le_bytes([*b0, *b1])) / 20.0;
            }
        }
        S_F_EVENLY_SPACED => {
            if let Some(&v) = prl.operand.first() {
                pgn.uneven = v == 0;
            }
        }
        S_DXA_COL_WIDTH | S_DXA_COL_SPACING => {
            if let [i, b0, b1, ..] = prl.operand {
                let v = u16::from_le_bytes([*b0, *b1]);
                // Columns past the section's count are dropped when the widths are resolved.
                if prl.op == S_DXA_COL_WIDTH {
                    if let Some(w) = pgn.col_widths.get_mut(usize::from(*i)) {
                        *w = Some(v);
                    }
                } else if let Some(sp) = pgn.col_spacing.get_mut(usize::from(*i)) {
                    *sp = v;
                }
            }
        }
        S_L_BETWEEN => {
            if let Some(&v) = prl.operand.first() {
                s.columns.separator = v != 0;
            }
        }
        // Vjc: 0 top, 1 center, 2 justified, 3 bottom; anything else is top.
        S_VJC => {
            if let Some(&v) = prl.operand.first() {
                s.valign = match v {
                    1 => PageVAlign::Center,
                    2 => PageVAlign::Justify,
                    3 => PageVAlign::Bottom,
                    _ => PageVAlign::Top,
                };
            }
        }
        S_NFC_PGN => {
            s.page_num_format = match prl.operand.first() {
                // nfc: 0 Arabic, 1 upper Roman, 2 lower Roman, 3 upper letter, 4 lower letter.
                Some(1) => NumFormat::UpperRoman,
                Some(2) => NumFormat::LowerRoman,
                Some(3) => NumFormat::UpperLetter,
                Some(4) => NumFormat::LowerLetter,
                _ => NumFormat::Decimal,
            };
        }
        S_LNC | S_NLNN_MOD | S_DXA_LNN | S_LNN_MIN => apply_line_numbering(pgn, prl),
        S_CLM | S_DYA_LINE_PITCH | S_DXT_CHAR_SPACE => apply_doc_grid(s, prl),
        S_BRC_TOP80 | S_BRC_LEFT80 | S_BRC_BOTTOM80 | S_BRC_RIGHT80 => {
            if let Some(b) = fmt::brc80(prl.operand) {
                set_page_border(s, prl.op, b);
            }
        }
        S_BRC_TOP | S_BRC_LEFT | S_BRC_BOTTOM | S_BRC_RIGHT => {
            if let Some(b) = fmt::brc(prl.operand) {
                set_page_border(s, prl.op, b);
            }
        }
        S_PGN_RESTART => pgn.restart = prl.operand.first().is_some_and(|&v| v != 0),
        S_PGN_START => {
            if let [b0, b1, ..] = prl.operand {
                pgn.start = Some(u32::from(u16::from_le_bytes([*b0, *b1])));
            }
        }
        // Printer paper source (the DMBIN code, as DOCX `w:paperSrc`); 0 is the SEP default.
        S_DM_BIN_FIRST | S_DM_BIN_OTHER => {
            if let [b0, b1, ..] = prl.operand {
                let bin = Some(u16::from_le_bytes([*b0, *b1])).filter(|&v| v != 0);
                if prl.op == S_DM_BIN_FIRST {
                    s.paper_src_first = bin;
                } else {
                    s.paper_src_other = bin;
                }
            }
        }
        // SBOrientationOperand ([MS-DOC] §2.9.236): 1 portrait, 2 landscape.
        S_ORIENTATION => match prl.operand.first() {
            Some(1) => s.landscape = false,
            Some(2) => s.landscape = true,
            _ => {}
        },
        S_HDR_TOP => {
            if let Some(v) = s16(prl) {
                s.header = tw(v as i32);
            }
        }
        S_HDR_BOTTOM => {
            if let Some(v) = s16(prl) {
                s.footer = tw(v as i32);
            }
        }
        S_XA_PAGE => {
            if let Some(v) = s16(prl) {
                s.page_w = tw(v as i32).max(7.2);
            }
        }
        S_YA_PAGE => {
            if let Some(v) = s16(prl) {
                s.page_h = tw(v as i32).max(7.2);
            }
        }
        S_DXA_LEFT => {
            if let Some(v) = s16(prl) {
                s.margin_left = tw(v as i32);
            }
        }
        S_DXA_RIGHT => {
            if let Some(v) = s16(prl) {
                s.margin_right = tw(v as i32);
            }
        }
        S_DYA_TOP => {
            if let Some(v) = s16(prl) {
                // Signed ([MS-DOC] §2.6.4): a negative top margin doesn't make room for the header.
                s.margin_top = tw((v as i32).abs());
                s.top_fixed = v < 0;
            }
        }
        S_DYA_BOTTOM => {
            if let Some(v) = s16(prl) {
                s.margin_bottom = tw((v as i32).abs());
                s.bottom_fixed = v < 0;
            }
        }
        S_DXA_GUTTER => {
            if let Some(v) = s16(prl) {
                s.gutter = tw(v as i32);
            }
        }
        _ => {}
    }
}

/// The header stories' CP ranges from `Plcfhdd` (a PLC of bare CPs): story i spans
/// `[cps[i], cps[i+1])`. Stories 0-5 are footnote/endnote separators; every section then owns
/// six stories in the order even header, odd (default) header, even footer, odd (default)
/// footer, first header, first footer. The CPs are relative to the header subdocument.
///
/// The CPs of a well-formed PLC ascend, so the stories are disjoint. A story starting before
/// the end of an earlier one (a hostile table pointing many sections at the same text, each
/// read into its own part) is left empty, like a slot linked to the previous section's:
/// every subdocument CP becomes at most one story.
pub(crate) fn header_stories(table: &[u8], fib: &Fib) -> Vec<(u32, u32)> {
    let Some((fc, lcb)) = fib.pair(pair::PLCF_HDD) else { return Vec::new() };
    let Some(plc) = table.get(fc as usize..(fc as usize).saturating_add(lcb as usize)) else { return Vec::new() };
    // Only the six separator stories and six per section (at most 1024) are ever used.
    let n = (lcb as usize / 4).min(6 + 6 * MAX_SECTIONS + 1);
    let cp_at = |i: usize| -> Option<u32> { plc.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
    let mut stories = Vec::with_capacity(n);
    let mut kept_to = 0u32;
    for i in 0..n.saturating_sub(1) {
        if let (Some(a), Some(b)) = (cp_at(i), cp_at(i + 1)) {
            let b = if a < kept_to { a } else { b.max(a) };
            kept_to = kept_to.max(b);
            stories.push((a, b));
        }
    }
    stories
}
