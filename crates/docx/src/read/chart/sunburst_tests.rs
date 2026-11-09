//! Office 2016 sunburst charts (`cx:series layoutId="sunburst"`) are drawn from their data: one
//! ring per category level around a one-ring hole, siblings largest first clockwise from 12
//! o'clock, each branch in its top-level accent colour, as Word draws them.

use super::chartex::chartex_items;
use super::sunburst::{SunburstSegment, sunburst_segments};
use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str = r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex""#;

/// (top level, middle level, leaf, value) in data order; "" = no node at that level.
const DATA: [(&str, &str, &str, f64); 16] = [
    ("Branch 1", "Stem 1", "Leaf 1", 22.0),
    ("Branch 1", "Stem 1", "Leaf 2", 12.0),
    ("Branch 1", "Stem 1", "Leaf 3", 18.0),
    ("Branch 1", "Stem 2", "Leaf 4", 87.0),
    ("Branch 1", "Stem 2", "Leaf 5", 88.0),
    ("Branch 1", "Leaf 6", "", 17.0),
    ("Branch 1", "Leaf 7", "", 14.0),
    ("Branch 2", "Stem 3", "Leaf 8", 25.0),
    ("Branch 2", "Leaf 9", "", 16.0),
    ("Branch 2", "Stem 4", "Leaf 10", 24.0),
    ("Branch 2", "Stem 4", "Leaf 11", 89.0),
    ("Branch 3", "Stem 5", "Leaf 12", 16.0),
    ("Branch 3", "Stem 5", "Leaf 13", 19.0),
    ("Branch 3", "Stem 6", "Leaf 14", 86.0),
    ("Branch 3", "Stem 6", "Leaf 15", 23.0),
    ("Branch 3", "Leaf 16", "", 21.0),
];

/// Word's plot rect inside a 432 x 252 pt chart frame with the default title (measured).
const PLOT: [f32; 4] = [5.82, 26.6, 420.36, 219.6];
const CX: f32 = 216.0;
const CY: f32 = 136.4;
const R: f32 = 109.8;

/// (path, group, a0, a1) of every node, measured from Word's PDF of a sunburst with this data.
const WORD: [(&[&str], usize, f32, f32); 25] = [
    (&["Branch 1"], 0, 0.0, 160.97),
    (&["Branch 3"], 2, 160.97, 263.91),
    (&["Branch 2"], 1, 263.91, 360.0),
    (&["Branch 1", "Stem 2"], 0, 0.0, 109.18),
    (&["Branch 1", "Stem 1"], 0, 109.18, 141.63),
    (&["Branch 1", "Leaf 6"], 0, 141.63, 152.24),
    (&["Branch 1", "Leaf 7"], 0, 152.24, 160.97),
    (&["Branch 3", "Stem 6"], 2, 160.97, 228.98),
    (&["Branch 3", "Stem 5"], 2, 228.98, 250.81),
    (&["Branch 3", "Leaf 16"], 2, 250.81, 263.91),
    (&["Branch 2", "Stem 4"], 1, 263.91, 334.43),
    (&["Branch 2", "Stem 3"], 1, 334.43, 350.02),
    (&["Branch 2", "Leaf 9"], 1, 350.02, 360.0),
    (&["Branch 1", "Stem 2", "Leaf 5"], 0, 0.0, 54.91),
    (&["Branch 1", "Stem 2", "Leaf 4"], 0, 54.91, 109.18),
    (&["Branch 1", "Stem 1", "Leaf 1"], 0, 109.18, 122.91),
    (&["Branch 1", "Stem 1", "Leaf 3"], 0, 122.91, 134.14),
    (&["Branch 1", "Stem 1", "Leaf 2"], 0, 134.14, 141.63),
    (&["Branch 3", "Stem 6", "Leaf 14"], 2, 160.97, 214.63),
    (&["Branch 3", "Stem 6", "Leaf 15"], 2, 214.63, 228.98),
    (&["Branch 3", "Stem 5", "Leaf 13"], 2, 228.98, 240.83),
    (&["Branch 3", "Stem 5", "Leaf 12"], 2, 240.83, 250.81),
    (&["Branch 2", "Stem 4", "Leaf 11"], 1, 263.91, 319.44),
    (&["Branch 2", "Stem 4", "Leaf 10"], 1, 319.44, 334.43),
    (&["Branch 2", "Stem 3", "Leaf 8"], 1, 334.43, 350.02),
];

fn leaves() -> Vec<(Vec<String>, f64)> {
    DATA.iter().map(|(b, s, l, v)| ([*b, *s, *l].iter().map(|n| n.to_string()).collect(), *v)).collect()
}

fn find<'a>(segs: &'a [SunburstSegment], path: &[&str]) -> &'a SunburstSegment {
    segs.iter().find(|s| s.path.iter().map(String::as_str).eq(path.iter().copied())).unwrap_or_else(|| panic!("no segment {path:?} in {segs:?}"))
}

#[test]
fn one_segment_per_node_and_blank_leaves_end_the_path() {
    let segs = sunburst_segments(&leaves(), PLOT);
    assert_eq!(segs.len(), 25, "3 top-level + 10 middle + 12 leaf nodes: {segs:?}");
    assert!(segs.iter().all(|s| s.path.iter().all(|n| !n.is_empty())), "no segment for a blank name");
    assert_eq!(segs.iter().filter(|s| s.path.len() == 3).count(), 12);
    // Nodes are summed: Branch 1 = 22+12+18+87+88+17+14.
    assert_eq!(find(&segs, &["Branch 1"]).value, 258.0);
    assert_eq!(find(&segs, &["Branch 1", "Stem 2"]).value, 175.0);
    assert_eq!(find(&segs, &["Branch 2", "Leaf 9"]).value, 16.0);
}

#[test]
fn siblings_go_largest_first_clockwise_from_twelve_o_clock() {
    let segs = sunburst_segments(&leaves(), PLOT);
    for (path, _, a0, a1) in WORD {
        let s = find(&segs, path);
        assert!((s.a0 - a0).abs() < 0.1 && (s.a1 - a1).abs() < 0.1, "{path:?}: {}..{}, Word {a0}..{a1}", s.a0, s.a1);
    }
}

#[test]
fn rings_are_equal_bands_around_a_one_band_hole() {
    let segs = sunburst_segments(&leaves(), PLOT);
    let band = R / 4.0; // 3 levels + the hole
    for s in &segs {
        let d = (s.path.len() - 1) as f32;
        assert!((s.r0 - (d + 1.0) * band).abs() < 0.1, "{:?} r0 {} want {}", s.path, s.r0, (d + 1.0) * band);
        assert!((s.r1 - (d + 2.0) * band).abs() < 0.1, "{:?} r1 {} want {}", s.path, s.r1, (d + 2.0) * band);
    }
    // The outer radius follows the shorter side of the plot.
    let wide = sunburst_segments(&leaves(), [0.0, 0.0, 400.0, 100.0]);
    assert!((find(&wide, &["Branch 1", "Stem 2", "Leaf 5"]).r1 - 50.0).abs() < 0.1);
}

#[test]
fn every_node_keeps_its_top_level_group() {
    let segs = sunburst_segments(&leaves(), PLOT);
    for (path, group, _, _) in WORD {
        assert_eq!(find(&segs, path).group, group, "{path:?}");
    }
}

#[test]
fn zero_and_negative_values_are_skipped() {
    let mut l = leaves();
    l.push((vec!["Branch 4".into(), "Stem 9".into(), "Leaf 99".into()], 0.0));
    l.push((vec!["Branch 1".into(), "Stem 1".into(), "Leaf 98".into()], -5.0));
    let segs = sunburst_segments(&l, PLOT);
    assert_eq!(segs.len(), 25, "{segs:?}");
    assert!((find(&segs, &["Branch 2"]).a1 - 360.0).abs() < 0.1);
}

fn space() -> El {
    let pt = |k: usize, v: &str| format!(r#"<cx:pt idx="{k}">{v}</cx:pt>"#);
    let lvl = |f: &dyn Fn(&(&str, &str, &str, f64)) -> String| -> String {
        format!(r#"<cx:lvl ptCount="16">{}</cx:lvl>"#, DATA.iter().enumerate().map(|(k, d)| pt(k, &f(d))).collect::<String>())
    };
    // Leaf level first, top level last, as Excel writes them.
    let cats = [lvl(&|d| d.2.to_string()), lvl(&|d| d.1.to_string()), lvl(&|d| d.0.to_string())].concat();
    let vals: String = DATA.iter().enumerate().map(|(k, d)| pt(k, &d.3.to_string())).collect();
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData><cx:data id="0"><cx:strDim type="cat"><cx:f>Sheet1!$A$2:$C$17</cx:f>{cats}</cx:strDim><cx:numDim type="size"><cx:f>Sheet1!$D$2:$D$17</cx:f><cx:lvl ptCount="16" formatCode="General">{vals}</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:title pos="t" align="ctr" overlay="0"/><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="sunburst"><cx:tx><cx:txData><cx:v>Series1</cx:v></cx:txData></cx:tx><cx:dataLabels pos="ctr"><cx:visibility seriesName="0" categoryName="1" value="0"/></cx:dataLabels><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

/// Bounding box [x0, y0, x1, y1] of a path's points.
fn bbox(segs: &[PathSeg]) -> [f32; 4] {
    let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    let pts = segs.iter().flat_map(|s| match *s {
        PathSeg::Move(x, y) | PathSeg::Line(x, y) => vec![[x, y]],
        PathSeg::Cubic(x1, y1, x2, y2, x, y) => vec![[x1, y1], [x2, y2], [x, y]],
        PathSeg::Close => vec![],
    });
    for [x, y] in pts {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

#[test]
fn chart_is_drawn_as_filled_sectors_in_branch_colours() {
    let items = chartex_items(&space(), &THEME_COLORS, 432.0, 252.0).expect("sunburst drawn, not the fallback");
    let white = Rgb(0xFF, 0xFF, 0xFF);
    let fills: Vec<(Rgb, [f32; 4])> = items
        .iter()
        .filter_map(|i| match i {
            GraphicItem::Path { segs, fill: Some(f), .. } if *f != white => Some((*f, bbox(segs))),
            _ => None,
        })
        .collect();
    let count = |c: Rgb| fills.iter().filter(|(f, _)| *f == c).count();
    // accent1..3 = THEME_COLORS[4..7]; Branch 1 has 10 nodes, Branch 2 has 7, Branch 3 has 8.
    assert_eq!((count(THEME_COLORS[4]), count(THEME_COLORS[5]), count(THEME_COLORS[6])), (10, 7, 8), "{fills:?}");
    // Every sector lies within Word's disc: centre (216, 136.4), radius 109.8 (Bézier control points may overshoot a little).
    for (_, b) in &fills {
        assert!(b[0] >= CX - R - 6.0 && b[2] <= CX + R + 6.0 && b[1] >= CY - R - 6.0 && b[3] <= CY + R + 6.0, "{b:?}");
    }
    let x0 = fills.iter().map(|f| f.1[0]).fold(f32::INFINITY, f32::min);
    let y1 = fills.iter().map(|f| f.1[3]).fold(f32::NEG_INFINITY, f32::max);
    assert!((x0 - (CX - R)).abs() < 1.5 && (y1 - (CY + R)).abs() < 1.5, "disc left {x0}, bottom {y1}");
    // Category names label the sectors.
    let texts: Vec<&str> = items.iter().filter_map(|i| if let GraphicItem::Text { text, .. } = i { Some(text.as_str()) } else { None }).collect();
    assert!(texts.contains(&"Branch 1") && texts.contains(&"Stem 2"), "{texts:?}");
}
