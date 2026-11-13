//! Tables: the table sprms (sgc 5) carried by a row-end (TTP) mark's grpprl, and the
//! assembly of cell-ended paragraphs into [`Table`] blocks ([MS-DOC] §2.6.3, §2.4.3).

use std::sync::Arc;

use wordcraft_doc::Rgb;
use wordcraft_doc::para::{Anchor, FloatAlign, Paragraph};
use wordcraft_doc::props::{Align, Border, Borders, CellProps, HeightRule, ParaProps, RowProps, TableFloat, TableProps, TextFrame, VAlign, VMerge};
use wordcraft_doc::table::{Cell, Row, Table};
use wordcraft_doc::{Block, Blocks, para_block};

use crate::fmt;
use crate::sprm::{self, Prl};

// Paragraph sprms for table membership.
const P_IN_TABLE: u16 = 0x2416;
const P_TTP: u16 = 0x2417;
/// `sprmPItap`: table depth (0 = not in a table).
const P_ITAP: u16 = 0x6649;
/// `sprmPFInnerTableCell` / `sprmPFInnerTtp`: the (ordinary) mark ends a cell / a row of a
/// table at depth 2 or more; the row-end mark carries that row's TAP.
const P_INNER_CELL: u16 = 0x244B;
const P_INNER_TTP: u16 = 0x244C;

// Table sprms ([MS-DOC] §2.6.3).
const T_JC: u16 = 0x5400;
const T_JC_90: u16 = 0x548A;
const T_DXA_LEFT: u16 = 0x9601;
const T_DXA_GAP_HALF: u16 = 0x9602;
const T_TABLE_HEADER: u16 = 0x3404;
const T_TABLE_BORDERS_80: u16 = 0xD605;
/// `sprmTTableBorders`: the same six borders as 8-byte `Brc`s ([MS-DOC] 2.6.3).
const T_TABLE_BORDERS: u16 = 0xD613;
const T_ROW_HEIGHT: u16 = 0x9407;
const T_DEF_TABLE: u16 = 0xD608;
const T_DEF_TABLE_SHD_80: u16 = 0xD609;
/// `sprmTDefTableShd` / `Shd2nd` / `Shd3rd`: 10-byte `Shd`s for cells 0-21, 22-43, 44-62.
const T_DEF_TABLE_SHD: u16 = 0xD612;
const T_DEF_TABLE_SHD_2ND: u16 = 0xD616;
const T_DEF_TABLE_SHD_3RD: u16 = 0xD60C;
/// `sprmTDefTableShdRaw` / `Raw2nd` / `Raw3rd`: the same `Shd` arrays over the same cell
/// ranges, which Word 2007+ writes next to the above.
const T_DEF_TABLE_SHD_RAW: u16 = 0xD670;
const T_DEF_TABLE_SHD_RAW_2ND: u16 = 0xD671;
const T_DEF_TABLE_SHD_RAW_3RD: u16 = 0xD672;
const T_CANT_SPLIT_90: u16 = 0x3403;
const T_MERGE: u16 = 0x5624;
const T_VERT_MERGE: u16 = 0xD62B;
const T_VERT_ALIGN: u16 = 0xD62C;
const T_SET_SHD: u16 = 0xD62D;
const T_CANT_SPLIT: u16 = 0x3466;
const T_CELL_WIDTH: u16 = 0xD635;
/// `sprmTTableWidth`: the table's preferred width, an `FtsWWidth_TablePart` ([MS-DOC] 2.6.4).
const T_TABLE_WIDTH: u16 = 0xF614;
/// `sprmTCellPadding` / `sprmTCellPaddingDefault`: a CSSA operand giving cell margins for a
/// cell range / the whole table ([MS-DOC] §2.6.4, §2.9.43).
const T_CELL_PADDING: u16 = 0xD632;
const T_CELL_PADDING_DEFAULT: u16 = 0xD634;
// Floating-table positioning ([MS-DOC] §2.6.4): the TAP counterparts of the paragraph frame
// sprms, with the same operands (PositionCodeOperand, XAS, YAS, twips).
const T_PC: u16 = 0x360D;
const T_DXA_ABS: u16 = 0x940E;
const T_DYA_ABS: u16 = 0x940F;
const T_DXA_FROM_TEXT: u16 = 0x9410;
const T_DYA_FROM_TEXT: u16 = 0x9411;
const T_DXA_FROM_TEXT_RIGHT: u16 = 0x941E;
const T_DYA_FROM_TEXT_BOTTOM: u16 = 0x941F;
const T_F_NO_ALLOW_OVERLAP: u16 = 0x3465;
/// `sprmTFAutofit` ([MS-DOC] 2.6.3): 1 lets the row's columns fit their content.
const T_F_AUTOFIT: u16 = 0x3615;

/// Most grid columns we build from a `sprmTDefTable` (the model's own cap is 63; one more
/// edge is fine but merged rows can sum spans slightly over, so clamp when assembling).
const MAX_COLS: usize = 63;

/// What a paragraph mark says about its table context, from its PAPX.
#[derive(Default)]
pub(crate) struct RowInfo {
    pub(crate) in_table: bool,
    pub(crate) ttp: bool,
    /// The mark itself was 0x07 (a cell end), set by the walker.
    pub(crate) cell_mark: bool,
    /// Table depth from `sprmPItap` (0 when absent: a depth-1 table).
    pub(crate) depth: u32,
    /// `sprmPFInnerTableCell`: ends a cell of a nested table.
    pub(crate) inner_cell: bool,
    /// `sprmPFInnerTtp`: ends a row of a nested table.
    pub(crate) inner_ttp: bool,
    /// Cell-edge positions (points) from `sprmTDefTable` — [MS-DOC] defines them per row,
    /// so each row keeps its own; ascending in well-formed rows, the first edge usually 0
    /// (the row origin). Rows without a `sprmTDefTable` keep this empty.
    pub(crate) edges: Vec<f32>,
    height: Option<f32>,
    height_rule: HeightRule,
    header: bool,
    cant_split: bool,
    align: Option<Align>,
    indent: Option<f32>,
    borders: Option<Borders>,
    /// Default cell margins `[top, left, bottom, right]` (points), from
    /// `sprmTCellPaddingDefault` with `sprmTDxaGapHalf` for an unset left or right side.
    cell_margins: Option<[f32; 4]>,
    /// The table's preferred width from `sprmTTableWidth`.
    width: Option<TableWidth>,
    /// Per-cell margin overrides from `sprmTCellPadding`, one slot per cell.
    cell_padding: Vec<(u8, [Option<f32>; 4])>,
    /// Horizontal merges: (itcFirst, itcLim) cell-index ranges.
    merges: Vec<(u8, u8)>,
    /// Horizontal merges from the `sprmTDefTable` TC80s; a `sprmTMerge` range wins where they
    /// overlap.
    tc_merges: Vec<(u8, u8)>,
    /// Per-cell overrides keyed by (pre-merge) cell index.
    vmerge: Vec<(u8, VMerge)>,
    /// Vertical merges from the `sprmTDefTable` TC80s; `vmerge` wins for a cell it names.
    tc_vmerge: Vec<(u8, VMerge)>,
    /// Later entries win: the `Shd` arrays override the `Shd80` fallback before them, and an
    /// automatic `Shd` (`None`) clears it.
    shading: Vec<(u8, Option<Rgb>)>,
    widths: Vec<(u8, f32)>,
    width_pcts: Vec<(u8, f32)>,
    valign: Vec<(u8, VAlign)>,
    /// Floating-table sprms from the TAP, as the paragraph frame sprm (opcode) each matches.
    tap_frame: Vec<(u16, [u8; 2])>,
    /// `sprmTDxaFromTextRight` / `sprmTDyaFromTextBottom`, points.
    tap_dist_right: Option<f32>,
    tap_dist_bottom: Option<f32>,
    tap_no_overlap: Option<bool>,
    /// `sprmTFAutofit` with operand 1: the row's columns fit their content. Without it the
    /// stored cell widths are final (the table has fixed column widths).
    autofit: bool,
}

/// A table's preferred width as `sprmTTableWidth` gives it.
#[derive(Clone, Copy)]
enum TableWidth {
    Auto,
    Points(f32),
    Percent(f32),
}

/// Decode a `sprmTTableWidth` operand: ftsWidth (1 byte, an Fts, [MS-DOC] 2.9.102) then
/// wWidth (2 bytes). ftsDxa (3) and ftsDxaSys (0x13) are twips, ftsPercent (2) is 50ths of
/// a percent; ftsNil (0) and ftsAuto (1) leave the width automatic. A truncated operand or
/// another fts gives `None`.
fn table_width(operand: &[u8]) -> Option<TableWidth> {
    let [fts, w0, w1, ..] = operand else { return None };
    match *fts {
        0x03 | 0x13 => Some(TableWidth::Points((i16::from_le_bytes([*w0, *w1]) as f32 / 20.0).clamp(0.0, 1584.0))),
        0x02 => Some(TableWidth::Percent((u16::from_le_bytes([*w0, *w1]) as f32 / 50.0).clamp(0.0, 100.0))),
        0x00 | 0x01 => Some(TableWidth::Auto),
        _ => None,
    }
}

/// Read the table context out of a paragraph mark's PAPX grpprl.
impl RowInfo {
    /// Does the mark belong to a table: inside one, ending a row, or ending a cell?
    pub(crate) fn is_table(&self) -> bool {
        self.in_table || self.ttp || self.cell_mark
    }

    /// The row height from `sprmTDyaRowHeight`, for tests.
    #[cfg(test)]
    pub(crate) fn height(&self) -> Option<f32> {
        self.height
    }

    /// The fill cell `i` ends up with (the last shading entry for it wins), for tests.
    #[cfg(test)]
    pub(crate) fn shading_of(&self, i: u8) -> Option<Rgb> {
        self.shading.iter().rfind(|(k, _)| *k == i).and_then(|(_, s)| *s)
    }

    /// The width cell `i` ends up with, for tests.
    #[cfg(test)]
    pub(crate) fn width_of(&self, i: u8) -> Option<f32> {
        self.widths.iter().rfind(|(k, _)| *k == i).map(|(_, w)| *w)
    }

    /// How many per-cell overrides the row keeps, for tests.
    #[cfg(test)]
    pub(crate) fn overrides(&self) -> usize {
        self.vmerge.len()
            + self.tc_vmerge.len()
            + self.cell_padding.len()
            + self.shading.len()
            + self.widths.len()
            + self.width_pcts.len()
            + self.valign.len()
            + self.merges.len()
            + self.tc_merges.len()
    }

    /// The width percentage cell `i` ends up with, for tests.
    #[cfg(test)]
    pub(crate) fn width_pct_of(&self, i: u8) -> Option<f32> {
        self.width_pcts.iter().rfind(|(k, _)| *k == i).map(|(_, w)| *w)
    }
}

/// Set cell `c`'s override to `v`, replacing an earlier one: one slot per cell, however often
/// a PAPX repeats the sprm, so a shared PAPX can't pile up copies on every paragraph mark.
fn set<T>(slots: &mut Vec<(u8, T)>, c: u8, v: T) {
    match slots.iter_mut().find(|(k, _)| *k == c) {
        Some(slot) => slot.1 = v,
        None => slots.push((c, v)),
    }
}

/// The first file version ([`crate::fib::Fib::version`]) written by Word 2002: from here on
/// `sprmTFCantSplit90` is kept only for Word 97/2000 and `sprmTFCantSplit` alone counts.
pub(crate) const VERSION_2002: u16 = 0x0101;
/// Word 97's `nFib`.
#[cfg(test)]
const VERSION_97: u16 = 0x00C1;

/// The table properties of a paragraph mark, as a Word 97 file would carry them.
#[cfg(test)]
pub(crate) fn decode(papx: &[u8]) -> RowInfo {
    decode_in(papx, VERSION_97, None)
}

/// The table properties of a paragraph mark in a file of `version`. `prc` holds the sprms of the
/// row's `sprmPTableProps` copy in the Data stream, when it has a readable one: that copy alone
/// decides whether the row may break across pages; everything else comes from `papx`.
pub(crate) fn decode_in(papx: &[u8], version: u16, prc: Option<&[u8]>) -> RowInfo {
    let mut r = RowInfo::default();
    let mut gap_half: Option<i16> = None;
    // The left edge of the row's first cell, twips from the margin, as the row's sprms set it
    // in order: `sprmTDefTable`'s rgdxaCenter[0], or `sprmTDxaLeft` less the final gap half.
    let mut edge0: Option<Edge0> = None;
    let mut padding: [Option<f32>; 4] = [None; 4];
    for prl in sprm::iter(papx) {
        // The membership sprms are paragraph sprms (sgc 1); the rest here are table sprms
        // (sgc 5), so match opcodes directly.
        match prl.op {
            P_IN_TABLE => r.in_table = prl.operand.first().copied().unwrap_or(0) != 0,
            P_TTP => r.ttp = prl.operand.first().copied().unwrap_or(0) != 0,
            P_INNER_CELL => r.inner_cell = prl.operand.first().copied().unwrap_or(0) != 0,
            P_INNER_TTP => r.inner_ttp = prl.operand.first().copied().unwrap_or(0) != 0,
            P_ITAP => r.depth = i32_of(&prl).unwrap_or(0).max(0) as u32,
            T_DEF_TABLE => {
                r.edges = def_table_edges(prl.operand);
                r.tc_vmerge = def_table_vmerge(prl.operand);
                r.tc_merges = def_table_hmerge(prl.operand);
                edge0 = first_edge(prl.operand).map(Edge0::Center).or(edge0);
                // The definition replaces each cell's alignment; a later sprmTVertAlign wins.
                apply_def_table_valign(&mut r.valign, prl.operand);
            }
            T_TABLE_HEADER => r.header = prl.operand.first().copied().unwrap_or(0) != 0,
            // Read after the loop, from the Data-stream copy when there is one.
            T_CANT_SPLIT | T_CANT_SPLIT_90 => {}
            T_ROW_HEIGHT => {
                if let Some(v) = s16(&prl) {
                    if v < 0 {
                        r.height_rule = HeightRule::Exact;
                        r.height = Some((-(v as f32) / 20.0).clamp(0.0, 1584.0));
                    } else {
                        r.height_rule = HeightRule::AtLeast;
                        r.height = Some((v as f32 / 20.0).clamp(0.0, 1584.0));
                    }
                }
            }
            T_JC | T_JC_90 => {
                r.align = match u16_of(&prl).unwrap_or(0) {
                    1 => Some(Align::Center),
                    2 => Some(Align::Right),
                    _ => Some(Align::Left),
                };
            }
            T_DXA_LEFT => edge0 = s16(&prl).map(Edge0::DxaLeft).or(edge0),
            T_DXA_GAP_HALF => gap_half = s16(&prl),
            // The row's own table borders; when a row has both sprms the later one wins.
            T_TABLE_BORDERS_80 => r.borders = table_borders(prl.operand),
            T_TABLE_BORDERS => r.borders = table_borders_brc(prl.operand),
            T_MERGE => {
                // Rows apply only the first MAX_COLS merges.
                if let [a, b, ..] = prl.operand
                    && b > a
                    && r.merges.len() < MAX_COLS
                {
                    r.merges.push((*a, *b));
                }
            }
            T_VERT_MERGE => {
                // cb, itc, flag: 1 = continues the merge from above, 3 = restarts it.
                if let [_, itc, flag, ..] = prl.operand {
                    let m = match flag {
                        3 => VMerge::Restart,
                        1 => VMerge::Continue,
                        _ => VMerge::None,
                    };
                    // The first override for a cell wins.
                    if m != VMerge::None && !r.vmerge.iter().any(|(k, _)| k == itc) {
                        r.vmerge.push((*itc, m));
                    }
                }
            }
            T_SET_SHD => {
                if let Some(rgb) = prl.operand.get(3..).and_then(shd_rgb)
                    && let [a, b, ..] = prl.operand.get(1..3).unwrap_or(&[])
                {
                    for c in *a..(*b).min(MAX_COLS as u8) {
                        set(&mut r.shading, c, Some(rgb));
                    }
                }
            }
            T_DEF_TABLE_SHD_80 => {
                let cells = prl.operand.get(1..).unwrap_or(&[]);
                for (i, b) in cells.as_chunks::<2>().0.iter().take(MAX_COLS).enumerate() {
                    if let Some(rgb) = shd80_rgb(b) {
                        set(&mut r.shading, i as u8, Some(rgb));
                    }
                }
            }
            T_DEF_TABLE_SHD | T_DEF_TABLE_SHD_RAW | T_DEF_TABLE_SHD_2ND | T_DEF_TABLE_SHD_RAW_2ND | T_DEF_TABLE_SHD_3RD | T_DEF_TABLE_SHD_RAW_3RD => {
                // Each array covers its own cell range (0-21, 22-43, 44-62); extra entries
                // are ignored so an over-long array can't override the next range.
                let (first, lim) = match prl.op {
                    T_DEF_TABLE_SHD_2ND | T_DEF_TABLE_SHD_RAW_2ND => (22, 44),
                    T_DEF_TABLE_SHD_3RD | T_DEF_TABLE_SHD_RAW_3RD => (44, MAX_COLS),
                    _ => (0, 22),
                };
                let cells = prl.operand.get(1..).unwrap_or(&[]);
                let raw = matches!(prl.op, T_DEF_TABLE_SHD_RAW | T_DEF_TABLE_SHD_RAW_2ND | T_DEF_TABLE_SHD_RAW_3RD);
                for (i, b) in cells.as_chunks::<10>().0.iter().take(lim - first).enumerate() {
                    // In the Raw arrays a nil Shd (ipat 0xFFFF) means "as the table style
                    // shades it", so it leaves any earlier fill alone.
                    if raw && b.get(8..10) == Some(&[0xFF, 0xFF]) {
                        continue;
                    }
                    set(&mut r.shading, (first + i) as u8, shd_rgb(b));
                }
            }
            T_TABLE_WIDTH => r.width = table_width(prl.operand).or(r.width),
            T_CELL_WIDTH => {
                // Operand: cb (1 byte), itcFirst (1 byte), itcLim (1 byte), ftsWidth (1 byte), wWidth (2 bytes).
                // [MS-DOC] §2.9.290 sprmTCellWidth (0xD635).
                // ftsWidth: 3 = twips (dxa), 2 = percentage (50ths of a percent).
                if let [_, a, b, fts, w0, w1, ..] = prl.operand {
                    if *fts == 3 {
                        let w = (i16::from_le_bytes([*w0, *w1]) as f32 / 20.0).clamp(0.0, 1584.0);
                        for c in *a..(*b).min(MAX_COLS as u8) {
                            set(&mut r.widths, c, w);
                        }
                    } else if *fts == 2 {
                        let pct = (u16::from_le_bytes([*w0, *w1]) as f32 / 50.0).clamp(0.0, 100.0);
                        for c in *a..(*b).min(MAX_COLS as u8) {
                            set(&mut r.width_pcts, c, pct);
                        }
                    }
                }
            }
            T_CELL_PADDING_DEFAULT => {
                if let Some((_, _, sides)) = cssa(prl.operand) {
                    merge_sides(&mut padding, &sides);
                }
            }
            T_CELL_PADDING => set_cell_padding(&mut r.cell_padding, prl.operand),
            T_PC | T_DXA_ABS | T_DYA_ABS | T_DXA_FROM_TEXT | T_DYA_FROM_TEXT => {
                let op = match prl.op {
                    T_PC => fmt::P_PC,
                    T_DXA_ABS => fmt::P_DXA_ABS,
                    T_DYA_ABS => fmt::P_DYA_ABS,
                    T_DXA_FROM_TEXT => fmt::P_DXA_FROM_TEXT,
                    _ => fmt::P_DYA_FROM_TEXT,
                };
                let operand = match prl.operand {
                    [a, b, ..] => [*a, *b],
                    [a] => [*a, 0],
                    [] => continue,
                };
                // Bounded: one slot per sprm, a later one replacing it.
                r.tap_frame.retain(|(o, _)| *o != op);
                r.tap_frame.push((op, operand));
            }
            T_DXA_FROM_TEXT_RIGHT => r.tap_dist_right = s16(&prl).map(dist_pt),
            T_DYA_FROM_TEXT_BOTTOM => r.tap_dist_bottom = s16(&prl).map(dist_pt),
            T_F_NO_ALLOW_OVERLAP => r.tap_no_overlap = prl.operand.first().map(|b| *b != 0),
            T_F_AUTOFIT => r.autofit = prl.operand.first() == Some(&1),
            T_VERT_ALIGN => {
                if let [_, a, b, va, ..] = prl.operand {
                    let v = match va {
                        1 => VAlign::Center,
                        2 => VAlign::Bottom,
                        _ => VAlign::Top,
                    };
                    for c in *a..(*b).min(MAX_COLS as u8) {
                        set(&mut r.valign, c, v);
                    }
                }
            }
            _ => {}
        }
    }
    r.cell_margins = cell_margins(padding, gap_half);
    r.indent = edge0.and_then(|e| row_indent(e, gap_half, r.cell_margins));
    r.cant_split = cant_split(prc.unwrap_or(papx), version);
    r
}

/// "Don't break this row across pages" from `grpprl`: `sprmTFCantSplit` in every version (a later
/// one overrides an earlier one), `sprmTFCantSplit90` only in Word 97/2000 files, where it can set
/// the flag but never clear one `sprmTFCantSplit` set.
fn cant_split(grpprl: &[u8], version: u16) -> bool {
    let (mut by_new, mut by_90) = (false, false);
    for prl in sprm::iter(grpprl) {
        let on = prl.operand.first().copied().unwrap_or(0) != 0;
        match prl.op {
            T_CANT_SPLIT => by_new = on,
            T_CANT_SPLIT_90 if version < VERSION_2002 => by_90 = on,
            _ => {}
        }
    }
    by_new || by_90
}

/// The table indent a row says, in points: where the first cell's text starts (Word 97-2003
/// compatibility, which every `.doc` is in), the first cell's edge plus the table's default
/// left cell margin. `None` within half a twip of the margin: an ordinary table.
fn row_indent(edge0: Edge0, gap_half: Option<i16>, margins: Option<[f32; 4]>) -> Option<f32> {
    let edge = match edge0 {
        Edge0::Center(c) => i32::from(c),
        Edge0::DxaLeft(l) => i32::from(l) - i32::from(gap_half.unwrap_or(0)),
    };
    let left = margins.map_or(0.0, |m| m[1]);
    let indent = (edge as f32 / 20.0 + left).clamp(-1584.0, 1584.0);
    (indent.abs() >= 0.025).then_some(indent)
}

/// Where a row's first cell edge came from (see `decode`).
#[derive(Clone, Copy)]
enum Edge0 {
    /// `sprmTDefTable`'s rgdxaCenter[0].
    Center(i16),
    /// `sprmTDxaLeft`, the edge plus the gap half.
    DxaLeft(i16),
}

/// rgdxaCenter[0] of a `sprmTDefTable` operand (cb as u16, itcMac, rgdxaCenter…), twips; `None`
/// for a row with no cells or a truncated operand.
fn first_edge(operand: &[u8]) -> Option<i16> {
    if operand.get(2).copied().unwrap_or(0) == 0 {
        return None;
    }
    match operand.get(3..5) {
        Some([a, b]) => Some(i16::from_le_bytes([*a, *b])),
        _ => None,
    }
}

/// The table's default cell margins: the padding sides set, with the gap half as the left and
/// right margin wherever the padding Prls leave them. `None` when neither is given.
fn cell_margins(padding: [Option<f32>; 4], gap_half: Option<i16>) -> Option<[f32; 4]> {
    let gap = gap_half.map(dist_pt);
    if padding.iter().all(Option::is_none) && gap.is_none() {
        return None;
    }
    let [top, left, bottom, right] = padding;
    Some([top.unwrap_or(0.0), left.or(gap).unwrap_or(0.0), bottom.unwrap_or(0.0), right.or(gap).unwrap_or(0.0)])
}

/// Apply a `sprmTCellPadding` operand to its cell range, one slot per cell.
fn set_cell_padding(slots: &mut Vec<(u8, [Option<f32>; 4])>, operand: &[u8]) {
    let Some((a, b, sides)) = cssa(operand) else { return };
    for c in a..b.min(MAX_COLS as u8) {
        match slots.iter_mut().find(|(k, _)| *k == c) {
            Some(slot) => merge_sides(&mut slot.1, &sides),
            None => slots.push((c, sides)),
        }
    }
}

/// A CSSA operand (after its cb byte): itcFirst, itcLim, grfbrc, ftsWidth, wWidth. Returns the
/// cell range and the margin (points) for each side grfbrc names, in `[top, left, bottom,
/// right]` order; `None` when truncated or the width isn't in twips (ftsNil and the rest).
fn cssa(operand: &[u8]) -> Option<(u8, u8, [Option<f32>; 4])> {
    let [_, first, lim, grfbrc, fts, w0, w1, ..] = operand else { return None };
    if *fts != 3 {
        return None;
    }
    let w = (f32::from(u16::from_le_bytes([*w0, *w1])) / 20.0).clamp(0.0, 1584.0);
    let mut sides = [None; 4];
    // grfbrc bits: top, left (start), bottom, right (end).
    for (i, bit) in [0x01, 0x02, 0x04, 0x08].into_iter().enumerate() {
        if grfbrc & bit != 0
            && let Some(s) = sides.get_mut(i)
        {
            *s = Some(w);
        }
    }
    Some((*first, *lim, sides))
}

/// Set each side `new` gives, keeping the rest: a later Prl wins for the same side.
fn merge_sides(into: &mut [Option<f32>; 4], new: &[Option<f32>; 4]) {
    for (d, s) in into.iter_mut().zip(new) {
        if s.is_some() {
            *d = *s;
        }
    }
}

/// A distance from the text in twips as points, clamped like the frame distances.
fn dist_pt(v: i16) -> f32 {
    (f32::from(v) / 20.0).clamp(0.0, 1584.0)
}

/// The table position a row says: its paragraph frame (`frame`, from the row's TTP or first
/// cell paragraph) with the TAP's floating-table sprms over it, field by field, mapped as
/// `w:tblpPr` is (ECMA-376 §17.4.58). `None` when neither positions the table.
fn row_float(frame: Option<TextFrame>, info: &RowInfo) -> Option<TableFloat> {
    // Only the position sprms float a table; distances and overlap only refine one.
    let positioned = info.tap_frame.iter().any(|(op, _)| matches!(*op, fmt::P_PC | fmt::P_DXA_ABS | fmt::P_DYA_ABS));
    if frame.is_none() && !positioned {
        return None;
    }
    let mut p = ParaProps { frame, ..Default::default() };
    for (op, operand) in &info.tap_frame {
        fmt::apply_frame(&mut p, &Prl { op: *op, operand });
    }
    let f = p.frame?;
    let anchor = |a: &str, text: Anchor| match a {
        "page" => Anchor::Page,
        "margin" => Anchor::Margin,
        _ => text,
    };
    let h_align = match f.x_align.as_deref() {
        Some("left") => Some(FloatAlign::Start),
        Some("center") => Some(FloatAlign::Center),
        Some("right") => Some(FloatAlign::End),
        Some("inside") => Some(FloatAlign::Inside),
        Some("outside") => Some(FloatAlign::Outside),
        _ => None,
    };
    let v_align = match f.y_align.as_deref() {
        Some("top") => Some(FloatAlign::Start),
        Some("center") => Some(FloatAlign::Center),
        Some("bottom") => Some(FloatAlign::End),
        Some("inside") => Some(FloatAlign::Inside),
        Some("outside") => Some(FloatAlign::Outside),
        _ => None,
    };
    Some(TableFloat {
        h_rel: anchor(&f.h_anchor, Anchor::Column),
        v_rel: anchor(&f.v_anchor, Anchor::Paragraph),
        x: f.x,
        y: f.y,
        h_align,
        v_align,
        dist: [f.h_space, f.v_space, info.tap_dist_right.unwrap_or(f.h_space), info.tap_dist_bottom.unwrap_or(f.v_space)],
        overlap: !info.tap_no_overlap.unwrap_or(false),
    })
}

/// A finished paragraph plus the bookkeeping the block assembly needs.
pub(crate) struct ParaOut {
    pub(crate) para: Paragraph,
    /// CP just past the terminating mark.
    pub(crate) end_cp: u32,
    /// Table context from the mark's PAPX; `None` for a paragraph outside a table.
    pub(crate) row: Option<Box<RowInfo>>,
}

/// Group consecutive in-table paragraphs into [`Table`] blocks; the rest pass through.
/// A table at depth 2 or more opens inside the current cell of its parent, in order with
/// the parent's paragraphs. Inner rows that never close, and anything nested deeper than
/// [`MAX_DEPTH`], degrade to paragraphs in the enclosing cell so no text is lost.
pub(crate) fn assemble(paras: Vec<ParaOut>) -> Blocks {
    let mut out: Blocks = Vec::new();
    // Open tables, outermost (depth 1) first; never longer than MAX_DEPTH.
    let mut open: Vec<TableBuilder> = Vec::new();
    for mut p in paras {
        let Some(info) = p.row.take() else {
            close_to(&mut open, 0, &mut out);
            out.push(para_block(p.para));
            continue;
        };
        // In a table the frame sprms position the table, not each paragraph: the first row's
        // say where the table floats, and no paragraph in it keeps a frame of its own.
        let frame = p.para.props.frame.take();
        let depth = (info.depth as usize).max(1);
        let level = depth.min(MAX_DEPTH);
        close_to(&mut open, level, &mut out);
        while open.len() < level {
            // Only the outermost table floats from the paragraph frame.
            let first_frame = if open.is_empty() { frame.clone() } else { None };
            open.push(TableBuilder { first_frame, ..Default::default() });
        }
        let Some(b) = open.last_mut() else { continue };
        if depth > MAX_DEPTH {
            // Too deep to build: the paragraph joins the deepest cell we do build.
            b.cell.push(para_block(p.para));
            continue;
        }
        if level == 1 {
            b.mark(p.para, frame, &info, info.cell_mark, info.ttp);
        } else {
            // Inner cells and rows end with ordinary paragraph marks carrying
            // sprmPFInnerTableCell / sprmPFInnerTtp; the latter holds the row's TAP.
            b.mark(p.para, None, &info, info.inner_cell || info.inner_ttp || info.cell_mark, info.inner_ttp);
        }
    }
    close_to(&mut open, 0, &mut out);
    out
}

/// Deepest table nesting built from a `.doc`, as the DOCX reader allows; paragraphs deeper
/// than this join the cell at this depth, which bounds the builder stack on hostile depths.
const MAX_DEPTH: usize = 24;

/// Finish every open table deeper than `depth`, innermost first: each goes into the current
/// cell of its parent, the outermost into `out`.
fn close_to(open: &mut Vec<TableBuilder>, depth: usize, out: &mut Blocks) {
    while open.len() > depth {
        let Some(b) = open.pop() else { break };
        match open.last_mut() {
            Some(parent) => b.finish_inner(&mut parent.cell),
            None => b.finish_outer(out),
        }
    }
}

/// One table being assembled from its paragraph marks.
#[derive(Default)]
struct TableBuilder {
    rows: Vec<Row>,
    // Each row's own cell edges, parallel to `rows` (a row without a `sprmTDefTable` has none).
    row_edges: Vec<Vec<f32>>,
    cells: Vec<Cell>,
    cell: Blocks,
    grid: Vec<f32>,
    // Table props are per-table (first row carrying the sprm wins within the table): a new
    // builder per table means one table's borders, align, indent and cell margins never leak
    // into the next (the repro's first table wrote Word's default 108-twip gap, and every
    // later table — including the change history with its own 40-twip gap — inherited 5.4 pt
    // margins).
    props: TableProps,
    /// A row's `sprmTTableWidth` has been taken (even an automatic one): later rows keep it.
    width_set: bool,
    /// The frame of the table's first paragraph, the fallback when its first row-end has none.
    first_frame: Option<TextFrame>,
    /// Some row-end carried `sprmTFAutofit` 1: the table fits its columns to their content.
    autofit: bool,
}

impl TableBuilder {
    /// Add one paragraph mark: `cell_end` says it ends a cell, `row_end` that it ends a row
    /// (with `info` carrying that row's TAP).
    fn mark(&mut self, para: Paragraph, frame: Option<TextFrame>, info: &RowInfo, cell_end: bool, row_end: bool) {
        if row_end && self.rows.is_empty() {
            self.props.float = row_float(frame.or_else(|| self.first_frame.take()), info);
        }
        if self.grid.is_empty() {
            // Column widths from the table's first `sprmTDefTable`.
            self.grid = info.edges.windows(2).take(MAX_COLS).map(|w| (w[1] - w[0]).max(0.0)).collect();
        }
        if self.props.align.is_none() {
            self.props.align = info.align;
        }
        if self.props.indent.is_none() {
            self.props.indent = info.indent;
        }
        if !self.width_set
            && let Some(w) = info.width
        {
            self.width_set = true;
            match w {
                TableWidth::Points(pt) => self.props.width = Some(pt),
                TableWidth::Percent(pct) => self.props.width_pct = Some(pct),
                TableWidth::Auto => {}
            }
        }
        if self.props.cell_margins.is_none() {
            self.props.cell_margins = info.cell_margins;
        }
        if !cell_end {
            self.cell.push(para_block(para));
            return;
        }
        // A row-end (TTP) mark terminates the row. Word writes rows as N content
        // cells each ended by their own cell mark, followed by an empty row-end mark
        // that carries the row's TDefTable (the 3GPP change history: 8 content marks
        // + an empty TTP whose TDefTable defines itcMac = 8). When that mark ends no
        // content of its own and the TDefTable already has a column per flushed cell,
        // it is a pure row terminator, not the end of a phantom (N+1)th cell — such a
        // cell has no column (width None) and shifts every real cell's span one grid
        // column over. A TTP mark with content, or one short of the TDefTable's
        // columns, still ends a real cell (rows without a TDefTable keep today's
        // shape: the count is unknowable, so the mark always flushes).
        let row_end_only =
            row_end && para.text.is_empty() && self.cell.is_empty() && !info.edges.is_empty() && self.cells.len() + 1 >= info.edges.len();
        // ...but a row-end mark with no cells before it still makes a (one-cell) row.
        let row_end_only = row_end_only && !self.cells.is_empty();
        // The cell mark is the paragraph mark of the cell's last paragraph ([MS-DOC] 2.4.3), so
        // an empty one is still a paragraph: a blank last line after the cell's text, or an
        // empty cell's only paragraph with its own style and formatting. After a nested table
        // that just closed into the cell an empty mark adds nothing (not measured in Word).
        let after_table = self.cell.last().is_some_and(|b| b.as_table().is_some());
        if !row_end_only && (!para.text.is_empty() || !after_table) {
            self.cell.push(para_block(para));
        }
        if !row_end_only {
            flush_cell(&mut self.cells, &mut self.cell);
        }
        if row_end {
            let mut row = build_row(std::mem::take(&mut self.cells), info);
            // Each row carries its own table borders: the table takes the first row's, and a
            // later row whose borders differ keeps them as its exceptions (`w:tblPrEx`), so a
            // borderless row under a bordered first row draws no grid.
            if self.rows.is_empty() {
                self.props.borders = info.borders;
            } else if info.borders != self.props.borders {
                let own = info.borders.unwrap_or_else(|| Borders::all(Border::default()));
                row.props.exceptions = Some(Box::new(TableProps { borders: Some(own), ..Default::default() }));
            }
            self.rows.push(row);
            self.row_edges.push(info.edges.clone());
            self.autofit |= info.autofit;
        }
    }

    /// The outermost table: a stray trailing row (no TTP of its own) is kept as a row.
    fn finish_outer(mut self, out: &mut Blocks) {
        if !self.cells.is_empty() || !self.cell.is_empty() {
            flush_cell(&mut self.cells, &mut self.cell);
            // A stray trailing row: size its cells from the grid.
            let mut stray = build_row(std::mem::take(&mut self.cells), &RowInfo::default());
            for (i, c) in stray.cells.iter_mut().enumerate() {
                if c.props.width.is_none() && c.props.span == 1 {
                    c.props.width = self.grid.get(i).copied();
                }
            }
            self.rows.push(stray);
            self.row_edges.push(Vec::new());
        }
        self.push_table(out);
    }

    /// A nested table, into its parent's cell: cells after its last closed row keep their
    /// text as paragraphs there (a row that never closed has no row properties to build).
    fn finish_inner(mut self, out: &mut Blocks) {
        let cells = std::mem::take(&mut self.cells);
        let cell = std::mem::take(&mut self.cell);
        self.push_table(out);
        for c in cells {
            out.extend(c.blocks);
        }
        out.extend(cell);
    }

    fn push_table(mut self, out: &mut Blocks) {
        if self.rows.is_empty() {
            return;
        }
        let aligned = align_rows(&mut self.rows, &self.row_edges);
        let grid = aligned.unwrap_or(self.grid);
        let mut table = Table { props: self.props, grid, rows: self.rows, controls: Default::default() };
        // When the rows' edges don't line up the grid comes from the first row's `sprmTDefTable`
        // only, so a later row with more cells has columns the grid doesn't size; layout then splits them evenly, and a
        // preferred width would stretch those even columns across the table instead of letting
        // the content autofit them (a text column a fifth of the table, one word per line).
        // Such a table stays automatic.
        if !table.grid.is_empty() && table.cols() > table.grid.len() {
            table.props.width = None;
            table.props.width_pct = None;
        } else if !self.autofit && !table.grid.is_empty() {
            // No row asks to autofit (`sprmTFAutofit` 1), so the stored cell widths are final:
            // the table has fixed column widths, and layout must not re-fit them to the text.
            table.props.fixed = true;
        }
        out.push(Arc::new(Block::Table(table)));
    }
}

/// Close the open cell: its blocks (an empty paragraph when it has none) go onto the row's cells.
fn flush_cell(cells: &mut Vec<Cell>, cell: &mut Blocks) {
    let mut blocks = std::mem::take(cell);
    if blocks.is_empty() {
        blocks.push(para_block(Paragraph::new()));
    }
    cells.push(Cell { props: CellProps { span: 1, ..Default::default() }, blocks, controls: Default::default() });
}

/// Turn a row's collected cells into a [`Row`]: the per-cell sprm overrides, a width from the
/// row's cell edges for cells that have none, then the horizontal merges.
fn build_row(mut cells: Vec<Cell>, info: &RowInfo) -> Row {
    // Per-cell overrides first (sprm cell indices count pre-merge cells), then merges.
    for (i, c) in cells.iter_mut().enumerate() {
        let i = i as u8;
        if let Some((_, m)) = info.vmerge.iter().chain(&info.tc_vmerge).find(|(k, _)| *k == i) {
            c.props.vmerge = *m;
        }
        if let Some((_, w)) = info.widths.iter().rfind(|(k, _)| *k == i) {
            c.props.width = Some(*w);
        }
        if let Some((_, pct)) = info.width_pcts.iter().rfind(|(k, _)| *k == i) {
            c.props.width_pct = Some(*pct);
        }
        // A cell with no width of its own is as wide as the row's `sprmTDefTable` edges make it,
        // the width Word's own docx export writes on every cell. Without one, a table that asks
        // for no width anywhere is laid out by its content alone and shrinks.
        if c.props.width.is_none() && c.props.width_pct.is_none() {
            c.props.width = edge_width(&info.edges, usize::from(i));
        }
        if let Some((_, s)) = info.shading.iter().rfind(|(k, _)| *k == i) {
            c.props.shading = *s;
        }
        if let Some((_, v)) = info.valign.iter().rfind(|(k, _)| *k == i) {
            c.props.valign = Some(*v);
        }
        if let Some((_, m)) = info.cell_padding.iter().find(|(k, _)| *k == i) {
            c.props.partial_margins = Some(*m);
        }
    }
    // Rightmost merge first: a merge drains only the cells after its first one, so the original
    // indices of the merges to its left stay valid.
    // The `sprmTMerge` ranges, then each TC80 range that overlaps none of them.
    let tc = info.tc_merges.iter().filter(|&&(a2, b2)| !info.merges.iter().any(|&(a1, b1)| a1 < b2 && a2 < b1));
    let mut merges: Vec<(u8, u8)> = info.merges.iter().chain(tc).take(MAX_COLS).copied().collect();
    merges.sort_by_key(|m| std::cmp::Reverse(m.0));
    for &(a, b) in &merges {
        let a = a as usize;
        let b = (b as usize).min(cells.len());
        if a < b && b <= cells.len() && b - a > 1 {
            // A merged cell is as wide as the cells it spans together.
            // When one of them has only a percentage width, the row's outer edges give the sum.
            let merged_width =
                cells.get(a..b).and_then(|s| s.iter().map(|c| c.props.width).sum::<Option<f32>>()).or_else(|| edge_span(&info.edges, a, b));
            let removed: Vec<Cell> = cells.drain(a + 1..b).collect();
            if let Some(first) = cells.get_mut(a) {
                first.props.span = ((b - a) as u32).min(63);
                if merged_width.is_some() {
                    first.props.width = merged_width;
                }
                // Only a first cell that was one empty paragraph gives it up to the content the
                // merged-away cells bring; a first cell with more paragraphs keeps them all.
                let lone_empty = first.blocks.len() == 1 && first.blocks.first().and_then(|x| x.as_para()).is_some_and(Paragraph::is_empty);
                for c in removed {
                    first.blocks.extend(c.blocks.into_iter().filter(|x| x.as_para().is_none_or(|p| !p.is_empty())));
                }
                if lone_empty && first.blocks.len() > 1 {
                    first.blocks.remove(0);
                }
            }
        }
    }
    Row {
        props: RowProps {
            height: info.height,
            height_rule: info.height_rule,
            header: info.header,
            cant_split: info.cant_split,
            ..Default::default()
        },
        cells,
        controls: Default::default(),
    }
}

/// Cell-edge positions (points) from a `sprmTDefTable` operand: cb u16, itcMac u8, then
/// itcMac+1 twip boundaries followed by the TC80s (read by `def_table_valign`). Edges are returned as stored:
/// per-row, in operand order; sorting and capping happen when the table grid is assembled.
fn def_table_edges(operand: &[u8]) -> Vec<f32> {
    let itc = operand.get(2).copied().unwrap_or(0) as usize;
    if itc == 0 {
        return Vec::new();
    }
    // itcMac is a u8, so this allocation stays tiny; the loop stops at the operand's end.
    let mut edges = Vec::with_capacity((itc + 1).min(256));
    for i in 0..=itc {
        match operand.get(3 + i * 2..3 + i * 2 + 2) {
            Some(b) => edges.push(i16::from_le_bytes([b[0], b[1]]) as f32 / 20.0),
            None => break,
        }
    }
    edges
}

/// Each cell's vertical merge from the TC80s of a `sprmTDefTable` operand: vertMerge is bits
/// 5-6 of a TC80's TCGRF word ([MS-DOC] 2.9.313, 2.9.314): 1 merges the cell with the one
/// above, 3 (and 2, which has the first-cell bit alone) starts a merged set. Only cells below
/// MAX_COLS with a complete TC80 count; unmerged cells are left out.
fn def_table_vmerge(operand: &[u8]) -> Vec<(u8, VMerge)> {
    let itc = operand.get(2).copied().unwrap_or(0) as usize;
    let start = 3 + (itc + 1) * 2;
    let mut out = Vec::new();
    for i in 0..itc.min(MAX_COLS) {
        let at = start + i * 20;
        let Some([lo, hi, ..]) = operand.get(at..at + 20) else { break };
        let m = match (u16::from_le_bytes([*lo, *hi]) >> 5) & 3 {
            1 => VMerge::Continue,
            2 | 3 => VMerge::Restart,
            _ => continue,
        };
        out.push((i as u8, m));
    }
    out
}

/// Each horizontally merged set of cells from the TC80s of a `sprmTDefTable` operand, as
/// (itcFirst, itcLim) ranges: TCGRF bit 0 (fFirstMerged) starts a set and bit 1 (fMerged) adds
/// the cell to the set the nearest first cell to its left started ([MS-DOC] 2.9.313, 2.9.314).
/// A merged cell with no set open stays an ordinary cell; a cell with neither bit closes the
/// set. Only cells below MAX_COLS with a complete TC80 count.
fn def_table_hmerge(operand: &[u8]) -> Vec<(u8, u8)> {
    let itc = operand.get(2).copied().unwrap_or(0) as usize;
    let start = 3 + (itc + 1) * 2;
    let mut out: Vec<(u8, u8)> = Vec::new();
    let mut open: Option<u8> = None;
    for i in 0..itc.min(MAX_COLS) {
        let at = start + i * 20;
        let Some([lo, hi, ..]) = operand.get(at..at + 20) else { break };
        let flags = u16::from_le_bytes([*lo, *hi]);
        let i = i as u8;
        if flags & 2 != 0 {
            if let Some(a) = open {
                match out.last_mut() {
                    Some(last) if last.0 == a => last.1 = i + 1,
                    _ => out.push((a, i + 1)),
                }
            }
        } else if flags & 1 != 0 {
            open = Some(i);
        } else {
            open = None;
        }
    }
    out
}

/// Record the alignments a `sprmTDefTable` operand's TC80s give its cells, replacing earlier
/// ones; a cell with none from the definition loses any earlier override.
fn apply_def_table_valign(slots: &mut Vec<(u8, VAlign)>, operand: &[u8]) {
    for (c, v) in def_table_valign(operand) {
        match v {
            Some(v) => set(slots, c, v),
            None => slots.retain(|(k, _)| *k != c),
        }
    }
}

/// The width between `sprmTDefTable` edges `a` and `b`, when they give a usable one.
fn edge_span(edges: &[f32], a: usize, b: usize) -> Option<f32> {
    let w = edges.get(b)? - edges.get(a)?;
    (w.is_finite() && w > 0.0).then_some(w)
}

/// The width of cell `i` between its `sprmTDefTable` edges, when they give a usable one.
fn edge_width(edges: &[f32], i: usize) -> Option<f32> {
    edge_span(edges, i, i + 1)
}

/// How far (points) two rows' cell edges may differ and still be the same column boundary.
const EDGE_TOL: f32 = 0.5;

/// A row of a .doc table carries its own cell edges, while the model's rows share one grid. The
/// grid is every row's edges together (two wide cells under a row of four narrow ones, or rows
/// whose edges only partly coincide), and each cell spans the grid columns its own edges cover.
/// `None` when the rows agree (the first row's grid then stands), or when a row has no usable edges
/// or its cells can't be mapped onto the grid: the grid and spans then stay as they were.
fn align_rows(rows: &mut [Row], row_edges: &[Vec<f32>]) -> Option<Vec<f32>> {
    if rows.is_empty() || rows.len() != row_edges.len() {
        return None;
    }
    if row_edges.iter().any(|e| e.len() < 2 || e.iter().any(|x| !x.is_finite()) || e.windows(2).any(|w| w[0] >= w[1])) {
        return None;
    }
    // The grid's boundaries: every row's edges, those within `EDGE_TOL` of each other as one.
    let mut all: Vec<f32> = row_edges.iter().flatten().copied().collect();
    all.sort_by(f32::total_cmp);
    let mut bounds: Vec<f32> = Vec::new();
    for e in all {
        if bounds.last().is_none_or(|&l| e - l > EDGE_TOL) {
            bounds.push(e);
        }
    }
    if bounds.len() > MAX_COLS + 1 {
        return None;
    }
    let col_of = |e: f32| bounds.iter().position(|f| (f - e).abs() <= EDGE_TOL);
    let mut cols_of_row: Vec<Vec<usize>> = Vec::with_capacity(row_edges.len());
    for edges in row_edges {
        let cols = edges.iter().map(|&e| col_of(e)).collect::<Option<Vec<usize>>>()?;
        if !cols.windows(2).all(|w| w[0] < w[1]) {
            return None;
        }
        cols_of_row.push(cols);
    }
    if cols_of_row.iter().all(|c| c.len() == bounds.len()) {
        return None;
    }
    // Work out every row's spans before changing any, so a row that can't be mapped leaves all as they were.
    let mut plans: Vec<(Vec<u32>, u32, u32)> = Vec::with_capacity(rows.len());
    for (row, cols) in rows.iter().zip(&cols_of_row) {
        let mut at = 0usize;
        let mut spans = Vec::with_capacity(row.cells.len());
        for c in &row.cells {
            let span = c.props.span.max(1) as usize;
            let (Some(&l), Some(&r)) = (cols.get(at), cols.get(at + span)) else { return None };
            spans.push(((r - l) as u32).min(63));
            at += span;
        }
        let first = cols.first().copied().unwrap_or(0);
        let last = cols.get(at).copied().unwrap_or(first);
        plans.push((spans, first as u32, (bounds.len() - 1).saturating_sub(last) as u32));
    }
    for (row, (spans, before, after)) in rows.iter_mut().zip(plans) {
        for (c, s) in row.cells.iter_mut().zip(spans) {
            c.props.span = s;
        }
        row.props.grid_before = before;
        row.props.grid_after = after;
    }
    Some(bounds.windows(2).map(|w| w[1] - w[0]).collect())
}

/// Each cell's vertical alignment from the TC80s of a `sprmTDefTable` operand: after the
/// itcMac+1 edges come up to itcMac 20-byte TC80s whose first word is the TCGRF flags, with
/// vertAlign in bits 7-8 ([MS-DOC] 2.9.313, 2.9.314). Yields every cell below MAX_COLS: Center
/// or Bottom when its TC80 says so, None (no explicit alignment) for top, the undefined value
/// 3, or a TC80 missing from the operand or cut short.
fn def_table_valign(operand: &[u8]) -> impl Iterator<Item = (u8, Option<VAlign>)> + '_ {
    let itc = operand.get(2).copied().unwrap_or(0) as usize;
    let start = 3 + (itc + 1) * 2;
    (0..itc.min(MAX_COLS)).map(move |i| {
        let at = start + i * 20;
        let v = operand.get(at..at + 20).and_then(|tc| match tc {
            [lo, hi, ..] => match (u16::from_le_bytes([*lo, *hi]) >> 7) & 3 {
                1 => Some(VAlign::Center),
                2 => Some(VAlign::Bottom),
                _ => None,
            },
            _ => None,
        });
        (i as u8, v)
    })
}

/// Table borders from `sprmTTableBorders80`: cb (0x18) then six Brc80s in the order top,
/// left, bottom, right, inside-horizontal, inside-vertical, read as paragraph borders are.
/// A truncated operand gives the row no table borders.
fn table_borders(operand: &[u8]) -> Option<Borders> {
    let six = operand.get(1..25)?;
    let b = |i: usize| six.get(i * 4..i * 4 + 4).and_then(fmt::brc80);
    Some(Borders { top: b(0), left: b(1), bottom: b(2), right: b(3), between: b(4), inside_v: b(5) })
}

/// Table borders from `sprmTTableBorders`: cb (0x30) then six 8-byte `Brc`s in the same order
/// as [`table_borders`]. A truncated operand gives the row no table borders.
fn table_borders_brc(operand: &[u8]) -> Option<Borders> {
    let six = operand.get(1..49)?;
    let b = |i: usize| {
        let brc = six.get(i * 8..i * 8 + 8)?;
        // `fmt::brc` reads a `BrcOperand`: cb (8) then the Brc.
        let op: Vec<u8> = std::iter::once(8).chain(brc.iter().copied()).collect();
        let mut border = fmt::brc(&op)?;
        // cv `00 00 00 FF` is cvAuto ([MS-DOC] 2.9.43 COLORREF): the automatic colour.
        if brc.get(..4) == Some(&[0, 0, 0, 0xFF]) {
            border.color = None;
        }
        Some(border)
    };
    Some(Borders { top: b(0), left: b(1), bottom: b(2), right: b(3), between: b(4), inside_v: b(5) })
}

/// Background colour from a 10-byte `Shd`: cvFore, cvBack (COLORREFs), ipat. Solid patterns
/// show the foreground; clear (0) and the other patterns show the background (Word writes a
/// plain cell fill as clear with cvBack); nil (0xFFFF) and automatic colours are no fill.
fn shd_rgb(b: &[u8]) -> Option<Rgb> {
    let cv = |b: &[u8]| -> Option<Rgb> {
        match b {
            [_, _, _, 0xFF] => None, // fAuto
            [r, g, bl, _] => Some(Rgb(*r, *g, *bl)),
            _ => None,
        }
    };
    let ipat = b.get(8..10).map(|x| u16::from_le_bytes([x[0], x[1]]))?;
    match ipat {
        0xFFFF => None,
        1 => cv(b.get(0..4)?),
        _ => cv(b.get(4..8)?),
    }
}

/// Background colour from a 2-byte `Shd80` (icoFore, icoBack, ipat bit-fields). As in
/// [`shd_rgb`], solid shows the foreground and clear (0) the background, which is how Word
/// 97 writes a plain fill; ico 0 is automatic (no fill).
fn shd80_rgb(b: &[u8]) -> Option<Rgb> {
    let v = u16::from_le_bytes([*b.first()?, *b.get(1)?]);
    let fore = v & 0x1F;
    let back = (v >> 5) & 0x1F;
    let ipat = v >> 10;
    if fore == 0x1F && back == 0x1F {
        return None;
    }
    fmt::ico(if ipat == 1 { fore as u8 } else { back as u8 })
}

fn u16_of(p: &Prl) -> Option<u16> {
    match p.operand {
        [b0, b1, ..] => Some(u16::from_le_bytes([*b0, *b1])),
        _ => None,
    }
}

fn s16(p: &Prl) -> Option<i16> {
    u16_of(p).map(|v| v as i16)
}

fn i32_of(p: &Prl) -> Option<i32> {
    match p.operand {
        [b0, b1, b2, b3, ..] => Some(i32::from_le_bytes([*b0, *b1, *b2, *b3])),
        _ => None,
    }
}
