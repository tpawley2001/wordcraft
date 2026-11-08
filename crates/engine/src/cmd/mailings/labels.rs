use super::merge::address_block;
use super::{
    Align, Block, CharProps, CmdError, CmdResult, Document, Paragraph, Pos, SectionProps, Selection, Session, StoryRef, Table, Value, json, p,
    para_block,
};

/// Common envelope sizes: name, width and height in points (long side first).
pub const ENVELOPE_SIZES: [(&str, f32, f32); 8] = [
    ("Envelope #10", 684.0, 297.0),
    ("Envelope #9", 639.0, 279.0),
    ("Envelope Monarch", 540.0, 279.0),
    ("Envelope DL", 623.6, 311.8),
    ("Envelope C4", 918.4, 649.1),
    ("Envelope C5", 649.1, 459.2),
    ("Envelope C6", 459.2, 323.1),
    ("Envelope B5", 708.7, 498.9),
];

/// Envelope size by name (`Envelope DL`, or just `DL`, any case), or `width`/`height` in points.
fn envelope_size(v: &Value) -> Result<(f32, f32), CmdError> {
    if let (Some(w), Some(h)) = (p::f32(v, "width"), p::f32(v, "height")) {
        let (w, h) = (w.clamp(144.0, 1584.0), h.clamp(144.0, 1584.0));
        return Ok((w.max(h), w.min(h)));
    }
    let name = p::str(v, "size").unwrap_or("Envelope #10").trim();
    ENVELOPE_SIZES
        .iter()
        .find(|(n, ..)| n.eq_ignore_ascii_case(name) || n.strip_prefix("Envelope ").is_some_and(|short| short.eq_ignore_ascii_case(name)))
        .map(|(_, w, h)| (*w, *h))
        .ok_or_else(|| CmdError::Params(format!("unknown envelope size `{name}`")))
}

/// Longest label or envelope address text accepted.
const MAX_LABEL_TEXT: usize = 2_000;

/// An optional label/envelope text parameter, rejected when longer than `MAX_LABEL_TEXT` characters.
fn label_text(v: &Value, key: &str) -> Result<Option<String>, CmdError> {
    match p::str(v, key) {
        Some(t) if t.chars().count() > MAX_LABEL_TEXT => Err(CmdError::Params(format!("`{key}` is longer than {MAX_LABEL_TEXT} characters"))),
        t => Ok(t.map(str::to_string)),
    }
}

/// Validated font overrides, applied only after all envelope parameters are checked.
#[derive(Default)]
struct EnvelopeFont {
    name: Option<String>,
    size: Option<f32>,
}

impl EnvelopeFont {
    fn read(v: &Value, key: &str) -> Result<Self, CmdError> {
        let Some(v) = v.get(key) else { return Ok(Self::default()) };
        if !v.is_object() {
            return Err(CmdError::Params(format!("`{key}` must be a font object")));
        }
        let name = match v.get("name") {
            None => None,
            Some(v) => Some(
                v.as_str()
                    .filter(|n| !n.trim().is_empty() && n.chars().count() <= 31)
                    .ok_or_else(|| CmdError::Params(format!("`{key}.name` must contain 1 to 31 characters")))?
                    .to_string(),
            ),
        };
        let size = envelope_number(v, "size", 1.0, 1638.0)?;
        Ok(Self { name, size })
    }

    fn apply(&self, d: &mut Document, id: &str, name: &str, size: f32) {
        use wordcraft_doc::{
            props::{LineSpacing, ParaProps},
            styles::Style,
        };
        let styles = std::sync::Arc::make_mut(&mut d.styles);
        if styles.get(id).is_none() {
            styles.styles.push(Style {
                id: id.into(),
                name: name.into(),
                based_on: Some("Normal".into()),
                builtin: true,
                chr: CharProps { size: Some(size), ..Default::default() },
                para: ParaProps {
                    space_before: Some(0.0),
                    space_after: Some(0.0),
                    line_spacing: Some(LineSpacing::Multiple(1.0)),
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        if let Some(style) = styles.get_mut(id) {
            if let Some(name) = &self.name {
                style.chr.font = Some(name.clone());
            }
            if let Some(size) = self.size {
                style.chr.size = Some(size);
            }
        }
    }
}

fn envelope_number(v: &Value, key: &str, min: f32, max: f32) -> Result<Option<f32>, CmdError> {
    let Some(value) = v.get(key) else { return Ok(None) };
    let n = value
        .as_f64()
        .filter(|n| n.is_finite() && (f64::from(min)..=f64::from(max)).contains(n))
        .map(|n| n as f32)
        .ok_or_else(|| CmdError::Params(format!("`{key}` must be between {min} and {max} pt")))?;
    Ok(Some(n))
}

fn envelope_frame(
    v: &Value,
    left: &str,
    top: &str,
    w: f32,
    h: f32,
    defaults: (f32, f32),
) -> Result<Option<wordcraft_doc::props::TextFrame>, CmdError> {
    let x = envelope_number(v, left, 0.0, w)?;
    let y = envelope_number(v, top, 0.0, h)?;
    Ok((x.is_some() || y.is_some()).then(|| wordcraft_doc::props::TextFrame {
        x: x.unwrap_or(defaults.0),
        y: y.unwrap_or(defaults.1),
        h_anchor: "page".into(),
        v_anchor: "page".into(),
        wrap: "around".into(),
        w: None,
        h: None,
        h_exact: false,
        x_align: None,
        y_align: None,
        anchor_lock: None,
        h_space: 9.0,
        v_space: 0.0,
    }))
}

/// Envelopes: a landscape page with the return address at the top left and the delivery address
/// in the middle. As a new document, or (`add`) as a section of its own at the start of this one.
pub(super) fn envelopes(s: &mut Session, v: &Value) -> CmdResult {
    let delivery = label_text(v, "delivery")?.unwrap_or_else(|| {
        if s.merge.rows.is_empty() { "Recipient Name\nStreet Address\nCity, ST 00000".into() } else { address_block(s, s.merge.record) }
    });
    let ret = label_text(v, "return")?.unwrap_or_default();
    let (w, h) = envelope_size(v)?;
    let delivery_frame = envelope_frame(v, "deliveryLeft", "deliveryTop", w, h, (w * 0.42, h * 0.45))?;
    let return_frame = envelope_frame(v, "returnLeft", "returnTop", w, h, (22.0, 22.0))?;
    let delivery_font = EnvelopeFont::read(v, "deliveryFont")?;
    let return_font = EnvelopeFont::read(v, "returnFont")?;
    let sect = SectionProps {
        page_w: w,
        page_h: h,
        landscape: true,
        margin_top: 22.0,
        margin_left: 22.0,
        margin_right: 22.0,
        margin_bottom: 22.0,
        ..Default::default()
    };
    // Addresses are capped: a pasted novel is not an address.
    let lines = |t: &str| t.split('\n').take(20).map(str::to_string).collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let ret_lines = if ret.is_empty() || p::bool(v, "omitReturn") == Some(true) { Vec::new() } else { lines(&ret) };
    for l in &ret_lines {
        let mut p = Paragraph::with_text(l, CharProps::default()).styled("EnvelopeReturn");
        p.props.frame = return_frame.clone();
        blocks.push(para_block(p));
    }
    let spacer = (6 - ret_lines.len().min(6)).max(1);
    for _ in 0..spacer + 2 {
        blocks.push(para_block(Paragraph::new().styled("NoSpacing")));
    }
    let mut last = None;
    for l in lines(&delivery) {
        let mut p = Paragraph::with_text(&l, CharProps::default()).styled("EnvelopeAddress");
        p.props.frame = delivery_frame.clone();
        if delivery_frame.is_none() {
            p.props.indent_left = Some(w * 0.42);
        }
        if let Some(prev) = last.replace(p) {
            blocks.push(para_block(prev));
        }
    }
    if p::bool(v, "add") == Some(true) {
        // The envelope's own section ends with its last address line.
        let mut end = last.unwrap_or_else(|| Paragraph::new().styled("NoSpacing"));
        end.section = Some(Box::new(sect));
        blocks.push(para_block(end));
        let n = blocks.len();
        delivery_font.apply(&mut s.doc, "EnvelopeAddress", "envelope address", 12.0);
        return_font.apply(&mut s.doc, "EnvelopeReturn", "envelope return", 10.0);
        s.doc.body.splice(0..0, blocks);
        s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
        return Ok(json!({"width": w, "height": h, "added": n}));
    }
    blocks.extend(last.map(para_block));
    let mut d = Document::new();
    delivery_font.apply(&mut d, "EnvelopeAddress", "envelope address", 12.0);
    return_font.apply(&mut d, "EnvelopeReturn", "envelope return", 10.0);
    d.last_section = sect;
    d.body = blocks;
    s.add_document(d);
    Ok(json!({"width": w, "height": h}))
}

/// A sheet of labels: the label size, how many across and down, and where the first one sits.
/// Lengths in points; the pitch is from one label's edge to the next one's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelProduct {
    pub id: &'static str,
    /// What the labels are for: `address`, `shipping` or `return`.
    pub kind: &'static str,
    /// Letter or A4.
    pub paper: &'static str,
    pub cols: usize,
    pub rows: usize,
    pub width: f32,
    pub height: f32,
    pub top: f32,
    pub side: f32,
    pub h_pitch: f32,
    pub v_pitch: f32,
}

const MM: f32 = 72.0 / 25.4;

/// Common label sheets (the sizes are the industry's; the names are ours).
pub const LABEL_PRODUCTS: [LabelProduct; 9] = [
    LabelProduct {
        id: "letter-3x10",
        kind: "address",
        paper: "Letter",
        cols: 3,
        rows: 10,
        width: 189.0,
        height: 72.0,
        top: 36.0,
        side: 13.5,
        h_pitch: 198.0,
        v_pitch: 72.0,
    },
    LabelProduct {
        id: "letter-2x10",
        kind: "address",
        paper: "Letter",
        cols: 2,
        rows: 10,
        width: 288.0,
        height: 72.0,
        top: 36.0,
        side: 11.25,
        h_pitch: 301.5,
        v_pitch: 72.0,
    },
    LabelProduct {
        id: "letter-2x7",
        kind: "address",
        paper: "Letter",
        cols: 2,
        rows: 7,
        width: 288.0,
        height: 96.0,
        top: 60.0,
        side: 11.25,
        h_pitch: 301.5,
        v_pitch: 96.0,
    },
    LabelProduct {
        id: "letter-2x5",
        kind: "shipping",
        paper: "Letter",
        cols: 2,
        rows: 5,
        width: 288.0,
        height: 144.0,
        top: 36.0,
        side: 11.25,
        h_pitch: 301.5,
        v_pitch: 144.0,
    },
    LabelProduct {
        id: "letter-4x20",
        kind: "return",
        paper: "Letter",
        cols: 4,
        rows: 20,
        width: 126.0,
        height: 36.0,
        top: 36.0,
        side: 21.6,
        h_pitch: 147.6,
        v_pitch: 36.0,
    },
    LabelProduct {
        id: "a4-3x7",
        kind: "address",
        paper: "A4",
        cols: 3,
        rows: 7,
        width: 63.5 * MM,
        height: 38.1 * MM,
        top: 15.15 * MM,
        side: 7.21 * MM,
        h_pitch: 66.04 * MM,
        v_pitch: 38.1 * MM,
    },
    LabelProduct {
        id: "a4-2x7",
        kind: "address",
        paper: "A4",
        cols: 2,
        rows: 7,
        width: 99.1 * MM,
        height: 38.1 * MM,
        top: 15.15 * MM,
        side: 4.65 * MM,
        h_pitch: 101.6 * MM,
        v_pitch: 38.1 * MM,
    },
    LabelProduct {
        id: "a4-3x8",
        kind: "address",
        paper: "A4",
        cols: 3,
        rows: 8,
        width: 63.5 * MM,
        height: 33.9 * MM,
        top: 12.9 * MM,
        side: 7.21 * MM,
        h_pitch: 66.04 * MM,
        v_pitch: 33.9 * MM,
    },
    LabelProduct {
        id: "a4-2x4",
        kind: "shipping",
        paper: "A4",
        cols: 2,
        rows: 4,
        width: 99.1 * MM,
        height: 67.7 * MM,
        top: 13.1 * MM,
        side: 4.65 * MM,
        h_pitch: 101.6 * MM,
        v_pitch: 67.7 * MM,
    },
];

impl LabelProduct {
    pub fn by_id(id: &str) -> Option<&'static LabelProduct> {
        LABEL_PRODUCTS.iter().find(|p| p.id == id)
    }
    /// Label size as people say it: inches on Letter, millimetres on A4.
    pub fn size_text(&self) -> String {
        if self.paper == "A4" {
            format!("{:.1} × {:.1} mm", self.width / MM, self.height / MM)
        } else {
            let i = |pt: f32| format!("{}", ((pt / 72.0) * 1000.0).round() / 1000.0);
            format!("{}\" × {}\"", i(self.width), i(self.height))
        }
    }
}

/// Validated sheet geometry, independent of a product's static display metadata.
struct LabelSheet {
    geometry: LabelProduct,
    page_w: f32,
    page_h: f32,
}

impl LabelSheet {
    fn read(v: &Value) -> Result<Self, CmdError> {
        let Some(custom) = v.get("custom") else {
            let mut geometry = match p::str(v, "product") {
                Some(id) => *LabelProduct::by_id(id).ok_or_else(|| CmdError::Params(format!("unknown label product `{id}`")))?,
                None => LABEL_PRODUCTS[0],
            };
            geometry.rows = p::u64(v, "rows").map_or(geometry.rows, |n| n.clamp(1, 40) as usize);
            geometry.cols = p::u64(v, "cols").map_or(geometry.cols, |n| n.clamp(1, 10) as usize);
            let (page_w, page_h) = if geometry.paper == "A4" { (595.3, 841.9) } else { (612.0, 792.0) };
            return Ok(Self { geometry, page_w, page_h });
        };
        if !custom.is_object() {
            return Err(CmdError::Params("custom must be an object".into()));
        }
        if custom.get("name").is_some_and(|n| !n.is_string()) {
            return Err(CmdError::Params("custom name must be a string".into()));
        }
        let number = |key: &str, default: Option<f32>, positive: bool| -> Result<f32, CmdError> {
            let n = match custom.get(key) {
                None => default,
                Some(v) => v.as_f64().filter(|n| n.is_finite() && if positive { *n > 0.0 } else { *n >= 0.0 }).map(|n| n as f32),
            };
            n.filter(|n| n.is_finite() && if positive { *n > 0.0 } else { *n >= 0.0 }).ok_or_else(|| {
                CmdError::Params(format!("custom {key} must be a {} number in points", if positive { "positive" } else { "nonnegative" }))
            })
        };
        let count = |key: &str, max: u64| -> Result<usize, CmdError> {
            custom
                .get(key)
                .and_then(Value::as_u64)
                .filter(|n| (1..=max).contains(n))
                .map(|n| n as usize)
                .ok_or_else(|| CmdError::Params(format!("custom {key} must be an integer from 1 to {max}")))
        };
        let width = number("width", None, true)?;
        let height = number("height", None, true)?;
        let geometry = LabelProduct {
            id: "custom",
            kind: "address",
            paper: "",
            width,
            height,
            top: number("top", None, false)?,
            side: number("side", None, false)?,
            h_pitch: number("hPitch", Some(width), true)?,
            v_pitch: number("vPitch", Some(height), true)?,
            cols: count("cols", 10)?,
            rows: count("rows", 40)?,
        };
        if geometry.h_pitch < width || geometry.v_pitch < height {
            return Err(CmdError::Params("Label pitch must be at least the label size.".into()));
        }
        let page_w = number("pageWidth", Some(612.0), true)?;
        let page_h = number("pageHeight", Some(792.0), true)?;
        // Compute in f64 so hostile finite f32 lengths cannot overflow during the fit check.
        let extent = |margin: f32, pitch: f32, count: usize, size: f32| f64::from(margin) + f64::from(pitch) * (count - 1) as f64 + f64::from(size);
        if extent(geometry.side, geometry.h_pitch, geometry.cols, width) > f64::from(page_w) + 0.01
            || extent(geometry.top, geometry.v_pitch, geometry.rows, height) > f64::from(page_h) + 0.01
        {
            return Err(CmdError::Params("The labels don't fit on the page.".into()));
        }
        Ok(Self { geometry, page_w, page_h })
    }
}

/// Labels: a new document with a table of labels laid out like the sheet (`product`; spacer
/// columns and rows stand for the gaps). A full page of the same `text`, as many sheets as
/// needed for one label per recipient, or (`single`: `{"row", "col"}`, from 1) one label
/// at that place on the sheet.
pub(super) fn labels(s: &mut Session, v: &Value) -> CmdResult {
    let sheet = LabelSheet::read(v)?;
    let prod = sheet.geometry;
    let (rows, cols) = (prod.rows, prod.cols);
    let single = v.get("single").filter(|x| !x.is_null()).map(|x| {
        let at = |k: &str| x.get(k).and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        (at("row").min(rows), at("col").min(cols))
    });
    let from_rec = single.is_none() && p::bool(v, "fromRecipients").unwrap_or(!s.merge.rows.is_empty() && p::str(v, "text").is_none());
    let text = label_text(v, "text")?.unwrap_or_else(|| "Name\nAddress\nCity, ST 00000".into());
    let (page_w, page_h) = (sheet.page_w, sheet.page_h);
    let h_gap = (prod.h_pitch - prod.width).max(0.0);
    let v_gap = (prod.v_pitch - prod.height).max(0.0);
    let mut d = Document::new();
    d.last_section.page_w = page_w;
    d.last_section.page_h = page_h;
    d.last_section.margin_top = prod.top;
    d.last_section.margin_bottom = 0.0;
    d.last_section.margin_left = prod.side;
    d.last_section.margin_right = (page_w - prod.side - prod.h_pitch * (cols - 1) as f32 - prod.width).max(0.0);
    d.last_section.header = 0.0;
    d.last_section.footer = 0.0;
    // Grid columns: label, gap, label, …; rows likewise.
    let grid_cols: Vec<Option<usize>> = (0..cols).flat_map(|c| [Some(c)].into_iter().chain((c + 1 < cols && h_gap > 0.5).then_some(None))).collect();
    let grid_rows: Vec<Option<usize>> = (0..rows).flat_map(|r| [Some(r)].into_iter().chain((r + 1 < rows && v_gap > 0.5).then_some(None))).collect();
    let width: f32 = grid_cols.iter().map(|c| if c.is_some() { prod.width } else { h_gap }).sum();
    let per_sheet = rows * cols;
    let sheets = if from_rec { s.merge.rows.len().div_ceil(per_sheet).max(1) } else { 1 };
    let mut k = 0usize;
    let mut made = 0usize;
    d.body.clear();
    for sheet in 0..sheets {
        let mut t = Table::new(grid_rows.len(), grid_cols.len(), width);
        t.props.style = None;
        t.props.borders = Some(wordcraft_doc::props::Borders::default());
        t.props.fixed = true;
        t.grid = grid_cols.iter().map(|c| if c.is_some() { prod.width } else { h_gap }).collect();
        for (row, r) in t.rows.iter_mut().zip(&grid_rows) {
            row.props.height = Some(if r.is_some() { prod.height } else { v_gap });
            row.props.height_rule = wordcraft_doc::props::HeightRule::Exact;
            for (cell, c) in row.cells.iter_mut().zip(&grid_cols) {
                cell.props.width = Some(if c.is_some() { prod.width } else { h_gap });
                let (Some(r), Some(c)) = (r, c) else { continue };
                let content = match single {
                    Some((sr, sc)) if (sr, sc) != (r + 1, c + 1) => String::new(),
                    _ if from_rec => {
                        let rec = if k < s.merge.rows.len() { address_block(s, k) } else { String::new() };
                        k += 1;
                        rec
                    }
                    _ => text.clone(),
                };
                if !content.is_empty() {
                    made += 1;
                }
                cell.props.valign = Some(wordcraft_doc::props::VAlign::Center);
                cell.blocks = content
                    .split('\n')
                    .take(20)
                    .map(|l| {
                        let mut p = Paragraph::with_text(l, CharProps::default()).styled("NoSpacing");
                        p.props.align = Some(Align::Left);
                        p.props.indent_left = Some(9.0);
                        para_block(p)
                    })
                    .collect();
            }
        }
        d.body.push(std::sync::Arc::new(Block::Table(t)));
        let mut end = Paragraph::new();
        if sheet + 1 < sheets {
            // End each sheet with the same next-page section, so the next table
            // starts at the product's top margin without a leading blank line.
            end.section = Some(Box::new(d.last_section.clone()));
        }
        d.body.push(para_block(end));
    }
    s.add_document(d);
    let _ = Pos::body(0, 0);
    Ok(json!({"labels": if single.is_some() { 1 } else { sheets * per_sheet }, "filled": made, "product": prod.id}))
}

#[cfg(test)]
mod custom_tests {
    use super::*;

    #[test]
    fn invalid_custom_measurements_do_not_create_documents() {
        let valid = json!({"top": 0, "side": 0, "width": 100, "height": 50, "cols": 2, "rows": 3});
        for (key, bad) in [
            ("top", json!(-1e-100)),
            ("side", json!(-1e-100)),
            ("width", json!(1e100)),
            ("height", Value::Null),
            ("cols", json!(2.5)),
            ("cols", json!(11)),
            ("rows", json!(41)),
            ("pageWidth", json!(0)),
            ("pageHeight", json!("Letter")),
            ("hPitch", json!(99)),
            ("vPitch", json!(49)),
        ] {
            let mut custom = valid.clone();
            custom[key] = bad;
            let mut s = Session::new(Document::from_text("Keep this"));
            assert!(matches!(labels(&mut s, &json!({"custom": custom})), Err(CmdError::Params(_))), "{key}");
            assert_eq!(s.doc.plain_text(StoryRef::Body).trim(), "Keep this");
        }
    }

    #[test]
    fn custom_counts_override_product_and_top_level_counts() {
        let mut s = Session::new(Document::new());
        let result = labels(
            &mut s,
            &json!({"text": "Label", "product": "unknown", "rows": 40, "cols": 10,
            "custom": {"top": 0, "side": 0, "width": 100, "height": 50, "cols": 2, "rows": 3}}),
        )
        .unwrap();
        assert_eq!(result["labels"], 6);
        assert_eq!(result["filled"], 6);
    }
}
