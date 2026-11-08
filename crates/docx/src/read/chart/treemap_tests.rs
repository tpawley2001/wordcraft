//! Office 2016 treemap charts (`cx:series layoutId="treemap"`) are drawn from their data: one
//! rectangle per leaf, area proportional to its size, nested by category level and tiled as Word
//! tiles them, coloured by top-level group.

use super::chartex::{chartex_items, treemap_rects};
use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str = r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex""#;

/// (top level, middle level, leaf, size, Word's tile [x0, y0, x1, y1] in a 420.36 x 190.8 pt plot
/// area at the origin). Measured from Word's own PDF of a treemap with this data.
const WORD: [(&str, &str, &str, f64, [f32; 4]); 16] = [
    ("Branch 1", "Stem 1", "Leaf 1", 22.0, [151.92, 0.00, 191.28, 80.76]),
    ("Branch 1", "Stem 1", "Leaf 2", 12.0, [151.92, 146.76, 191.28, 190.80]),
    ("Branch 1", "Stem 1", "Leaf 3", 18.0, [151.92, 80.76, 191.28, 146.76]),
    ("Branch 1", "Stem 2", "Leaf 4", 87.0, [76.44, 0.00, 151.92, 166.20]),
    ("Branch 1", "Stem 2", "Leaf 5", 88.0, [0.00, 0.00, 76.44, 166.20]),
    ("Branch 1", "Stem 2", "Leaf 6", 17.0, [0.00, 166.20, 99.36, 190.80]),
    ("Branch 1", "Stem 2", "Leaf 7", 9.0, [99.36, 166.20, 151.92, 190.80]),
    ("Branch 2", "Stem 3", "Leaf 8", 25.0, [191.28, 133.92, 254.64, 190.80]),
    ("Branch 2", "Stem 3", "Leaf 9", 23.0, [254.64, 133.92, 313.08, 190.80]),
    ("Branch 2", "Stem 4", "Leaf 10", 24.0, [191.28, 105.48, 313.08, 133.92]),
    ("Branch 2", "Stem 4", "Leaf 11", 89.0, [191.28, 0.00, 313.08, 105.48]),
    ("Branch 3", "Stem 5", "Leaf 12", 16.0, [371.28, 143.76, 420.36, 190.80]),
    ("Branch 3", "Stem 5", "Leaf 13", 19.0, [313.08, 143.76, 371.28, 190.80]),
    ("Branch 3", "Stem 6", "Leaf 14", 86.0, [313.08, 0.00, 420.36, 115.56]),
    ("Branch 3", "Stem 6", "Leaf 15", 10.0, [369.24, 115.56, 420.36, 143.76]),
    ("Branch 3", "Stem 6", "Leaf 16", 11.0, [313.08, 115.56, 369.24, 143.76]),
];
const PLOT_W: f32 = 420.36;
const PLOT_H: f32 = 190.8;

fn leaves() -> Vec<(Vec<String>, f64)> {
    WORD.iter().map(|(b, s, l, v, _)| (vec![b.to_string(), s.to_string(), l.to_string()], *v)).collect()
}

fn near(a: [f32; 4], b: [f32; 4], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

#[test]
fn top_level_groups_are_full_height_columns_in_proportion() {
    let got = treemap_rects(&leaves(), 0.0, 0.0, PLOT_W, PLOT_H);
    assert_eq!(got.len(), 16);
    // Branch k spans from its leftmost to its rightmost leaf: 0..191.28, 191.28..313.08, 313.08..420.36.
    for (branch, want) in [("Branch 1", [0.0, 191.28]), ("Branch 2", [191.28, 313.08]), ("Branch 3", [313.08, 420.36])] {
        let rs: Vec<[f32; 4]> = WORD.iter().zip(&got).filter(|(w, _)| w.0 == branch).map(|(_, r)| *r).collect();
        let x0 = rs.iter().map(|r| r[0]).fold(f32::INFINITY, f32::min);
        let x1 = rs.iter().map(|r| r[2]).fold(f32::NEG_INFINITY, f32::max);
        let y0 = rs.iter().map(|r| r[1]).fold(f32::INFINITY, f32::min);
        let y1 = rs.iter().map(|r| r[3]).fold(f32::NEG_INFINITY, f32::max);
        assert!((x0 - want[0]).abs() < 0.5 && (x1 - want[1]).abs() < 0.5, "{branch} x {x0}..{x1}, want {want:?}");
        assert!(y0.abs() < 0.5 && (y1 - PLOT_H).abs() < 0.5, "{branch} full height: {y0}..{y1}");
    }
}

#[test]
fn every_leaf_area_is_proportional_to_its_size() {
    let got = treemap_rects(&leaves(), 10.0, 20.0, PLOT_W, PLOT_H);
    let total: f64 = WORD.iter().map(|w| w.3).sum();
    for (w, r) in WORD.iter().zip(&got) {
        let area = ((r[2] - r[0]) * (r[3] - r[1])) as f64;
        let want = (PLOT_W * PLOT_H) as f64 * w.3 / total;
        assert!((area / want - 1.0).abs() < 0.01, "{}: area {area}, want {want} ({r:?})", w.2);
        assert!(r[0] >= 10.0 - 0.01 && r[1] >= 20.0 - 0.01 && r[2] <= 10.0 + PLOT_W + 0.01 && r[3] <= 20.0 + PLOT_H + 0.01, "{} inside: {r:?}", w.2);
    }
}

#[test]
fn leaves_tile_as_word_tiles_them() {
    let got = treemap_rects(&leaves(), 0.0, 0.0, PLOT_W, PLOT_H);
    let bad: Vec<String> =
        WORD.iter().zip(&got).filter(|(w, r)| !near(**r, w.4, 0.5)).map(|(w, r)| format!("{} got {r:?} want {:?}", w.2, w.4)).collect();
    assert!(bad.is_empty(), "tiles differ from Word's:\n{}", bad.join("\n"));
}

#[test]
fn zero_and_negative_sizes_take_no_area() {
    let ls = vec![(vec!["A".to_string()], 3.0), (vec!["B".to_string()], 0.0), (vec!["C".to_string()], -2.0), (vec!["D".to_string()], 1.0)];
    let got = treemap_rects(&ls, 0.0, 0.0, 400.0, 100.0);
    assert_eq!(got.len(), 4);
    for k in [1, 2] {
        let r = got[k];
        assert!((r[2] - r[0]) * (r[3] - r[1]) < 1e-3, "leaf {k} has no area: {r:?}");
    }
    let a = (got[0][2] - got[0][0]) * (got[0][3] - got[0][1]);
    let d = (got[3][2] - got[3][0]) * (got[3][3] - got[3][1]);
    assert!((a - 30000.0).abs() < 1.0 && (d - 10000.0).abs() < 1.0, "A {a}, D {d}");
}

/// The chartSpace of the measured chart: `cx:lvl`s innermost first, sizes in `numDim type="size"`.
fn space() -> El {
    let mut lvls = String::new();
    for k in [2, 1, 0] {
        lvls.push_str(r#"<cx:lvl ptCount="16">"#);
        for (i, w) in WORD.iter().enumerate() {
            let name = [w.0, w.1, w.2][k];
            lvls.push_str(&format!(r#"<cx:pt idx="{i}">{name}</cx:pt>"#));
        }
        lvls.push_str("</cx:lvl>");
    }
    let vals: String = WORD.iter().enumerate().map(|(i, w)| format!(r#"<cx:pt idx="{i}">{}</cx:pt>"#, w.3)).collect();
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData><cx:data id="0"><cx:strDim type="cat">{lvls}</cx:strDim><cx:numDim type="size"><cx:lvl ptCount="16" formatCode="General">{vals}</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:title pos="t" align="ctr" overlay="0"/><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="treemap"><cx:tx><cx:txData><cx:v>Series1</cx:v></cx:txData></cx:tx><cx:dataLabels pos="inEnd"><cx:visibility seriesName="0" categoryName="1" value="0"/></cx:dataLabels><cx:dataId val="0"/><cx:layoutPr><cx:parentLabelLayout val="overlapping"/></cx:layoutPr></cx:series></cx:plotAreaRegion></cx:plotArea><cx:legend pos="t" align="ctr" overlay="0"/></cx:chart></cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

#[test]
fn treemap_is_drawn_with_one_tile_per_leaf_in_group_colours() {
    let items = chartex_items(&space(), &THEME_COLORS, 432.0, 252.0).expect("treemap is drawn, not the fallback picture");
    // Leaf tiles: filled shapes taller and wider than a legend marker (7.2 pt).
    let tiles: Vec<Rgb> = items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Shape { rect, fill: Some(c), .. } if rect[2] > 8.0 && rect[3] > 8.0 && rect[2] < 400.0 => Some(*c),
            _ => None,
        })
        .collect();
    for (k, accent) in [THEME_COLORS[4], THEME_COLORS[5], THEME_COLORS[6]].iter().enumerate() {
        let want = WORD.iter().filter(|w| w.0 == format!("Branch {}", k + 1)).count();
        let n = tiles.iter().filter(|c| *c == accent).count();
        assert_eq!(n, want, "Branch {} tiles in accent {}: {tiles:?}", k + 1, k + 1);
    }
    let texts: Vec<&str> = items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for t in ["Branch 1", "Branch 2", "Branch 3", "Leaf 5", "Leaf 11", "Leaf 14"] {
        assert!(texts.contains(&t), "label {t} in {texts:?}");
    }
}
