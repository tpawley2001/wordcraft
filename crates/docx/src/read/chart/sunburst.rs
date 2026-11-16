//! Office 2016 sunburst charts (`cx:series layoutId="sunburst"`): one ring per category level
//! around a hole, each node an annular sector whose angle is proportional to its value.

use std::collections::HashMap;

use wordcraft_doc::graphic::{GraphicItem, PathSeg, TextAlign};
use wordcraft_doc::props::Rgb;

use super::canvas::text_w;
use super::{FONT, LINE, MAX_POINTS};

/// Most category levels (rings) read from one sunburst.
pub(super) const MAX_LEVELS: usize = 16;
/// Sector outline: white, 0.75 pt.
const OUTLINE: Rgb = Rgb(0xFF, 0xFF, 0xFF);
const OUTLINE_W: f32 = 0.75;
/// Data labels: white Calibri at the chart's default size.
const LABEL_FONT: &str = "Calibri";

/// One drawn node of a sunburst: radii in points from the centre, angles in degrees clockwise
/// from 12 o'clock.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SunburstSegment {
    /// Category names from the top level down to this node.
    pub path: Vec<String>,
    /// Index of the top-level ancestor in first-appearance data order (picks the accent colour).
    pub group: usize,
    /// Sum of the node's leaves.
    pub value: f64,
    pub r0: f32,
    pub r1: f32,
    pub a0: f32,
    pub a1: f32,
}

/// A node of the category tree.
struct Node {
    path: Vec<String>,
    group: usize,
    value: f64,
    children: Vec<usize>,
}

/// Lays out a sunburst of `leaves` (category path top level first, value) in `plot` ([x, y, w, h]).
pub(crate) fn sunburst_segments(leaves: &[(Vec<String>, f64)], plot: [f32; 4]) -> Vec<SunburstSegment> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    // (parent, name) → node; `None` parent for the top level.
    let mut index: HashMap<(Option<usize>, &str), usize> = HashMap::new();
    let mut levels = 0usize;
    for (path, value) in leaves.iter().take(MAX_POINTS) {
        let v = *value;
        if !(v.is_finite() && v > 0.0) {
            continue;
        }
        // The first blank name ends the path.
        let names: Vec<&str> = path.iter().map(String::as_str).take_while(|n| !n.is_empty()).take(MAX_LEVELS).collect();
        if names.is_empty() {
            continue;
        }
        levels = levels.max(names.len());
        let mut parent: Option<usize> = None;
        let mut group = 0;
        for (d, name) in names.iter().enumerate() {
            let k = match index.get(&(parent, *name)) {
                Some(&k) => k,
                None => {
                    let k = nodes.len();
                    if d == 0 {
                        group = roots.len();
                        roots.push(k);
                    }
                    let path = names.iter().take(d + 1).map(|n| n.to_string()).collect();
                    nodes.push(Node { path, group, value: 0.0, children: Vec::new() });
                    if let Some(p) = parent.and_then(|p| nodes.get_mut(p)) {
                        p.children.push(k);
                    }
                    index.insert((parent, name), k);
                    k
                }
            };
            if let Some(n) = nodes.get_mut(k) {
                n.value += v;
                group = n.group;
            }
            parent = Some(k);
        }
    }

    let [x, y, w, h] = plot;
    let r = (w.min(h) / 2.0).max(0.0);
    if levels == 0 || !r.is_finite() || !(x + w / 2.0).is_finite() || !(y + h / 2.0).is_finite() {
        return Vec::new();
    }
    let band = r / (levels + 1) as f32;
    let value_of = |k: usize| nodes.get(k).map_or(0.0, |n| n.value);
    // Siblings largest first; the sort is stable, so ties keep data order.
    let sorted = |list: &[usize]| -> Vec<usize> {
        let mut list = list.to_vec();
        list.sort_by(|a, b| value_of(*b).total_cmp(&value_of(*a)));
        list
    };

    let mut out = Vec::new();
    let total: f64 = roots.iter().map(|k| value_of(*k)).sum();
    // (node, a0, a1) of the current depth in angle order.
    let mut ring: Vec<(usize, f64, f64)> = spread(&sorted(&roots), 0.0, 360.0, total, &value_of);
    let mut depth = 0usize;
    while !ring.is_empty() && depth < MAX_LEVELS {
        let mut next = Vec::new();
        for &(k, a0, a1) in &ring {
            let Some(n) = nodes.get(k) else { continue };
            out.push(SunburstSegment {
                path: n.path.clone(),
                group: n.group,
                value: n.value,
                r0: (depth + 1) as f32 * band,
                r1: (depth + 2) as f32 * band,
                a0: a0 as f32,
                a1: a1 as f32,
            });
            // Children share the parent's span by value; a blank leaf under it leaves a gap.
            let span = a1 - a0;
            let scale = if n.value > 0.0 { span / n.value } else { 0.0 };
            let kids = sorted(&n.children);
            let kid_total: f64 = kids.iter().map(|c| value_of(*c)).sum();
            next.extend(spread(&kids, a0, a0 + kid_total * scale, kid_total, &value_of));
        }
        ring = next;
        depth += 1;
    }
    out
}

/// `list` laid end to end from `a0` to `a1` in proportion to their values.
fn spread(list: &[usize], a0: f64, a1: f64, total: f64, value_of: &dyn Fn(usize) -> f64) -> Vec<(usize, f64, f64)> {
    if !(total > 0.0 && total.is_finite()) {
        return Vec::new();
    }
    let mut at = a0;
    let mut sum = 0.0;
    list.iter()
        .map(|&k| {
            sum += value_of(k);
            let end = a0 + (a1 - a0) * (sum / total);
            let span = (k, at, end);
            at = end;
            span
        })
        .collect()
}

/// The point at radius `r`, `a` degrees clockwise from 12 o'clock around (`cx`, `cy`).
fn polar(cx: f32, cy: f32, r: f32, a: f32) -> (f32, f32) {
    let t = a.to_radians();
    (cx + r * t.sin(), cy - r * t.cos())
}

/// Widest arc drawn as one cubic, degrees: short curves keep their control points close to the
/// circle, so a sector's bounds stay on its disc.
const ARC_STEP: f32 = 15.0;

/// Cubic Béziers along the circle of radius `r` from angle `a0` to `a1` (either way), at most
/// [`ARC_STEP`] each.
fn arc(segs: &mut Vec<PathSeg>, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
    let sweep = a1 - a0;
    let n = (sweep.abs() / ARC_STEP).ceil().clamp(1.0, 360.0 / ARC_STEP);
    let step = sweep / n;
    let k = 4.0 / 3.0 * (step.to_radians() / 4.0).tan() * r;
    for i in 0..n as usize {
        let s = a0 + step * i as f32;
        let e = s + step;
        let (ts, te) = (s.to_radians(), e.to_radians());
        let (sx, sy) = polar(cx, cy, r, s);
        let (ex, ey) = polar(cx, cy, r, e);
        // The clockwise tangent at angle t is (cos t, sin t).
        segs.push(PathSeg::Cubic(sx + k * ts.cos(), sy + k * ts.sin(), ex - k * te.cos(), ey - k * te.sin(), ex, ey));
    }
}

/// The annular sector of `s` around (`cx`, `cy`).
fn sector(cx: f32, cy: f32, s: &SunburstSegment) -> Vec<PathSeg> {
    let (x, y) = polar(cx, cy, s.r1, s.a0);
    let mut segs = vec![PathSeg::Move(x, y)];
    arc(&mut segs, cx, cy, s.r1, s.a0, s.a1);
    let (x, y) = polar(cx, cy, s.r0, s.a1);
    segs.push(PathSeg::Line(x, y));
    arc(&mut segs, cx, cy, s.r0, s.a1, s.a0);
    segs.push(PathSeg::Close);
    segs
}

/// Draws the sunburst of `leaves` in `plot`: each node a filled sector in its branch's colour
/// (`colour(group)`), outlined in white, with its category name when `labels` and it fits.
pub(super) fn draw_sunburst(out: &mut Vec<GraphicItem>, leaves: &[(Vec<String>, f64)], plot: [f32; 4], colour: &dyn Fn(usize) -> Rgb, labels: bool) {
    let [x, y, w, h] = plot;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let segs = sunburst_segments(leaves, plot);
    for s in &segs {
        out.push(GraphicItem::Path { segs: sector(cx, cy, s), fill: Some(colour(s.group)), stroke: Some(OUTLINE), stroke_width: OUTLINE_W });
    }
    if !labels {
        return;
    }
    let lh = FONT * LINE;
    for s in &segs {
        let Some(name) = s.path.last() else { continue };
        let mid_r = (s.r0 + s.r1) / 2.0;
        let room = (s.a1 - s.a0).to_radians() * mid_r;
        let tw = text_w(name, FONT);
        if tw > room.min(2.0 * mid_r) || lh > s.r1 - s.r0 {
            continue;
        }
        let (lx, ly) = polar(cx, cy, mid_r, (s.a0 + s.a1) / 2.0);
        let rw = tw + 4.0;
        out.push(GraphicItem::Text {
            rect: [lx - rw / 2.0, ly - lh / 2.0, rw, lh],
            text: name.clone(),
            size: FONT,
            color: OUTLINE,
            bold: false,
            align: TextAlign::Center,
            font: Some(LABEL_FONT.to_string()),
            rotation: 0.0,
        });
    }
}

#[cfg(test)]
mod hostile_tests {
    use super::*;

    #[test]
    fn hostile_values_and_paths_draw_nothing_odd() {
        let deep: Vec<String> = (0..100).map(|k| format!("L{k}")).collect();
        let leaves = vec![
            (deep, 1.0),
            (vec!["A".into()], f64::INFINITY),
            (vec!["B".into()], f64::NAN),
            (vec![String::new(), "C".into()], 5.0),
            (vec!["D".into()], f64::MAX),
            (vec!["D".into()], f64::MAX),
        ];
        let segs = sunburst_segments(&leaves, [0.0, 0.0, 100.0, 100.0]);
        assert!(segs.iter().all(|s| s.path.len() <= MAX_LEVELS && s.a0.is_finite() && s.r1.is_finite()), "{segs:?}");
        assert!(sunburst_segments(&leaves, [f32::NAN, 0.0, f32::INFINITY, 1.0]).is_empty());
        let mut out = Vec::new();
        draw_sunburst(&mut out, &leaves, [0.0, 0.0, 100.0, 100.0], &|_| Rgb(0, 0, 0), true);
    }
}
