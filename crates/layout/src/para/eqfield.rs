//! Lay out an EQ field (ECMA-376 Part 1 §17.16.5.20) from its parsed code
//! ([`wordcraft_doc::eq::parse_eq`]) as one atomic cluster: each part's box (width, ascent,
//! descent) is computed from its children, text is placed at offsets from the cluster's left
//! edge and baseline, and rules (fraction bars, radical bars, box sides, underlined gaps) are
//! drawn as lines over the cluster. Text at another size than the run's (radical indexes,
//! integral limits, stretched brackets) is drawn like ruby text, with its own style.

use super::builder::Builder;
use super::model::{ClKind, Cluster, Enclosed, Glyph, RubyLayout};
use super::{Arc, ResolvedChar};
use wordcraft_doc::eq::{EqNode, OAlign};

/// Text placed in the field: its left edge and baseline (up is positive) from the field's.
struct Piece {
    text: String,
    size: f32,
    x: f32,
    y: f32,
}

/// A rule from (`x`, `y`) to (`x + w`, `y - h`) (up is positive).
#[derive(Clone, Copy)]
struct Rule {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke: f32,
}

#[derive(Default)]
struct EqBox {
    w: f32,
    asc: f32,
    desc: f32,
    pieces: Vec<Piece>,
    rules: Vec<Rule>,
}

impl EqBox {
    /// Add `o` with its origin at (`dx`, `dy`), growing the extents to fit.
    fn put(&mut self, o: EqBox, dx: f32, dy: f32) {
        self.asc = self.asc.max(o.asc + dy);
        self.desc = self.desc.max(o.desc - dy);
        self.pieces.extend(o.pieces.into_iter().map(|p| Piece { x: p.x + dx, y: p.y + dy, ..p }));
        self.rules.extend(o.rules.into_iter().map(|r| Rule { x: r.x + dx, y: r.y + dy, ..r }));
    }
    /// Add `o` after what is there.
    fn append(&mut self, o: EqBox) {
        let (x, w) = (self.w, o.w);
        self.put(o, x, 0.0);
        self.w += w;
    }
}

/// Text sizes are kept within what a run can have.
fn clamp_size(s: f32) -> f32 {
    if s.is_finite() { s.clamp(1.0, 1584.0) } else { 12.0 }
}

struct Lay<'a, 'b> {
    b: &'a mut Builder<'b>,
    rc: &'a Arc<ResolvedChar>,
    start: usize,
    end: usize,
}

impl Lay<'_, '_> {
    fn rc_at(&self, size: f32) -> Arc<ResolvedChar> {
        if (size - self.rc.size).abs() < 0.01 {
            return self.rc.clone();
        }
        let mut r = (**self.rc).clone();
        (r.size, r.size_cs) = (size, size);
        Arc::new(r)
    }

    /// The text's width and its font's line ascent and descent at `size`.
    fn measure(&mut self, text: &str, size: f32) -> (f32, f32, f32) {
        let rc = self.rc_at(size);
        let (c0, g0, s0) = (self.b.clusters.len(), self.b.glyphs.len(), self.b.shown.len());
        self.b.shape_atomic(text, self.start, self.end, &rc);
        let m = self.b.clusters.get(c0..).and_then(|cs| cs.last()).map(|c| {
            let (a, d) = self.b.styles.get(c.style as usize).map_or((size * 0.8, size * 0.2), |s| (s.ascent, s.descent));
            (c.adv, a, d)
        });
        self.b.clusters.truncate(c0);
        self.b.glyphs.truncate(g0);
        self.b.shown.truncate(s0);
        m.unwrap_or((0.0, size * 0.8, size * 0.2))
    }

    /// The font's ascent and descent at `size`.
    fn line(&mut self, size: f32) -> (f32, f32) {
        let (_, a, d) = self.measure("x", size);
        (a, d)
    }

    fn text(&mut self, t: &str, size: f32) -> EqBox {
        if t.is_empty() {
            return EqBox::default();
        }
        let (w, asc, desc) = self.measure(t, size);
        EqBox { w, asc, desc, pieces: vec![Piece { text: t.to_string(), size, x: 0.0, y: 0.0 }], rules: Vec::new() }
    }

    /// A character drawn at least `size`, larger to span `h` points centred on `mid`.
    fn tall_char(&mut self, c: char, size: f32, (h, mid): (f32, f32), stretch: f32) -> EqBox {
        let (a, d) = self.line(size);
        let line = (a + d).max(0.1);
        let k = if h > line * 1.1 { clamp_size(size * h / line * stretch) } else { size };
        let mut b = self.text(&c.to_string(), k);
        let dy = mid - (b.asc - b.desc) / 2.0;
        let mut o = EqBox { w: b.w, ..Default::default() };
        let w = b.w;
        b.w = 0.0;
        o.put(b, 0.0, if k > size { dy } else { 0.0 });
        o.w = w;
        o
    }

    /// Rows of `items` in `cols` columns, centred on the middle of a line of text.
    fn stack(&mut self, items: &[EqNode], size: f32, (align, cols): (OAlign, usize), (vs, hs): (f32, f32)) -> EqBox {
        let boxes: Vec<EqBox> = items.iter().map(|i| self.node(i, size)).collect();
        let cols = cols.max(1);
        let mut widths = vec![0.0f32; cols.min(boxes.len().max(1))];
        for (i, b) in boxes.iter().enumerate() {
            if let Some(w) = widths.get_mut(i % cols) {
                *w = w.max(b.w);
            }
        }
        let (a, d) = self.line(size);
        let pitch = a + d + vs;
        let mut out = EqBox::default();
        let (mut y, mut prev_desc) = (0.0f32, None::<f32>);
        let mut boxes = boxes;
        for row in boxes.chunks_mut(cols) {
            let asc = row.iter().map(|b| b.asc).fold(0.0f32, f32::max);
            if let Some(pd) = prev_desc {
                y -= pitch.max(pd + asc + vs);
            }
            prev_desc = Some(row.iter().map(|b| b.desc).fold(0.0f32, f32::max));
            let mut x = 0.0;
            for (c, b) in row.iter_mut().enumerate() {
                let cw = widths.get(c).copied().unwrap_or(0.0);
                let dx = match align {
                    OAlign::Left => 0.0,
                    OAlign::Center => (cw - b.w) / 2.0,
                    OAlign::Right => cw - b.w,
                };
                out.put(std::mem::take(b), x + dx, y);
                x += cw + hs;
            }
        }
        out.w = widths.iter().sum::<f32>() + hs.max(0.0) * widths.len().saturating_sub(1) as f32;
        // The stack's middle on the middle of a line of text.
        let shift = (a - d) / 2.0 - (out.asc - out.desc) / 2.0;
        let mut o = EqBox { w: out.w, ..Default::default() };
        o.put(out, 0.0, shift);
        o
    }

    fn seq(&mut self, items: &[EqNode], size: f32, sep: Option<&str>) -> EqBox {
        let mut out = EqBox::default();
        for (i, n) in items.iter().enumerate() {
            if let (Some(s), true) = (sep, i > 0) {
                let t = self.text(s, size);
                out.append(t);
            }
            let b = self.node(n, size);
            out.append(b);
        }
        out
    }

    fn rule_width(size: f32) -> f32 {
        (size * 0.05).clamp(0.4, 4.0)
    }

    fn node(&mut self, n: &EqNode, size: f32) -> EqBox {
        let small = clamp_size(size * 0.6);
        match n {
            EqNode::Text(t) => self.text(t, size),
            EqNode::Seq(v) => self.seq(v, size, None),
            EqNode::List(v) => self.seq(v, size, Some(",")),
            EqNode::Array { align, cols, vs, hs, items } => self.stack(items, size, (*align, usize::from(*cols)), (*vs, *hs)),
            EqNode::Overstrike { align, items } => {
                let boxes: Vec<EqBox> = items.iter().map(|i| self.node(i, size)).collect();
                let w = boxes.iter().map(|b| b.w).fold(0.0f32, f32::max);
                let mut out = EqBox::default();
                for b in boxes {
                    let dx = match align {
                        OAlign::Left => 0.0,
                        OAlign::Center => (w - b.w) / 2.0,
                        OAlign::Right => w - b.w,
                    };
                    out.put(b, dx, 0.0);
                }
                out.w = w;
                out
            }
            EqNode::Script { up, ai, di, items } => {
                let inner = match items.as_slice() {
                    [one] => self.node(one, size),
                    many => self.stack(many, size, (OAlign::Left, 1), (0.0, 0.0)),
                };
                let mut out = EqBox { w: inner.w, ..Default::default() };
                out.put(inner, 0.0, *up);
                out.asc += ai.max(0.0);
                out.desc += di.max(0.0);
                out
            }
            EqNode::Fraction(num, den) => {
                let (nb, db) = (self.node(num, size), self.node(den, size));
                let t = Self::rule_width(size);
                let (axis, gap, pad) = (size * 0.25, size * 0.06, size * 0.08);
                let w = nb.w.max(db.w) + 2.0 * pad;
                // Line descents and ascents include the font's leading; the ink sits closer.
                let ny = axis + t / 2.0 + gap + nb.desc * 0.6;
                let dy = axis - t / 2.0 - gap - db.asc * 0.8;
                let mut out = EqBox { w, ..Default::default() };
                let (nw, dw) = (nb.w, db.w);
                out.put(nb, (w - nw) / 2.0, ny);
                out.put(db, (w - dw) / 2.0, dy);
                out.rules.push(Rule { x: 0.0, y: axis, w, h: 0.0, stroke: t });
                out
            }
            EqNode::Brackets { left, right, body } => {
                let b = self.node(body, size);
                let span = (b.asc + b.desc, (b.asc - b.desc) / 2.0);
                let mut out = EqBox::default();
                if let Some(c) = left {
                    let l = self.tall_char(*c, size, span, 1.0);
                    out.append(l);
                }
                out.append(b);
                if let Some(c) = right {
                    let r = self.tall_char(*c, size, span, 1.0);
                    out.append(r);
                }
                out
            }
            EqNode::Radical { index, base } => {
                let b = self.node(base, size);
                let (a, _) = self.line(size);
                let top = b.asc.max(a);
                let sign = self.tall_char('\u{221A}', size, (b.asc + b.desc, (b.asc - b.desc) / 2.0), 1.0);
                let mut out = EqBox::default();
                if let Some(i) = index {
                    let ib = self.node(i, small);
                    // The index sits over the radical's left arm.
                    let lead = (ib.w - sign.w * 0.4).max(0.0);
                    out.put(ib, 0.0, size * 0.45);
                    out.w = lead;
                }
                out.append(sign);
                let t = Self::rule_width(size);
                let (x, bw) = (out.w, b.w);
                out.append(b);
                // The bar over the base, near the top of its line.
                out.rules.push(Rule { x, y: top * 0.92, w: bw, h: 0.0, stroke: t });
                out
            }
            EqNode::Boxed { top, bottom, left, right, body } => {
                let b = self.node(body, size);
                let (t, pad) = (Self::rule_width(size), size * 0.08);
                let (w, asc, desc) = (b.w + 2.0 * pad, b.asc, b.desc);
                let mut out = EqBox { w, ..Default::default() };
                out.put(b, pad, 0.0);
                let h = asc + desc;
                for (on, r) in [
                    (*top, Rule { x: 0.0, y: asc, w, h: 0.0, stroke: t }),
                    (*bottom, Rule { x: 0.0, y: -desc, w, h: 0.0, stroke: t }),
                    (*left, Rule { x: 0.0, y: asc, w: 0.0, h, stroke: t }),
                    (*right, Rule { x: w, y: asc, w: 0.0, h, stroke: t }),
                ] {
                    if on {
                        out.rules.push(r);
                    }
                }
                out
            }
            EqNode::Displace { by, line } => {
                let mut out = EqBox { w: *by, ..Default::default() };
                if *line && by.abs() > 0.0 {
                    out.rules.push(Rule { x: by.min(0.0), y: -size * 0.12, w: by.abs(), h: 0.0, stroke: Self::rule_width(size) });
                }
                out
            }
            EqNode::Integral { symbol, variable, inline, lo, hi, body } => {
                let b = self.node(body, size);
                let k = if *variable { clamp_size(size * 1.4) } else { size };
                let span = (b.asc + b.desc, (b.asc - b.desc) / 2.0);
                let sym = if *variable { self.tall_char(*symbol, k, span, 1.1) } else { self.text(&symbol.to_string(), k) };
                let (lb, hb) = (self.node(lo, small), self.node(hi, small));
                let mut out = EqBox::default();
                if *inline {
                    let (sw, sa, sd) = (sym.w, sym.asc, sym.desc);
                    out.append(sym);
                    let lw = lb.w.max(hb.w);
                    let (ha, ld) = (hb.asc, lb.desc);
                    out.put(hb, sw, sa - ha * 0.6);
                    out.put(lb, sw, -sd + ld * 0.6);
                    out.w = sw + lw;
                } else {
                    let w = sym.w.max(lb.w).max(hb.w);
                    let (sw, sa, sd, lw, hw) = (sym.w, sym.asc, sym.desc, lb.w, hb.w);
                    let (la, hd) = (lb.asc, hb.desc);
                    out.put(sym, (w - sw) / 2.0, 0.0);
                    out.put(hb, (w - hw) / 2.0, sa + hd);
                    out.put(lb, (w - lw) / 2.0, -sd - la);
                    out.w = w;
                }
                out.append(b);
                out
            }
        }
    }
}

/// Lay out the parsed EQ field `node` as one cluster for `start..end`: text at the run's size in
/// the cluster, text at other sizes in `rubies`, rules in `enclosures`. The line grows to the
/// field's ascent and descent (the cluster's `obj_h` and `obj_d`).
pub(super) fn eq_field(
    b: &mut Builder,
    node: &EqNode,
    (start, end): (usize, usize),
    rc: &Arc<ResolvedChar>,
    enclosures: &mut Vec<(usize, Enclosed)>,
    rubies: &mut Vec<RubyLayout>,
) {
    let size = clamp_size(rc.size);
    let bx = Lay { b, rc, start, end }.node(node, size);
    let k = b.clusters.len();
    // Text at other sizes, drawn with its own style.
    for p in bx.pieces.iter().filter(|p| (p.size - size).abs() >= 0.01) {
        let rc2 = {
            let mut r = (**rc).clone();
            (r.size, r.size_cs) = (p.size, p.size);
            Arc::new(r)
        };
        let (c0, g0, s0) = (b.clusters.len(), b.glyphs.len(), b.shown.len());
        b.shape_atomic(&p.text, start, end, &rc2);
        let mut glyphs = Vec::new();
        let mut ranges = Vec::new();
        if let Some(c) = b.clusters.get(c0) {
            let shown = b.shown.get(s0).filter(|s| s.0 == c0).map(|s| s.2.clone()).unwrap_or_default();
            for (n, g) in b.glyphs.get(c.g0 as usize..c.g1 as usize).into_iter().flatten().enumerate() {
                glyphs.push((c.style, Glyph { dx: g.dx + p.x, dy: g.dy + p.y, ..*g }));
                ranges.push(shown.get(n).cloned().unwrap_or(0..p.text.len()));
            }
        }
        b.clusters.truncate(c0);
        b.glyphs.truncate(g0);
        b.shown.truncate(s0);
        if !glyphs.is_empty() {
            rubies.push(RubyLayout { cluster: k, glyphs, text: p.text.clone(), ranges, raise: 0.0 });
        }
    }
    // Text at the run's size: one cluster, each piece's glyphs moved to its place.
    let (c0, s0, gs) = (b.clusters.len(), b.shown.len(), b.glyphs.len());
    let (mut text, mut ranges) = (String::new(), Vec::new());
    let mut style = None;
    for p in bx.pieces.iter().filter(|p| (p.size - size).abs() < 0.01) {
        let (cn, sn) = (b.clusters.len(), b.shown.len());
        b.shape_atomic(&p.text, start, end, rc);
        let Some(c) = b.clusters.get(cn).cloned() else { continue };
        if c.kind == ClKind::Text {
            style.get_or_insert(c.style);
        }
        for g in b.glyphs.get_mut(c.g0 as usize..c.g1 as usize).into_iter().flatten() {
            g.dx += p.x;
            g.dy += p.y;
        }
        let off = text.len();
        let n = c.g1.saturating_sub(c.g0) as usize;
        match b.shown.get(sn).filter(|s| s.0 == cn) {
            Some(s) => ranges.extend(s.2.iter().map(|r| r.start + off..r.end + off)),
            None => ranges.extend(std::iter::repeat_n(off..off + p.text.len(), n)),
        }
        text.push_str(&p.text);
        b.clusters.truncate(cn);
        b.shown.truncate(sn);
    }
    b.clusters.truncate(c0);
    b.shown.truncate(s0);
    let g1 = b.glyphs.len() as u32;
    let style = match style {
        Some(s) => s,
        None => b.style(rc, None, false),
    };
    if !text.is_empty() {
        b.shown.push((k, text, ranges));
    }
    for r in &bx.rules {
        enclosures
            .push((k, Enclosed { kind: wordcraft_doc::para::ShapeKind::Line, x: r.x, top: r.y, w: r.w.max(0.0), h: r.h.max(0.0), stroke: r.stroke }));
    }
    b.clusters.push(Cluster {
        start,
        end,
        adv: bx.w.max(0.0),
        kind: ClKind::Text,
        style,
        g0: gs as u32,
        g1,
        break_after: false,
        obj_h: bx.asc.max(0.0),
        obj_d: bx.desc.max(0.0),
        dot: false,
    });
}
