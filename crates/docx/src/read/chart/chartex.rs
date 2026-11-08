//! Office 2016 charts (`cx:chartSpace`, chartex namespace) drawn from their data: waterfall and
//! box & whisker charts. Other layouts return `None`, so the caller keeps the fallback picture.

use std::collections::{BTreeMap, HashMap};

use wordcraft_doc::graphic::{GraphicItem, TextAlign};
use wordcraft_doc::props::Rgb;

use super::canvas::{Rect, clip_text, fill, legend, rect_segs, runs, seg, stroke, text, text_at, text_w, widest};
use super::model::Pos;
use super::parse::truthy;
use super::plot::{Scale, label};
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
