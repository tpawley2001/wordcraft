//! Positioned frames and drop caps (RTF 1.9.1, "Positioned Objects and Frames"): the paragraph
//! words read into [`FrameSt`], and the words written for a paragraph's frame or drop cap.
//!
//! RTF keeps a drop cap as Word does in DOCX: a positioned one-letter paragraph (`\dropcapli`
//! with a position word), then the paragraph it starts. The model holds the two as one
//! paragraph: [`merge_drop_caps`] joins them after import and [`split_drop_caps`] parts them
//! again before export. Between the paragraph words and those passes the cap paragraph carries
//! a marker frame ([`CAP_MARK`]), the one place the flow model keeps for it.

use super::Reader;
use crate::model::{FBlock, MAX_DEPTH};
use std::sync::Arc;
use wordcraft_doc::props::{LineSpacing, ParaProps, TextFrame};
use wordcraft_doc::{Block, Blocks, Document, Paragraph};

/// Longest frame length kept, points (as the DOCX reader clamps `w:framePr`).
const MAX_LEN: f32 = 1584.0;

/// A paragraph's frame and drop cap words, until `\pard` resets them.
#[derive(Clone, Debug, Default)]
pub(super) struct FrameSt {
    /// Some frame word was read: the paragraph is framed (unless it is a drop cap).
    pub(super) framed: bool,
    pub(super) frame: TextFrame,
    /// `\dxfrtext`, and the per-axis `\dfrmtxtx` / `\dfrmtxty` that override it, twips.
    pub(super) dist: Option<i32>,
    pub(super) dist_x: Option<i32>,
    pub(super) dist_y: Option<i32>,
    /// `\dropcapli` (lines) and whether `\dropcapt2` puts the cap in the margin.
    pub(super) drop_lines: Option<u8>,
    pub(super) drop_margin: bool,
    /// A position or size word was read (`\pvpara`, `\phmrg`, `\posxl`, `\absh`…): Word shows
    /// a `\dropcapli` paragraph as a drop cap only with one.
    pub(super) placed: bool,
    /// The horizontal anchor word read (`\phmrg`, `\phpg`, `\phcol`): `margin`, `page`, `text`.
    pub(super) h_anchor: Option<&'static str>,
}

/// The vertical anchor of the marker frame a drop cap's own paragraph carries between the
/// paragraph words and [`merge_drop_caps`] (import), or between [`split_drop_caps`] and
/// [`frame_words`] (export). No file can give an anchor holding a NUL.
const CAP_MARK: &str = "\u{0}drop cap";

/// Longest text of a cap paragraph that joins the paragraph after it, bytes (as the DOCX reader).
const MAX_CAP_TEXT: usize = 16;

/// The words that place or size a frame: one of them makes a `\dropcapli` paragraph a cap.
const PLACE_WORDS: [&str; 23] = [
    "pvpara", "pvmrg", "pvpg", "phmrg", "phpg", "phcol", "posx", "posnegx", "posxl", "posxc", "posxr", "posxi", "posxo", "posy", "posnegy", "posyt",
    "posyc", "posyb", "posyil", "posyin", "posyout", "absw", "absh",
];

fn len(twips: i32) -> f32 {
    (twips as f32 / 20.0).clamp(-MAX_LEN, MAX_LEN)
}

fn space(twips: Option<i32>) -> f32 {
    twips.map(len).unwrap_or(0.0).max(0.0)
}

impl FrameSt {
    /// Fills the anchors and wrap no word set with RTF's defaults.
    fn fill_defaults(&mut self) {
        let f = &mut self.frame;
        for (s, d) in [(&mut f.h_anchor, "text"), (&mut f.v_anchor, "margin"), (&mut f.wrap, "auto")] {
            if s.is_empty() {
                *s = d.into();
            }
        }
    }

    /// Applies the frame or drop cap word `w` with parameter `pv`; false when `w` is neither.
    fn word(&mut self, w: &str, pv: i32) -> bool {
        let f = &mut self.frame;
        let set = |s: &str| Some(s.to_string());
        match w {
            "dropcapli" => self.drop_lines = Some(pv.clamp(1, 10) as u8),
            "dropcapt" => self.drop_margin = pv == 2,
            "dxfrtext" => self.dist = Some(pv),
            "dfrmtxtx" => self.dist_x = Some(pv),
            "dfrmtxty" => self.dist_y = Some(pv),
            _ => match w {
                "absw" => f.w = (pv > 0).then(|| len(pv)),
                "absh" => (f.h, f.h_exact) = (if pv == 0 { None } else { Some(len(pv.saturating_abs())) }, pv < 0),
                "phmrg" => f.h_anchor = "margin".into(),
                "phpg" => f.h_anchor = "page".into(),
                "phcol" => f.h_anchor = "text".into(),
                "pvmrg" => f.v_anchor = "margin".into(),
                "pvpg" => f.v_anchor = "page".into(),
                "pvpara" => f.v_anchor = "text".into(),
                "posx" | "posnegx" => (f.x, f.x_align) = (len(pv), None),
                "posxl" => f.x_align = set("left"),
                "posxc" => f.x_align = set("center"),
                "posxr" => f.x_align = set("right"),
                "posxi" => f.x_align = set("inside"),
                "posxo" => f.x_align = set("outside"),
                "posy" | "posnegy" => (f.y, f.y_align) = (len(pv), None),
                "posyt" => f.y_align = set("top"),
                "posyc" => f.y_align = set("center"),
                "posyb" => f.y_align = set("bottom"),
                "posyin" => f.y_align = set("inside"),
                "posyout" => f.y_align = set("outside"),
                "posyil" => (f.y, f.y_align) = (0.0, None),
                "abslock" => f.anchor_lock = Some(true),
                "wrapdefault" => f.wrap = "auto".into(),
                "wraparound" => f.wrap = "around".into(),
                "wraptight" => f.wrap = "tight".into(),
                "wrapthrough" => f.wrap = "through".into(),
                "nowrap" => f.wrap = "notBeside".into(),
                _ => return false,
            },
        }
        if !w.starts_with("dropcap") {
            self.framed = true;
        }
        if PLACE_WORDS.contains(&w) {
            self.placed = true;
        }
        match w {
            "phmrg" => self.h_anchor = Some("margin"),
            "phpg" => self.h_anchor = Some("page"),
            "phcol" => self.h_anchor = Some("text"),
            _ => {}
        }
        true
    }

    /// Sets the drop cap, or else the frame, these words give on `x`.
    pub(super) fn apply(&self, x: &mut ParaProps) {
        if let Some(lines) = self.drop_lines {
            let distance = self.dist.filter(|d| *d > 0).map(|d| (d as f32 / 20.0).min(wordcraft_doc::resolve::MAX_DROP_CAP_DISTANCE));
            if self.placed {
                // A cap paragraph: marked until `merge_drop_caps` joins it to its text.
                x.frame = Some(
                    Cap { lines, anchor: self.h_anchor.unwrap_or_default().into(), dropcapt_margin: self.drop_margin, distance, height: None }.mark(),
                );
            } else {
                x.drop_cap = Some(lines);
                x.drop_cap_margin = self.drop_margin.then_some(true);
                x.drop_cap_distance = distance;
            }
        } else if self.framed {
            let mut f = self.frame.clone();
            f.h_space = space(self.dist_x.or(self.dist));
            f.v_space = space(self.dist_y.or(self.dist));
            x.frame = Some(f);
        }
    }
}

impl Reader {
    /// Reads a frame or drop cap paragraph word; false when `w` is neither.
    pub(super) fn frame_word(&mut self, w: &str, pv: i32) -> bool {
        if self.st.frame.is_none() {
            // Most words reach here and aren't frame words: try them without allocating.
            let mut f = FrameSt::default();
            if !f.word(w, pv) {
                return false;
            }
            f.fill_defaults();
            self.st.frame = Some(Box::new(f));
            return true;
        }
        self.st.frame.as_mut().is_some_and(|f| f.word(w, pv))
    }
}

fn twips(pt: f32) -> i64 {
    (pt.clamp(-MAX_LEN, MAX_LEN) * 20.0).round() as i64
}

/// The frame or drop cap words of a paragraph with formatting `x`.
pub(super) fn frame_words(x: &ParaProps) -> String {
    let mut out = String::new();
    if let Some(cap) = x.frame.as_ref().and_then(Cap::read) {
        return cap.words();
    }
    if let Some(lines) = x.drop_cap.filter(|l| *l > 0) {
        let kind = if x.drop_cap_margin == Some(true) { 2 } else { 1 };
        out.push_str(&format!("\\dropcapli{}\\dropcapt{kind}", lines.min(10)));
        if let Some(d) = x.drop_cap_distance.filter(|d| d.is_finite() && *d > 0.0) {
            out.push_str(&format!("\\dxfrtext{}", twips(d)));
        }
        return out;
    }
    let Some(f) = &x.frame else { return out };
    if let Some(w) = f.w.filter(|w| w.is_finite() && *w > 0.0) {
        out.push_str(&format!("\\absw{}", twips(w)));
    }
    if let Some(h) = f.h.filter(|h| h.is_finite() && *h > 0.0) {
        let h = twips(h);
        out.push_str(&format!("\\absh{}", if f.h_exact { -h } else { h }));
    }
    out.push_str(match f.h_anchor.as_str() {
        "margin" => "\\phmrg",
        "page" => "\\phpg",
        _ => "\\phcol",
    });
    match f.x_align.as_deref() {
        Some("left") => out.push_str("\\posxl"),
        Some("center") => out.push_str("\\posxc"),
        Some("right") => out.push_str("\\posxr"),
        Some("inside") => out.push_str("\\posxi"),
        Some("outside") => out.push_str("\\posxo"),
        _ if f.x < 0.0 => out.push_str(&format!("\\posnegx{}", twips(f.x))),
        _ => out.push_str(&format!("\\posx{}", twips(f.x.max(0.0)))),
    }
    out.push_str(match f.v_anchor.as_str() {
        "margin" => "\\pvmrg",
        "page" => "\\pvpg",
        _ => "\\pvpara",
    });
    match f.y_align.as_deref() {
        Some("top") => out.push_str("\\posyt"),
        Some("center") => out.push_str("\\posyc"),
        Some("bottom") => out.push_str("\\posyb"),
        Some("inside") => out.push_str("\\posyin"),
        Some("outside") => out.push_str("\\posyout"),
        _ if f.y < 0.0 => out.push_str(&format!("\\posnegy{}", twips(f.y))),
        _ => out.push_str(&format!("\\posy{}", twips(f.y.max(0.0)))),
    }
    if f.anchor_lock == Some(true) {
        out.push_str("\\abslock");
    }
    for (word, v) in [("dfrmtxtx", f.h_space), ("dfrmtxty", f.v_space)] {
        if v.is_finite() && v > 0.0 {
            out.push_str(&format!("\\{word}{}", twips(v)));
        }
    }
    out.push_str(match f.wrap.as_str() {
        "around" => "\\wraparound",
        "tight" => "\\wraptight",
        "through" => "\\wrapthrough",
        "notBeside" | "none" => "\\nowrap",
        _ => "\\wrapdefault",
    });
    out
}

/// A drop cap's own paragraph, as its marker frame holds it.
#[derive(Clone, Debug, PartialEq)]
struct Cap {
    /// `\dropcapli`, 1..=10.
    lines: u8,
    /// The horizontal anchor word read (`margin`, `page`, `text`; empty for none).
    anchor: String,
    /// `\dropcapt2`.
    dropcapt_margin: bool,
    /// `\dxfrtext`, points, when above zero.
    distance: Option<f32>,
    /// The frame's exact height, points (export only).
    height: Option<f32>,
}

impl Cap {
    fn mark(&self) -> TextFrame {
        TextFrame {
            w: Some(f32::from(self.lines)),
            h: self.height,
            h_exact: self.dropcapt_margin,
            h_anchor: self.anchor.clone(),
            v_anchor: CAP_MARK.into(),
            h_space: self.distance.unwrap_or(0.0),
            ..Default::default()
        }
    }

    fn read(f: &TextFrame) -> Option<Cap> {
        if f.v_anchor != CAP_MARK {
            return None;
        }
        let lines = f.w.filter(|l| l.is_finite()).map_or(1, |l| l.clamp(1.0, 10.0) as u8);
        Some(Cap {
            lines,
            anchor: f.h_anchor.clone(),
            dropcapt_margin: f.h_exact,
            distance: Some(f.h_space).filter(|d| d.is_finite() && *d > 0.0),
            height: f.h.filter(|h| h.is_finite() && *h > 0.0),
        })
    }

    /// Whether the cap hangs in the margin: the page anchor puts it there and the margin or
    /// column anchor drops it into the text, whatever `\dropcapt` says; without an anchor
    /// `\dropcapt2` decides.
    fn in_margin(&self) -> bool {
        match self.anchor.as_str() {
            "page" => true,
            "margin" | "text" => false,
            _ => self.dropcapt_margin,
        }
    }

    /// The drop cap of a cap paragraph that stays on its own (`\dropcapt` decides, as before).
    fn set_alone(&self, x: &mut ParaProps) {
        x.drop_cap = Some(self.lines);
        x.drop_cap_margin = self.dropcapt_margin.then_some(true);
        x.drop_cap_distance = self.distance;
    }

    /// The words of the cap paragraph Word shows as a drop cap: the cap's lines, where it is
    /// and its exact height (its line spacing, written with the paragraph's spacing, matches).
    fn words(&self) -> String {
        let margin = self.anchor == "page";
        let mut out = format!("\\dropcapli{}\\dropcapt{}\\pvpara", self.lines.clamp(1, 10), if margin { 2 } else { 1 });
        out.push_str(if margin { "\\phpg" } else { "\\phmrg\\posxl" });
        if let Some(h) = self.height {
            out.push_str(&format!("\\absh-{}", twips(h)));
        }
        if let Some(d) = self.distance {
            out.push_str(&format!("\\dxfrtext{}", twips(d)));
        }
        out
    }
}

fn cap_of(x: &ParaProps) -> Option<Cap> {
    x.frame.as_ref().and_then(Cap::read)
}

/// Gives a marked cap paragraph's formatting before a tracked change its drop cap back.
fn settle_old(x: &mut ParaProps) {
    if let Some(old) = x.fmt_change.as_deref_mut().map(|c| &mut c.old)
        && let Some(cap) = cap_of(old)
    {
        old.frame = None;
        cap.set_alone(old);
    }
}

fn marked(p: &Paragraph) -> bool {
    cap_of(&p.props).is_some() || p.props.fmt_change.as_deref().is_some_and(|c| cap_of(&c.old).is_some())
}

/// Joins each drop cap paragraph read from RTF to the paragraph it starts, in every story (the
/// body, table cells, headers, footers, notes, comments and text boxes), as the DOCX reader
/// joins a framed drop cap: the cap paragraph has at most [`MAX_CAP_TEXT`] bytes, doesn't end a
/// section, and is followed in its own story or cell by a paragraph that isn't a drop cap. The
/// joined paragraph has the second paragraph's formatting and the cap's drop cap. Cap
/// paragraphs that join nothing keep their drop cap on their own.
pub(super) fn merge_drop_caps(doc: &mut Document) {
    merge_in(&mut doc.body, 0);
    for part in doc.parts.values_mut() {
        merge_in(&mut part.blocks, 0);
    }
}

fn has_marks(bl: &Blocks, depth: usize) -> bool {
    bl.iter().any(|b| match &**b {
        Block::Para(p) => marked(p),
        Block::Table(t) => depth < MAX_DEPTH && t.rows.iter().flat_map(|r| &r.cells).any(|c| has_marks(&c.blocks, depth + 1)),
    })
}

fn merge_in(bl: &mut Blocks, depth: usize) {
    if !has_marks(bl, depth) {
        return;
    }
    let old = std::mem::take(bl);
    bl.reserve(old.len());
    let mut it = old.into_iter().peekable();
    while let Some(mut b) = it.next() {
        let Block::Para(p) = Arc::make_mut(&mut b) else {
            if depth < MAX_DEPTH
                && let Block::Table(t) = Arc::make_mut(&mut b)
            {
                for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    merge_in(&mut c.blocks, depth + 1);
                }
            }
            bl.push(b);
            continue;
        };
        settle_old(&mut p.props);
        let Some(cap) = cap_of(&p.props) else {
            bl.push(b);
            continue;
        };
        p.props.frame = None;
        let joins = p.text.len() <= MAX_CAP_TEXT
            && p.section.is_none()
            && it.peek().is_some_and(|n| matches!(&**n, Block::Para(q) if q.props.drop_cap.is_none() && cap_of(&q.props).is_none()));
        let next = if joins { it.next() } else { None };
        match next.map(Arc::unwrap_or_clone) {
            Some(Block::Para(body)) => join(p, body, &cap),
            Some(other) => {
                cap.set_alone(&mut p.props);
                bl.push(b);
                b = Arc::new(other);
            }
            None => cap.set_alone(&mut p.props),
        }
        bl.push(b);
    }
}

/// Joins cap paragraph `first` and the paragraph `body` it starts into `first`.
fn join(first: &mut Paragraph, mut body: Paragraph, cap: &Cap) {
    let mut props = std::mem::take(&mut body.props);
    props.frame = None;
    props.drop_cap = Some(cap.lines);
    props.drop_cap_margin = cap.in_margin().then_some(true);
    props.drop_cap_distance = cap.distance;
    if props.drop_cap_margin == Some(true) && first.props.indent_first != props.indent_first {
        props.drop_cap_indent_first = first.props.indent_first;
    }
    // A letter in a font of its own: the drop cap's font, the letter itself in the text's.
    let cap_font = first.runs.first().and_then(|r| r.props.font.clone());
    let text_font = body.runs.first().and_then(|r| r.props.font.clone());
    if cap_font.is_some() && cap_font != text_font {
        for r in first.runs.iter_mut().filter(|r| r.props.font == cap_font) {
            r.props.font = text_font.clone();
        }
        props.drop_cap_font = cap_font.map(String::into_boxed_str);
    }
    settle_old(&mut props);
    let mark = std::mem::take(&mut body.mark);
    first.append(body);
    first.props = props;
    first.mark = mark;
}

/// Gives the marked cap paragraphs of a flow read without the import passes their drop cap on
/// their own (as [`merge_drop_caps`] leaves those that join nothing).
pub(super) fn settle_flow(blocks: &mut [FBlock], depth: usize) {
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                for x in std::iter::once(p.extra.as_deref_mut())
                    .chain(std::iter::once(p.fmt_change.as_deref_mut().and_then(|c| c.old.extra.as_deref_mut())))
                    .flatten()
                {
                    if let Some(cap) = cap_of(x) {
                        x.frame = None;
                        cap.set_alone(x);
                    }
                }
            }
            FBlock::Table(t) if depth < MAX_DEPTH => {
                for c in t.rows.iter_mut().flatten() {
                    settle_flow(&mut c.blocks, depth + 1);
                }
            }
            FBlock::Table(_) => {}
        }
    }
}

/// The document to write as RTF: each drop cap paragraph (with more text than its first
/// character, which is ordinary text) as two paragraphs, the cap paragraph holding the letter
/// (marked for [`frame_words`]) and the text paragraph without the drop cap, as Word stores
/// them. `None` when there is none to split.
pub(super) fn split_drop_caps(doc: &Document) -> Option<Document> {
    if !needs_split(&doc.body, 0) && !doc.parts.values().any(|p| needs_split(&p.blocks, 0)) {
        return None;
    }
    let mut d = doc.clone();
    split_in(&mut d.body, 0);
    for part in d.parts.values_mut() {
        split_in(&mut part.blocks, 0);
    }
    Some(d)
}

fn needs_split(bl: &Blocks, depth: usize) -> bool {
    bl.iter().any(|b| match &**b {
        Block::Para(p) => cap_letter(p).is_some(),
        Block::Table(t) => depth < MAX_DEPTH && t.rows.iter().flat_map(|r| &r.cells).any(|c| needs_split(&c.blocks, depth + 1)),
    })
}

/// The byte length of the drop cap letter of `p`, when `p` is written as two paragraphs.
fn cap_letter(p: &Paragraph) -> Option<usize> {
    p.props.drop_cap.filter(|l| *l > 0)?;
    let first = p.text.chars().next().filter(|c| !matches!(*c, wordcraft_doc::para::OBJ | '\t' | '\n' | '\u{c}' | '\u{e}'))?;
    (p.text.len() > first.len_utf8()).then_some(first.len_utf8())
}

fn split_in(bl: &mut Blocks, depth: usize) {
    if !needs_split(bl, depth) {
        return;
    }
    let old = std::mem::take(bl);
    bl.reserve(old.len().saturating_add(1));
    for mut b in old {
        match &*b {
            Block::Para(p) => {
                if let Some((cap, body)) = split(p) {
                    bl.push(Arc::new(Block::Para(cap)));
                    bl.push(Arc::new(Block::Para(body)));
                    continue;
                }
            }
            Block::Table(_) if depth < MAX_DEPTH => {
                if let Block::Table(t) = Arc::make_mut(&mut b) {
                    for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                        split_in(&mut c.blocks, depth + 1);
                    }
                }
            }
            Block::Table(_) => {}
        }
        bl.push(b);
    }
}

/// The cap paragraph and the text paragraph `p` is written as.
fn split(p: &Paragraph) -> Option<(Paragraph, Paragraph)> {
    let at = cap_letter(p)?;
    let lines = p.props.drop_cap?.clamp(1, 10);
    let mut head = p.clone();
    let mut body = head.split_off(at).ok()?;
    // Word's cap is as tall as its lines of the text's first run: 23 twips a point a line.
    let size = body.runs.first().and_then(|r| r.props.size).filter(|s| s.is_finite() && *s > 0.0).map_or(12.0, |s| s.min(1638.0));
    let height = f32::from(lines) * (size * 23.0).round() / 20.0;
    let margin = p.props.drop_cap_margin == Some(true);
    let cap = Cap {
        lines,
        anchor: if margin { "page" } else { "margin" }.into(),
        dropcapt_margin: margin,
        distance: p.props.drop_cap_distance.filter(|d| d.is_finite() && *d > 0.0),
        height: Some(height),
    };
    head.props = ParaProps { line_spacing: Some(LineSpacing::Exactly(height)), frame: Some(cap.mark()), ..Default::default() };
    if margin {
        head.props.indent_first = p.props.drop_cap_indent_first;
    }
    if let Some(f) = p.props.drop_cap_font.as_ref().filter(|f| !f.trim().is_empty()) {
        for r in &mut head.runs {
            r.props.font = Some(f.to_string());
        }
    }
    head.section = None;
    let x = &mut body.props;
    (x.drop_cap, x.drop_cap_margin, x.drop_cap_distance, x.drop_cap_font, x.drop_cap_indent_first) = (None, None, None, None, None);
    Some((head, body))
}

#[cfg(test)]
mod cap_paragraph_tests {
    use crate::rtf::{export, import, parse};
    use wordcraft_doc::{Block, Blocks, Document};

    const CAP: &str = r"\pard\plain\dropcapli3\dropcapt1\pvpara\phmrg\posxl\absh-828\sl-828\slmult0 ";

    fn open(body: &str) -> Document {
        import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}{{\f1 Arial;}}}}{body}}}").as_bytes()).unwrap()
    }

    fn texts(bl: &Blocks) -> Vec<(String, Option<u8>)> {
        bl.iter().filter_map(|b| b.as_para()).filter(|p| !p.text.is_empty()).map(|p| (p.text.clone(), p.props.drop_cap)).collect()
    }

    fn cells(d: &Document) -> Vec<Vec<(String, Option<u8>)>> {
        d.body
            .iter()
            .filter_map(|b| match &**b {
                Block::Table(t) => Some(t),
                Block::Para(_) => None,
            })
            .flat_map(|t| t.rows.iter().flat_map(|r| r.cells.iter().map(|c| texts(&c.blocks))))
            .collect()
    }

    fn no_marker_frames(d: &Document) -> bool {
        fn walk(bl: &Blocks) -> bool {
            bl.iter().all(|b| match &**b {
                Block::Para(p) => p.props.frame.is_none(),
                Block::Table(t) => t.rows.iter().flat_map(|r| &r.cells).all(|c| walk(&c.blocks)),
            })
        }
        walk(&d.body) && d.parts.values().all(|p| walk(&p.blocks))
    }

    #[test]
    fn a_cap_in_a_cell_joins_the_next_paragraph_of_that_cell_only() {
        let row = format!(
            r"\trowd\cellx3000\cellx6000\cellx9000{CAP}\intbl D\par\pard\plain\intbl rop.\cell{CAP}\intbl E\cell\pard\plain\intbl nd\cell\row"
        );
        let d = open(&row);
        // The second cell's cap is the last paragraph of its cell: the third cell's text stays there.
        assert_eq!(cells(&d), [[("Drop.".to_string(), Some(3))], [("E".to_string(), Some(3))], [("nd".to_string(), None)]]);
        assert!(no_marker_frames(&d));
    }

    #[test]
    fn a_cap_before_a_table_or_a_section_break_stays_alone() {
        let d = open(&format!(r"{CAP}D\par\trowd\cellx3000\pard\plain\intbl rop\cell\row"));
        assert_eq!(texts(&d.body), [("D".to_string(), Some(3))]);
        assert_eq!(cells(&d), [vec![("rop".to_string(), None)]]);
        let d = open(&format!(r"{CAP}D\sect\pard\plain rop.\par"));
        assert_eq!(texts(&d.body), [("D".to_string(), Some(3)), ("rop.".to_string(), None)]);
        // 16 bytes still join; 17 don't.
        let d = open(&format!(r"{CAP}ABCDEFGHIJKLMNOP\par\pard\plain rest.\par"));
        assert_eq!(texts(&d.body), [("ABCDEFGHIJKLMNOPrest.".to_string(), Some(3))]);
        let d = open(&format!(r"{CAP}ABCDEFGHIJKLMNOPQ\par\pard\plain rest.\par"));
        assert_eq!(texts(&d.body), [("ABCDEFGHIJKLMNOPQ".to_string(), Some(3)), ("rest.".to_string(), None)]);
        assert!(no_marker_frames(&d));
    }

    #[test]
    fn a_cap_in_a_header_joins_its_text() {
        let d = open(&format!(r"{{\header {CAP}H\par\pard\plain eader text\par}}\pard\plain Body\par"));
        let header: Vec<_> = d.parts.values().flat_map(|p| texts(&p.blocks)).collect();
        assert_eq!(header, [("Header text".to_string(), Some(3))]);
    }

    #[test]
    fn a_tracked_formatting_change_keeps_no_marker() {
        let d = open(&format!(r"{CAP}{{\*\oldpprops\pard\dropcapli2\dropcapt1\phmrg}}D\par\pard\plain rop.\par"));
        let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
        assert_eq!(p.text, "Drop.");
        assert!(no_marker_frames(&d));
        assert!(p.props.fmt_change.as_deref().is_none_or(|c| c.old.frame.is_none()));
    }

    #[test]
    fn the_flow_reader_gives_a_lone_cap_its_drop_cap_and_no_frame() {
        let flow = parse(format!(r"{{\rtf1 {CAP}D\par\pard\plain rop.\par}}").as_bytes()).unwrap();
        let x = flow.blocks.iter().find_map(|b| match b {
            crate::model::FBlock::Para(p) => p.extra.as_deref(),
            crate::model::FBlock::Table(_) => None,
        });
        let x = x.unwrap();
        assert_eq!((x.drop_cap, x.frame.as_ref()), (Some(3), None));
    }

    #[test]
    fn export_puts_the_drop_cap_font_on_the_letter_and_the_section_on_the_text() {
        let d = open(&format!(r"{CAP}{{\f1 D}}\par\pard\plain\fs20 rop.\sect\pard\plain Next.\par"));
        let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
        assert_eq!((p.text.as_str(), p.props.drop_cap_font.as_deref(), p.section.is_some()), ("Drop.", Some("Arial"), true));
        let rtf = export(&d);
        // 10 pt text: 3 x 230 twips.
        let cap = rtf.split("\\pard").find(|c| c.contains("\\dropcapli3")).unwrap();
        assert!(cap.contains("\\absh-690") && cap.contains("\\sl-690") && !cap.contains("\\sect"), "{cap}");
        let back = import(rtf.as_bytes()).unwrap();
        assert_eq!(texts(&back.body), [("Drop.".to_string(), Some(3)), ("Next.".to_string(), None)]);
        let p = back.body.iter().find_map(|b| b.as_para()).unwrap();
        assert_eq!((p.props.drop_cap_font.as_deref(), p.section.is_some()), (Some("Arial"), true));
    }

    #[test]
    fn hostile_drop_cap_words_do_not_panic() {
        for words in [r"\dropcapli-5\posx-99999999", r"\dropcapli99999\phpg\absh-2147483648", r"\dropcapli1\pvpg\dxfrtext-2147483648"] {
            let d = open(&format!(r"\pard{words} {{\fs99999 \u-1?}}\par\pard\fs0 x\par"));
            let _ = export(&d);
        }
    }
}
