//! Pie-of-pie and bar-of-pie charts (`c:ofPieChart`, ECMA-376 Part 1 §21.2.2.126).
//!
//! Some points of the series are split off ([`of_pie_second`]) into a second plot to the right of
//! the first pie: a pie (`c:ofPieType` = `pie`) or a stacked bar (`bar`), `c:secondPieSize`
//! percent of the first pie's diameter tall, `c:gapWidth` percent of its radius away. The first
//! pie holds the other points and one "Other" slice, their sum, centred at 3 o'clock facing the
//! second plot; slice angles are exactly proportional to the values. The second pie's first point
//! is centred at 9 o'clock, facing back; the bar stacks its points from the top. With
//! `c:serLines`, two lines join the edges of the Other slice to the top and bottom of the second
//! plot.

use super::canvas::{Rect, fill, rect_segs, stroke};
use super::model::DataLabel;
use super::polar::{arc, draw_data_labels, label_of, line_to, polar, pt};
use super::stock::outline;
use super::{Chart, El, FRAC_PI_2, GraphicItem, MAX_POINTS, PathSeg, Rgb, TAU, int};

/// Points split off when the file gives no `c:splitPos` (with `c:splitType` auto or pos).
const DEFAULT_SPLIT: usize = 3;
/// Width of the bar of a bar-of-pie chart, fraction of its height.
const BAR_W: f64 = 1.0 / 3.0;

/// How a `c:ofPieChart` splits its points and draws the second plot.
pub(super) struct OfPie {
    bar: bool,
    /// Gap between the plots, fraction of the second plot's half-height.
    gap: f64,
    split_type: String,
    split_pos: Option<f64>,
    cust: Vec<usize>,
    /// Size of the second plot, fraction of the first pie's.
    second: f64,
    /// Colour and width of the series lines, when drawn.
    lines: Option<(Rgb, f32)>,
}

impl OfPie {
    pub(super) fn parse(ty: &El, theme: &[Rgb]) -> OfPie {
        let val = |n: &str| ty.child(n).and_then(|e| e.attr("val"));
        let num = |n: &str| val(n).and_then(int);
        let cust = ty
            .child("c:custSplit")
            .map(|c| {
                c.children("c:secondPiePt")
                    .take(MAX_POINTS)
                    .filter_map(|p| p.attr("val").and_then(int))
                    .filter_map(|v| usize::try_from(v).ok())
                    .collect()
            })
            .unwrap_or_default();
        OfPie {
            bar: val("c:ofPieType") == Some("bar"),
            gap: num("c:gapWidth").unwrap_or(150).clamp(0, 500) as f64 / 100.0,
            split_type: val("c:splitType").unwrap_or("auto").to_string(),
            split_pos: val("c:splitPos").and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite()),
            cust,
            second: num("c:secondPieSize").unwrap_or(75).clamp(5, 200) as f64 / 100.0,
            lines: ty.child("c:serLines").and_then(|l| outline(l.child("c:spPr"), theme)),
        }
    }
}

/// Which points of a pie-of-pie or bar-of-pie chart go to the second plot (`c:splitType`,
/// `c:splitPos`, `c:custSplit`): their indices, ascending.
///
/// `pos`: the last `splitPos` points; `val`: the points under `splitPos`; `percent`: the points
/// under `splitPos` percent of the total; `cust`: the given points (those out of range dropped).
/// `auto`, an unknown type, or `val`/`percent` without a `splitPos`: the last `splitPos` points,
/// else the last 3. The first plot always keeps at least one point (the lowest index stays).
pub(super) fn of_pie_second(vals: &[f64], split_type: &str, split_pos: Option<f64>, cust: &[usize]) -> Vec<usize> {
    let n = vals.len();
    if n < 2 {
        return Vec::new();
    }
    let last = || {
        let k = split_pos.map_or(DEFAULT_SPLIT, |p| p.round().clamp(0.0, n as f64) as usize);
        (n - k.min(n)..n).collect::<Vec<usize>>()
    };
    let below = |limit: f64, scale: f64| (0..n).filter(|i| vals.get(*i).is_some_and(|v| v * scale < limit)).collect::<Vec<usize>>();
    let mut out = match (split_type, split_pos) {
        ("val", Some(p)) => below(p, 1.0),
        ("percent", Some(p)) => {
            let total: f64 = vals.iter().filter(|v| v.is_finite()).sum();
            if total > 0.0 { below(p, 100.0 / total) } else { Vec::new() }
        }
        ("cust", _) => {
            let mut v: Vec<usize> = cust.iter().copied().filter(|i| *i < n).collect();
            v.sort_unstable();
            v.dedup();
            v
        }
        _ => last(),
    };
    if out.len() >= n && !out.is_empty() {
        out.remove(0);
    }
    out
}

/// A pie-of-pie or bar-of-pie chart in `area`.
pub(super) fn draw_of_pie(out: &mut Vec<GraphicItem>, c: &Chart, area: Rect) {
    let Some(o) = &c.of_pie else { return };
    let Some(grp) = c.groups.first() else { return };
    let Some(s) = grp.series.first() else { return };
    let vals: Vec<f64> = s.values.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    if !(total > 0.0 && total.is_finite()) {
        return;
    }
    let second = of_pie_second(&vals, &o.split_type, o.split_pos, &o.cust);
    let other: f64 = second.iter().filter_map(|i| vals.get(*i)).sum();
    // Sizes: the first pie's diameter d1, the second plot d2 = second x d1 tall, gap x d2 / 2
    // between them; the pair centred in the area.
    let w2 = if o.bar { BAR_W } else { 1.0 };
    let span = 1.0 + o.second * (o.gap / 2.0 + w2);
    let d1 = ((area.w as f64 - 4.0) / span).min(area.h as f64 - 4.0);
    if d1.is_nan() || d1 < 2.0 {
        return;
    }
    let (r1, d2) = (d1 / 2.0, d1 * o.second);
    let x0 = area.x as f64 + (area.w as f64 - d1 * span) / 2.0;
    let (cx1, cy) = (x0 + r1, (area.y + area.h / 2.0) as f64);
    let x2 = x0 + d1 + o.gap * d2 / 2.0;
    let mut labels = Vec::new();

    // The first pie: the kept points, then Other centred at 3 o'clock.
    let o_span = other / total * TAU;
    let mut a = FRAC_PI_2 + o_span / 2.0;
    for (i, v) in vals.iter().enumerate().filter(|(i, _)| !second.contains(i)) {
        let b = a + v / total * TAU;
        if *v > 0.0 {
            fill(out, slice((cx1, cy, r1), a, b), c.point_colour(grp, 0, i));
            push_label(&mut labels, s, i, polar(cx1, cy, r1 * 0.65, (a + b) / 2.0));
        }
        a = b;
    }
    if other <= 0.0 {
        draw_data_labels(out, &labels, c.text_size, false);
        return;
    }
    let (top_edge, bottom_edge) = (FRAC_PI_2 - o_span / 2.0, FRAC_PI_2 + o_span / 2.0);
    fill(out, slice((cx1, cy, r1), top_edge, bottom_edge), c.accent(vals.len()));

    // The second plot.
    let (top, bottom) = if o.bar {
        let mut y = cy - d2 / 2.0;
        for i in &second {
            let v = vals.get(*i).copied().unwrap_or(0.0);
            let h = v / other * d2;
            if v > 0.0 {
                fill(out, rect_segs(x2 as f32, y as f32, (x2 + d2 * BAR_W) as f32, (y + h) as f32), c.point_colour(grp, 0, *i));
                push_label(&mut labels, s, *i, (x2 + d2 * BAR_W / 2.0, y + h / 2.0));
            }
            y += h;
        }
        ((x2, cy - d2 / 2.0), (x2, cy + d2 / 2.0))
    } else {
        let r2 = d2 / 2.0;
        let cx2 = x2 + r2;
        let first = second.first().and_then(|i| vals.get(*i)).copied().unwrap_or(0.0);
        let mut a = 3.0 * FRAC_PI_2 - first / other * TAU / 2.0;
        for i in &second {
            let v = vals.get(*i).copied().unwrap_or(0.0);
            let b = a + v / other * TAU;
            if v > 0.0 {
                fill(out, slice((cx2, cy, r2), a, b), c.point_colour(grp, 0, *i));
                push_label(&mut labels, s, *i, polar(cx2, cy, r2 * 0.65, (a + b) / 2.0));
            }
            a = b;
        }
        ((cx2, cy - r2), (cx2, cy + r2))
    };
    if let Some((color, w)) = o.lines {
        for (edge, end) in [(top_edge, top), (bottom_edge, bottom)] {
            stroke(out, vec![pt(polar(cx1, cy, r1, edge)), line_to(end)], color, w);
        }
    }
    draw_data_labels(out, &labels, c.text_size, false);
}

/// A pie slice from angle `a` to `b` of the circle (`cx`, `cy`, `r`).
fn slice(circle: (f64, f64, f64), a: f64, b: f64) -> Vec<PathSeg> {
    let (cx, cy, r) = circle;
    let mut segs = vec![pt((cx, cy)), line_to(polar(cx, cy, r, a))];
    arc(&mut segs, circle, a, b);
    segs.push(PathSeg::Close);
    segs
}

fn push_label(labels: &mut Vec<DataLabel>, s: &super::model::Series, i: usize, (x, y): (f64, f64)) {
    if let Some(text) = label_of(s, i) {
        labels.push(DataLabel { x: x as f32, y: y as f32, text, inside: true });
    }
}

#[cfg(test)]
mod tests {
    use super::super::chart_items;
    use super::*;

    /// The first pie's Other slice spans the split points' share of the full turn: with D and E
    /// (15 of 100) split off, it is 54° wide, centred at 3 o'clock.
    #[test]
    fn other_slice_is_the_sum_of_the_split_points() {
        let pts: String = [50.0, 30.0, 5.0, 8.0, 7.0].iter().enumerate().map(|(k, v)| format!(r#"<c:pt idx="{k}"><c:v>{v}</c:v></c:pt>"#)).collect();
        let x = format!(
            r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:ofPieChart><c:ofPieType val="pie"/><c:varyColors val="1"/><c:ser><c:val><c:numLit><c:ptCount val="5"/>{pts}</c:numLit></c:val></c:ser><c:splitType val="pos"/><c:splitPos val="2"/></c:ofPieChart></c:plotArea></c:chart></c:chartSpace>"#
        );
        let space = crate::xml::parse(x.as_bytes()).unwrap();
        let items = chart_items(&space, &[], 400.0, 200.0);
        let slices: Vec<&Vec<PathSeg>> =
            items.iter().filter_map(|it| if let GraphicItem::Path { segs, fill: Some(_), .. } = it { Some(segs) } else { None }).collect();
        // A, B, C, then Other (its last segment before Close is the arc's end).
        let Some(other) = slices.get(3) else { panic!("{slices:?}") };
        let (Some(PathSeg::Move(cx, cy)), Some(PathSeg::Line(x0, y0)), Some(PathSeg::Cubic(.., x1, y1))) =
            (other.first(), other.get(1), other.get(other.len() - 2))
        else {
            panic!("{other:?}")
        };
        let angle = |x: f32, y: f32| (x - cx).atan2(cy - y).to_degrees();
        let (a, b) = (angle(*x0, *y0), angle(*x1, *y1));
        assert!((b - a - 54.0).abs() < 0.01, "{a}..{b}");
        assert!(((a + b) / 2.0 - 90.0).abs() < 0.01, "{a}..{b}");
    }
}
