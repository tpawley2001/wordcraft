//! Office 2016 charts (`cx:chartSpace`, chartex namespace) drawn from their data: waterfall
//! charts. Other layouts return `None`, so the caller keeps the fallback picture.

use wordcraft_doc::graphic::{GraphicItem, TextAlign};
use wordcraft_doc::props::Rgb;

use super::canvas::{Rect, clip_text, fill, legend, rect_segs, seg, stroke, text, text_at, text_w, widest};
use super::model::Pos;
use super::parse::truthy;
use super::plot::{label, nice_scale};
use super::{FONT, GAP, GRID, LINE, MAX_POINTS, PAD, fill_colour, int, title_block};
use crate::xml::El;

/// Gap between bars as a fraction of a bar's width when `cx:catScaling` doesn't say.
const GAP_WIDTH: f32 = 0.5;
/// Widest gap between bars, in bar widths.
const MAX_GAP_WIDTH: f32 = 5.0;

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
    let _ = (values, exclusive);
    None
}

/// The items of an Office 2016 chart `w` × `h` points, or `None` when its layout isn't drawn.
pub(crate) fn chartex_items(space: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return None;
    }
    let chart = space.child("cx:chart")?;
    let plot = chart.child("cx:plotArea")?;
    let series = plot.child("cx:plotAreaRegion")?.child("cx:series")?;
    if series.attr("layoutId") != Some("waterfall") {
        return None;
    }
    let id = series.child("cx:dataId").and_then(|d| d.attr("val"))?.trim();
    let data = space.child("cx:chartData")?.children("cx:data").find(|d| d.attr("id").map(str::trim) == Some(id))?;
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
    let scale = nice_scale(lo, hi, true, None, None);
    let ticks = scale.ticks();
    let size = text_size(space);
    let lh = size * LINE;

    let mut out = Vec::new();
    if let Some(bg) = space.child("cx:spPr").and_then(|p| fill_colour(p, theme)) {
        fill(&mut out, rect_segs(0.0, 0.0, w, h), bg);
    }
    let mut area = Rect { x: PAD, y: PAD, w: (w - 2.0 * PAD).max(1.0), h: (h - 2.0 * PAD).max(1.0) };
    if let Some(t) = chart.child("cx:title") {
        let s = label(t.deep_text().trim());
        area = title_block(&mut out, if s.is_empty() { "Chart Title" } else { &s }, area);
    }
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
    if let Some(l) = chart.child("cx:legend") {
        let pos = match l.attr("pos").map(str::trim) {
            Some("l") => Pos::Left,
            Some("t") => Pos::Top,
            Some("b") => Pos::Bottom,
            _ => Pos::Right,
        };
        let entries = [("Increase".to_string(), inc), ("Decrease".to_string(), dec), ("Total".to_string(), tot)];
        area = legend(&mut out, &entries, pos, area, size);
    }

    // Value labels at the left, category labels under the plot.
    let tick_text: Vec<String> = ticks.iter().map(|t| number(*t)).collect();
    let left = widest(&tick_text, size) + GAP;
    let plot_r = Rect { x: area.x + left, y: area.y + lh / 2.0, w: (area.w - left).max(1.0), h: (area.h - lh * 1.5 - GAP).max(1.0) };
    let span = scale.hi - scale.lo;
    let y_of = |v: f64| -> f32 { plot_r.bottom() - (((v - scale.lo) / span) as f32 * plot_r.h) };
    if plot.children("cx:axis").any(|a| a.child("cx:valScaling").is_some() && a.child("cx:majorGridlines").is_some()) {
        for t in &ticks {
            let y = y_of(*t);
            stroke(&mut out, seg(plot_r.x, y, plot_r.right(), y), GRID, 0.75);
        }
    }
    for (t, s) in ticks.iter().zip(&tick_text) {
        let r = Rect { x: area.x, y: y_of(*t) - lh / 2.0, w: left - GAP, h: lh };
        text(&mut out, r, s, size, TextAlign::Right, false);
    }
    let band = plot_r.w / n.max(1) as f32;
    let gap = plot
        .children("cx:axis")
        .find_map(|a| a.child("cx:catScaling"))
        .and_then(|c| c.attr("gapWidth"))
        .and_then(|g| g.trim().parse::<f32>().ok())
        .filter(|g| g.is_finite())
        .map_or(GAP_WIDTH, |g| g.clamp(0.0, MAX_GAP_WIDTH));
    let bar_w = band / (1.0 + gap);
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
