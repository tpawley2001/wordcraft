//! Trendlines (`c:trendline`, ECMA-376 Part 1 §21.2.2.209): a curve fitted to a series' points by
//! least squares (linear, exponential, logarithmic, power, polynomial) or its moving average,
//! drawn over the series in its own line style, from the first point less `c:backward` to the last
//! plus `c:forward`, clipped to the plot area.

use super::canvas::{Rect, stroke};
use super::model::{Group, Kind, Series};
use super::plot::Frame;
use super::{Chart, El, GraphicItem, PathSeg, Rgb, fill_colour, int, measure};

/// Most trendlines read from one series.
const MAX_TRENDLINES: usize = 4;
/// Highest polynomial order (§21.2.2.155).
const MAX_ORDER: usize = 6;
/// Longest moving-average period.
const MAX_PERIOD: usize = 255;
/// Vertices of a curved trendline.
const CURVE_STEPS: usize = 100;
/// Furthest a trendline is extended forward or backward, in x units.
const MAX_REACH: f64 = 1e9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrendKind {
    Linear,
    Exp,
    Log,
    Power,
    Poly,
    MovingAvg,
}

/// One `c:trendline` of a series.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Trendline {
    pub(super) kind: TrendKind,
    /// Polynomial order, 2..=6.
    pub(super) order: usize,
    /// Moving-average period, 2..=255.
    pub(super) period: usize,
    pub(super) forward: f64,
    pub(super) backward: f64,
    /// Fixed y intercept of a linear trendline.
    pub(super) intercept: Option<f64>,
    /// Line colour and width from `c:spPr/a:ln`, when given (else the series').
    pub(super) stroke: (Option<Rgb>, Option<f32>),
    /// `a:ln/a:noFill`: the trendline isn't drawn.
    pub(super) hidden: bool,
}

/// The trendlines of a `c:ser` (at most [`MAX_TRENDLINES`]); unknown types are skipped.
pub(super) fn parse(ser: &El, theme: &[Rgb]) -> Vec<Trendline> {
    ser.children("c:trendline").filter_map(|t| parse_one(t, theme)).take(MAX_TRENDLINES).collect()
}

fn parse_one(t: &El, theme: &[Rgb]) -> Option<Trendline> {
    let val = |name: &str| t.child(name).and_then(|e| e.attr("val")).map(str::trim);
    let kind = match val("c:trendlineType").unwrap_or("linear") {
        "linear" => TrendKind::Linear,
        "exp" => TrendKind::Exp,
        "log" => TrendKind::Log,
        "power" => TrendKind::Power,
        "poly" => TrendKind::Poly,
        "movingAvg" => TrendKind::MovingAvg,
        _ => return None,
    };
    let whole = |name: &str, lo: usize, hi: usize, default: usize| {
        val(name).and_then(int).map_or(default, |v| usize::try_from(v.max(0)).unwrap_or(hi).clamp(lo, hi))
    };
    let real = |name: &str| val(name).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
    let reach = |name: &str| real(name).map_or(0.0, |v| v.clamp(0.0, MAX_REACH));
    let ln = t.child("c:spPr").and_then(|p| p.child("a:ln"));
    let color = ln.and_then(|l| fill_colour(l, theme));
    let width = ln.and_then(|l| l.attr("w")).and_then(|v| measure(v, 12_700.0)).map(|w| w.clamp(0.0, 20.0));
    Some(Trendline {
        kind,
        order: whole("c:order", 2, MAX_ORDER, 2),
        period: whole("c:period", 2, MAX_PERIOD, 2),
        forward: reach("c:forward"),
        backward: reach("c:backward"),
        intercept: real("c:intercept"),
        stroke: (color, width),
        hidden: ln.is_some_and(|l| l.child("a:noFill").is_some()),
    })
}

/// A fitted trendline.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Fit {
    /// y = m x + b
    Linear {
        m: f64,
        b: f64,
    },
    /// y = a e^(b x)
    Exp {
        a: f64,
        b: f64,
    },
    /// y = a ln x + b
    Log {
        a: f64,
        b: f64,
    },
    /// y = a x^b
    Power {
        a: f64,
        b: f64,
    },
    Poly(Poly),
    /// The vertices of a moving average.
    Moving(Vec<(f64, f64)>),
}

/// A polynomial in t = (x − `mid`) / `scale` (fitted that way so large x values stay well
/// conditioned), coefficients from t⁰ up.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Poly {
    pub(super) coef: Vec<f64>,
    pub(super) mid: f64,
    pub(super) scale: f64,
}

impl Poly {
    pub(super) fn eval(&self, x: f64) -> f64 {
        let t = (x - self.mid) / self.scale;
        self.coef.iter().rev().fold(0.0, |acc, c| acc * t + c)
    }
}

impl Fit {
    /// The curve's y at `x` (not a number where it has none).
    pub(super) fn eval(&self, x: f64) -> f64 {
        match self {
            Fit::Linear { m, b } => m * x + b,
            Fit::Exp { a, b } => a * (b * x).exp(),
            Fit::Log { a, b } if x > 0.0 => a * x.ln() + b,
            Fit::Power { a, b } if x > 0.0 => a * x.powf(*b),
            Fit::Poly(p) => p.eval(x),
            Fit::Log { .. } | Fit::Power { .. } | Fit::Moving(_) => f64::NAN,
        }
    }
}

/// Least-squares line through `pts` as (m, b); with `intercept` the line passes through
/// (0, intercept). `None` for fewer than 2 points or no spread in x.
pub(super) fn linear_fit(pts: &[(f64, f64)], intercept: Option<f64>) -> Option<(f64, f64)> {
    if pts.len() < 2 {
        return None;
    }
    let fit = if let Some(v) = intercept {
        let sxx: f64 = pts.iter().map(|(x, _)| x * x).sum();
        let sxy: f64 = pts.iter().map(|(x, y)| x * (y - v)).sum();
        (sxx > 0.0).then(|| (sxy / sxx, v))?
    } else {
        let n = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
        let sxx: f64 = pts.iter().map(|(x, _)| (x - mx) * (x - mx)).sum();
        let sxy: f64 = pts.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
        if sxx <= 0.0 {
            return None;
        }
        let m = sxy / sxx;
        (m, my - m * mx)
    };
    (fit.0.is_finite() && fit.1.is_finite()).then_some(fit)
}

/// y = a e^(b x) as (a, b): a line through (x, ln y); points with y ≤ 0 are skipped.
pub(super) fn exp_fit(pts: &[(f64, f64)]) -> Option<(f64, f64)> {
    let t: Vec<(f64, f64)> = pts.iter().filter(|p| p.1 > 0.0).map(|(x, y)| (*x, y.ln())).collect();
    linear_fit(&t, None).map(|(m, b)| (b.exp(), m))
}

/// y = a ln x + b as (a, b); points with x ≤ 0 are skipped.
pub(super) fn log_fit(pts: &[(f64, f64)]) -> Option<(f64, f64)> {
    let t: Vec<(f64, f64)> = pts.iter().filter(|p| p.0 > 0.0).map(|(x, y)| (x.ln(), *y)).collect();
    linear_fit(&t, None)
}

/// y = a x^b as (a, b): a line through (ln x, ln y); points with x ≤ 0 or y ≤ 0 are skipped.
pub(super) fn power_fit(pts: &[(f64, f64)]) -> Option<(f64, f64)> {
    let t: Vec<(f64, f64)> = pts.iter().filter(|p| p.0 > 0.0 && p.1 > 0.0).map(|(x, y)| (x.ln(), y.ln())).collect();
    linear_fit(&t, None).map(|(m, b)| (b.exp(), m))
}

/// Least-squares polynomial of `order` (capped at the number of points − 1) through `pts`, by
/// the normal equations. `None` for fewer than 2 points or a singular system.
pub(super) fn poly_fit(pts: &[(f64, f64)], order: usize) -> Option<Poly> {
    if pts.len() < 2 {
        return None;
    }
    let order = order.clamp(1, MAX_ORDER).min(pts.len() - 1);
    let n = pts.len() as f64;
    let mid = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let scale = pts.iter().map(|p| (p.0 - mid).abs()).fold(0.0, f64::max);
    if !(scale > 0.0 && scale.is_finite()) {
        return None;
    }
    let k = order + 1;
    // Augmented normal matrix: row r = [Σ t^(r+c) for c in 0..k, Σ y t^r].
    let mut sums = vec![0.0f64; 2 * k - 1];
    let mut rhs = vec![0.0f64; k];
    for (x, y) in pts {
        let t = (x - mid) / scale;
        let mut p = 1.0;
        for (e, s) in sums.iter_mut().enumerate() {
            *s += p;
            if let Some(r) = rhs.get_mut(e) {
                *r += y * p;
            }
            p *= t;
        }
    }
    let mut m: Vec<Vec<f64>> = (0..k).map(|r| sums.iter().skip(r).take(k).copied().chain(rhs.get(r).copied()).collect()).collect();
    let coef = solve(&mut m)?;
    Some(Poly { coef, mid, scale })
}

/// Solves the augmented square system `m` (k rows of k + 1) by Gaussian elimination with partial
/// pivoting; `None` when it is singular.
fn solve(m: &mut [Vec<f64>]) -> Option<Vec<f64>> {
    let k = m.len();
    let norm = m.iter().flatten().fold(0.0f64, |a, v| a.max(v.abs()));
    for col in 0..k {
        let piv = (col..k).max_by(|a, b| {
            let va = m.get(*a).and_then(|r| r.get(col)).map_or(0.0, |v| v.abs());
            let vb = m.get(*b).and_then(|r| r.get(col)).map_or(0.0, |v| v.abs());
            va.total_cmp(&vb)
        })?;
        m.swap(col, piv);
        let pivot_row = m.get(col)?.clone();
        let p = *pivot_row.get(col)?;
        if !(p.abs() > 1e-12 * norm) {
            return None;
        }
        for row in m.iter_mut().skip(col + 1) {
            let f = row.get(col).copied().unwrap_or(0.0) / p;
            for (v, pv) in row.iter_mut().zip(&pivot_row).skip(col) {
                *v -= f * pv;
            }
        }
    }
    let mut x = vec![0.0f64; k];
    for r in (0..k).rev() {
        let row = m.get(r)?;
        let known: f64 = (r + 1..k).map(|c| row.get(c).copied().unwrap_or(0.0) * x.get(c).copied().unwrap_or(0.0)).sum();
        let v = (row.get(k).copied()? - known) / row.get(r).copied()?;
        if !v.is_finite() {
            return None;
        }
        *x.get_mut(r)? = v;
    }
    Some(x)
}

/// The moving average of `pts` (in order): from the `period`-th point on, the mean of the last
/// `period` values, at that point's x.
pub(super) fn moving_average(pts: &[(f64, f64)], period: usize) -> Vec<(f64, f64)> {
    let period = period.max(1);
    pts.windows(period).filter_map(|w| Some((w.last()?.0, w.iter().map(|p| p.1).sum::<f64>() / period as f64))).collect()
}

/// The fit of trendline `t` to `pts`, or `None` when there's too little to fit.
pub(super) fn fit(t: &Trendline, pts: &[(f64, f64)]) -> Option<Fit> {
    Some(match t.kind {
        TrendKind::Linear => {
            let (m, b) = linear_fit(pts, t.intercept)?;
            Fit::Linear { m, b }
        }
        TrendKind::Exp => {
            let (a, b) = exp_fit(pts)?;
            Fit::Exp { a, b }
        }
        TrendKind::Log => {
            let (a, b) = log_fit(pts)?;
            Fit::Log { a, b }
        }
        TrendKind::Power => {
            let (a, b) = power_fit(pts)?;
            Fit::Power { a, b }
        }
        TrendKind::Poly => Fit::Poly(poly_fit(pts, t.order)?),
        TrendKind::MovingAvg => {
            let v = moving_average(pts, t.period);
            if v.len() < 2 {
                return None;
            }
            Fit::Moving(v)
        }
    })
}

/// The curve's vertices in data units over the points' x range widened by `backward`/`forward`.
fn vertices(t: &Trendline, f: &Fit, pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if let Fit::Moving(v) = f {
        return v.clone();
    }
    let lo = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) - t.backward;
    let hi = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) + t.forward;
    if !(lo.is_finite() && hi.is_finite() && hi > lo) {
        return Vec::new();
    }
    let steps = if matches!(f, Fit::Linear { .. }) { 1 } else { CURVE_STEPS };
    (0..=steps)
        .map(|k| {
            let x = lo + (hi - lo) * k as f64 / steps as f64;
            (x, f.eval(x))
        })
        .collect()
}

/// The (x, y) points of series `s` a trendline fits: scatter x values, or category numbers from
/// 1 over the frame's `n` categories.
fn data_points(s: &Series, scatter: bool, n: usize) -> Vec<(f64, f64)> {
    let pts: Vec<(f64, f64)> = if scatter {
        s.xs.iter().zip(&s.values).filter_map(|(x, y)| Some(((*x)?, (*y)?))).collect()
    } else {
        s.values.iter().take(n).enumerate().filter_map(|(i, v)| Some(((i + 1) as f64, (*v)?))).collect()
    };
    pts.into_iter().filter(|(x, y)| x.is_finite() && y.is_finite()).collect()
}

/// The x range a scatter series' trendlines reach beyond its points, so the x axis takes it in.
pub(super) fn x_reach(s: &Series) -> Vec<f64> {
    let lo = s.xs.iter().flatten().copied().filter(|x| x.is_finite()).fold(f64::INFINITY, f64::min);
    let hi = s.xs.iter().flatten().copied().filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    if !(lo.is_finite() && hi.is_finite()) {
        return Vec::new();
    }
    s.trendlines.iter().filter(|t| !t.hidden && t.kind != TrendKind::MovingAvg).flat_map(|t| [lo - t.backward, hi + t.forward]).collect()
}

/// Draws the trendlines of the series of a flat line, bar/column or scatter chart type (stacked
/// types have none).
pub(super) fn draw(out: &mut Vec<GraphicItem>, c: &Chart, grp: &Group, g: &Frame) {
    if grp.stacked() || grp.depth.is_some() {
        return;
    }
    let scatter = grp.kind == Kind::Scatter;
    for (j, s) in grp.series.iter().enumerate().filter(|(_, s)| !s.trendlines.is_empty()) {
        let pts = data_points(s, scatter, g.n);
        for t in s.trendlines.iter().filter(|t| !t.hidden) {
            let Some(f) = fit(t, &pts) else { continue };
            let screen: Vec<Option<(f64, f64)>> =
                vertices(t, &f, &pts).into_iter().map(|(x, y)| if y.is_finite() { g.point_at(x, y, scatter) } else { None }).collect();
            let color = t.stroke.0.unwrap_or_else(|| c.series_colour(grp, j));
            stroke(out, clip(&screen, g.plot), color, t.stroke.1.unwrap_or(s.line_w));
        }
    }
}

/// A polyline (a gap at `None`) clipped to `r`, as path segments.
fn clip(pts: &[Option<(f64, f64)>], r: Rect) -> Vec<PathSeg> {
    let mut segs = Vec::new();
    let mut pen: Option<(f64, f64)> = None;
    for w in pts.windows(2) {
        let [Some(a), Some(b)] = w else { continue };
        let Some((p, q)) = clip_segment(*a, *b, r) else { continue };
        let joined = pen.is_some_and(|e| (e.0 - p.0).abs() < 1e-6 && (e.1 - p.1).abs() < 1e-6);
        if !joined {
            segs.push(PathSeg::Move(p.0 as f32, p.1 as f32));
        }
        segs.push(PathSeg::Line(q.0 as f32, q.1 as f32));
        pen = Some(q);
    }
    segs
}

/// The part of segment `a`–`b` inside `r` (Liang–Barsky), if any.
fn clip_segment(a: (f64, f64), b: (f64, f64), r: Rect) -> Option<((f64, f64), (f64, f64))> {
    let (x0, y0) = (f64::from(r.x), f64::from(r.y));
    let (x1, y1) = (f64::from(r.right()), f64::from(r.bottom()));
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    if !(t0 <= t1) {
        return None;
    }
    let at = |t: f64| (a.0 + t * dx, a.1 + t * dy);
    Some((at(t0), at(t1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PTS: [(f64, f64); 4] = [(1.0, 2.0), (2.0, 4.0), (3.0, 5.0), (4.0, 8.0)];

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn fits_match_least_squares() {
        let (m, b) = linear_fit(&PTS, None).unwrap();
        assert!(close(m, 1.9) && close(b, 0.0), "{m} {b}");
        let (m, b) = linear_fit(&PTS, Some(1.0)).unwrap();
        assert!(close(m, 1.566_666_7) && close(b, 1.0), "{m} {b}");
        let (a, b) = exp_fit(&PTS).unwrap();
        assert!(close(a, std::f64::consts::SQRT_2) && close(b, 0.438_202_66), "{a} {b}");
        let (a, b) = log_fit(&PTS).unwrap();
        assert!(close(a, 3.929_367_2) && close(b, 1.628_064_9), "{a} {b}");
        let (a, b) = power_fit(&PTS).unwrap();
        assert!(close(a, 1.990_122_7) && close(b, 0.948_862_4), "{a} {b}");
        let p = poly_fit(&PTS, 2).unwrap();
        for x in [0.0, 1.0, 2.5, 7.0] {
            assert!(close(p.eval(x), 0.25 * x * x + 0.65 * x + 1.25), "{x}: {}", p.eval(x));
        }
        assert_eq!(moving_average(&PTS, 2), vec![(2.0, 3.0), (3.0, 4.5), (4.0, 6.5)]);
        assert_eq!(moving_average(&PTS, 5), Vec::new());
    }

    #[test]
    fn exp_skips_points_at_or_below_zero() {
        let mut pts = PTS.to_vec();
        pts.push((5.0, 0.0));
        pts.push((6.0, -3.0));
        assert_eq!(exp_fit(&pts), exp_fit(&PTS));
        // Only one positive point left: nothing to fit.
        assert_eq!(exp_fit(&[(1.0, 2.0), (2.0, -1.0)]), None);
    }

    #[test]
    fn one_point_or_no_spread_draws_no_line() {
        let t = |kind| Trendline { kind, order: 2, period: 2, forward: 0.0, backward: 0.0, intercept: None, stroke: (None, None), hidden: false };
        for kind in [TrendKind::Linear, TrendKind::Exp, TrendKind::Log, TrendKind::Power, TrendKind::Poly, TrendKind::MovingAvg] {
            assert_eq!(fit(&t(kind), &[(1.0, 2.0)]), None, "{kind:?}");
            assert_eq!(fit(&t(kind), &[]), None, "{kind:?}");
        }
        assert_eq!(linear_fit(&[(2.0, 1.0), (2.0, 5.0)], None), None);
        assert_eq!(poly_fit(&[(2.0, 1.0), (2.0, 5.0), (2.0, 3.0)], 2), None);
    }

    #[test]
    fn poly_order_is_capped_by_the_points() {
        // Two points: a straight line through both.
        let p = poly_fit(&[(1.0, 1.0), (3.0, 5.0)], 6).unwrap();
        assert_eq!(p.coef.len(), 2);
        assert!(close(p.eval(2.0), 3.0));
        // Large x values (dates) stay well conditioned.
        let pts: Vec<(f64, f64)> = (0..10).map(|k| (45_000.0 + f64::from(k), f64::from(k * k))).collect();
        let p = poly_fit(&pts, 6).unwrap();
        assert!(close(p.eval(45_005.0), 25.0), "{}", p.eval(45_005.0));
    }

    #[test]
    fn parse_caps_order_period_and_count() {
        let x = format!(
            r#"<c:ser xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">{}{}<c:trendline><c:trendlineType val="spline"/></c:trendline>{}</c:ser>"#,
            r#"<c:trendline><c:trendlineType val="poly"/><c:order val="7"/></c:trendline>"#,
            r#"<c:trendline><c:trendlineType val="movingAvg"/><c:period val="99999"/><c:forward val="-3"/><c:backward val="NaN"/></c:trendline>"#,
            r#"<c:trendline><c:trendlineType val="linear"/><c:order val="-5"/></c:trendline>"#.repeat(5)
        );
        let ser = crate::xml::parse(x.as_bytes()).unwrap();
        let t = parse(&ser, &[]);
        assert_eq!(t.len(), MAX_TRENDLINES);
        assert_eq!((t[0].kind, t[0].order), (TrendKind::Poly, 6));
        assert_eq!((t[1].kind, t[1].period, t[1].forward, t[1].backward), (TrendKind::MovingAvg, MAX_PERIOD, 0.0, 0.0));
        assert_eq!((t[2].kind, t[2].order), (TrendKind::Linear, 2));
    }

    #[test]
    fn clipping_keeps_the_part_inside() {
        let r = Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
        let segs = clip(&[Some((-5.0, 5.0)), Some((5.0, 5.0)), Some((15.0, 5.0)), None, Some((20.0, 20.0)), Some((30.0, 30.0))], r);
        assert_eq!(segs, vec![PathSeg::Move(0.0, 5.0), PathSeg::Line(5.0, 5.0), PathSeg::Line(10.0, 5.0)]);
    }
}
