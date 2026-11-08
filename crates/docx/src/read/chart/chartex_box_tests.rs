//! Office 2016 box & whisker charts (`cx:series layoutId="boxWhisker"`) are drawn from their data:
//! one box per category and series from q1 to q3 (Excel's exclusive or inclusive quartiles),
//! whiskers to the last value within 1.5 IQR, outliers beyond, as Word draws them.

use super::chartex::{BoxStats, box_stats, chartex_items};
use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str = r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex""#;
const A: [f64; 9] = [-3.0, 1.0, -6.0, 10.0, 34.0, 128.0, 22.0, -12.0, -28.0];
const B: [f64; 7] = [6.0, 31.0, 3.0, 12.0, -12.0, -13.0, 6.0];
const C: [f64; 6] = [2.0, 4.0, 4.0, 5.0, 7.0, 9.0];

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn check(values: &[f64], exclusive: bool, want: [f64; 6], outliers: &[f64]) {
    let s: BoxStats = box_stats(values, exclusive).unwrap_or_else(|| panic!("stats of {values:?}"));
    let got = [s.q1, s.median, s.q3, s.lo, s.hi, s.mean];
    for (i, name) in ["q1", "median", "q3", "lo", "hi", "mean"].iter().enumerate() {
        assert!(close(got[i], want[i]), "{name} of {values:?} (exclusive {exclusive}): got {}, want {}", got[i], want[i]);
    }
    assert_eq!(s.outliers, outliers, "outliers of {values:?} (exclusive {exclusive})");
}

#[test]
fn box_stats_use_excel_exclusive_and_inclusive_quartiles() {
    check(&A, true, [-9.0, 1.0, 28.0, -28.0, 34.0, 146.0 / 9.0], &[128.0]);
    check(&A, false, [-6.0, 1.0, 22.0, -28.0, 34.0, 146.0 / 9.0], &[128.0]);
    check(&B, true, [-12.0, 6.0, 12.0, -13.0, 31.0, 33.0 / 7.0], &[]);
    check(&B, false, [-4.5, 6.0, 9.0, -13.0, 12.0, 33.0 / 7.0], &[31.0]);
    check(&C, true, [3.5, 4.5, 7.5, 2.0, 9.0, 31.0 / 6.0], &[]);
    check(&[5.0], true, [5.0; 6], &[]);
    check(&[5.0], false, [5.0; 6], &[]);
    assert!(box_stats(&[], true).is_none());
    assert!(box_stats(&[f64::NAN], true).is_none());
}

/// A `cx:data` with values split over categories (`cats[k]` labels value k).
fn data(id: usize, groups: &[(&str, &[f64])]) -> String {
    let mut cats = String::new();
    let mut vals = String::new();
    let mut k = 0;
    for (cat, vs) in groups {
        for v in *vs {
            cats.push_str(&format!(r#"<cx:pt idx="{k}">{cat}</cx:pt>"#));
            vals.push_str(&format!(r#"<cx:pt idx="{k}">{v}</cx:pt>"#));
            k += 1;
        }
    }
    format!(
        r#"<cx:data id="{id}"><cx:strDim type="cat"><cx:lvl ptCount="{k}">{cats}</cx:lvl></cx:strDim><cx:numDim type="val"><cx:lvl ptCount="{k}" formatCode="General">{vals}</cx:lvl></cx:numDim></cx:data>"#
    )
}

fn series(id: usize, method: &str) -> String {
    format!(
        r#"<cx:series layoutId="boxWhisker"><cx:tx><cx:txData><cx:v>Series{n}</cx:v></cx:txData></cx:tx><cx:dataId val="{id}"/><cx:layoutPr><cx:visibility meanLine="0" meanMarker="1" nonoutliers="0" outliers="1"/><cx:statistics quartileMethod="{method}"/></cx:layoutPr></cx:series>"#,
        n = id + 1
    )
}

/// Series 1: A in "Category 1", C in "Category 2"; series 2: B and C.
fn space(method: &str) -> El {
    let d = data(0, &[("Category 1", &A), ("Category 2", &C)]) + &data(1, &[("Category 1", &B), ("Category 2", &C)]);
    let s = series(0, method) + &series(1, method);
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData>{d}</cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion>{s}</cx:plotAreaRegion><cx:axis id="0"><cx:catScaling gapWidth="1"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea></cx:chart></cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

/// Filled items with their bounding box [x0, y0, x1, y1].
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

fn boxes(method: &str) -> Vec<(Rgb, [f32; 4])> {
    let items = chartex_items(&space(method), &THEME_COLORS, 400.0, 300.0).expect("box & whisker is drawn");
    let b = filled(&items);
    assert_eq!(b.len(), 4, "one filled box per category and series, markers not filled: {b:?}");
    b
}

#[test]
fn boxes_go_category_by_category_in_series_colours() {
    let b = boxes("exclusive");
    let fills: Vec<Rgb> = b.iter().map(|x| x.0).collect();
    assert_eq!(fills, vec![THEME_COLORS[4], THEME_COLORS[5], THEME_COLORS[4], THEME_COLORS[5]]);
    for k in 1..4 {
        assert!(b[k].1[0] > b[k - 1].1[0], "box {k} right of box {}: {b:?}", k - 1);
    }
    let inside = b[1].1[0] - b[0].1[2];
    let between = b[2].1[0] - b[1].1[2];
    assert!(between > inside + 1.0, "categories further apart ({between}) than series in one ({inside}): {b:?}");
}

#[test]
fn box_edges_sit_on_q1_and_q3_on_one_value_scale() {
    let b = boxes("exclusive");
    // (q1, q3) per box: A exc, B exc, C exc, C exc.
    let qs = [(-9.0, 28.0), (-12.0, 12.0), (3.5, 7.5), (3.5, 7.5)];
    // Points per unit and the y of value 0 from box A (y grows down).
    let scale = (b[0].1[3] - b[0].1[1]) as f64 / 37.0;
    let zero = b[0].1[3] as f64 - 9.0 * scale;
    assert!(scale > 0.0);
    for (k, &(q1, q3)) in qs.iter().enumerate() {
        let (top, bottom) = (b[k].1[1] as f64, b[k].1[3] as f64);
        assert!((top - (zero - q3 * scale)).abs() < 0.5, "box {k} top at q3 {q3}: {:?}", b[k]);
        assert!((bottom - (zero - q1 * scale)).abs() < 0.5, "box {k} bottom at q1 {q1}: {:?}", b[k]);
    }
}

#[test]
fn inclusive_quartiles_change_the_box() {
    let exc = boxes("exclusive");
    let inc = boxes("inclusive");
    // Box C (category 2) is the same either way: 4 units exclusive, 2.5 inclusive (4..6.5).
    let unit_exc = (exc[0].1[3] - exc[0].1[1]) as f64 / 37.0;
    let unit_inc = (inc[2].1[3] - inc[2].1[1]) as f64 / 2.5;
    let a_inc = (inc[0].1[3] - inc[0].1[1]) as f64 / unit_inc;
    assert!((a_inc - 28.0).abs() < 0.2, "box A inclusive spans 28 units (-6..22), got {a_inc}");
    assert!(unit_exc > 0.0);
}

#[test]
fn value_axis_labels_cover_whiskers_and_outliers() {
    // -78 and 128 as outliers/whisker ends: Word's axis is -100..150 step 50.
    let d = data(0, &[("Category 1", &[-78.0, -10.0, 0.0, 5.0, 10.0, 128.0])]);
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData>{d}</cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion>{s}</cx:plotAreaRegion><cx:axis id="0"><cx:catScaling gapWidth="1"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea></cx:chart></cx:chartSpace>"#,
        s = series(0, "exclusive")
    );
    let items = chartex_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, 400.0, 300.0).expect("drawn");
    let texts: Vec<&str> = items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for t in ["-100", "-50", "0", "50", "100", "150", "Category 1"] {
        assert!(texts.contains(&t), "label {t} in {texts:?}");
    }
    assert!(!texts.contains(&"200") && !texts.contains(&"-150"), "axis stops at the nice ends: {texts:?}");
}

#[test]
fn a_box_series_mixed_with_another_layout_keeps_the_fallback() {
    let d = data(0, &[("Category 1", &A)]);
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData>{d}</cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion>{s}<cx:series layoutId="paretoLine"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#,
        s = series(0, "exclusive")
    );
    assert!(chartex_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, 400.0, 300.0).is_none());
}
