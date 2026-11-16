use super::canvas::{Rect, clip_text, fill, rect_segs, runs, seg, stroke, text, text_at, text_w, widest, wrap};
use super::format::format_value;
use super::model::{AxisTitle, DataLabel, Group, Grouping, Kind, Series};
use super::parse::category_text;
use super::polar::{draw_data_labels, label_of};
use super::stock::draw_stock;
use super::three_d::{Projection, areas_3d, bars_3d, lines_3d, walls};
use super::{AXIS, Axis, Chart, El, GAP, GraphicItem, LINE, MAX_LABEL, MAX_TICKS, PathSeg, Rgb, ShapeKind, TextAlign, fill_colour, measure};

pub(super) fn label(s: &str) -> String {
    s.chars().take(MAX_LABEL).collect()
}

/// Chart area: the chart space's fill and border (white and borderless by default).
pub(super) fn backdrop(out: &mut Vec<GraphicItem>, space: &El, theme: &[Rgb], w: f32, h: f32) {
    let sp = space.child("c:spPr");
    let fill = match sp {
        Some(p) if p.child("a:noFill").is_some() => None,
        Some(p) => fill_colour(p, theme).or(Some(Rgb::WHITE)),
        None => Some(Rgb::WHITE),
    };
    let ln = sp.and_then(|p| p.child("a:ln"));
    let stroke = ln.and_then(|l| fill_colour(l, theme));
    let stroke_width = ln.and_then(|l| l.attr("w")).and_then(|v| measure(v, 12_700.0)).unwrap_or(0.75).clamp(0.0, 20.0);
    if fill.is_some() || stroke.is_some() {
        out.push(GraphicItem::Shape { rect: [0.0, 0.0, w, h], kind: ShapeKind::Rectangle, fill, stroke, stroke_width });
    }
}

/// An axis scale: `lo` to `hi` in steps of `step`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Scale {
    pub(super) lo: f64,
    pub(super) hi: f64,
    pub(super) step: f64,
}

impl Scale {
    pub(super) fn ticks(&self) -> Vec<f64> {
        let n = (((self.hi - self.lo) / self.step + 1e-9).floor().max(0.0) as usize).min(MAX_TICKS);
        (0..=n).map(|k| self.lo + k as f64 * self.step).filter(|t| *t <= self.hi + 1e-9).collect()
    }
}

/// A scale for data `dlo..dhi` (zero included when `zero`), as Excel and Word pick it: an end
/// away from zero sits 5% of the data span beyond the data (an end at zero stays there), rounded
/// out to a step of 1, 2, 2.5, 5 or 10 times a power of ten (about seven steps). `min`/`max`
/// from the file replace the ends. A `major` unit from the file (`c:majorUnit`) is the step, and
/// an automatic end is rounded out to a multiple of it; one that would give more than
/// [`MAX_TICKS`] steps (a hostile file) is ignored for the automatic step.
pub(super) fn nice_scale(dlo: f64, dhi: f64, zero: bool, min: Option<f64>, max: Option<f64>, major: Option<f64>) -> Scale {
    let (dlo, dhi) = if dhi > dlo { (dlo, dhi) } else { (dlo, dlo + 1.0) };
    let span = dhi - dlo;
    // With zero, the ends reach it: all-negative data still starts its bars at zero.
    let (ends_lo, ends_hi) = if zero { (dlo.min(0.0), dhi.max(0.0)) } else { (dlo, dhi) };
    let lo = min.unwrap_or(if ends_lo < 0.0 { ends_lo - 0.05 * span } else { ends_lo });
    let hi = max.unwrap_or(if ends_hi > 0.0 { ends_hi + 0.05 * span } else { ends_hi });
    if !(hi > lo && hi.is_finite() && lo.is_finite()) {
        return Scale { lo: 0.0, hi: 1.0, step: 0.2 };
    }
    // The ends with step `step`: an automatic end rounded out to a multiple of it.
    let ends = |step: f64| -> Option<Scale> {
        let lo_r = if min.is_some() { lo } else { ((lo / step) + 1e-9).floor() * step };
        let hi_r = if max.is_some() { hi } else { ((hi / step) - 1e-9).ceil() * step };
        (step.is_finite() && step > 0.0 && lo_r.is_finite() && hi_r.is_finite() && hi_r > lo_r).then_some(Scale { lo: lo_r, hi: hi_r, step })
    };
    if let Some(s) = major.filter(|m| m.is_finite() && *m > 0.0).and_then(ends)
        && (s.hi - s.lo) / s.step <= MAX_TICKS as f64 + 1e-9
    {
        return s;
    }
    let raw = (hi - lo) / 7.5;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    ends(step).unwrap_or(Scale { lo: 0.0, hi: 1.0, step: 0.2 })
}

/// The plot rectangle and the scales that map values and categories onto it.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub(super) plot: Rect,
    pub(super) horiz: bool,
    pub(super) n: usize,
    vs: Scale,
    xs: Option<Scale>,
    mid: bool,
}

impl Frame {
    /// Width of one category band.
    pub(super) fn band(&self) -> f32 {
        let len = if self.horiz { self.plot.h } else { self.plot.w };
        len / self.n.max(1) as f32
    }

    /// Start of category `i`'s band: its left edge, or for bars its top edge (category 0 at the
    /// bottom).
    fn band_start(&self, i: usize) -> f32 {
        let k = i as f32;
        if self.horiz { self.plot.bottom() - (k + 1.0) * self.band() } else { self.plot.x + k * self.band() }
    }

    /// A value's position along the value direction: y for columns, x for bars.
    pub(super) fn val(&self, v: f64) -> f32 {
        let t = ((v - self.vs.lo) / (self.vs.hi - self.vs.lo)).clamp(0.0, 1.0) as f32;
        if self.horiz { self.plot.x + t * self.plot.w } else { self.plot.bottom() - t * self.plot.h }
    }

    /// x of category `i` for line and area points.
    pub(super) fn cat_x(&self, i: usize) -> f32 {
        if !self.mid {
            return self.plot.x + (i as f32 + 0.5) * self.band();
        }
        if self.n <= 1 { self.plot.x + self.plot.w / 2.0 } else { self.plot.x + self.plot.w * i as f32 / (self.n - 1) as f32 }
    }

    /// x of a scatter point.
    fn scat_x(&self, x: f64) -> f32 {
        match self.xs {
            Some(s) => self.plot.x + ((x - s.lo) / (s.hi - s.lo)).clamp(0.0, 1.0) as f32 * self.plot.w,
            None => self.plot.x,
        }
    }
}

/// The value scale and, for scatter charts, the x scale, with their ticks and tick texts; the
/// secondary value scale when a chart type is plotted on one.
pub(super) struct Axes {
    pub(super) vs: Scale,
    vticks: Vec<(f64, String)>,
    xs: Option<Scale>,
    xticks: Vec<(f64, String)>,
    sec: Option<(Scale, Vec<(f64, String)>)>,
}

impl Axes {
    pub(super) fn of(c: &Chart) -> Axes {
        let (vs, vticks) = value_scale(c, false, &c.val, &c.val_fmt);
        let sec = c.sec.as_ref().filter(|_| c.groups.iter().any(|g| g.secondary)).map(|a| value_scale(c, true, a, &c.sec_fmt));
        let xs = (c.kind == Kind::Scatter).then(|| {
            let (xl, xh) = x_extent(c);
            nice_scale(xl, xh, false, c.cat.min, c.cat.max, c.cat.major)
        });
        // Scatter x values show in the axis's own format, else their cache's (dates included).
        let xfmt = c
            .cat
            .fmt
            .clone()
            .or_else(|| c.groups.iter().flat_map(|g| &g.series).find_map(|s| s.x_fmt.clone()))
            .unwrap_or_else(|| "General".to_string());
        let xticks = xs.map(|s| s.ticks().into_iter().map(|t| (t, category_text(t, &xfmt, s.step, c.date1904))).collect()).unwrap_or_default();
        Axes { vs, vticks, xs, xticks, sec }
    }
}

/// The scale of the primary (or `secondary`) value axis `a` over the values of the chart types
/// plotted on it, with its ticks in format `fmt`.
fn value_scale(c: &Chart, secondary: bool, a: &Axis, fmt: &str) -> (Scale, Vec<(f64, String)>) {
    let on = || c.groups.iter().filter(move |g| g.secondary == secondary);
    let (dlo, dhi) = extent(on(), c.n);
    let (vmin, vmax, major) = if on().any(|g| g.grouping == Grouping::Percent) { (Some(0.0), Some(1.0), a.major) } else { (a.min, a.max, a.major) };
    // Lines and scatters don't need zero for a narrow range (1000..1050 stays on its own scale).
    let zero = !on().all(|g| matches!(g.kind, Kind::Line | Kind::Scatter | Kind::Stock)) || !narrow(dlo, dhi);
    let vs = nice_scale(dlo, dhi, zero, vmin, vmax, major);
    let ticks = vs.ticks().into_iter().map(|t| (t, format_value(t, fmt, vs.step))).collect();
    (vs, ticks)
}

/// The room around the plot for axis labels, and the category labels as they'll be drawn.
struct Margins {
    left: f32,
    /// Room for the secondary value axis' labels.
    right: f32,
    top: f32,
    bottom: f32,
    /// Category labels in lines (none for scatter charts or a deleted category axis).
    wrapped: Vec<Vec<String>>,
    /// Every `skip`-th category label is shown under a column chart.
    skip: usize,
}

/// The margins the axis labels need around the plot in `area`. Category labels under a column
/// chart wrap to the band; labels that still don't fit show every k-th category.
fn axis_margins(c: &Chart, ax: &Axes, area: Rect) -> Margins {
    let fs = c.text_size;
    let lh = fs * LINE;
    let scatter = c.kind == Kind::Scatter;
    let (cat_on, val_on) = (!c.cat.deleted, !c.val.deleted);
    let left = if c.horizontal {
        if cat_on { widest(&c.cats, fs).min(area.w * 0.4) + GAP } else { 0.0 }
    } else if val_on {
        ax.vticks.iter().map(|(_, s)| text_w(s, fs)).fold(0.0, f32::max) + GAP
    } else {
        0.0
    };
    // The secondary value axis: on the right of a column chart, along the top of bars across.
    let sec_on = ax.sec.is_some() && c.sec.as_ref().is_some_and(|a| !a.deleted);
    let right = match &ax.sec {
        Some((_, ticks)) if sec_on && !c.horizontal => ticks.iter().map(|(_, s)| text_w(s, fs)).fold(0.0, f32::max) + GAP,
        _ => 0.0,
    };
    let band_w = (area.w - left - right).max(1.0) / c.n as f32;
    let wrapped: Vec<Vec<String>> = if cat_on && !scatter {
        let width = if c.horizontal { left - GAP } else { band_w };
        c.cats.iter().map(|s| wrap(s, width, fs)).collect()
    } else {
        Vec::new()
    };
    let widest_line = wrapped.iter().flatten().map(|s| text_w(s, fs)).fold(0.0, f32::max);
    let skip = ((widest_line / band_w).ceil() as usize).max(1);
    let label_lines = wrapped.iter().map(Vec::len).max().unwrap_or(1) as f32;
    let bottom = if cat_on && !c.horizontal {
        if scatter { lh + GAP } else { label_lines * lh + GAP }
    } else if c.horizontal && val_on {
        lh + GAP
    } else {
        0.0
    };
    let top = if c.horizontal {
        if sec_on { lh + GAP } else { 0.0 }
    } else if val_on || sec_on {
        lh / 2.0
    } else {
        0.0
    };
    Margins { left, right, top, bottom, wrapped, skip }
}

/// A bar, column, line, area, stock or scatter chart in `area`, or several on one frame:
/// gridlines, the chart types back to front, axes, then labels.
pub(super) fn cartesian(out: &mut Vec<GraphicItem>, c: &Chart, area: Rect) {
    if c.n == 0 {
        return;
    }
    // Axis titles take room from the plot: the vertical axis's at the left, the horizontal
    // axis's at the bottom (bars across: the value axis is the horizontal one).
    let (h_axis, v_axis) = if c.horizontal { (&c.val, &c.cat) } else { (&c.cat, &c.val) };
    let (h_title, v_title) = (h_axis.title.as_ref(), v_axis.title.as_ref());
    let full = area;
    let mut area = area;
    if let Some(t) = v_title {
        let room = title_room(t).min(area.w / 2.0);
        area.x += room;
        area.w -= room;
    }
    if let Some(t) = h_title {
        area.h -= title_room(t).min(area.h / 2.0);
    }
    let ax = Axes::of(c);
    let m = axis_margins(c, &ax, area);
    let plot = Rect { x: area.x + m.left, y: area.y + m.top, w: area.w - m.left - m.right, h: area.h - m.top - m.bottom };
    if plot.w < 1.0 || plot.h < 1.0 {
        return;
    }
    // 3-D chart types: the front plane gives up room for the depth behind it.
    let depth = Projection::of(c, plot);
    let plot = depth.as_ref().map_or(plot, |p| p.front(plot));
    let g = Frame { plot, horiz: c.horizontal, n: c.n, vs: ax.vs, xs: ax.xs, mid: c.val.mid };
    if let Some(p) = &depth {
        walls(out, c, &g, &ax.vticks, p);
    } else {
        draw_grids(out, c, &g, &ax);
    }
    // Secondary value axis: its own scale on the same frame (Word draws gridlines from the
    // primary axis only).
    let g2 = ax.sec.as_ref().map(|(vs, _)| Frame { vs: *vs, ..g });
    let mut labels = Vec::new();
    let mut order: Vec<&Group> = c.groups.iter().collect();
    order.sort_by_key(|grp| grp.kind.layer());
    for grp in order {
        let f = if grp.secondary { g2.as_ref().unwrap_or(&g) } else { &g };
        if let (Some(p), Some(_)) = (&depth, grp.depth) {
            match grp.kind {
                Kind::Bar => bars_3d(out, c, grp, f, p, &mut labels),
                Kind::Line => lines_3d(out, c, grp, f, p, &mut labels),
                Kind::Area => areas_3d(out, c, grp, f, p, &mut labels),
                Kind::Stock | Kind::Scatter | Kind::Pie | Kind::Doughnut | Kind::Radar => {}
            }
            continue;
        }
        match grp.kind {
            Kind::Bar => bars(out, c, grp, f, &mut labels),
            Kind::Line | Kind::Stock => {
                draw_stock(out, grp, f);
                lines(out, c, grp, f, &mut labels);
            }
            Kind::Area => areas(out, c, grp, f, &mut labels),
            Kind::Scatter => draw_scatter(out, c, grp, f, &mut labels),
            Kind::Pie | Kind::Doughnut | Kind::Radar => {}
        }
    }
    draw_axes(out, c, &g);
    draw_data_labels(out, &labels, c.text_size, g.horiz && c.kind == Kind::Bar);
    if !c.val.deleted {
        draw_value_labels(out, c, &g, &ax.vticks, Rect { w: m.left, ..area });
    }
    if let (Some(g2), Some((_, ticks)), Some(sec)) = (g2, &ax.sec, &c.sec)
        && !sec.deleted
    {
        let (fs, lh) = (c.text_size, c.text_size * LINE);
        if g.horiz {
            // Bars across: the axis runs along the top, its labels in a row above the plot, each
            // centred on its tick.
            if sec.line {
                stroke(out, seg(plot.x, plot.y, plot.right(), plot.y), AXIS, 0.75);
            }
            for (t, s) in ticks {
                text_at(out, g2.val(*t), plot.y - GAP / 2.0 - lh, s, fs);
            }
        } else {
            if sec.line {
                stroke(out, seg(plot.right(), plot.y, plot.right(), plot.bottom()), AXIS, 0.75);
            }
            for (t, s) in ticks {
                let r = Rect { x: plot.right() + GAP, y: g2.val(*t) - lh / 2.0, w: (m.right - GAP).max(0.0), h: lh };
                text(out, r, s, fs, TextAlign::Left, false);
            }
        }
    }
    if !c.cat.deleted {
        draw_category_labels(out, c, &g, &ax.xticks, &m, area);
    }
    if let Some(t) = v_title {
        // Turned to read bottom to top, left of the tick labels, centred on the plot's height.
        let lh = t.size * LINE;
        let s = clip_text(&t.text, plot.h.max(full.h * 0.5), t.size);
        let w = text_w(&s, t.size) + 4.0;
        let (cx, cy) = (full.x + lh / 2.0, plot.y + plot.h / 2.0);
        title_item(out, t, s, Rect { x: cx - w / 2.0, y: cy - lh / 2.0, w, h: lh }, 270.0);
    }
    if let Some(t) = h_title {
        // Centred under the category labels.
        let lh = t.size * LINE;
        let s = clip_text(&t.text, area.w, t.size);
        title_item(out, t, s, Rect { x: plot.x + plot.w / 2.0 - area.w / 2.0, y: full.bottom() - lh, w: area.w, h: lh }, 0.0);
    }
}

/// Gap between an axis title and the axis labels, as a share of the title's size.
const TITLE_GAP: f32 = 0.4;

/// Room an axis title takes from the plot: its line and a gap.
fn title_room(t: &AxisTitle) -> f32 {
    t.size * (LINE + TITLE_GAP)
}

fn title_item(out: &mut Vec<GraphicItem>, t: &AxisTitle, s: String, r: Rect, rotation: f32) {
    out.push(GraphicItem::Text {
        rect: r.arr(),
        text: s,
        size: t.size,
        color: t.color,
        bold: t.bold,
        align: TextAlign::Center,
        font: None,
        rotation,
    });
}

/// Most gridlines drawn along one axis (a category axis has one per category).
const MAX_GRIDLINES: usize = 1000;

/// Gridlines: the minor ones under the major ones, of the value axis (across the value ticks) and
/// of the category axis (scatter: at the x ticks; else major at the category boundaries, minor at
/// the categories' centres). Minor ticks are a fifth of the major unit, Word's automatic minor
/// unit.
fn draw_grids(out: &mut Vec<GraphicItem>, c: &Chart, g: &Frame, ax: &Axes) {
    let plot = g.plot;
    // A line across the plot at `p` along the value direction, or along the category direction.
    let val_line = |p: f32| if g.horiz { seg(p, plot.y, p, plot.bottom()) } else { seg(plot.x, p, plot.right(), p) };
    let cat_line = |p: f32| if g.horiz { seg(plot.x, p, plot.right(), p) } else { seg(p, plot.y, p, plot.bottom()) };
    let (v, k) = (&c.val, &c.cat);
    if v.minor_grid {
        for t in minor_ticks(ax.vs) {
            stroke(out, val_line(g.val(t)), v.minor_style.0, v.minor_style.1);
        }
    }
    if k.minor_grid {
        match ax.xs {
            Some(xs) => minor_ticks(xs).into_iter().for_each(|t| stroke(out, cat_line(g.scat_x(t)), k.minor_style.0, k.minor_style.1)),
            None => category_marks(g, true).into_iter().for_each(|p| stroke(out, cat_line(p), k.minor_style.0, k.minor_style.1)),
        }
    }
    if v.grid {
        for (t, _) in &ax.vticks {
            stroke(out, val_line(g.val(*t)), v.grid_style.0, v.grid_style.1);
        }
    }
    if k.grid {
        match ax.xs {
            Some(_) => ax.xticks.iter().for_each(|(t, _)| stroke(out, cat_line(g.scat_x(*t)), k.grid_style.0, k.grid_style.1)),
            None => category_marks(g, false).into_iter().for_each(|p| stroke(out, cat_line(p), k.grid_style.0, k.grid_style.1)),
        }
    }
}

/// The minor ticks of `s` (a fifth of its step) that aren't major ticks.
fn minor_ticks(s: Scale) -> Vec<f64> {
    let minor = s.step / 5.0;
    if !(minor.is_finite() && minor > 0.0) {
        return Vec::new();
    }
    let n = (((s.hi - s.lo) / minor + 1e-9).floor().max(0.0) as usize).min(MAX_TICKS * 5);
    (1..=n).filter(|k| k % 5 != 0).map(|k| s.lo + k as f64 * minor).collect()
}

/// Positions along the category direction of the category axis's tick marks (`centres`: of the
/// categories' centres instead). Categories between the ticks (`crossBetween` = `between`) have
/// their ticks at the band edges; categories on the ticks (`midCat`) at the categories, with
/// the centres halfway between them. At most [`MAX_GRIDLINES`], evenly picked.
fn category_marks(g: &Frame, centres: bool) -> Vec<f32> {
    let band = g.band();
    let along = |t: f32| if g.horiz { g.plot.bottom() - t * band } else { g.plot.x + t * band };
    let pts: Vec<f32> = if g.mid && !g.horiz {
        let at: Vec<f32> = (0..g.n).map(|i| g.cat_x(i)).collect();
        if centres { at.windows(2).filter_map(|w| Some((w.first()? + w.get(1)?) / 2.0)).collect() } else { at }
    } else if centres {
        (0..g.n).map(|i| along(i as f32 + 0.5)).collect()
    } else {
        (0..=g.n).map(|i| along(i as f32)).collect()
    };
    let stride = pts.len().div_ceil(MAX_GRIDLINES).max(1);
    pts.into_iter().step_by(stride).collect()
}

/// The category axis line, the plot area's outline and the value axis line, as the file shows them.
fn draw_axes(out: &mut Vec<GraphicItem>, c: &Chart, g: &Frame) {
    let plot = g.plot;
    if !c.cat.deleted && c.cat.line {
        let segs = if g.horiz { seg(plot.x, plot.y, plot.x, plot.bottom()) } else { seg(plot.x, plot.bottom(), plot.right(), plot.bottom()) };
        stroke(out, segs, AXIS, 0.75);
    }
    if let Some(color) = c.plot_border {
        stroke(out, rect_segs(plot.x, plot.y, plot.right(), plot.bottom()), color, 0.75);
    }
    if !c.val.deleted && c.val.line {
        let segs = if g.horiz { seg(plot.x, plot.bottom(), plot.right(), plot.bottom()) } else { seg(plot.x, plot.y, plot.x, plot.bottom()) };
        stroke(out, segs, AXIS, 0.75);
    }
}

/// Value tick labels: under the plot for bars, else right-aligned in `margin` (the area's left
/// margin).
fn draw_value_labels(out: &mut Vec<GraphicItem>, c: &Chart, g: &Frame, ticks: &[(f64, String)], margin: Rect) {
    let (fs, lh) = (c.text_size, c.text_size * LINE);
    for (t, s) in ticks {
        let p = g.val(*t);
        if g.horiz {
            text_at(out, p, g.plot.bottom() + GAP / 2.0, s, fs);
        } else {
            text(out, Rect { x: margin.x, y: p - lh / 2.0, w: margin.w - GAP, h: lh }, s, fs, TextAlign::Right, false);
        }
    }
}

/// Category labels: scatter x ticks under the plot, bar labels in the left margin, column
/// labels under their bands (every `m.skip`-th).
fn draw_category_labels(out: &mut Vec<GraphicItem>, c: &Chart, g: &Frame, xticks: &[(f64, String)], m: &Margins, area: Rect) {
    let (fs, lh) = (c.text_size, c.text_size * LINE);
    let plot = g.plot;
    if c.kind == Kind::Scatter {
        for (t, s) in xticks {
            text_at(out, g.scat_x(*t), plot.bottom() + GAP / 2.0, s, fs);
        }
    } else if g.horiz {
        for (i, lines) in m.wrapped.iter().enumerate() {
            let top = g.band_start(i) + g.band() / 2.0 - lines.len() as f32 * lh / 2.0;
            for (k, s) in lines.iter().enumerate() {
                text(out, Rect { x: area.x, y: top + k as f32 * lh, w: m.left - GAP, h: lh }, s, fs, TextAlign::Right, false);
            }
        }
    } else {
        let band = g.band();
        for (i, lines) in m.wrapped.iter().enumerate().filter(|(i, _)| i % m.skip == 0) {
            for (k, s) in lines.iter().enumerate() {
                let y = plot.bottom() + GAP / 2.0 + k as f32 * lh;
                text(out, Rect { x: g.cat_x(i) - band / 2.0, y, w: band, h: lh }, s, fs, TextAlign::Center, false);
            }
        }
    }
}

/// Whether data `lo..hi` is a narrow band away from zero: its span is under a sixth of the end
/// farther from zero (the mirror image for negative data).
pub(super) fn narrow(lo: f64, hi: f64) -> bool {
    if lo > 0.0 {
        (hi - lo) / hi < 1.0 / 6.0
    } else if hi < 0.0 {
        (hi - lo) / lo.abs() < 1.0 / 6.0
    } else {
        false
    }
}

/// Lowest and highest value of the chart types `groups` over `n` categories; stacked series add
/// up per category.
fn extent<'a>(groups: impl Iterator<Item = &'a Group>, n: usize) -> (f64, f64) {
    let mut pts: Vec<f64> = Vec::new();
    for grp in groups {
        if grp.stacked() {
            for i in 0..n {
                let (mut p, mut q) = (0.0, 0.0);
                for s in &grp.series {
                    match s.values.get(i).copied().flatten() {
                        Some(v) if v >= 0.0 => p += v,
                        Some(v) => q += v,
                        None => {}
                    }
                }
                pts.push(p);
                pts.push(q);
            }
        } else {
            pts.extend(grp.series.iter().flat_map(|s| s.values.iter().flatten().copied()));
        }
    }
    let lo = pts.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = pts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if lo.is_finite() && hi.is_finite() { (lo, hi) } else { (0.0, 0.0) }
}

/// Range of scatter x values (index + 1 where missing).
fn x_extent(c: &Chart) -> (f64, f64) {
    let xs: Vec<f64> = c.groups.iter().flat_map(|g| &g.series).flat_map(|s| s.xs.iter().flatten().copied()).collect();
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if lo.is_finite() && hi.is_finite() { (lo, hi) } else { (0.0, 1.0) }
}

fn bars(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, labels: &mut Vec<DataLabel>) {
    for b in bar_rects(c, grp, g) {
        fill(out, rect_segs(b.x0, b.y0, b.x1, b.y1), b.color);
        labels.extend(b.label);
    }
}

/// One bar of a bar or column chart: its rectangle (the front face of a 3-D bar), colour, depth
/// row and data label.
pub(super) struct BarRect {
    pub(super) x0: f32,
    pub(super) y0: f32,
    pub(super) x1: f32,
    pub(super) y1: f32,
    pub(super) color: Rgb,
    /// Row along the depth axis (a 3-D chart's series one behind another), 0 at the front.
    pub(super) row: usize,
    pub(super) label: Option<DataLabel>,
}

/// The bars of a bar chart type, series by series, in drawing order for a flat chart.
pub(super) fn bar_rects(c: &Chart, grp: &Group, g: &Frame) -> Vec<BarRect> {
    let stacked = grp.stacked();
    // A 3-D chart of series in rows: each series has the whole slot, in its own row.
    let rows = grp.depth.is_some_and(|d| d.rows) && !stacked;
    let slots = if stacked || rows { 1 } else { grp.series.len().max(1) };
    let ov = if stacked || rows { 0.0 } else { grp.overlap };
    let ns = slots as f32;
    let band = g.band();
    let thick = band / (ns - (ns - 1.0) * ov + grp.gap);
    let group = thick * (ns - (ns - 1.0) * ov);
    let mut pos = vec![0.0f64; g.n];
    let mut neg = vec![0.0f64; g.n];
    let mut out = Vec::new();
    for (j, s) in grp.series.iter().enumerate().filter(|(_, s)| has_values(s)) {
        let slot = if stacked || rows { 0 } else { j };
        let slot = (if g.horiz { slots.saturating_sub(1 + slot) } else { slot }) as f32;
        for i in 0..g.n {
            let Some(v) = s.values.get(i).copied().flatten() else { continue };
            let (v0, v1) = if !stacked {
                (0.0, v)
            } else if v >= 0.0 {
                let Some(p) = pos.get_mut(i) else { continue };
                let b = *p;
                *p += v;
                (b, *p)
            } else {
                let Some(q) = neg.get_mut(i) else { continue };
                let b = *q;
                *q += v;
                (b, *q)
            };
            let a = g.band_start(i) + (band - group) / 2.0 + slot * thick * (1.0 - ov);
            let (p0, p1) = (g.val(v0), g.val(v1));
            let (p0, p1) = (p0.min(p1), p0.max(p1));
            let (x0, y0, x1, y1) = if g.horiz { (p0, a, p1, a + thick) } else { (a, p0, a + thick, p1) };
            let label = label_of(s, i).map(|text| {
                let mid = a + thick / 2.0;
                let (x, y) = match (g.horiz, stacked) {
                    (true, true) => ((p0 + p1) / 2.0, mid),
                    (false, true) => (mid, (p0 + p1) / 2.0),
                    // Beyond the bar's end: right of a positive bar, above a positive column.
                    (true, false) => (if v >= 0.0 { p1 } else { p0 }, mid),
                    (false, false) => (mid, if v >= 0.0 { p0 } else { p1 }),
                };
                DataLabel { x, y, text, inside: stacked }
            });
            out.push(BarRect { x0, y0, x1, y1, color: c.point_colour(grp, j, i), row: if rows { j } else { 0 }, label });
        }
    }
    out
}

/// Whether a series has any point to draw.
pub(super) fn has_values(s: &Series) -> bool {
    s.values.iter().any(Option::is_some)
}

/// Line series; stacked ones are drawn at the running total of the series up to them. Stock
/// series without a line (`a:ln/a:noFill`) only place their data labels.
fn lines(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, labels: &mut Vec<DataLabel>) {
    for (j, pts) in line_points(grp, g) {
        if let Some(s) = grp.series.get(j) {
            for (i, p) in pts.iter().enumerate() {
                if let (Some((x, y)), Some(text)) = (p, label_of(s, i)) {
                    labels.push(DataLabel { x: *x, y: *y, text, inside: false });
                }
            }
            if grp.kind != Kind::Stock || s.line {
                stroke(out, runs(pts.into_iter()), c.series_colour(grp, j), s.line_w);
            }
        }
    }
}

/// The points of each line series with values (by series index); stacked ones at the running
/// total of the series up to them.
pub(super) fn line_points(grp: &Group, g: &Frame) -> Vec<(usize, Vec<Option<(f32, f32)>>)> {
    let mut total = vec![0.0f64; g.n];
    let mut out = Vec::new();
    for (j, s) in grp.series.iter().enumerate().filter(|(_, s)| has_values(s)) {
        let pts: Vec<Option<(f32, f32)>> = (0..g.n)
            .map(|i| {
                let v = s.values.get(i).copied().flatten()?;
                let y = match total.get_mut(i) {
                    Some(t) if grp.stacked() => {
                        *t += v;
                        *t
                    }
                    _ => v,
                };
                Some((g.cat_x(i), g.val(y)))
            })
            .collect();
        out.push((j, pts));
    }
    out
}

fn areas(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, labels: &mut Vec<DataLabel>) {
    for (j, top, base) in area_bands(grp, g) {
        let mut segs = Vec::with_capacity(2 * g.n + 1);
        for (i, t) in top.iter().enumerate() {
            let (x, y) = (g.cat_x(i), g.val(*t));
            segs.push(if i == 0 { PathSeg::Move(x, y) } else { PathSeg::Line(x, y) });
        }
        for (i, b) in base.iter().enumerate().rev() {
            segs.push(PathSeg::Line(g.cat_x(i), g.val(*b)));
        }
        segs.push(PathSeg::Close);
        fill(out, segs, c.series_colour(grp, j));
        area_labels(grp, j, &top, &base, g, labels);
    }
}

/// The top and base value of each category for every area series with values (by series index):
/// stacked series sit on the ones before them.
pub(super) fn area_bands(grp: &Group, g: &Frame) -> Vec<(usize, Vec<f64>, Vec<f64>)> {
    let mut base = vec![0.0f64; g.n];
    let mut out = Vec::new();
    for (j, s) in grp.series.iter().enumerate().filter(|(_, s)| has_values(s)) {
        let vals = (0..g.n).map(|i| s.values.get(i).copied().flatten().unwrap_or(0.0));
        let top: Vec<f64> = base.iter().zip(vals).map(|(b, v)| b + v).collect();
        let next = if grp.stacked() { top.clone() } else { base.clone() };
        out.push((j, top, std::mem::replace(&mut base, next)));
    }
    out
}

/// Data labels of area series `j`, halfway between its base and top.
pub(super) fn area_labels(grp: &Group, j: usize, top: &[f64], base: &[f64], g: &Frame, labels: &mut Vec<DataLabel>) {
    let Some(s) = grp.series.get(j) else { return };
    for (i, (t, b)) in top.iter().zip(base).enumerate() {
        if let Some(text) = label_of(s, i) {
            labels.push(DataLabel { x: g.cat_x(i), y: g.val((t + b) / 2.0), text, inside: true });
        }
    }
}

/// Scatter series as lines, or as small markers when the series has no line.
fn draw_scatter(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame, labels: &mut Vec<DataLabel>) {
    const MARKER: f32 = 6.0;
    for (j, s) in grp.series.iter().enumerate() {
        let pts: Vec<Option<(f32, f32)>> =
            s.xs.iter()
                .zip(&s.values)
                .map(|(x, y)| match (x, y) {
                    (Some(x), Some(y)) => Some((g.scat_x(*x), g.val(*y))),
                    _ => None,
                })
                .collect();
        for (i, p) in pts.iter().enumerate() {
            if let (Some((x, y)), Some(text)) = (p, label_of(s, i)) {
                labels.push(DataLabel { x: *x, y: *y - MARKER / 2.0, text, inside: false });
            }
        }
        if s.line {
            stroke(out, runs(pts.iter().copied()), c.series_colour(grp, j), s.line_w);
            continue;
        }
        for (x, y) in pts.iter().flatten() {
            let rect = [x - MARKER / 2.0, y - MARKER / 2.0, MARKER, MARKER];
            out.push(GraphicItem::Shape { rect, kind: ShapeKind::Ellipse, fill: Some(c.series_colour(grp, j)), stroke: None, stroke_width: 0.0 });
        }
    }
}
