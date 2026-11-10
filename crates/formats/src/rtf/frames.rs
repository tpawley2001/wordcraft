//! Positioned frames and drop caps (RTF 1.9.1, "Positioned Objects and Frames"): the paragraph
//! words read into [`FrameSt`], and the words written for a paragraph's frame or drop cap.

use super::Reader;
use wordcraft_doc::props::{ParaProps, TextFrame};

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
}

fn len(twips: i32) -> f32 {
    (twips as f32 / 20.0).clamp(-MAX_LEN, MAX_LEN)
}

fn space(twips: Option<i32>) -> f32 {
    twips.map(len).unwrap_or(0.0).max(0.0)
}

impl FrameSt {
    fn new() -> Self {
        let frame = TextFrame { h_anchor: "text".into(), v_anchor: "margin".into(), wrap: "auto".into(), ..Default::default() };
        FrameSt { frame, ..Default::default() }
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
        true
    }

    /// Sets the drop cap, or else the frame, these words give on `x`.
    pub(super) fn apply(&self, x: &mut ParaProps) {
        if let Some(lines) = self.drop_lines {
            x.drop_cap = Some(lines);
            x.drop_cap_margin = self.drop_margin.then_some(true);
            x.drop_cap_distance = self.dist.filter(|d| *d > 0).map(|d| (d as f32 / 20.0).min(wordcraft_doc::resolve::MAX_DROP_CAP_DISTANCE));
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
            let mut f = FrameSt::new();
            if !f.word(w, pv) {
                return false;
            }
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
