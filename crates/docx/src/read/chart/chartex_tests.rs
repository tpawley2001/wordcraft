//! Office 2016 waterfall charts (`cx:series layoutId="waterfall"`) are drawn from their data:
//! totals stand on the axis, other points float from the running sum, coloured increase
//! (accent1), decrease (accent2) and total (accent3), as Word draws them.

use super::chartex::chartex_items;
use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str = r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex""#;
const VALS: [f64; 8] = [100.0, 20.0, 50.0, -40.0, 130.0, -60.0, 70.0, 140.0];
/// (from, to) of each bar in value units: running sum, totals at 0, 4 and 7.
const SPANS: [(f64, f64); 8] =
    [(0.0, 100.0), (100.0, 120.0), (120.0, 170.0), (130.0, 170.0), (0.0, 130.0), (70.0, 130.0), (70.0, 140.0), (0.0, 140.0)];
/// accent1, accent2, accent3 of the theme passed in (dk1, lt1, dk2, lt2, accent1… order).
const INC: Rgb = THEME_COLORS[4];
const DEC: Rgb = THEME_COLORS[5];
const TOT: Rgb = THEME_COLORS[6];

fn space(layout: &str, labels: bool) -> El {
    let pts: String = VALS.iter().enumerate().map(|(k, v)| format!(r#"<cx:pt idx="{k}">{v}</cx:pt>"#)).collect();
    let dl = if labels { r#"<cx:dataLabels pos="outEnd"><cx:visibility seriesName="0" categoryName="0" value="1"/></cx:dataLabels>"# } else { "" };
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:f>Sheet1!$A$2:$A$9</cx:f><cx:lvl ptCount="8" formatCode="General">{pts}</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="{layout}"><cx:tx><cx:txData><cx:v>Series1</cx:v></cx:txData></cx:tx>{dl}<cx:dataId val="0"/><cx:layoutPr><cx:subtotals><cx:idx val="0"/><cx:idx val="4"/><cx:idx val="7"/></cx:subtotals></cx:layoutPr></cx:series></cx:plotAreaRegion><cx:axis id="0"><cx:catScaling gapWidth="0.5"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea></cx:chart></cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

/// Filled items (shapes or paths) with their bounding box [x0, y0, x1, y1].
fn filled(items: &[GraphicItem]) -> Vec<(Rgb, [f32; 4])> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Shape { rect, fill: Some(c), .. } => Some((*c, [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]])),
            GraphicItem::Path { segs, fill: Some(c), .. } => {
                let pts: Vec<(f32, f32)> = segs
                    .iter()
                    .flat_map(|s| match *s {
                        PathSeg::Move(x, y) | PathSeg::Line(x, y) => vec![(x, y)],
                        PathSeg::Cubic(a, b, c, d, e, f) => vec![(a, b), (c, d), (e, f)],
                        PathSeg::Close => vec![],
                    })
                    .collect();
                let x0 = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
                let y0 = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
                let x1 = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
                let y1 = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
                Some((*c, [x0, y0, x1, y1]))
            }
            _ => None,
        })
        .collect()
}

fn bars() -> Vec<(Rgb, [f32; 4])> {
    let items = chartex_items(&space("waterfall", false), &THEME_COLORS, 400.0, 300.0).expect("waterfall is drawn");
    let b = filled(&items);
    assert_eq!(b.len(), 8, "one filled bar per point and nothing else filled: {b:?}");
    b
}

#[test]
fn waterfall_bars_take_increase_decrease_and_total_colours() {
    let fills: Vec<Rgb> = bars().iter().map(|b| b.0).collect();
    assert_eq!(fills, vec![TOT, INC, INC, DEC, TOT, DEC, INC, TOT]);
}

#[test]
fn waterfall_bars_run_left_to_right_in_point_order() {
    let b = bars();
    for k in 1..b.len() {
        assert!(b[k].1[0] > b[k - 1].1[2] - 0.01, "bar {k} starts after bar {} ends: {:?}", k - 1, b);
    }
}

#[test]
fn waterfall_bars_float_on_the_running_sum_with_totals_on_the_axis() {
    let b = bars();
    // Points per value unit, from total bar 0 (0..100).
    let scale = (b[0].1[3] - b[0].1[1]) as f64 / 100.0;
    let base = b[0].1[3] as f64; // y of value 0 (y grows down)
    assert!(scale > 0.0);
    for (k, &(from, to)) in SPANS.iter().enumerate() {
        let (top, bottom) = (b[k].1[1] as f64, b[k].1[3] as f64);
        assert!((bottom - (base - from * scale)).abs() < 0.5, "bar {k} bottom at value {from}: {:?}", b[k]);
        assert!((top - (base - to * scale)).abs() < 0.5, "bar {k} top at value {to}: {:?}", b[k]);
    }
}

#[test]
fn waterfall_value_labels_show_each_point() {
    let items = chartex_items(&space("waterfall", true), &THEME_COLORS, 400.0, 300.0).expect("waterfall is drawn");
    let texts: Vec<&str> = items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for v in ["100", "20", "50", "-40", "130", "-60", "70", "140"] {
        assert!(texts.contains(&v), "data label {v} in {texts:?}");
    }
}

#[test]
fn other_office_2016_layouts_keep_their_fallback() {
    for layout in ["treemap", "sunburst", "regionMap"] {
        assert!(chartex_items(&space(layout, false), &THEME_COLORS, 400.0, 300.0).is_none(), "{layout}");
    }
    assert!(chartex_items(&space("waterfall", false), &THEME_COLORS, 0.0, 300.0).is_none());
}
