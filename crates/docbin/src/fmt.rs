//! Sprms → model properties, and style-chain resolution.

use wordcraft_doc::Rgb;
use wordcraft_doc::props::{
    Align, Border, BorderStyle, Borders, CharProps, Highlight, LineSpacing, NumRef, ParaProps, TabAlign, TabLeader, TabStop, TextColor, TextFrame,
    Underline, VertAlign,
};
use wordcraft_doc::styles::{Style, StyleKind, StyleSheet};

use crate::sprm::{self, Prl, Toggle};

// Character sprms ([MS-DOC] §2.6.1).
const C_BOLD: u16 = 0x0835;
const C_ITALIC: u16 = 0x0836;
const C_STRIKE: u16 = 0x0837;
const C_SMALL_CAPS: u16 = 0x083A;
const C_CAPS: u16 = 0x083B;
const C_HIDDEN: u16 = 0x083C; // sprmCFVanish
const C_KUL: u16 = 0x2A3E; // underline style (Kul)
const C_ICO: u16 = 0x2A42; // palette colour
const C_CR: u16 = 0x6870; // explicit COLORREF
const C_HPS: u16 = 0x4A43; // half-points
const C_HPS_POS: u16 = 0x4845; // raised (+) / lowered (-) by signed half-points: w:position
const C_ISS: u16 = 0x2A48; // superscript / subscript: w:vertAlign
const C_FTC0: u16 = 0x4A4F; // font index (ASCII)
const C_FTC1: u16 = 0x4A50; // East Asian font index (sprmCRgFtc1)
const C_FTC_BI: u16 = 0x4A5E; // complex-script font index (sprmCFtcBi)
const C_HIGHLIGHT: u16 = 0x2A0C;
const C_DXA_SPACE: u16 = 0x8840; // sprmCDxaSpace: letter spacing in twips
const C_CHAR_SCALE: u16 = 0x4852; // sprmCCharScale: horizontal scale in percent
const C_HPS_KERN: u16 = 0x484B; // sprmCHpsKern: kern from this size up, half-points
pub(crate) const C_ISTD: u16 = 0x4A30; // character style

// Paragraph sprms ([MS-DOC] §2.6.2).
// Frame (absolute position) sprms.
pub(crate) const P_PC: u16 = 0x261B;
pub(crate) const P_DXA_ABS: u16 = 0x8418;
pub(crate) const P_DYA_ABS: u16 = 0x8419;
const P_DXA_WIDTH: u16 = 0x841A;
const P_WR: u16 = 0x2423;
const P_WHEIGHT_ABS: u16 = 0x442B;
pub(crate) const P_DYA_FROM_TEXT: u16 = 0x842E;
pub(crate) const P_DXA_FROM_TEXT: u16 = 0x842F;
const P_JC: u16 = 0x2461;
const P_JC80: u16 = 0x2403;
const P_DXA_LEFT: u16 = 0x845E;
const P_DXA_LEFT80: u16 = 0x840F;
const P_DXA_RIGHT: u16 = 0x845D;
const P_DXA_RIGHT80: u16 = 0x840E;
const P_DXA_FIRST: u16 = 0x8460; // sprmPDxaLeft1
const P_DXA_FIRST80: u16 = 0x8411; // sprmPDxaLeft1_80
// Indents in hundredths of a character (sprmPDxcRight, sprmPDxcLeft, sprmPDxcLeft1).
const P_DXC_RIGHT: u16 = 0x4455;
const P_DXC_LEFT: u16 = 0x4456;
const P_DXC_FIRST: u16 = 0x4457;
const P_DYA_BEFORE: u16 = 0xA413;
const P_DYA_AFTER: u16 = 0xA414;
// Automatic (HTML) space before/after, Bool8 (sprmPFDyaBeforeAuto, sprmPFDyaAfterAuto).
const P_F_DYA_BEFORE_AUTO: u16 = 0x245B;
const P_F_DYA_AFTER_AUTO: u16 = 0x245C;
const P_DYA_LINE: u16 = 0x6412;
const P_KEEP: u16 = 0x2405; // keep lines together
const P_KEEP_FOLLOW: u16 = 0x2406; // keep with next
const P_PAGE_BREAK_BEFORE: u16 = 0x2407;
const P_OUTLVL: u16 = 0x2640;
// Paragraph borders ([MS-DOC] §2.6.2). The 80 forms carry a Brc80 (palette colour), the
// newer ones a BrcOperand: cb(1) + Brc(8, with a full COLORREF).
const P_BRC_TOP80: u16 = 0x6424;
const P_BRC_LEFT80: u16 = 0x6425;
const P_BRC_BOTTOM80: u16 = 0x6426;
const P_BRC_RIGHT80: u16 = 0x6427;
const P_BRC_BETWEEN80: u16 = 0x6428;
const P_BRC_TOP: u16 = 0xC64E;
const P_BRC_LEFT: u16 = 0xC64F;
const P_BRC_BOTTOM: u16 = 0xC650;
const P_BRC_RIGHT: u16 = 0xC651;
const P_BRC_BETWEEN: u16 = 0xC652;

/// Word's 16-colour `Ico` palette ([MS-DOC] §2.9.135), 0 = automatic.
const ICO: [Rgb; 17] = [
    Rgb(0, 0, 0),
    Rgb(0, 0, 0),
    Rgb(0, 0, 255),
    Rgb(0, 255, 255),
    Rgb(0, 255, 0),
    Rgb(255, 0, 255),
    Rgb(255, 0, 0),
    Rgb(255, 255, 0),
    Rgb(255, 255, 255),
    Rgb(0, 0, 128),
    Rgb(0, 128, 128),
    Rgb(0, 128, 0),
    Rgb(128, 0, 128),
    Rgb(128, 0, 0),
    Rgb(128, 128, 0),
    Rgb(128, 128, 128),
    Rgb(192, 192, 192),
];

/// Highlight palette ([MS-DOC] §2.9.135 Highlight, order of Word's highlighter row).
const HL: [Highlight; 17] = [
    Highlight::None,
    Highlight::Yellow,
    Highlight::BrightGreen,
    Highlight::Turquoise,
    Highlight::Pink,
    Highlight::Blue,
    Highlight::Red,
    Highlight::DarkBlue,
    Highlight::Teal,
    Highlight::Green,
    Highlight::Violet,
    Highlight::DarkRed,
    Highlight::DarkYellow,
    Highlight::Gray50,
    Highlight::Gray25,
    Highlight::None,
    Highlight::None,
];

/// The `Ico` palette colour (0 = automatic → `None`).
pub(crate) fn ico(i: u8) -> Option<Rgb> {
    if i == 0 {
        return None;
    }
    ICO.get(i as usize).copied()
}

/// Whether `i` is an `Ico` palette index (0 = automatic included).
pub(crate) fn is_ico(i: u8) -> bool {
    usize::from(i) < ICO.len()
}

/// The model border style a `brcType` stands for ([MS-DOC] §2.9.22 BrcType; 0x02, thick, is in
/// the BRC of Microsoft's Word 97-2007 specification). Types the model doesn't carry (the gap
/// variants, 3-D, inset/outset) draw as single lines, as the DOCX reader maps them.
fn brc_style(t: u8) -> BorderStyle {
    match t {
        0x00 => BorderStyle::None,
        0x02 => BorderStyle::Thick,
        0x03 => BorderStyle::Double,
        0x06 => BorderStyle::Dotted,
        0x07 | 0x16 => BorderStyle::Dashed,
        0x08 | 0x09 => BorderStyle::DotDash,
        0x0A => BorderStyle::Triple,
        0x14 | 0x15 => BorderStyle::Wave,
        _ => BorderStyle::Single,
    }
}

/// The border width in points: `dptLineWidth` in 1/8-point steps from 2 to 96, as DOCX `w:sz`
/// reads (1-point steps for page borders, which never reach a paragraph property).
fn brc_width(t: u8, dpt: u8) -> f32 {
    if t >= 0x40 { f32::from(dpt) } else { (f32::from(dpt.max(2)) / 8.0).min(12.0) }
}

/// One border from a Brc80 operand ([MS-DOC] §2.9.18). All bits set means "no border"; a
/// truncated operand is ignored.
pub(crate) fn brc80(op: &[u8]) -> Option<Border> {
    let [dpt, ty, ico_index, tail, ..] = op else { return None };
    if [*dpt, *ty, *ico_index, *tail] == [0xFF; 4] {
        return Some(Border::default());
    }
    Some(Border { style: brc_style(*ty), width: brc_width(*ty, *dpt), color: ico(*ico_index), space: f32::from(tail & 0x1F) })
}

/// One border from a `BrcOperand` ([MS-DOC] §2.9.21): cb(1, MUST be 8) + Brc(8).
pub(crate) fn brc(op: &[u8]) -> Option<Border> {
    if op.first() != Some(&8) {
        return None;
    }
    let b = op.get(1..9)?;
    if b.iter().all(|x| *x == 0xFF) {
        return Some(Border::default());
    }
    let color = match b.get(..4) {
        Some([0, 0, 0, 0]) => None, // automatic
        Some(px) => Some(Rgb(px[0], px[1], px[2])),
        None => None,
    };
    Some(Border { style: brc_style(b[5]), width: brc_width(b[5], b[4]), color, space: f32::from(b[6] & 0x1F) })
}

/// Put a decoded border on the side its sprm names (`between` = the border between this
/// paragraph and the next).
fn set_border(p: &mut ParaProps, op: u16, b: Border) {
    let borders = p.borders.get_or_insert_with(Borders::default);
    let slot = match op {
        P_BRC_TOP80 | P_BRC_TOP => &mut borders.top,
        P_BRC_LEFT80 | P_BRC_LEFT => &mut borders.left,
        P_BRC_BOTTOM80 | P_BRC_BOTTOM => &mut borders.bottom,
        P_BRC_RIGHT80 | P_BRC_RIGHT => &mut borders.right,
        _ => &mut borders.between,
    };
    *slot = Some(b);
}

fn u16_of(p: &Prl) -> Option<u16> {
    match p.operand {
        [b0, b1, ..] => Some(u16::from_le_bytes([*b0, *b1])),
        _ => None,
    }
}

fn s16_of(p: &Prl) -> Option<i16> {
    u16_of(p).map(|v| v as i16)
}

/// A toggle sprm's value: absolute, or the base's (the paragraph style's) value kept or
/// inverted. A kept value the base leaves unset stays as it was.
fn set(props: &mut Option<bool>, t: Toggle, base: Option<bool>) {
    *props = Some(match t {
        Toggle::On => true,
        Toggle::Off => false,
        Toggle::Invert => !base.unwrap_or(false),
        Toggle::Keep => match base {
            Some(b) => b,
            None => return,
        },
    });
}

/// Apply a character-property sprm. `base` carries the style-inherited values so toggle
/// operands can invert them; `fonts` maps sprmCRgFtc0 indices to names.
pub(crate) fn apply_char(p: &mut CharProps, prl: &Prl, base: &CharProps, fonts: &[String]) {
    let op = prl.op;
    match op {
        C_BOLD => set(&mut p.bold, sprm::toggle(prl), base.bold),
        C_ITALIC => set(&mut p.italic, sprm::toggle(prl), base.italic),
        C_STRIKE => set(&mut p.strike, sprm::toggle(prl), base.strike),
        C_SMALL_CAPS => set(&mut p.small_caps, sprm::toggle(prl), base.small_caps),
        C_CAPS => set(&mut p.caps, sprm::toggle(prl), base.caps),
        C_HIDDEN => set(&mut p.hidden, sprm::toggle(prl), base.hidden),
        C_KUL => {
            p.underline = match prl.operand.first().copied().unwrap_or(0) {
                0x00 => Some(Underline::None),
                0x01 => Some(Underline::Single),
                0x02 => Some(Underline::Words),
                0x03 => Some(Underline::Double),
                0x04 => Some(Underline::Dotted),
                0x06 => Some(Underline::Thick),
                0x07 => Some(Underline::Dash),
                0x09 => Some(Underline::DotDash),
                0x0A => Some(Underline::DotDotDash),
                0x0B => Some(Underline::Wave),
                0x14 => Some(Underline::Dotted),
                0x17 => Some(Underline::Thick),
                0x18 => Some(Underline::Dash),
                0x19 => Some(Underline::DotDash),
                0x1A => Some(Underline::DotDotDash),
                0x1B => Some(Underline::DoubleWave),
                _ => Some(Underline::Single),
            };
        }
        C_ICO => {
            if let Some(rgb) = ICO.get(prl.operand.first().copied().unwrap_or(0) as usize) {
                p.color = Some(if rgb == &Rgb::BLACK && prl.operand.first() == Some(&0) { TextColor::Auto } else { TextColor::Rgb(*rgb) });
            }
        }
        C_CR => {
            // COLORREF 0x00BBGGRR, stored little-endian: operand bytes are R, G, B.
            if let [r, g, b, ..] = prl.operand
                && let (r, g, b) = (*r, *g, *b)
            {
                p.color = Some(TextColor::Rgb(Rgb(r, g, b)));
            }
        }
        C_HPS => {
            if let Some(hps) = u16_of(prl) {
                p.size = Some((hps as f32 / 2.0).clamp(1.0, 1638.0));
            }
        }
        // [MS-DOC] §2.6.1: half-points the text is raised or lowered at full size, which is
        // OOXML `w:position` (ECMA-376 Part 1 §17.3.2.24), not superscript or subscript.
        C_HPS_POS => {
            if let Some(pos) = s16_of(prl) {
                p.position = Some((f32::from(pos) / 2.0).clamp(-1584.0, 1584.0));
            }
        }
        C_ISS => {
            p.vert_align = match prl.operand.first() {
                Some(0) => Some(VertAlign::Baseline),
                Some(1) => Some(VertAlign::Superscript),
                Some(2) => Some(VertAlign::Subscript),
                _ => p.vert_align,
            };
        }
        C_FTC0 => {
            if let Some(name) = u16_of(prl).and_then(|i| fonts.get(i as usize)) {
                p.font = Some(name.clone());
            }
        }
        C_FTC1 => {
            if let Some(name) = u16_of(prl).and_then(|i| fonts.get(i as usize)) {
                p.font_ea = Some(name.clone());
            }
        }
        C_FTC_BI => {
            if let Some(name) = u16_of(prl).and_then(|i| fonts.get(i as usize)) {
                p.font_cs = Some(name.clone());
            }
        }
        C_HIGHLIGHT => {
            if let Some(h) = HL.get(prl.operand.first().copied().unwrap_or(0) as usize) {
                p.highlight = Some(*h);
            }
        }
        C_ISTD => {} // CHPX style references are resolved in char_props.
        C_DXA_SPACE => {
            if let Some(dxa) = s16_of(prl) {
                p.spacing = Some((f32::from(dxa) / 20.0).clamp(-1584.0, 1584.0));
            }
        }
        C_CHAR_SCALE => {
            if let Some(pct) = u16_of(prl).filter(|v| (1..=600).contains(v)) {
                p.scale = Some(f32::from(pct));
            }
        }
        C_HPS_KERN => {
            if let Some(hps) = u16_of(prl) {
                p.kern = Some(f32::from(hps) / 2.0);
            }
        }
        _ => {}
    }
}

/// Custom tab stops from `sprmPChgTabsPapx` / `sprmPChgTabs` ([MS-DOC] PChgTabsPapxOperand,
/// PChgTabsOperand): the stops removed become `Clear` stops (they hide inherited ones) and the
/// stops added carry their alignment and leader (TBD: jc in bits 0–2, tlc in bits 3–5).
fn change_tabs(p: &mut ParaProps, prl: &Prl) {
    let op = prl.operand;
    let i16_at = |i: usize| op.get(i..i + 2).map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 20.0);
    // Both operands start with cb; sprmPChgTabs's removals also carry a "close" distance each.
    let close = prl.op == sprm::P_CHG_TABS;
    let mut at = 1;
    let Some(&del) = op.get(at) else { return };
    let del = usize::from(del.min(64));
    at += 1;
    let mut removed = Vec::with_capacity(del);
    for k in 0..del {
        if let Some(x) = i16_at(at + 2 * k) {
            removed.push(x);
        }
    }
    at += 2 * del * if close { 2 } else { 1 };
    let Some(&add) = op.get(at) else { return };
    let add = usize::from(add.min(64));
    at += 1;
    let mut tabs = p.tabs.take().unwrap_or_default();
    // Removed within 25 twips (1.25 pt), as the format says.
    tabs.retain(|t| !removed.iter().any(|x| (t.pos - x).abs() <= 1.25));
    for x in removed {
        tabs.push(TabStop { pos: x, align: TabAlign::Clear, leader: TabLeader::None });
    }
    for k in 0..add {
        let (Some(pos), Some(&tbd)) = (i16_at(at + 2 * k), op.get(at + 2 * add + k)) else { break };
        let align = match tbd & 7 {
            1 => TabAlign::Center,
            2 => TabAlign::Right,
            3 => TabAlign::Decimal,
            4 => TabAlign::Bar,
            _ => TabAlign::Left,
        };
        let leader = match (tbd >> 3) & 7 {
            1 => TabLeader::Dot,
            2 => TabLeader::Hyphen,
            3 | 4 => TabLeader::Underscore,
            5 => TabLeader::MiddleDot,
            _ => TabLeader::None,
        };
        tabs.retain(|t| (t.pos - pos).abs() > 0.05);
        tabs.push(TabStop { pos, align, leader });
    }
    tabs.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    p.tabs = Some(tabs);
}

/// Whether `op` is one of the paragraph frame sprms [`apply_frame`] reads.
pub(crate) fn is_frame_sprm(op: u16) -> bool {
    matches!(op, P_PC | P_DXA_ABS | P_DYA_ABS | P_DXA_WIDTH | P_WR | P_WHEIGHT_ABS | P_DYA_FROM_TEXT | P_DXA_FROM_TEXT)
}

/// A frame with the PAP defaults: anchored to the column horizontally and the margin vertically,
/// at 0, sized to its text, no distance from the text.
fn default_frame() -> TextFrame {
    TextFrame { h_anchor: "text".into(), v_anchor: "margin".into(), wrap: "auto".into(), ..Default::default() }
}

/// Apply a frame sprm to the paragraph's frame, creating it with the PAP defaults. Each sprm
/// changes only its own fields, so a frame merges field by field along the style chain.
pub(crate) fn apply_frame(p: &mut ParaProps, prl: &Prl) {
    let len = |v: i16| (f32::from(v) / 20.0).clamp(-1584.0, 1584.0);
    let byte = prl.operand.first().copied();
    let word = s16_of(prl);
    // A short operand changes nothing (and creates no frame).
    let short = if matches!(prl.op, P_PC | P_WR) { byte.is_none() } else { word.is_none() };
    if short {
        return;
    }
    let f = p.frame.get_or_insert_with(default_frame);
    match (prl.op, byte, word) {
        // PositionCodeOperand: pcVert in bits 4-5, pcHorz in bits 6-7; 3 keeps the anchor.
        (P_PC, Some(b), _) => {
            match (b >> 4) & 3 {
                0 => f.v_anchor = "margin".into(),
                1 => f.v_anchor = "page".into(),
                2 => f.v_anchor = "text".into(),
                _ => {}
            }
            match (b >> 6) & 3 {
                0 => f.h_anchor = "text".into(),
                1 => f.h_anchor = "margin".into(),
                2 => f.h_anchor = "page".into(),
                _ => {}
            }
        }
        // XAS: negative codes are alignments, anything else a position in twips.
        (P_DXA_ABS, _, Some(v)) => {
            let align = match v {
                -4 => Some("center"),
                -8 => Some("right"),
                -12 => Some("inside"),
                -16 => Some("outside"),
                _ => None,
            };
            f.x = if align.is_some() { 0.0 } else { len(v) };
            f.x_align = align.map(str::to_string);
        }
        // YAS, likewise.
        (P_DYA_ABS, _, Some(v)) => {
            let align = match v {
                -4 => Some("top"),
                -8 => Some("center"),
                -12 => Some("bottom"),
                -16 => Some("inside"),
                -20 => Some("outside"),
                _ => None,
            };
            f.y = if align.is_some() { 0.0 } else { len(v) };
            f.y_align = align.map(str::to_string);
        }
        (P_DXA_WIDTH, _, Some(v)) => f.w = (v != 0).then(|| len(v).max(0.0)),
        // dyaHeight in bits 0-14; bit 15 (fMinHeight) makes it a minimum rather than exact.
        (P_WHEIGHT_ABS, _, Some(v)) => {
            let v = v as u16;
            let h = v & 0x7FFF;
            f.h = (h != 0).then(|| (f32::from(h) / 20.0).min(1584.0));
            f.h_exact = h != 0 && v & 0x8000 == 0;
        }
        (P_DXA_FROM_TEXT, _, Some(v)) => f.h_space = len(v).max(0.0),
        (P_DYA_FROM_TEXT, _, Some(v)) => f.v_space = len(v).max(0.0),
        // Wr ([MS-DOC] §2.9.354).
        (P_WR, Some(b), _) => {
            f.wrap = match b {
                1 => "notBeside",
                2 => "around",
                3 => "none",
                4 => "tight",
                5 => "through",
                _ => "auto",
            }
            .into();
        }
        _ => {}
    }
}

/// The effective frame of paragraph style `istd`: its chain's frame sprms, root first.
pub(crate) fn style_frame(raw: &[crate::stsh::RawStyle], istd: usize) -> Option<TextFrame> {
    let mut para = ParaProps::default();
    for s in style_chain(raw, istd).into_iter().filter_map(|i| raw.get(i)) {
        for prl in sprm::iter(&s.papx) {
            if is_frame_sprm(prl.op) {
                apply_frame(&mut para, &prl);
            }
        }
    }
    para.frame
}

/// Apply a paragraph-property sprm.
pub(crate) fn apply_para(p: &mut ParaProps, prl: &Prl) {
    let tw = |v: i32| (v as f32 / 20.0).clamp(-1000.0, 1000.0);
    let s16 = |prl: &Prl| -> Option<i16> { u16_of(prl).map(|v| v as i16) };
    match prl.op {
        op if is_frame_sprm(op) => apply_frame(p, prl),
        sprm::P_CHG_TABS | sprm::P_CHG_TABS_PAPX => change_tabs(p, prl),
        P_JC | P_JC80 => {
            p.align = match prl.operand.first().copied().unwrap_or(0) {
                1 => Some(Align::Center),
                2 => Some(Align::Right),
                3 => Some(Align::Justify),
                4 => Some(Align::Distribute),
                _ => Some(Align::Left),
            };
        }
        P_DXA_LEFT | P_DXA_LEFT80 => {
            if let Some(v) = s16(prl) {
                p.indent_left = Some(tw(v as i32));
            }
        }
        P_DXA_RIGHT | P_DXA_RIGHT80 => {
            if let Some(v) = s16(prl) {
                p.indent_right = Some(tw(v as i32));
            }
        }
        // A two-byte operand, like the other indents (spra 4).
        P_DXA_FIRST | P_DXA_FIRST80 => {
            if let Some(v) = s16(prl) {
                p.indent_first = Some(tw(v as i32));
            }
        }
        P_DXC_RIGHT | P_DXC_LEFT | P_DXC_FIRST => {
            if let Some(v) = s16(prl) {
                let chars = Some(f32::from(v) / 100.0);
                match prl.op {
                    P_DXC_RIGHT => p.indent_right_chars = chars,
                    P_DXC_LEFT => p.indent_left_chars = chars,
                    _ => p.indent_first_chars = chars,
                }
            }
        }
        P_DYA_BEFORE => {
            if let Some(v) = s16(prl) {
                p.space_before = Some((v as f32 / 20.0).clamp(0.0, 3168.0));
            }
        }
        P_DYA_AFTER => {
            if let Some(v) = s16(prl) {
                p.space_after = Some((v as f32 / 20.0).clamp(0.0, 3168.0));
            }
        }
        // The stored dyaBefore/dyaAfter stay: the resolver lets the flag win, and a later 0
        // (direct formatting turning a style's Auto off) brings them back.
        P_F_DYA_BEFORE_AUTO | P_F_DYA_AFTER_AUTO => {
            if let Some(&b) = prl.operand.first() {
                if prl.op == P_F_DYA_BEFORE_AUTO {
                    p.space_before_auto = Some(b != 0);
                } else {
                    p.space_after_auto = Some(b != 0);
                }
            }
        }
        // LSPD ([MS-DOC] §2.9.150): a signed dyaLine, then fMultLinespace. In lines (240ths) when
        // fMultLinespace is 1; else twips, at least when positive and exactly when negative.
        P_DYA_LINE => {
            if let Some(v) = s16(prl) {
                let mult = prl.operand.get(2..4).is_some_and(|b| b == [1, 0]);
                let v = f32::from(v);
                p.line_spacing = Some(if mult {
                    LineSpacing::Multiple((v.abs() / 240.0).clamp(0.06, 132.0))
                } else if v < 0.0 {
                    LineSpacing::Exactly((-v / 20.0).clamp(0.7, 1584.0))
                } else {
                    LineSpacing::AtLeast((v / 20.0).clamp(0.0, 1584.0))
                });
            }
        }
        P_KEEP => {
            if matches!(sprm::toggle(prl), Toggle::On) {
                p.keep_lines = Some(true);
            }
        }
        P_KEEP_FOLLOW => {
            if matches!(sprm::toggle(prl), Toggle::On) {
                p.keep_next = Some(true);
            }
        }
        P_PAGE_BREAK_BEFORE => {
            if matches!(sprm::toggle(prl), Toggle::On) {
                p.page_break_before = Some(true);
            }
        }
        P_OUTLVL => {
            if let Some(&l) = prl.operand.first() {
                p.outline_level = Some(l.min(8));
            }
        }
        P_BRC_TOP80 | P_BRC_LEFT80 | P_BRC_BOTTOM80 | P_BRC_RIGHT80 | P_BRC_BETWEEN80 => {
            if let Some(b) = brc80(prl.operand) {
                set_border(p, prl.op, b);
            }
        }
        P_BRC_TOP | P_BRC_LEFT | P_BRC_BOTTOM | P_BRC_RIGHT | P_BRC_BETWEEN => {
            if let Some(b) = brc(prl.operand) {
                set_border(p, prl.op, b);
            }
        }
        _ => {}
    }
}

/// Build the model stylesheet: resolve each raw style's chain (base → this) and the ids we
/// reference from paragraphs.
pub(crate) fn stylesheet(raw: &[crate::stsh::RawStyle], ids: &[String], fonts: &[String], standard: crate::stsh::StandardFonts) -> StyleSheet {
    let mut sheet = StyleSheet::empty();
    // Word's base character properties: the stylesheet's standard fonts (Aptos in a Word 365
    // file, whose font table still starts with Times New Roman, #5066), else font index 0 of the
    // table; 10 pt ([MS-DOC] sprmCHps).
    let font = |i: Option<u16>| i.and_then(|i| fonts.get(usize::from(i))).cloned();
    sheet.default_chr.font = font(standard.ascii).or_else(|| fonts.first().cloned());
    sheet.default_chr.font_ea = font(standard.east_asian);
    sheet.default_chr.font_cs = font(standard.bidi);
    if sheet.default_chr.size.is_none() {
        sheet.default_chr.size = Some(10.0);
    }
    for (istd, st) in raw.iter().enumerate() {
        if st.name.is_empty() {
            continue;
        }
        let builtin = builtin_by_sti(st.sti);
        let id = ids.get(istd).cloned().unwrap_or_default();
        let kind = match st.stk {
            2 => StyleKind::Character,
            3 => StyleKind::Table,
            4 => StyleKind::Numbering,
            _ => StyleKind::Paragraph,
        };
        let base = base_of(raw, istd).and_then(|i| ids.get(i)).cloned();
        // Character properties come from the base chain, root first, then our own grpprl.
        // Paragraph properties are resolved the same way, but the style keeps only its own
        // ([MS-DOC] §2.9.260: the UPX holds the style's own settings): what it merely inherits
        // comes through `based_on`. So a style with a list and no indent of its own takes the
        // list level's indents, not a copy of its base's (Word; the same as DOCX `w:ind`).
        let mut chr = CharProps::default();
        let mut inherited = ParaProps::default();
        let chain = style_chain(raw, istd);
        let (own_ix, bases) = chain.split_last().map_or((None, &[][..]), |(o, b)| (Some(*o), b));
        for s in chain.iter().filter_map(|&i| raw.get(i)) {
            apply_chain(&mut chr, &s.chpx, fonts);
        }
        for s in bases.iter().filter_map(|&i| raw.get(i)) {
            apply_chain_para(&mut inherited, &s.papx);
        }
        let own_papx = own_ix.and_then(|i| raw.get(i)).map_or(&[][..], |s| s.papx.as_slice());
        let mut full = inherited.clone();
        apply_chain_para(&mut full, own_papx);
        let mut own = ParaProps::default();
        apply_chain_para(&mut own, own_papx);
        let mut para = if bases.is_empty() { full.clone() } else { full.clone().own_part(&own, &inherited) };
        // A heading without an outline level anywhere in its chain gets its heading's.
        if full.outline_level.is_none() && kind == StyleKind::Paragraph {
            let h = builtin.as_ref().map_or_else(|| heading_level(&st.name), |b| b.2);
            if h > 0 {
                para.outline_level = Some(h - 1);
            }
        }
        let name = builtin.map_or_else(|| display_name(st, istd), |b| b.1);
        // Word's own styles (`sti` below istiUser) are built in, as the DOCX reader marks them.
        sheet.styles.push(Style { id, name, kind, based_on: base, chr, para, builtin: st.sti < crate::stsh::STI_USER, ..Default::default() });
    }
    sheet
}

/// Most `istdBase` links followed; a longer chain (or a cycle) in a corrupt file stops there.
const MAX_STYLE_DEPTH: usize = 16;

/// The parent of style `istd`: its zero-based `istdBase` ([MS-DOC] §2.9.260), when that names
/// another existing, named style (0x0FFF means none).
fn base_of(raw: &[crate::stsh::RawStyle], istd: usize) -> Option<usize> {
    let b = raw.get(istd)?.istd_base;
    let b = usize::from(b);
    (b != 0x0FFF && b != istd && raw.get(b).is_some_and(|s| !s.name.is_empty())).then_some(b)
}

/// Style `istd` and its ancestors, root first and `istd` last. Cycles and chains deeper than
/// [`MAX_STYLE_DEPTH`] are cut.
fn style_chain(raw: &[crate::stsh::RawStyle], istd: usize) -> Vec<usize> {
    let mut chain = vec![istd];
    let mut cur = istd;
    while chain.len() < MAX_STYLE_DEPTH {
        let Some(b) = base_of(raw, cur) else { break };
        if chain.contains(&b) {
            break;
        }
        chain.push(b);
        cur = b;
    }
    chain.reverse();
    chain
}

/// The built-in style an invariant `sti` names: (id, shown name, heading level). The `sti` is
/// the same in every language ([MS-DOC] StdfBase), so "Überschrift 1" is still heading 1.
fn builtin_by_sti(sti: u16) -> Option<(String, String, u8)> {
    match sti {
        0 => Some(("Normal".into(), "Normal".into(), 0)),
        1..=9 => Some((format!("Heading{sti}"), format!("Heading {sti}"), sti as u8)),
        19..=27 => Some((format!("TOC{}", sti - 18), format!("TOC {}", sti - 18), 0)),
        _ => None,
    }
}

/// A stored style's id: by its built-in index when it has one, else by its name. A heading name
/// whose `sti` is unspecified (0x0FFF, neither Word's nor marked as the user's) keeps the id of
/// its name (`Heading 1`); Word's own headings carry `sti` 1-9 and get Word's ids.
fn raw_style_id(raw: &crate::stsh::RawStyle) -> String {
    if let Some(b) = builtin_by_sti(raw.sti) {
        return b.0;
    }
    let h = heading_level(&raw.name);
    if raw.sti == 0x0FFF && h > 0 {
        return format!("Heading {h}");
    }
    style_id(&raw.name)
}

/// Each stored style's id by `istd` (empty without a name). Word's own styles take theirs first;
/// one already taken gets a number: "Heading 1" next to "Überschrift 1" is `Heading1_2`.
pub(crate) fn style_ids(raw: &[crate::stsh::RawStyle]) -> Vec<String> {
    let mut ids = vec![String::new(); raw.len()];
    let mut taken = std::collections::BTreeSet::new();
    // The next number to try for each id, so many styles of one name stay linear.
    let mut next: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let named = |i: &usize, word: bool| raw.get(*i).is_some_and(|s| !s.name.is_empty() && (s.sti < crate::stsh::STI_USER) == word);
    let order = (0..raw.len()).filter(|i| named(i, true)).chain((0..raw.len()).filter(|i| named(i, false)));
    for i in order {
        let Some(st) = raw.get(i) else { continue };
        let base = raw_style_id(st);
        // Ids differing only in case count as taken: Word treats them as the same style.
        let mut id = base.clone();
        while taken.contains(&id.to_ascii_lowercase()) {
            let n = next.entry(base.to_ascii_lowercase()).or_insert(2);
            id = format!("{base}_{n}");
            *n += 1;
        }
        taken.insert(id.to_ascii_lowercase());
        if let Some(slot) = ids.get_mut(i) {
            *slot = id;
        }
    }
    ids
}

/// The style-id form of a stored (lower-case) built-in name, matching the docx reader's ids.
pub(crate) fn style_id(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    // Word's ids for the TOC levels and headings, which References › Update Table and the
    // Heading 1-3 commands (Ctrl+Alt+1-3) name.
    if let Some(n) = toc_level(&lower) {
        return format!("TOC{n}");
    }
    let h = heading_level(&lower);
    if h > 0 {
        return format!("Heading{h}");
    }
    match lower.as_str() {
        "normal" => "Normal".into(),
        "title" => "Title".into(),
        "subtitle" => "Subtitle".into(),
        "quote" | "body text" | "body text 2" | "body text 3" | "body text indent" | "body text indent 2" | "body text indent 3" => {
            let mut c = lower.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        }
        "default paragraph font" => "Default Paragraph Font".into(),
        _ => {
            let mut out = String::with_capacity(name.len());
            let mut up = true;
            for ch in name.trim().chars() {
                if ch == ' ' {
                    up = true;
                    out.push(ch);
                } else if up {
                    out.extend(ch.to_uppercase());
                    up = false;
                } else {
                    out.push(ch);
                }
            }
            out
        }
    }
}

/// "heading 3" → 3; 0 when not a heading.
fn heading_level(name: &str) -> u8 {
    let lower = name.trim().to_ascii_lowercase();
    lower.strip_prefix("heading ").and_then(|d| d.parse().ok()).filter(|&l: &u8| (1..=9).contains(&l)).unwrap_or(0)
}

/// "toc 2" → 2; `None` when not a table of contents level.
fn toc_level(lower: &str) -> Option<u8> {
    lower.strip_prefix("toc ").and_then(|d| d.parse().ok()).filter(|l: &u8| (1..=9).contains(l))
}

fn display_name(st: &crate::stsh::RawStyle, istd: usize) -> String {
    let name = &st.name;
    let lower = name.trim().to_ascii_lowercase();
    let h = heading_level(&lower);
    // STSH reserves istd 1–9 for the corresponding headings ([MS-DOC] §2.9.271).
    // Tolerate an unspecified sti there when the name agrees with the reserved slot.
    let fixed_heading = st.sti == 0x0FFF && h > 0 && istd == usize::from(h);
    // Likewise a TOC level with an unspecified sti: Update Table names its entries' styles so.
    let toc = st.sti == 0x0FFF && toc_level(&lower).is_some();
    if st.sti >= 0x0FFE && !fixed_heading && !toc {
        return name.clone();
    }
    if h > 0 {
        return format!("Heading {h}");
    }
    if let Some(n) = toc_level(&lower) {
        return format!("TOC {n}");
    }
    match lower.as_str() {
        "body text" => "Body Text".into(),
        _ => style_id(name),
    }
}

/// Apply a grpprl of character sprms with a toggle base.
fn apply_chain(chr: &mut CharProps, grpprl: &[u8], fonts: &[String]) {
    let base = chr.clone();
    for prl in sprm::iter(grpprl) {
        if prl.sgc() == 2 {
            apply_char(chr, &prl, &base, fonts);
        }
    }
}

/// Apply a grpprl of paragraph sprms.
fn apply_chain_para(para: &mut ParaProps, grpprl: &[u8]) {
    let (mut ilfo, mut ilvl) = (None, None);
    for prl in sprm::iter(grpprl) {
        match prl.op {
            crate::P_ILFO => ilfo = Some(crate::ilfo_operand(prl.operand)),
            crate::P_ILVL => ilvl = Some(crate::ilvl_operand(prl.operand)),
            _ if prl.sgc() == 1 => apply_para(para, &prl),
            _ => {}
        }
    }
    // A list style ("List Number", a numbered paragraph style): its paragraphs are numbered
    // without a list of their own. A style based on it may change the level or end the list.
    match (ilfo, &mut para.numbering) {
        (Some(ilfo), n) => {
            let level = ilvl.or(n.map(|n| n.level)).unwrap_or(0);
            *n = match crate::num_of(ilfo) {
                Some(num) => Some(NumRef { num, level }),
                // Ending the based-on style's list is `num: 0`: no value would inherit it.
                None => n.map(|_| NumRef { num: 0, level }),
            };
        }
        (None, Some(n)) => n.level = ilvl.unwrap_or(n.level),
        (None, None) => {}
    }
}

#[cfg(test)]
mod style_id_tests {
    use super::*;
    use crate::stsh::RawStyle;

    #[test]
    fn built_ins_reserve_ids_before_user_styles() {
        let raw = [
            RawStyle { sti: 0x0FFE, name: "Heading 1".into(), ..Default::default() },
            RawStyle { sti: 0x0FFF, name: "heading 1 2".into(), ..Default::default() },
            RawStyle { sti: 1, name: "heading 1".into(), ..Default::default() },
            RawStyle { sti: 0x0FFF, name: "HEADING 1".into(), ..Default::default() },
            RawStyle::default(),
        ];
        let ids = style_ids(&raw);
        // Word's own heading 1 (sti 1) keeps `Heading1`; user styles named like it are numbered.
        assert_eq!(ids, ["Heading1_2", "Heading 1 2", "Heading1", "Heading 1", ""]);
        assert_eq!(display_name(&raw[0], 0), "Heading 1");
        assert_eq!(display_name(&raw[1], 1), "heading 1 2");
        assert_eq!(display_name(&raw[2], 2), "Heading 1");
        assert_eq!(display_name(&raw[3], 3), "HEADING 1");
    }

    #[test]
    fn user_heading_names_are_preserved_outside_unspecified_fixed_headings() {
        let mut st = RawStyle { sti: 0x0FFE, name: "heading 1".into(), ..Default::default() };
        assert_eq!(display_name(&st, 1), "heading 1");
        st.sti = 0x0FFF;
        assert_eq!(display_name(&st, 15), "heading 1");
        assert_eq!(display_name(&st, 2), "heading 1");
        assert_eq!(display_name(&st, 1), "Heading 1");
    }

    #[test]
    fn character_references_use_distinct_ids() {
        let raw = [
            RawStyle { sti: 0x0FFF, stk: 2, name: "Emphasis".into(), ..Default::default() },
            RawStyle { sti: 0x0FFF, stk: 2, name: "emphasis".into(), ..Default::default() },
        ];
        let ids = style_ids(&raw);
        let base = CharProps::default();
        let first = crate::char_props(&[0x30, 0x4A, 0, 0], &[], &base, &raw, &ids);
        let second = crate::char_props(&[0x30, 0x4A, 1, 0], &[], &base, &raw, &ids);
        assert_eq!(first.style.as_deref(), Some("Emphasis"));
        assert_eq!(second.style.as_deref(), Some("Emphasis_2"));
    }
}

/// A style with a list and no indent of its own, based on an indented Normal, takes its list
/// level's indents ([MS-DOC] §2.9.260: the style's UPX holds only its own settings); one that sets
/// its own left indent keeps it over the level's.
#[cfg(test)]
mod style_list_indent_tests {
    use super::*;
    use crate::stsh::RawStyle;
    use wordcraft_doc::{Document, Paragraph};

    fn prl(op: u16, operand: &[u8]) -> Vec<u8> {
        [op.to_le_bytes().as_slice(), operand].concat()
    }

    /// A `PlfLst` with one simple list (lsid 7) whose only level has left 1701 / first line
    /// −1701 twips, then a `PlfLfo` with one LFO bound to it. Returns (table, fc of each).
    fn lists() -> (Vec<u8>, u32, u32) {
        let mut t = 1u16.to_le_bytes().to_vec(); // cLst
        t.extend_from_slice(&7i32.to_le_bytes()); // lsid
        t.extend_from_slice(&0u32.to_le_bytes()); // tplc
        for _ in 0..9 {
            t.extend_from_slice(&0x0FFFu16.to_le_bytes());
        }
        t.extend_from_slice(&[1, 0]); // fSimpleList, grfhic
        let papx = [prl(0x840F, &1701i16.to_le_bytes()), prl(0x8411, &(-1701i16).to_le_bytes())].concat();
        let mut lvlf = vec![0u8; 28];
        lvlf[0] = 1; // iStartAt
        lvlf[6] = 1; // the placeholder is the first character
        lvlf[25] = papx.len() as u8; // cbGrpprlPapx
        t.extend_from_slice(&lvlf);
        t.extend_from_slice(&papx);
        t.extend_from_slice(&1u16.to_le_bytes()); // Xst: one character, the level placeholder
        t.extend_from_slice(&0u16.to_le_bytes());
        let lfo_at = t.len() as u32;
        t.extend_from_slice(&1u32.to_le_bytes()); // lfoMac
        t.extend_from_slice(&7i32.to_le_bytes()); // LFO.lsid
        t.extend_from_slice(&[0u8; 12]); // unused, clfolvl 0, ibstFltAutoNum, grfhic, unused
        t.extend_from_slice(&0u32.to_le_bytes()); // LFOData.cp
        (t, 0, lfo_at)
    }

    /// The opened document's styles and lists: Normal (left 1134, right 170, justified) and
    /// "Numbered" on list 1 at level 0, with `own` added to its grpprl.
    fn doc(own: &[u8]) -> Document {
        let normal = [prl(0x2461, &[3]), prl(0x845D, &170i16.to_le_bytes()), prl(0x845E, &1134i16.to_le_bytes())].concat();
        let numbered = [prl(0x460B, &1i16.to_le_bytes()), prl(0x260A, &[0]), own.to_vec()].concat();
        let raw = vec![
            RawStyle { sti: 0, stk: 1, istd_base: 0x0FFF, name: "Normal".into(), papx: normal, chpx: Vec::new() },
            RawStyle { sti: 0x0FFE, stk: 1, istd_base: 0, name: "Numbered".into(), papx: numbered, chpx: Vec::new() },
        ];
        let fonts = vec!["Times New Roman".to_string()];
        let ids = style_ids(&raw);
        let (table, lst_at, lfo_at) = lists();
        let mut pairs = vec![0u32; 2 * 75];
        pairs[2 * crate::fib::pair::PLF_LST] = lst_at;
        pairs[2 * crate::fib::pair::PLF_LST + 1] = lfo_at;
        pairs[2 * crate::fib::pair::PLF_LFO] = lfo_at;
        pairs[2 * crate::fib::pair::PLF_LFO + 1] = table.len() as u32 - lfo_at;
        let fib = crate::fib::Fib { which_tbl_stm: false, ccp: Default::default(), pairs, word6: None };
        let mut d = Document::default();
        d.styles = std::sync::Arc::new(stylesheet(&raw, &ids, &fonts, Default::default()));
        d.numbering = std::sync::Arc::new(crate::list::parse(&table, &fib, &fonts));
        d
    }

    /// The left and first-line indents of a paragraph in style "Numbered", as layout places it:
    /// the list level's, unless the style chain from the style giving the list sets its own.
    fn indents(d: &Document) -> (f32, f32) {
        let p = Paragraph { props: ParaProps { style: Some("Numbered".into()), ..Default::default() }, ..Default::default() };
        let r = d.styles.resolve_para(&p.props);
        let n = r.numbering.expect("the style's list");
        let lvl = d.numbering.level(n.num, n.level).expect("level");
        let (style_left, style_first) = d.styles.indents_over_list("Numbered");
        (if style_left { r.indent_left } else { lvl.indent }, if style_first { r.indent_first } else { -lvl.hanging })
    }

    #[test]
    fn a_list_style_without_its_own_indent_takes_the_levels() {
        let d = doc(&[]);
        let st = d.styles.get("Numbered").expect("style");
        assert_eq!(st.para.indent_left, None, "not a copy of Normal's");
        let (left, first) = indents(&d);
        assert!((left - 85.05).abs() < 0.01, "left: {left}");
        assert!((first + 85.05).abs() < 0.01, "first line: {first}");
    }

    #[test]
    fn a_list_style_with_its_own_indent_beats_the_level() {
        let d = doc(&prl(0x845E, &1134i16.to_le_bytes()));
        let (left, first) = indents(&d);
        assert!((left - 56.7).abs() < 0.01, "left: {left}");
        assert!((first + 85.05).abs() < 0.01, "first line stays the level's: {first}");
    }
}
