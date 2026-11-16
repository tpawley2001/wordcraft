//! Office 2016 charts (`cx:chartSpace`, chartex namespace) drawn from their data: waterfall,
//! box & whisker, treemap and sunburst charts. Other layouts return `None`, so the caller keeps the fallback picture.

use std::collections::{BTreeMap, HashMap};

use wordcraft_doc::graphic::{GraphicItem, TextAlign};
use wordcraft_doc::props::Rgb;

use super::canvas::{Rect, clip_text, fill, legend, rect_segs, runs, seg, stroke, text, text_at, text_w, widest};
use super::model::Pos;
use super::parse::truthy;
use super::plot::{Scale, label};
use super::sunburst::{MAX_LEVELS, draw_sunburst};
use super::{FONT, GAP, GRID, LINE, MAX_POINTS, MAX_SERIES, PAD, ShapeKind, TEXT, TITLE_SIZE, fill_colour, int, measure};
use crate::xml::El;

/// Gap between bars as a fraction of a bar's width when `cx:catScaling` doesn't say.
const GAP_WIDTH: f32 = 0.5;
/// Widest gap between bars, in bar widths.
const MAX_GAP_WIDTH: f32 = 5.0;
/// Gap between box groups as a fraction of a box's width when `cx:catScaling` doesn't say.
const BOX_GAP_WIDTH: f32 = 1.0;
/// Outlier and mean marker size, points.
const MARKER: f32 = 5.0;

/// One point of a waterfall series.
struct Point {
    /// The value as written in the data, for its label.
    raw: String,
    v: f64,
    total: bool,
}

/// The five-number summary of one box in a box & whisker chart, with its mean and outliers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoxStats {
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    /// Lower whisker end: the smallest value within 1.5 IQR below `q1`.
    pub lo: f64,
    /// Upper whisker end: the largest value within 1.5 IQR above `q3`.
    pub hi: f64,
    pub mean: f64,
    /// Values beyond the whiskers, ascending.
    pub outliers: Vec<f64>,
}

/// Quartiles (exclusive or inclusive median method, as `cx:statistics quartileMethod`), whisker
/// ends, mean and outliers of `values`; `None` when there are no finite values.
pub(crate) fn box_stats(values: &[f64], exclusive: bool) -> Option<BoxStats> {
    let mut s: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if s.is_empty() {
        return None;
    }
    s.sort_by(f64::total_cmp);
    let n = s.len();
    let nf = n as f64;
    // Position h is 1-based; s[n + 1] reads as s[n].
    let at = |k: usize| -> f64 { s.get(k.saturating_sub(1)).or(s.last()).copied().unwrap_or(0.0) };
    let quartile = |p: f64| -> f64 {
        let h = if exclusive { p * (nf + 1.0) } else { p * (nf - 1.0) + 1.0 }.clamp(1.0, nf);
        let i = h.floor();
        let k = i as usize;
        at(k) + (h - i) * (at(k + 1) - at(k))
    };
    let (q1, median, q3) = (quartile(0.25), quartile(0.5), quartile(0.75));
    let iqr = q3 - q1;
    let (fence_lo, fence_hi) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let inside = |v: &&f64| **v >= fence_lo && **v <= fence_hi;
    let lo = s.iter().find(inside).copied().unwrap_or(q1);
    let hi = s.iter().rev().find(inside).copied().unwrap_or(q3);
    let outliers = s.iter().copied().filter(|v| *v < fence_lo || *v > fence_hi).collect();
    let mean = s.iter().sum::<f64>() / nf;
    Some(BoxStats { q1, median, q3, lo, hi, mean, outliers })
}

/// Treemap tiles: one `[x0, y0, x1, y1]` per leaf of `leaves` (category path outermost-first, size),
/// in input order, inside the rectangle at (`x`, `y`) of size `w` x `h`. Sizes that aren't finite
/// and positive get an empty tile at (`x`, `y`).
///
/// Each level is tiled with the squarified algorithm (Bruls, Huizing and van Wijk): items largest
/// first, a strip along the shorter side of the free rectangle grows while its worst tile improves,
/// then the rest is tiled the same way; a group's children are tiled inside its tile. As Word does,
/// a tile is scored against [`TREEMAP_RATIO`] rather than against a square: its thickness across
/// the strip over its length along it, `t / l`, scores `max(t / (l * r), r * l / t)`. So two
/// near-equal items share a strip as tall-ish tiles where a square-seeking layout would stack them.
pub(crate) fn treemap_rects(leaves: &[(Vec<String>, f64)], x: f32, y: f32, w: f32, h: f32) -> Vec<[f32; 4]> {
    let mut out = vec![[x, y, x, y]; leaves.len()];
    if !([x, y, w, h].iter().all(|v| v.is_finite()) && w > 0.0 && h > 0.0) {
        return out;
    }
    // Sizes relative to the largest, so sums stay finite.
    let max = leaves.iter().map(|l| l.1).filter(|v| v.is_finite() && *v > 0.0).fold(0.0f64, f64::max);
    if max <= 0.0 {
        return out;
    }
    let sizes: Vec<f64> = leaves.iter().map(|l| if l.1.is_finite() && l.1 > 0.0 { l.1 / max } else { 0.0 }).collect();
    let members: Vec<usize> = (0..leaves.len()).filter(|&i| sizes.get(i).is_some_and(|v| *v > 0.0)).collect();
    let (x, y) = (f64::from(x), f64::from(y));
    let tree = Treemap { leaves, sizes: &sizes };
    tree.level(&members, 0, [x, y, x + f64::from(w), y + f64::from(h)], &mut out);
    out
}

/// Deepest category level a treemap nests; deeper path parts are ignored.
const MAX_TREEMAP_LEVELS: usize = 16;
/// The tile shape treemaps aim for, as thickness across a strip over length along it. Fitted to
/// Word's tiling of a three-level treemap: every value from 1.10 to 1.26 reproduces all its tiles,
/// 1 (a plain squarified layout) stacks two near-equal leaves Word puts side by side.
const TREEMAP_RATIO: f64 = 1.2;

struct Treemap<'a> {
    leaves: &'a [(Vec<String>, f64)],
    sizes: &'a [f64],
}

impl Treemap<'_> {
    /// Tiles leaves `members` (all sharing the first `depth` path parts) into `rect`.
    fn level(&self, members: &[usize], depth: usize, rect: [f64; 4], out: &mut [[f32; 4]]) {
        // (total size, leaves, is a group), groups in order of first appearance.
        let mut items: Vec<(f64, Vec<usize>, bool)> = Vec::new();
        let mut group_of: HashMap<&str, usize> = HashMap::new();
        for &i in members {
            let (Some((path, _)), Some(&v)) = (self.leaves.get(i), self.sizes.get(i)) else { continue };
            let name = if depth + 1 < path.len().min(MAX_TREEMAP_LEVELS) { path.get(depth) } else { None };
            match name {
                Some(name) => {
                    let k = *group_of.entry(name.as_str()).or_insert_with(|| {
                        items.push((0.0, Vec::new(), true));
                        items.len() - 1
                    });
                    if let Some(item) = items.get_mut(k) {
                        item.0 += v;
                        item.1.push(i);
                    }
                }
                None => items.push((v, vec![i], false)),
            }
        }
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        let values: Vec<f64> = items.iter().map(|it| it.0).collect();
        for ((_, ids, group), r) in items.iter().zip(squarify(&values, rect, TREEMAP_RATIO)) {
            if *group {
                self.level(ids, depth + 1, r, out);
            } else if let Some(slot) = ids.first().and_then(|&i| out.get_mut(i)) {
                *slot = r.map(|v| v as f32);
            }
        }
    }
}

/// Squarified tiles `[x0, y0, x1, y1]` of positive `values` (largest first) in `rect`.
fn squarify(values: &[f64], rect: [f64; 4], ratio: f64) -> Vec<[f64; 4]> {
    let [mut x0, mut y0, x1, y1] = rect;
    let mut rest: f64 = values.iter().sum();
    let mut out = Vec::with_capacity(values.len());
    let mut i = 0;
    while i < values.len() {
        let (fw, fh) = ((x1 - x0).max(0.0), (y1 - y0).max(0.0));
        // Wide free space takes a column at its left, tall free space a row at its top.
        let column = fw >= fh;
        let side = if column { fh } else { fw };
        let depth = if column { fw } else { fh };
        let score = |strip: &[f64]| -> f64 {
            let sum: f64 = strip.iter().sum();
            let t = depth * sum / rest;
            if !(t > 0.0 && side > 0.0) {
                return 0.0;
            }
            strip.iter().map(|v| side * v / sum).map(|l| (t / (l * ratio)).max(ratio * l / t)).fold(0.0, f64::max)
        };
        let mut j = i + 1;
        let mut best = values.get(i..j).map_or(0.0, score);
        while let Some(next) = values.get(i..j + 1).map(score) {
            if next > best {
                break;
            }
            best = next;
            j += 1;
        }
        let strip = values.get(i..j).unwrap_or_default();
        let sum: f64 = strip.iter().sum();
        let t = if j >= values.len() || rest <= 0.0 { depth } else { depth * sum / rest };
        let mut acc = 0.0;
        for (k, v) in strip.iter().enumerate() {
            let a = side * acc / sum;
            acc += v;
            let b = if k + 1 == strip.len() { side } else { side * acc / sum };
            out.push(if column { [x0, y0 + a, x0 + t, y0 + b] } else { [x0 + a, y0, x0 + b, y0 + t] });
        }
        if column {
            x0 += t;
        } else {
            y0 += t;
        }
        rest -= sum;
        i = j;
    }
    out
}

/// The items of an Office 2016 chart `w` × `h` points, or `None` when its layout isn't drawn.
pub(crate) fn chartex_items(space: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return None;
    }
    let chart = space.child("cx:chart")?;
    let plot = chart.child("cx:plotArea")?;
    let region = plot.child("cx:plotAreaRegion")?;
    let series = region.child("cx:series")?;
    match series.attr("layoutId") {
        Some("waterfall") => waterfall(space, chart, plot, series, theme, w, h),
        Some("boxWhisker") if region.children("cx:series").all(|s| s.attr("layoutId") == Some("boxWhisker")) => {
            box_whisker(space, chart, plot, region, theme, w, h)
        }
        Some("treemap") => treemap(space, chart, series, theme, w, h),
        Some("sunburst") => sunburst(space, chart, series, theme, w, h),
        _ => None,
    }
}

/// The chart background and title; returns the items so far and the area left for the rest.
fn frame(space: &El, chart: &El, theme: &[Rgb], w: f32, h: f32) -> (Vec<GraphicItem>, Rect) {
    let mut out = Vec::new();
    if let Some(bg) = space.child("cx:spPr").and_then(|p| fill_colour(p, theme)) {
        fill(&mut out, rect_segs(0.0, 0.0, w, h), bg);
    }
    let ln = space.child("cx:spPr").and_then(|p| p.child("a:ln"));
    let border = match ln {
        Some(l) if l.child("a:noFill").is_some() => None,
        Some(l) => fill_colour(l, theme).or(Some(GRID)),
        None => Some(GRID),
    };
    if let Some(colour) = border {
        let width = ln.and_then(|l| l.attr("w")).and_then(|v| measure(v, 12_700.0)).unwrap_or(0.75).max(0.0);
        stroke(&mut out, rect_segs(0.0, 0.0, w, h), colour, width);
    }
    let mut area = Rect { x: PAD, y: PAD, w: (w - 2.0 * PAD).max(1.0), h: (h - 2.0 * PAD).max(1.0) };
    if let Some(t) = chart.child("cx:title") {
        let s = label(t.deep_text().trim());
        let lh = TITLE_SIZE * LINE;
        text(
            &mut out,
            Rect { x: area.x, y: area.y, w: area.w, h: lh },
            if s.is_empty() { "Chart Title" } else { &s },
            TITLE_SIZE,
            TextAlign::Center,
            false,
        );
        area = Rect { y: area.y + lh + PAD, h: (area.h - lh - PAD).max(1.0), ..area };
    }
    (out, area)
}

/// The chart's `cx:legend` with `entries`, when it has one; returns the area left.
fn chart_legend(out: &mut Vec<GraphicItem>, chart: &El, entries: &[(String, Rgb)], area: Rect, size: f32) -> Rect {
    let Some(l) = chart.child("cx:legend") else { return area };
    let pos = match l.attr("pos").map(str::trim) {
        Some("l") => Pos::Left,
        Some("t") => Pos::Top,
        Some("b") => Pos::Bottom,
        _ => Pos::Right,
    };
    legend(out, entries, pos, area, size)
}

/// Office 2016 value axes use about ten intervals with steps of 1, 2 or 5 times a
/// power of ten. Explicit bounds replace the padded ends and are not rounded.
fn value_scale(plot: &El, lo: f64, hi: f64) -> Scale {
    let scaling = plot.children("cx:axis").find_map(|a| a.child("cx:valScaling"));
    let bound = |name| scaling.and_then(|s| s.attr(name)).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite());
    let (min, max) = (bound("min"), bound("max"));
    let (lo, hi) = (lo.min(0.0), hi.max(0.0));
    let hi = if hi > lo { hi } else { lo + 1.0 };
    let span = hi - lo;
    let lo = min.unwrap_or(if lo < 0.0 { lo - 0.05 * span } else { lo });
    let hi = max.unwrap_or(if hi > 0.0 { hi + 0.05 * span } else { hi });
    // Invalid or overflowing input must not produce non-finite drawing coordinates.
    let fallback = Scale { lo: 0.0, hi: 1.0, step: 0.1 };
    if !(hi > lo && hi.is_finite() && lo.is_finite()) {
        return fallback;
    }
    let raw = (hi - lo) / 10.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let lo = if min.is_some() { lo } else { (lo / step + 1e-9).floor() * step };
    let hi = if max.is_some() { hi } else { (hi / step - 1e-9).ceil() * step };
    if !(step.is_finite() && step > 0.0 && lo.is_finite() && hi.is_finite() && hi > lo) {
        return fallback;
    }
    Scale { lo, hi, step }
}

/// A value axis on `scale`: tick labels at the left of `area`, gridlines when the value axis has
/// them, room for category labels underneath. Returns the plot rectangle.
fn value_axis(out: &mut Vec<GraphicItem>, plot: &El, area: Rect, scale: &Scale, size: f32) -> Rect {
    let lh = size * LINE;
    let ticks = scale.ticks();
    let tick_text: Vec<String> = ticks.iter().map(|t| number(*t)).collect();
    let left = widest(&tick_text, size) + GAP;
    let plot_r = Rect { x: area.x + left, y: area.y + lh / 2.0, w: (area.w - left).max(1.0), h: (area.h - lh * 1.5 - GAP).max(1.0) };
    if plot.children("cx:axis").any(|a| a.child("cx:valScaling").is_some() && a.child("cx:majorGridlines").is_some()) {
        for t in &ticks {
            let y = y_on(plot_r, scale, *t);
            stroke(out, seg(plot_r.x, y, plot_r.right(), y), GRID, 0.75);
        }
    }
    for (t, s) in ticks.iter().zip(&tick_text) {
        let r = Rect { x: area.x, y: y_on(plot_r, scale, *t) - lh / 2.0, w: left - GAP, h: lh };
        text(out, r, s, size, TextAlign::Right, false);
    }
    plot_r
}

/// The y of value `v` in `plot_r` on `scale` (y grows down).
fn y_on(plot_r: Rect, scale: &Scale, v: f64) -> f32 {
    plot_r.bottom() - (((v - scale.lo) / (scale.hi - scale.lo)) as f32 * plot_r.h)
}

/// The category axis' `cx:catScaling gapWidth`, `default` when absent.
fn gap_width(plot: &El, default: f32) -> f32 {
    plot.children("cx:axis")
        .find_map(|a| a.child("cx:catScaling"))
        .and_then(|c| c.attr("gapWidth"))
        .and_then(|g| g.trim().parse::<f32>().ok())
        .filter(|g| g.is_finite())
        .map_or(default, |g| g.clamp(0.0, MAX_GAP_WIDTH))
}

/// The `cx:data` a series' `cx:dataId` names.
fn series_data<'a>(space: &'a El, series: &El) -> Option<&'a El> {
    let id = series.child("cx:dataId").and_then(|d| d.attr("val"))?.trim();
    space.child("cx:chartData")?.children("cx:data").find(|d| d.attr("id").map(str::trim) == Some(id))
}

/// A waterfall chart: one series of rising and falling bars on a running sum.
fn waterfall(space: &El, chart: &El, plot: &El, series: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    let data = series_data(space, series)?;
    let totals: Vec<usize> = series
        .child("cx:layoutPr")
        .and_then(|l| l.child("cx:subtotals"))
        .into_iter()
        .flat_map(|s| s.children("cx:idx"))
        .filter_map(|i| i.attr("val").and_then(index))
        .collect();
    let points = read_points(data, &totals);
    if points.iter().all(Option::is_none) {
        return None;
    }
    let n = points.len();
    let cats = categories(data, n);

    // Bars: (from, to) in value units, on the running sum; totals stand on the axis.
    let mut sum = 0.0f64;
    let spans: Vec<Option<(f64, f64)>> = points
        .iter()
        .map(|p| {
            let p = p.as_ref()?;
            let span = if p.total { (0.0, p.v) } else { (sum, sum + p.v) };
            sum = span.1;
            Some(span)
        })
        .collect();
    let ends = spans.iter().flatten().flat_map(|&(a, b)| [a, b]);
    let (lo, hi) = ends.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let scale = value_scale(plot, lo, hi);
    let size = text_size(space);
    let lh = size * LINE;

    let (mut out, area) = frame(space, chart, theme, w, h);
    let (inc, dec, tot) = (accent(theme, 4), accent(theme, 5), accent(theme, 6));
    let own = series.child("cx:spPr").and_then(|p| fill_colour(p, theme));
    let colour = |k: usize, p: &Point| -> Rgb {
        let pt = series.children("cx:dataPt").find(|d| d.attr("idx").and_then(index) == Some(k));
        pt.and_then(|d| d.child("cx:spPr")).and_then(|sp| fill_colour(sp, theme)).or(own).unwrap_or(if p.total {
            tot
        } else if p.v >= 0.0 {
            inc
        } else {
            dec
        })
    };
    let entries = [("Increase".to_string(), inc), ("Decrease".to_string(), dec), ("Total".to_string(), tot)];
    let area = chart_legend(&mut out, chart, &entries, area, size);

    // Value labels at the left, category labels under the plot.
    let plot_r = value_axis(&mut out, plot, area, &scale, size);
    let y_of = |v: f64| -> f32 { y_on(plot_r, &scale, v) };
    let band = plot_r.w / n.max(1) as f32;
    let bar_w = band / (1.0 + gap_width(plot, GAP_WIDTH));
    for (k, cat) in cats.iter().enumerate() {
        let cx = plot_r.x + band * (k as f32 + 0.5);
        text_at(&mut out, cx, plot_r.bottom() + GAP, &clip_text(cat, band, size), size);
    }

    let dl = series.child("cx:dataLabels").filter(|d| d.child("cx:visibility").and_then(|v| v.attr("value")).is_none_or(truthy));
    let inside = dl.and_then(|d| d.attr("pos")).map(str::trim) == Some("inEnd");
    let mut labels = Vec::new();
    for (k, (p, s)) in points.iter().zip(&spans).enumerate() {
        let (Some(p), Some((from, to))) = (p, *s) else { continue };
        let x0 = plot_r.x + band * k as f32 + (band - bar_w) / 2.0;
        let (ya, yb) = (y_of(from), y_of(to));
        fill(&mut out, rect_segs(x0, ya.min(yb), x0 + bar_w, ya.max(yb)), colour(k, p));
        if dl.is_some() {
            // The bar's end: its top when it rises, its bottom when it falls.
            let up = p.total || p.v >= 0.0;
            let (top, bottom) = (ya.min(yb), ya.max(yb));
            let y = match (up, inside) {
                (true, false) => top - lh,
                (true, true) => top,
                (false, false) => bottom,
                (false, true) => bottom - lh,
            };
            labels.push((x0 + bar_w / 2.0, y, p.raw.clone()));
        }
    }
    for (cx, y, s) in labels {
        let s = clip_text(&s, band.max(text_w("0", size)), size);
        text_at(&mut out, cx, y, &s, size);
    }
    Some(out)
}

/// Treemap label inset from a tile's left edge, points.
const TILE_INSET: f32 = 7.0;
/// Treemap label first baseline below a tile's top edge, points.
const TILE_BASELINE: f32 = 16.0;
/// Treemap tile outline width, points.
const TILE_OUTLINE: f32 = 1.5;

/// A treemap: one tile per leaf, nested by category level, coloured by top-level group.
fn treemap(space: &El, chart: &El, series: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    let data = series_data(space, series)?;
    let leaves = treemap_leaves(data);
    if leaves.is_empty() {
        return None;
    }
    // Top-level groups, largest first (ties in data order), as the tiles are placed.
    let mut groups: Vec<(String, f64)> = Vec::new();
    for (path, v) in &leaves {
        let name = path.first().cloned().unwrap_or_default();
        let v = if v.is_finite() && *v > 0.0 { *v } else { 0.0 };
        match groups.iter_mut().find(|g| g.0 == name) {
            Some(g) => g.1 += v,
            None => groups.push((name, v)),
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    let colour_of = |k: usize| accent(theme, 4 + k % 6);
    let group_of = |path: &[String]| -> usize {
        let name = path.first().map_or("", String::as_str);
        groups.iter().position(|g| g.0 == name).unwrap_or(0)
    };

    let size = text_size(space);
    let (mut out, area) = frame(space, chart, theme, w, h);
    let entries: Vec<(String, Rgb)> = groups.iter().enumerate().map(|(k, g)| (g.0.clone(), colour_of(k))).collect();
    let area = chart_legend(&mut out, chart, &entries, area, size);
    let tiles = treemap_rects(&leaves, area.x, area.y, area.w, area.h);

    for ((path, _), r) in leaves.iter().zip(&tiles) {
        let (tw, th) = (r[2] - r[0], r[3] - r[1]);
        if tw > 0.0 && th > 0.0 {
            let rect = [r[0], r[1], tw, th];
            let fill = Some(colour_of(group_of(path)));
            out.push(GraphicItem::Shape { rect, kind: ShapeKind::Rectangle, fill, stroke: Some(Rgb(0xFF, 0xFF, 0xFF)), stroke_width: TILE_OUTLINE });
        }
    }

    let Some(dl) = series.child("cx:dataLabels") else { return Some(out) };
    let vis = dl.child("cx:visibility");
    if !flag(vis, "categoryName", true) {
        return Some(out);
    }
    let parents = series
        .child("cx:layoutPr")
        .and_then(|l| l.child("cx:parentLabelLayout"))
        .and_then(|p| p.attr("val"))
        .map(str::trim)
        .is_none_or(|v| v != "none");
    let lh = size * LINE;
    let label_at = |out: &mut Vec<GraphicItem>, r: [f32; 4], line: f32, s: &str, colour: Rgb, bold: bool| {
        let top = r[1] + TILE_BASELINE - size + line * lh;
        let width = r[2] - r[0] - 2.0 * TILE_INSET;
        if s.is_empty() || width < text_w("0", size) || top + lh > r[3] {
            return;
        }
        let rect = Rect { x: r[0] + TILE_INSET, y: top, w: width, h: lh };
        let text = clip_text(s, width, size);
        out.push(GraphicItem::Text { rect: rect.arr(), text, size, color: colour, bold, align: TextAlign::Left, font: None, rotation: 0.0 });
    };
    // Group labels sit at their group's top-left, over the first tile, whose own label moves down.
    let mut group_tiles: Vec<Option<[f32; 4]>> = vec![None; groups.len()];
    if parents {
        for ((path, _), r) in leaves.iter().zip(&tiles) {
            if path.len() < 2 || r[2] <= r[0] || r[3] <= r[1] {
                continue;
            }
            if let Some(slot) = group_tiles.get_mut(group_of(path)) {
                let u = slot.get_or_insert(*r);
                *u = [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])];
            }
        }
    }
    for ((path, _), r) in leaves.iter().zip(&tiles) {
        let k = group_of(path);
        let colour = label_colour(colour_of(k));
        let under_group =
            path.len() >= 2 && group_tiles.get(k).copied().flatten().is_some_and(|g| (g[0] - r[0]).abs() < 0.01 && (g[1] - r[1]).abs() < 0.01);
        let name = path.last().map_or("", String::as_str);
        label_at(&mut out, *r, if under_group { 1.0 } else { 0.0 }, name, colour, false);
    }
    for ((name, _), (k, g)) in groups.iter().zip(group_tiles.iter().enumerate()) {
        if let Some(g) = g {
            label_at(&mut out, *g, 0.0, name, label_colour(colour_of(k)), true);
        }
    }
    Some(out)
}

/// Text colour on a tile filled `fill`: white on dark fills, the chart text colour on light ones.
fn label_colour(fill: Rgb) -> Rgb {
    let luma = 0.299 * f32::from(fill.0) + 0.587 * f32::from(fill.1) + 0.114 * f32::from(fill.2);
    if luma < 160.0 { Rgb(0xFF, 0xFF, 0xFF) } else { TEXT }
}

/// A treemap's leaves: category path outermost-first (`cx:strDim type="cat"` levels are written
/// innermost first) and size (`cx:numDim type="size"`), for every point with a size.
fn treemap_leaves(data: &El) -> Vec<(Vec<String>, f64)> {
    let Some(lvl) = data.children("cx:numDim").find(|d| d.attr("type") == Some("size")).and_then(|d| d.child("cx:lvl")) else {
        return Vec::new();
    };
    let mut sizes: BTreeMap<usize, f64> = BTreeMap::new();
    for p in lvl.children("cx:pt").take(MAX_POINTS) {
        let (Some(k), Ok(v)) = (p.attr("idx").and_then(index), p.text().trim().parse::<f64>()) else { continue };
        sizes.insert(k, v);
    }
    let levels: Vec<HashMap<usize, String>> = data
        .children("cx:strDim")
        .find(|d| d.attr("type") == Some("cat"))
        .into_iter()
        .flat_map(|d| d.children("cx:lvl"))
        .take(MAX_TREEMAP_LEVELS)
        .map(|l| l.children("cx:pt").filter_map(|p| Some((p.attr("idx").and_then(index)?, label(p.text().trim())))).collect())
        .collect();
    sizes
        .into_iter()
        .map(|(k, v)| {
            let mut path: Vec<String> = levels.iter().rev().map(|l| l.get(&k).cloned().unwrap_or_default()).collect();
            // A missing inner name makes the point a leaf one level up.
            while path.len() > 1 && path.last().is_some_and(String::is_empty) {
                path.pop();
            }
            (path, v)
        })
        .collect()
}

/// A sunburst chart: one ring per category level, each branch in its top-level accent colour.
fn sunburst(space: &El, chart: &El, series: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    let data = series_data(space, series)?;
    let leaves = category_paths(data);
    if !leaves.iter().any(|(_, v)| v.is_finite() && *v > 0.0) {
        return None;
    }
    let (mut out, area) = frame(space, chart, theme, w, h);
    // Word's sunburst reaches up to just under the title.
    let area = if chart.child("cx:title").is_some() { Rect { y: area.y - GAP, h: area.h + GAP, ..area } } else { area };
    let labels = series.child("cx:dataLabels").is_some_and(|d| d.child("cx:visibility").and_then(|v| v.attr("categoryName")).is_some_and(truthy));
    let colour = |group: usize| accent(theme, 4 + group % 6);
    draw_sunburst(&mut out, &leaves, area.arr(), &colour, labels);
    Some(out)
}

/// Each point's category path, top level first (`cx:strDim type="cat"` lists the leaf level
/// first), with its `cx:numDim type="size"` value; blank names where a level has none.
fn category_paths(data: &El) -> Vec<(Vec<String>, f64)> {
    let Some(lvl) = data.children("cx:numDim").find(|d| d.attr("type") == Some("size")).and_then(|d| d.child("cx:lvl")) else {
        return Vec::new();
    };
    let values: Vec<(usize, f64)> =
        lvl.children("cx:pt").filter_map(|p| Some((p.attr("idx").and_then(index)?, p.text().trim().parse::<f64>().ok()?))).take(MAX_POINTS).collect();
    let mut levels: Vec<HashMap<usize, String>> = data
        .children("cx:strDim")
        .find(|d| d.attr("type") == Some("cat"))
        .into_iter()
        .flat_map(|d| d.children("cx:lvl"))
        .take(MAX_LEVELS)
        .map(|l| l.children("cx:pt").take(MAX_POINTS).filter_map(|p| Some((p.attr("idx").and_then(index)?, label(p.text().trim())))).collect())
        .collect();
    levels.reverse();
    values.into_iter().map(|(k, v)| (levels.iter().map(|l| l.get(&k).cloned().unwrap_or_default()).collect(), v)).collect()
}

/// What one box & whisker series shows (`cx:layoutPr`).
struct BoxSeries {
    name: String,
    colour: Rgb,
    mean_marker: bool,
    mean_line: bool,
    outliers: bool,
    nonoutliers: bool,
    /// (category, stats) for each category the series has values in.
    boxes: Vec<(usize, BoxStats)>,
}

/// A `cx:visibility` flag, `default` when absent.
fn flag(vis: Option<&El>, name: &str, default: bool) -> bool {
    vis.and_then(|v| v.attr(name)).map_or(default, truthy)
}

/// A box & whisker chart: per category, one box per series from q1 to q3 with whiskers, mean
/// marker and outliers.
fn box_whisker(space: &El, chart: &El, plot: &El, region: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    let mut cats: Vec<String> = Vec::new();
    let mut cat_of: HashMap<String, usize> = HashMap::new();
    let mut list: Vec<BoxSeries> = Vec::new();
    // Every value of each (category, series) group, for the stats and the nonoutlier points.
    let mut groups: Vec<BTreeMap<usize, Vec<f64>>> = Vec::new();
    for (i, series) in region.children("cx:series").take(MAX_SERIES).enumerate() {
        let layout = series.child("cx:layoutPr");
        let vis = layout.and_then(|l| l.child("cx:visibility"));
        let exclusive =
            layout.and_then(|l| l.child("cx:statistics")).and_then(|s| s.attr("quartileMethod")).map(str::trim).is_none_or(|m| m != "inclusive");
        let mut by_cat: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        if let Some(data) = series_data(space, series) {
            let labels = cat_labels(data);
            for (k, p) in read_points(data, &[]).iter().enumerate() {
                let Some(p) = p else { continue };
                let name = labels.as_ref().map_or(String::new(), |l| l.get(k).cloned().flatten().unwrap_or_default());
                let c = *cat_of.entry(name).or_insert_with_key(|name| {
                    cats.push(name.clone());
                    cats.len() - 1
                });
                by_cat.entry(c).or_default().push(p.v);
            }
        }
        // Sorted by category, as `by_cat` is, for `find_box`.
        let boxes = by_cat.iter().filter_map(|(c, vs)| Some((*c, box_stats(vs, exclusive)?))).collect();
        let name = label(series.child("cx:tx").and_then(|t| t.find("cx:v")).map(|v| v.deep_text()).unwrap_or_default().trim());
        list.push(BoxSeries {
            name: if name.is_empty() { format!("Series {}", i + 1) } else { name },
            colour: series.child("cx:spPr").and_then(|p| fill_colour(p, theme)).unwrap_or(accent(theme, 4 + i % 6)),
            mean_marker: flag(vis, "meanMarker", true),
            mean_line: flag(vis, "meanLine", false),
            outliers: flag(vis, "outliers", true),
            nonoutliers: flag(vis, "nonoutliers", false),
            boxes,
        });
        groups.push(by_cat);
    }
    let drawn = list.iter().flat_map(|s| s.boxes.iter().map(move |(_, b)| (s, b)));
    let values = drawn.flat_map(|(s, b)| {
        let mut v = vec![b.lo, b.hi, b.q1, b.q3];
        if s.mean_marker || s.mean_line {
            v.push(b.mean);
        }
        if s.outliers {
            v.extend(b.outliers.iter().copied());
        }
        v
    });
    let (lo, hi) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    if cats.is_empty() {
        cats.push(String::new());
    }
    let scale = value_scale(plot, lo, hi);
    let size = text_size(space);

    let (mut out, area) = frame(space, chart, theme, w, h);
    let entries: Vec<(String, Rgb)> = list.iter().map(|s| (s.name.clone(), s.colour)).collect();
    let area = chart_legend(&mut out, chart, &entries, area, size);
    let plot_r = value_axis(&mut out, plot, area, &scale, size);
    let y_of = |v: f64| -> f32 { y_on(plot_r, &scale, v) };
    let band = plot_r.w / cats.len() as f32;
    let gap = gap_width(plot, BOX_GAP_WIDTH);
    let box_w = band / (list.len().max(1) as f32 + gap);
    for (k, cat) in cats.iter().enumerate() {
        let cx = plot_r.x + band * (k as f32 + 0.5);
        text_at(&mut out, cx, plot_r.bottom() + GAP, &clip_text(cat, band, size), size);
    }
    let left_of = |c: usize, i: usize| -> f32 { plot_r.x + band * c as f32 + gap * box_w / 2.0 + box_w * i as f32 };
    let r = MARKER / 2.0;
    let circle = |out: &mut Vec<GraphicItem>, x: f32, v: f64, colour: Rgb| {
        let rect = [x - r, y_of(v) - r, MARKER, MARKER];
        out.push(GraphicItem::Shape { rect, kind: ShapeKind::Ellipse, fill: None, stroke: Some(colour), stroke_width: 0.75 });
    };

    for c in 0..cats.len() {
        for (i, s) in list.iter().enumerate() {
            let Some(b) = find_box(&s.boxes, c) else { continue };
            let x0 = left_of(c, i);
            let (x1, cx) = (x0 + box_w, x0 + box_w / 2.0);
            let (top, bottom) = (y_of(b.q3), y_of(b.q1));
            fill(&mut out, rect_segs(x0, top, x1, bottom), s.colour);
            // Whiskers with caps half a box wide.
            let cap = box_w / 4.0;
            for (end, edge) in [(b.hi, top), (b.lo, bottom)] {
                let y = y_of(end);
                stroke(&mut out, seg(cx, edge, cx, y), s.colour, 0.75);
                stroke(&mut out, seg(cx - cap, y, cx + cap, y), s.colour, 0.75);
            }
            let ym = y_of(b.median);
            stroke(&mut out, seg(x0, ym, x1, ym), Rgb(0xFF, 0xFF, 0xFF), 1.0);
            if s.mean_marker {
                // White on the box, grey beside it.
                let mark = if b.mean >= b.q1 && b.mean <= b.q3 { Rgb(0xFF, 0xFF, 0xFF) } else { TEXT };
                let y = y_of(b.mean);
                stroke(&mut out, seg(cx - r, y - r, cx + r, y + r), mark, 1.0);
                stroke(&mut out, seg(cx - r, y + r, cx + r, y - r), mark, 1.0);
            }
            if s.outliers {
                for v in &b.outliers {
                    circle(&mut out, cx, *v, s.colour);
                }
            }
            if s.nonoutliers {
                let vs = groups.get(i).and_then(|g| g.get(&c)).map(Vec::as_slice).unwrap_or_default();
                for v in vs.iter().filter(|v| **v >= b.lo && **v <= b.hi) {
                    circle(&mut out, cx, *v, s.colour);
                }
            }
        }
    }
    for (i, s) in list.iter().enumerate().filter(|(_, s)| s.mean_line) {
        let pts = (0..cats.len()).map(|c| {
            let b = find_box(&s.boxes, c)?;
            Some((left_of(c, i) + box_w / 2.0, y_of(b.mean)))
        });
        stroke(&mut out, runs(pts), s.colour, 1.0);
    }
    Some(out)
}

/// The box of category `c` in `boxes` (sorted by category).
fn find_box(boxes: &[(usize, BoxStats)], c: usize) -> Option<&BoxStats> {
    boxes.binary_search_by_key(&c, |b| b.0).ok().and_then(|k| boxes.get(k)).map(|b| &b.1)
}

/// The category label of each point (`cx:strDim type="cat"`, first level), `None` with no
/// categories.
fn cat_labels(data: &El) -> Option<Vec<Option<String>>> {
    let lvl = data.children("cx:strDim").find(|d| d.attr("type") == Some("cat")).and_then(|d| d.child("cx:lvl"))?;
    let mut out: Vec<Option<String>> = Vec::new();
    for p in lvl.children("cx:pt") {
        let Some(k) = p.attr("idx").and_then(index) else { continue };
        if out.len() <= k {
            out.resize(k + 1, None);
        }
        if let Some(slot) = out.get_mut(k) {
            *slot = Some(label(p.text().trim()));
        }
    }
    Some(out)
}

/// The series' values (`cx:numDim type="val"`, first level), `None` where a point is missing.
fn read_points(data: &El, totals: &[usize]) -> Vec<Option<Point>> {
    let Some(lvl) = data.children("cx:numDim").find(|d| d.attr("type") == Some("val")).and_then(|d| d.child("cx:lvl")) else {
        return Vec::new();
    };
    let mut pts: Vec<(usize, String, f64)> = lvl
        .children("cx:pt")
        .filter_map(|p| {
            let k = p.attr("idx").and_then(index)?;
            let raw = p.text().trim().to_string();
            let v = raw.parse::<f64>().ok().filter(|v| v.is_finite())?;
            Some((k, label(&raw), v))
        })
        .take(MAX_POINTS)
        .collect();
    let count = lvl.attr("ptCount").and_then(index).unwrap_or(0);
    let n = pts.iter().map(|p| p.0 + 1).max().unwrap_or(0).max(count).min(MAX_POINTS);
    pts.retain(|p| p.0 < n);
    let mut out: Vec<Option<Point>> = (0..n).map(|_| None).collect();
    for (k, raw, v) in pts {
        if let Some(slot) = out.get_mut(k) {
            *slot = Some(Point { raw, v, total: totals.contains(&k) });
        }
    }
    out
}

/// The category labels (`cx:strDim type="cat"`); 1..n when there are none, as Word shows them.
fn categories(data: &El, n: usize) -> Vec<String> {
    let mut out: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
    let Some(lvl) = data.children("cx:strDim").find(|d| d.attr("type") == Some("cat")).and_then(|d| d.child("cx:lvl")) else {
        return out;
    };
    for p in lvl.children("cx:pt") {
        if let Some(slot) = p.attr("idx").and_then(index).and_then(|k| out.get_mut(k)) {
            *slot = label(p.text().trim());
        }
    }
    out
}

fn index(v: &str) -> Option<usize> {
    v.trim().parse::<u32>().ok().and_then(|v| usize::try_from(v).ok()).filter(|v| *v < MAX_POINTS)
}

/// Theme colour `i`, black when the theme is short.
fn accent(theme: &[Rgb], i: usize) -> Rgb {
    theme.get(i).copied().unwrap_or(Rgb(0, 0, 0))
}

/// An axis value without float noise (`0.30000000000000004` → `0.3`).
fn number(v: f64) -> String {
    let r = (v * 1e9).round() / 1e9;
    if r == 0.0 { "0".to_string() } else { format!("{r}") }
}

/// The chart's text size (its `cx:txPr` default run size), points.
fn text_size(space: &El) -> f32 {
    space
        .child("cx:txPr")
        .and_then(|t| t.find("a:defRPr"))
        .and_then(|d| d.attr("sz"))
        .and_then(int)
        .map_or(FONT, |v| (v as f32 / 100.0).clamp(6.0, 24.0))
}

#[cfg(test)]
mod polish_tests {
    use super::*;

    fn plot(bounds: &str) -> El {
        crate::xml::parse(format!(r#"<cx:plotArea xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:axis><cx:valScaling {bounds}/></cx:axis></cx:plotArea>"#).as_bytes()).unwrap()
    }

    #[test]
    fn explicit_value_bounds_are_not_rounded() {
        let p = plot(r#"min="3" max="173""#);
        assert_eq!(value_scale(&p, 0.0, 170.0), Scale { lo: 3.0, hi: 173.0, step: 20.0 });
        let p = plot(r#"max="173""#);
        assert_eq!(value_scale(&p, 0.0, 170.0), Scale { lo: 0.0, hi: 173.0, step: 20.0 });
    }

    #[test]
    fn automatic_value_scale_includes_zero_and_pads_the_full_span() {
        let p = plot("");
        assert_eq!(value_scale(&p, 150.0, 170.0), Scale { lo: 0.0, hi: 180.0, step: 20.0 });
        assert_eq!(value_scale(&p, -170.0, -150.0), Scale { lo: -180.0, hi: 0.0, step: 20.0 });
        let zero = value_scale(&p, 0.0, 0.0);
        assert!(zero.lo == 0.0 && zero.hi >= 1.0 && zero.step > 0.0);
    }

    #[test]
    fn invalid_value_bounds_and_extreme_spans_have_finite_scales() {
        for p in [plot(r#"min="NaN" max="inf""#), plot(r#"min="10" max="-10""#), plot("")] {
            for (lo, hi) in [(0.0, 170.0), (-f64::MAX, f64::MAX), (0.0, f64::MIN_POSITIVE)] {
                let s = value_scale(&p, lo, hi);
                assert!(s.lo.is_finite() && s.hi.is_finite() && s.step.is_finite());
                assert!(s.hi > s.lo && s.step > 0.0);
            }
        }
    }
}

#[cfg(test)]
mod treemap_hostile_tests {
    use super::*;

    fn leaf(path: &[&str], v: f64) -> (Vec<String>, f64) {
        (path.iter().map(|s| s.to_string()).collect(), v)
    }

    #[test]
    fn hostile_sizes_and_rects_stay_finite_and_inside() {
        let ls = vec![
            leaf(&["A", "x"], f64::NAN),
            leaf(&["A", "y"], f64::INFINITY),
            leaf(&[], 5.0),
            leaf(&["B"], f64::MAX),
            leaf(&["B", "z", "deep"], f64::MAX),
            leaf(&["C"], f64::MIN_POSITIVE),
        ];
        for (w, h) in [(100.0, 50.0), (0.0, 50.0), (-3.0, 4.0), (f32::NAN, 1.0), (1e-30, 1e30)] {
            let got = treemap_rects(&ls, 1.0, 2.0, w, h);
            assert_eq!(got.len(), ls.len());
            for r in &got {
                assert!(r.iter().all(|v| v.is_finite()) && r[2] >= r[0] && r[3] >= r[1], "{w}x{h}: {r:?}");
            }
        }
        let got = treemap_rects(&ls, 0.0, 0.0, 100.0, 50.0);
        let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
        assert!(area(got[0]) == 0.0 && area(got[1]) == 0.0);
        assert!((area(got[3]) - 2500.0).abs() < 1.0 && (area(got[4]) - 2500.0).abs() < 1.0, "{got:?}");
    }

    #[test]
    fn mixed_depth_paths_nest_inside_their_group() {
        let ls = vec![leaf(&["A", "a1"], 2.0), leaf(&["A"], 2.0), leaf(&["B", "b1", "b2"], 4.0)];
        let got = treemap_rects(&ls, 0.0, 0.0, 80.0, 10.0);
        let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
        for r in &got {
            assert!((area(*r) - 200.0).abs() < 0.5 || (area(*r) - 400.0).abs() < 0.5, "{got:?}");
        }
        // The two leaves of group A share one half of the plot.
        assert!(got[0][2].max(got[1][2]) <= 40.01 || got[0][0].min(got[1][0]) >= 39.99, "{got:?}");
    }

    #[test]
    fn treemap_without_sizes_keeps_the_fallback() {
        let x = br#"<cx:chartSpace xmlns:cx="x"><cx:chartData><cx:data id="0"><cx:strDim type="cat"><cx:lvl><cx:pt idx="0">A</cx:pt></cx:lvl></cx:strDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="treemap"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#;
        let space = crate::xml::parse(x).unwrap();
        assert!(chartex_items(&space, &[], 300.0, 200.0).is_none());
    }
}
