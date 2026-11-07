//! 3-D bar, line, area and pie charts (ECMA-376 Part 1 §21.2.2.16 `c:bar3DChart`, §21.2.2.96
//! `c:line3DChart`, §21.2.2.4 `c:area3DChart`, §21.2.2.140 `c:pie3DChart`), seen as `c:view3D`
//! (§21.2.2.228) says.
//!
//! The data, scales, axes, labels and legend are those of the 2-D chart of the same data; depth is
//! added by an oblique (cabinet-style) projection. The front plane is drawn unprojected (the plot
//! rectangle, shrunk to make room), and the depth axis runs along one direction on the page:
//! right and up, its slant from `c:rotY` (the right half turn leans right, the left half left)
//! and `c:rotX` (higher elevation, steeper), never flatter than 45° away from either axis, so a
//! view with no rotation still shows depth. A negative elevation is drawn as its absolute value
//! (the tops show, not the bottoms). The depth of a row is half the width of a bar times
//! `c:depthPercent`, with `c:gapDepth` between rows, the whole depth kept between a few points and
//! 30% of the plot. `c:perspective` and `c:rAngAx` are ignored: every view is this parallel
//! projection.
//!
//! Bars are boxes (`c:shape` cylinder, cone and pyramid draw as boxes too): a front face in the
//! series colour, a top face at [`TOP`] and a side face at [`SIDE`] of its lightness, drawn back
//! to front (back rows first, then along the depth direction) so near faces cover far ones.
//! Series of a `standard` grouping sit one behind another, the first in front. Lines are ribbons
//! (a band in the series colour with its upper surface shaded), each series in its own row. Areas
//! have a shaded top surface and end. The floor, side wall and back wall are drawn when their
//! `c:spPr` gives a fill or line, and value gridlines run across the back wall and the side wall
//! (the floor for bars across).
//!
//! A 3-D pie is an ellipse whose height is its width times the sine of the elevation (at least
//! 10°), with the front half of each slice's rim drawn as a darker wall below it; slices start at
//! `c:rotY` and their angles are exactly proportional to the values.

use super::canvas::{Rect, fill, rect_segs, seg, stroke};
use super::model::{DataLabel, Depth, Group};
use super::plot::{BarRect, Frame, area_bands, area_labels, bar_rects, line_points};
use super::polar::{draw_data_labels, ellipse_arc, label_of, line_to, pt};
use super::{Chart, El, FRAC_PI_2, GRID, GraphicItem, PathSeg, Rgb, TAU, fill_colour, int};

/// Lightness of a top face (and a ribbon's or area's upper surface) against the front face.
const TOP: f32 = 0.85;
/// Lightness of a side face (and a pie's rim) against the front face.
const SIDE: f32 = 0.7;
/// Shortest depth drawn, points: a view with no rotation still shows some.
const MIN_DEPTH: f32 = 6.0;
/// Most of the plot's smaller side the depth takes.
const MAX_DEPTH: f32 = 0.3;
/// Depth on the page of a row as deep as a bar is wide (oblique projections halve depth).
const OBLIQUE: f32 = 0.5;
/// Lowest elevation a 3-D pie is drawn at, degrees.
const MIN_ELEVATION: f64 = 10.0;
/// Thickness of a 3-D pie, fraction of its radius before tilting.
const PIE_THICK: f64 = 0.2;

/// `c:view3D` and the walls and floor of a 3-D chart.
pub(super) struct View3d {
    /// Elevation, degrees (-90..90).
    pub(super) rot_x: f64,
    /// Rotation, degrees (0..360).
    pub(super) rot_y: f64,
    /// Depth as a percentage of a bar's width (20..2000).
    depth_pct: f32,
    floor: Wall,
    side: Wall,
    back: Wall,
}

impl View3d {
    pub(super) fn parse(chart: &El, theme: &[Rgb]) -> View3d {
        let v = chart.child("c:view3D");
        let num = |n: &str| v.and_then(|v| v.child(n)).and_then(|e| e.attr("val")).and_then(int);
        View3d {
            rot_x: num("c:rotX").unwrap_or(0).clamp(-90, 90) as f64,
            rot_y: num("c:rotY").unwrap_or(0).clamp(0, 360) as f64,
            depth_pct: num("c:depthPercent").unwrap_or(100).clamp(20, 2000) as f32,
            floor: Wall::parse(chart.child("c:floor"), theme),
            side: Wall::parse(chart.child("c:sideWall"), theme),
            back: Wall::parse(chart.child("c:backWall"), theme),
        }
    }
}

/// The fill and outline of a wall or the floor, when its `c:spPr` gives them.
#[derive(Clone, Copy)]
struct Wall {
    fill: Option<Rgb>,
    line: Option<Rgb>,
}

impl Wall {
    fn parse(el: Option<&El>, theme: &[Rgb]) -> Wall {
        let sp = el.and_then(|e| e.child("c:spPr"));
        let fill = sp.filter(|p| p.child("a:noFill").is_none()).and_then(|p| fill_colour(p, theme));
        let ln = sp.and_then(|p| p.child("a:ln")).filter(|l| l.child("a:noFill").is_none());
        Wall { fill, line: ln.and_then(|l| fill_colour(l, theme)) }
    }

    fn draw(self, out: &mut Vec<GraphicItem>, segs: Vec<PathSeg>) {
        if self.fill.is_some() || self.line.is_some() {
            out.push(GraphicItem::Path { segs, fill: self.fill, stroke: self.line, stroke_width: 0.75 });
        }
    }
}

impl Depth {
    /// The depth layout of a 3-D chart type element; none for a flat one.
    pub(super) fn parse(ty: &El) -> Option<Depth> {
        let grouping = ty.child("c:grouping").and_then(|g| g.attr("val"));
        let rows = match ty.name.as_str() {
            "c:line3DChart" => true,
            "c:bar3DChart" => grouping == Some("standard"),
            // The schema's default grouping of an area chart is standard.
            "c:area3DChart" => grouping.is_none_or(|g| g == "standard"),
            _ => return None,
        };
        let gap = ty.child("c:gapDepth").and_then(|g| g.attr("val")).and_then(int).unwrap_or(150).clamp(0, 500);
        Some(Depth { rows, gap_depth: gap as f32 / 100.0 })
    }
}

/// Where the depth axis goes on the page, and how deep the rows along it are.
pub(super) struct Projection {
    /// Unit direction of the depth axis (x right, y down; y is never positive).
    ux: f32,
    uy: f32,
    /// Depth of one row along it, points.
    row: f32,
    /// Gap between rows, points.
    gap: f32,
    rows: usize,
}

impl Projection {
    /// The projection of chart `c` whose plot (before depth) is `plot`; none when no chart type
    /// of it is 3-D.
    pub(super) fn of(c: &Chart, plot: Rect) -> Option<Projection> {
        let deep: Vec<(&Group, Depth)> = c.groups.iter().filter_map(|g| Some((g, g.depth?))).collect();
        let (first, d) = deep.first().copied()?;
        let rows = deep.iter().map(|(g, d)| if d.rows { g.series.len() } else { 1 }).max().unwrap_or(1).max(1);
        let len = if c.horizontal { plot.h } else { plot.w };
        let thick = len / c.n.max(1) as f32 / (1.0 + first.gap);
        let units = rows as f32 + (rows - 1) as f32 * d.gap_depth;
        let raw = units * thick * c.view.depth_pct / 100.0 * OBLIQUE;
        let most = MAX_DEPTH * plot.w.min(plot.h);
        let total = if raw.is_finite() { raw.clamp(MIN_DEPTH.min(most), most.max(0.0)) } else { MIN_DEPTH.min(most) };
        let (sy, sx) = (c.view.rot_x.to_radians().sin() as f32, c.view.rot_y.to_radians().sin() as f32);
        let ex = (0.5 + 0.5 * sx.abs()) * if sx < 0.0 { -1.0 } else { 1.0 };
        let ey = -(0.5 + 0.5 * sy.abs());
        let norm = ex.hypot(ey);
        let row = total / units.max(1.0);
        Some(Projection { ux: ex / norm, uy: ey / norm, row, gap: row * d.gap_depth, rows })
    }

    /// The depth axis' whole length on the page as a vector.
    fn whole(&self) -> (f32, f32) {
        let len = self.rows as f32 * self.row + (self.rows - 1) as f32 * self.gap;
        (self.ux * len, self.uy * len)
    }

    /// Where row `k`'s front sits relative to the front plane.
    fn offset(&self, k: usize) -> (f32, f32) {
        let len = k.min(self.rows) as f32 * (self.row + self.gap);
        (self.ux * len, self.uy * len)
    }

    /// One row's depth as a vector.
    fn depth(&self) -> (f32, f32) {
        (self.ux * self.row, self.uy * self.row)
    }

    /// The front plane: `plot` less the room the depth axis needs above and beside it.
    pub(super) fn front(&self, plot: Rect) -> Rect {
        let (dx, dy) = self.whole();
        Rect { x: plot.x + (-dx).max(0.0), y: plot.y - dy, w: (plot.w - dx.abs()).max(1.0), h: (plot.h + dy).max(1.0) }
    }
}

/// `c` with each channel scaled by `k` (0..1): the same hue, darker.
fn shade(c: Rgb, k: f32) -> Rgb {
    let f = |v: u8| (v as f32 * k).floor().clamp(0.0, 255.0) as u8;
    Rgb(f(c.0), f(c.1), f(c.2))
}

fn quad(p: [(f32, f32); 4]) -> Vec<PathSeg> {
    let [a, b, c, d] = p;
    vec![PathSeg::Move(a.0, a.1), PathSeg::Line(b.0, b.1), PathSeg::Line(c.0, c.1), PathSeg::Line(d.0, d.1), PathSeg::Close]
}

fn add(p: (f32, f32), d: (f32, f32)) -> (f32, f32) {
    (p.0 + d.0, p.1 + d.1)
}

/// The floor, side wall and back wall where the file styles them, and the value gridlines across
/// the back wall and the side wall (the floor, for bars across).
pub(super) fn walls(out: &mut Vec<GraphicItem>, c: &Chart, g: &Frame, ticks: &[(f64, String)], p: &Projection) {
    let (dx, dy) = p.whole();
    let r = g.plot;
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    // The side wall is on the side the depth axis leans away from.
    let sx = if dx >= 0.0 { x0 } else { x1 };
    c.view.back.draw(out, rect_segs(x0 + dx, y0 + dy, x1 + dx, y1 + dy));
    c.view.side.draw(out, quad([(sx, y0), (sx, y1), (sx + dx, y1 + dy), (sx + dx, y0 + dy)]));
    c.view.floor.draw(out, quad([(x0, y1), (x1, y1), (x1 + dx, y1 + dy), (x0 + dx, y1 + dy)]));
    if !c.val.grid {
        return;
    }
    for (t, _) in ticks {
        let v = g.val(*t);
        if g.horiz {
            stroke(out, seg(v + dx, y0 + dy, v + dx, y1 + dy), GRID, 0.75);
            stroke(out, seg(v, y1, v + dx, y1 + dy), GRID, 0.75);
        } else {
            stroke(out, seg(x0 + dx, v + dy, x1 + dx, v + dy), GRID, 0.75);
            stroke(out, seg(sx, v, sx + dx, v + dy), GRID, 0.75);
        }
    }
}

/// A 3-D bar chart type: each bar a box, back to front.
pub(super) fn bars_3d(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, p: &Projection, labels: &mut Vec<DataLabel>) {
    let mut bars = bar_rects(c, grp, g);
    // Along the depth direction: what lies further that way is nearer (its front face covers the
    // others' tops and sides), so it's drawn later.
    let key = |b: &BarRect| -> (f32, f32) {
        let (mx, my) = ((b.x0 + b.x1) / 2.0 * p.ux, (b.y0 + b.y1) / 2.0 * p.uy);
        if g.horiz { (my, mx) } else { (mx, my) }
    };
    bars.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        b.row.cmp(&a.row).then(ka.0.total_cmp(&kb.0)).then(ka.1.total_cmp(&kb.1))
    });
    let e = p.depth();
    for b in bars {
        let o = p.offset(b.row);
        let (x0, y0, x1, y1) = (b.x0 + o.0, b.y0 + o.1, b.x1 + o.0, b.y1 + o.1);
        fill(out, quad([(x0, y0), (x1, y0), add((x1, y0), e), add((x0, y0), e)]), shade(b.color, TOP));
        let sx = if e.0 >= 0.0 { x1 } else { x0 };
        fill(out, quad([(sx, y0), (sx, y1), add((sx, y1), e), add((sx, y0), e)]), shade(b.color, SIDE));
        fill(out, rect_segs(x0, y0, x1, y1), b.color);
        labels.extend(b.label.map(|l| DataLabel { x: l.x + o.0, y: l.y + o.1, ..l }));
    }
}

/// A 3-D line chart type: each series a ribbon in its own row, the back row first.
pub(super) fn lines_3d(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, p: &Projection, labels: &mut Vec<DataLabel>) {
    let e = p.depth();
    let rows = grp.depth.is_some_and(|d| d.rows);
    let mut series = line_points(grp, g);
    if rows {
        series.reverse();
    }
    for (j, pts) in series {
        let Some(s) = grp.series.get(j) else { continue };
        let o = p.offset(if rows { j } else { 0 });
        let col = c.series_colour(grp, j);
        let band = (s.line_w * 2.0).clamp(2.0, 8.0);
        let pts: Vec<Option<(f32, f32)>> = pts.into_iter().map(|q| q.map(|q| add(q, o))).collect();
        let mut steps: Vec<((f32, f32), (f32, f32))> =
            pts.windows(2).filter_map(|w| if let [Some(a), Some(b)] = w { Some((*a, *b)) } else { None }).collect();
        if e.0 < 0.0 {
            steps.reverse();
        }
        for (a, b) in steps {
            fill(out, quad([a, b, add(b, e), add(a, e)]), shade(col, TOP));
            fill(out, quad([a, b, (b.0, b.1 + band), (a.0, a.1 + band)]), col);
        }
        for (i, q) in pts.iter().enumerate() {
            if let (Some((x, y)), Some(text)) = (q, label_of(s, i)) {
                labels.push(DataLabel { x: *x, y: *y, text, inside: false });
            }
        }
    }
}

/// A 3-D area chart type: each series' front face, its shaded top surface and end; series of a
/// standard grouping in rows, the back row first.
pub(super) fn areas_3d(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, p: &Projection, labels: &mut Vec<DataLabel>) {
    let e = p.depth();
    let rows = grp.depth.is_some_and(|d| d.rows);
    let mut bands = area_bands(grp, g);
    if rows {
        bands.reverse();
    }
    for (j, top, base) in bands {
        let o = p.offset(if rows { j } else { 0 });
        let col = c.series_colour(grp, j);
        let at = |i: usize, v: f64| add((g.cat_x(i), g.val(v)), o);
        let mut segs = Vec::with_capacity(2 * g.n + 1);
        for (i, t) in top.iter().enumerate() {
            let (x, y) = at(i, *t);
            segs.push(if i == 0 { PathSeg::Move(x, y) } else { PathSeg::Line(x, y) });
        }
        for (i, b) in base.iter().enumerate().rev() {
            let (x, y) = at(i, *b);
            segs.push(PathSeg::Line(x, y));
        }
        segs.push(PathSeg::Close);
        fill(out, segs, col);
        for (i, w) in top.windows(2).enumerate() {
            if let [t0, t1] = w {
                let (a, b) = (at(i, *t0), at(i + 1, *t1));
                fill(out, quad([a, b, add(b, e), add(a, e)]), shade(col, TOP));
            }
        }
        // The end the depth axis leans towards.
        let end = if e.0 >= 0.0 { g.n.saturating_sub(1) } else { 0 };
        if let (Some(t), Some(b)) = (top.get(end), base.get(end))
            && t != b
        {
            let (a, d) = (at(end, *t), at(end, *b));
            fill(out, quad([a, d, add(d, e), add(a, e)]), shade(col, SIDE));
        }
        let mut own = Vec::new();
        area_labels(grp, j, &top, &base, g, &mut own);
        labels.extend(own.into_iter().map(|l| DataLabel { x: l.x + o.0, y: l.y + o.1, ..l }));
    }
}

/// A 3-D pie in `area`: the visible rim of each slice, then the slices on top.
pub(super) fn pie_3d(out: &mut Vec<GraphicItem>, c: &Chart, area: Rect) {
    let Some(grp) = c.groups.first() else { return };
    let Some(s) = grp.series.first() else { return };
    let vals: Vec<f64> = s.values.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    let elev = c.view.rot_x.abs().max(MIN_ELEVATION).to_radians();
    let (sin, thick) = (elev.sin(), PIE_THICK * elev.cos());
    let rx = ((area.w / 2.0 - 2.0) as f64).min((area.h as f64 - 4.0) / (2.0 * sin + thick));
    if !(total > 0.0 && total.is_finite() && rx >= 1.0) {
        return;
    }
    let (ry, h) = (rx * sin, rx * thick);
    let (cx, cy) = ((area.x + area.w / 2.0) as f64, (area.y + area.h / 2.0) as f64 - h / 2.0);
    let at = |r: f64, t: f64, dy: f64| (cx + r * rx * t.sin(), cy + dy - r * ry * t.cos());
    let mut slices = Vec::new();
    let mut a = c.first_angle.to_radians();
    for (i, v) in vals.iter().enumerate() {
        let b = a + v / total * TAU;
        if *v > 0.0 {
            slices.push((i, a, b));
        }
        a = b;
    }
    // The rim shows where it faces the viewer: from 3 o'clock round through 6 to 9.
    for (i, a, b) in &slices {
        for k in 0..3 {
            let lo = FRAC_PI_2 + TAU * k as f64;
            let (s0, s1) = (a.max(lo), b.min(lo + std::f64::consts::PI));
            if s1 <= s0 {
                continue;
            }
            let mut segs = vec![pt(at(1.0, s0, 0.0))];
            ellipse_arc(&mut segs, (cx, cy, rx, ry), s0, s1);
            segs.push(line_to(at(1.0, s1, h)));
            ellipse_arc(&mut segs, (cx, cy + h, rx, ry), s1, s0);
            segs.push(PathSeg::Close);
            fill(out, segs, shade(c.point_colour(grp, 0, *i), SIDE));
        }
    }
    let mut labels = Vec::new();
    for (i, a, b) in slices {
        let mut segs = vec![pt((cx, cy)), line_to(at(1.0, a, 0.0))];
        ellipse_arc(&mut segs, (cx, cy, rx, ry), a, b);
        segs.push(PathSeg::Close);
        fill(out, segs, c.point_colour(grp, 0, i));
        if let Some(text) = label_of(s, i) {
            let (x, y) = at(0.65, (a + b) / 2.0, 0.0);
            labels.push(DataLabel { x: x as f32, y: y as f32, text, inside: true });
        }
    }
    draw_data_labels(out, &labels, c.text_size, false);
}

#[cfg(test)]
mod tests {
    use super::super::chart_items;
    use super::*;

    const NS: &str =
        r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

    fn points(items: &[GraphicItem]) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        for it in items {
            if let GraphicItem::Path { segs, .. } = it {
                for s in segs {
                    match s {
                        PathSeg::Move(x, y) | PathSeg::Line(x, y) | PathSeg::Cubic(_, _, _, _, x, y) => out.push((*x, *y)),
                        _ => {}
                    }
                }
            }
        }
        out
    }

    /// Every 3-D chart type, standard (rows) groupings and views turned either way stay inside the
    /// chart's frame.
    #[test]
    fn depth_stays_inside_the_chart() {
        let ser = |k: usize| {
            format!(
                r#"<c:ser><c:idx val="{k}"/><c:val><c:numLit><c:ptCount val="3"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>-1</c:v></c:pt><c:pt idx="2"><c:v>7</c:v></c:pt></c:numLit></c:val></c:ser>"#
            )
        };
        let sers = format!("{}{}{}", ser(0), ser(1), ser(2));
        let views = ["", r#"<c:view3D><c:rotX val="-40"/><c:rotY val="300"/><c:depthPercent val="2000"/></c:view3D>"#];
        let plots = [
            format!(r#"<c:bar3DChart><c:barDir val="col"/><c:grouping val="standard"/>{sers}<c:gapDepth val="500"/></c:bar3DChart>"#),
            format!(r#"<c:bar3DChart><c:barDir val="bar"/><c:grouping val="clustered"/>{sers}</c:bar3DChart>"#),
            format!(r#"<c:line3DChart>{sers}</c:line3DChart>"#),
            format!(r#"<c:area3DChart>{sers}</c:area3DChart>"#),
            format!(r#"<c:pie3DChart>{}</c:pie3DChart>"#, ser(0)),
        ];
        for view in views {
            for plot in &plots {
                let x = format!(
                    r#"<c:chartSpace {NS}><c:chart>{view}<c:plotArea>{plot}<c:valAx><c:majorGridlines/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#
                );
                let space = crate::xml::parse(x.as_bytes()).unwrap();
                let items = chart_items(&space, &[], 300.0, 200.0);
                let pts = points(&items);
                assert!(!pts.is_empty(), "{plot}");
                for (x, y) in pts {
                    assert!((-0.5..=300.5).contains(&x) && (-0.5..=200.5).contains(&y), "({x}, {y}) outside in {view} {plot}");
                }
            }
        }
    }

    #[test]
    fn shade_darkens_every_channel() {
        assert_eq!(shade(Rgb(100, 200, 50), 0.5), Rgb(50, 100, 25));
        assert_eq!(shade(Rgb(255, 255, 255), 1.0), Rgb(255, 255, 255));
    }
}
