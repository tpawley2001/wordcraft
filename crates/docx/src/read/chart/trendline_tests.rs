//! `c:trendline` (ECMA-376 Part 1 §21.2.2.209) drawn over a chart from Word. A red series with the
//! values 2, 4, 5, 8 at x = 1..4 and a blue trendline: the red series' own vertices give the
//! data -> point mapping, and every blue vertex must lie on the fitted curve. Expected fits from
//! numpy (polyfit on x, ln x, ln y as each type says).

use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str =
    r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;
const RED: Rgb = Rgb(0xFF, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 0xFF);
const YS: [f64; 4] = [2.0, 4.0, 5.0, 8.0];

fn ln(color: &str) -> String {
    format!(r#"<c:spPr><a:ln w="19050"><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></a:ln></c:spPr>"#)
}

fn num_cache(vals: &[f64]) -> String {
    let pts: String = vals.iter().enumerate().map(|(k, v)| format!(r#"<c:pt idx="{k}"><c:v>{v}</c:v></c:pt>"#)).collect();
    format!(r#"<c:numRef><c:f>Sheet1!$B$2:$B$5</c:f><c:numCache><c:ptCount val="{}"/>{pts}</c:numCache></c:numRef>"#, vals.len())
}

/// A line chart (categories 1..4) or a scatter chart (x 1..4) whose one series carries `trend`.
fn chart(scatter: bool, trend: &str) -> Vec<GraphicItem> {
    let xs = [1.0, 2.0, 3.0, 4.0];
    let plot = if scatter {
        format!(
            r#"<c:scatterChart><c:scatterStyle val="lineMarker"/><c:ser><c:idx val="0"/><c:order val="0"/>{}<c:marker><c:symbol val="none"/></c:marker>{trend}<c:xVal>{}</c:xVal><c:yVal>{}</c:yVal></c:ser><c:axId val="1"/><c:axId val="2"/></c:scatterChart><c:valAx><c:axId val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:valAx><c:valAx><c:axId val="2"/><c:axPos val="l"/><c:crossAx val="1"/></c:valAx>"#,
            ln("FF0000"),
            num_cache(&xs),
            num_cache(&YS)
        )
    } else {
        let cats: String = (1..=4).map(|k| format!(r#"<c:pt idx="{}"><c:v>{k}</c:v></c:pt>"#, k - 1)).collect();
        format!(
            r#"<c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="0"/><c:order val="0"/>{}<c:marker><c:symbol val="none"/></c:marker>{trend}<c:cat><c:strRef><c:strCache><c:ptCount val="4"/>{cats}</c:strCache></c:strRef></c:cat><c:val>{}</c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:lineChart><c:catAx><c:axId val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:axPos val="l"/><c:crossAx val="1"/></c:valAx>"#,
            ln("FF0000"),
            num_cache(&YS)
        )
    };
    let x = format!(r#"<c:chartSpace {NS}><c:chart><c:plotArea>{plot}</c:plotArea></c:chart></c:chartSpace>"#);
    chart_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, 400.0, 300.0)
}

fn trend(kind: &str, extra: &str) -> String {
    format!(r#"<c:trendline>{}<c:trendlineType val="{kind}"/>{extra}</c:trendline>"#, ln("0000FF"))
}

/// The vertices of the stroked paths in `color`.
fn vertices(items: &[GraphicItem], color: Rgb) -> Vec<(f64, f64)> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Path { segs, stroke: Some(c), .. } if *c == color => Some(segs),
            _ => None,
        })
        .flatten()
        .filter_map(|s| match s {
            PathSeg::Move(x, y) | PathSeg::Line(x, y) => Some((f64::from(*x), f64::from(*y))),
            _ => None,
        })
        .collect()
}

/// The blue vertices as data (x, y), mapped back through the red series' first and last points.
fn trend_in_data(items: &[GraphicItem]) -> Vec<(f64, f64)> {
    let red = vertices(items, RED);
    assert!(red.len() >= 4, "the series is drawn as a red line: {red:?}");
    let (p1, p4) = (red[0], red[red.len() - 1]);
    let (ax, ay) = ((p4.0 - p1.0) / 3.0, (p4.1 - p1.1) / (YS[3] - YS[0]));
    vertices(items, BLUE).into_iter().map(|(sx, sy)| (1.0 + (sx - p1.0) / ax, YS[0] + (sy - p1.1) / ay)).collect()
}

/// Every blue vertex on `f`, and the blue line spans x `from`..`to`.
fn assert_on(items: &[GraphicItem], f: impl Fn(f64) -> f64, from: f64, to: f64, what: &str) {
    let pts = trend_in_data(items);
    assert!(pts.len() >= 2, "{what}: no trendline drawn");
    for (x, y) in &pts {
        assert!((y - f(*x)).abs() < 0.05, "{what}: vertex ({x:.3}, {y:.3}) is off the curve ({:.3}); {pts:?}", f(*x));
    }
    let lo = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let hi = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    assert!((lo - from).abs() < 0.02 && (hi - to).abs() < 0.02, "{what}: spans {lo:.3}..{hi:.3}, not {from}..{to}");
}

#[test]
fn no_trendline_draws_nothing_blue() {
    assert!(vertices(&chart(false, ""), BLUE).is_empty());
}

#[test]
fn linear_trendline_on_a_line_chart_and_a_scatter_chart() {
    for scatter in [false, true] {
        assert_on(&chart(scatter, &trend("linear", "")), |x| 1.9 * x, 1.0, 4.0, if scatter { "scatter" } else { "line" });
    }
}

#[test]
fn curved_trendlines_follow_their_fits() {
    let cases: [(&str, &str, fn(f64) -> f64); 4] = [
        ("exp", "", |x| std::f64::consts::SQRT_2 * (0.438_202_66 * x).exp()),
        ("log", "", |x| 3.929_367_2 * x.ln() + 1.628_064_9),
        ("power", "", |x| 1.990_122_7 * x.powf(0.948_862_4)),
        ("poly", r#"<c:order val="2"/>"#, |x| 0.25 * x * x + 0.65 * x + 1.25),
    ];
    for (kind, extra, f) in cases {
        let items = chart(true, &trend(kind, extra));
        assert!(vertices(&items, BLUE).len() >= 10, "{kind}: drawn smooth, with many vertices");
        assert_on(&items, f, 1.0, 4.0, kind);
    }
}

#[test]
fn intercept_forward_and_backward() {
    assert_on(&chart(true, &trend("linear", r#"<c:intercept val="1"/>"#)), |x| 1.566_666_7 * x + 1.0, 1.0, 4.0, "intercept 1");
    assert_on(&chart(true, &trend("linear", r#"<c:forward val="1"/><c:backward val="0.5"/>"#)), |x| 1.9 * x, 0.5, 5.0, "forward 1, backward 0.5");
}

#[test]
fn moving_average_joins_the_means() {
    let pts = trend_in_data(&chart(true, &trend("movingAvg", r#"<c:period val="2"/>"#)));
    let want = [(2.0, 3.0), (3.0, 4.5), (4.0, 6.5)];
    assert_eq!(pts.len(), want.len(), "{pts:?}");
    for ((x, y), (wx, wy)) in pts.iter().zip(want) {
        assert!((x - wx).abs() < 0.02 && (y - wy).abs() < 0.05, "{pts:?}");
    }
}
