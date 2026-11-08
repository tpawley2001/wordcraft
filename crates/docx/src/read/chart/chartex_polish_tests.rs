//! Office 2016 charts (`cx:chartSpace`) as Word draws them: a regular-weight title, a light grey
//! border round the chart when the chart space sets no line, and a value axis of about ten
//! intervals in steps of 1, 2 or 5 x 10^k (measured from Word's PDF of a waterfall chart).

use super::chartex::chartex_items;
use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str =
    r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;
const W: f32 = 400.0;
const H: f32 = 300.0;

/// A waterfall chart whose running sum peaks at 170, with an empty title; `sppr` goes in the
/// chart space.
fn waterfall(sppr: &str) -> El {
    let vals = [100.0, 20.0, 50.0, -40.0, 130.0, -60.0, 70.0, 140.0];
    let pts: String = vals.iter().enumerate().map(|(k, v)| format!(r#"<cx:pt idx="{k}">{v}</cx:pt>"#)).collect();
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="8" formatCode="General">{pts}</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:title/><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/><cx:layoutPr><cx:subtotals><cx:idx val="0"/><cx:idx val="4"/><cx:idx val="7"/></cx:subtotals></cx:layoutPr></cx:series></cx:plotAreaRegion><cx:axis id="0"><cx:catScaling gapWidth="0.5"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea></cx:chart>{sppr}</cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

/// A box & whisker chart of values -78..128 (no outliers), no categories.
fn box_whisker() -> El {
    let vals = [-78, -10, 0, 5, 10, 128];
    let pts: String = vals.iter().enumerate().map(|(k, v)| format!(r#"<cx:pt idx="{k}">{v}</cx:pt>"#)).collect();
    let x = format!(
        r#"<cx:chartSpace {NS}><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="6" formatCode="General">{pts}</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="boxWhisker"><cx:dataId val="0"/><cx:layoutPr><cx:visibility meanLine="0" meanMarker="1" nonoutliers="0" outliers="1"/><cx:statistics quartileMethod="exclusive"/></cx:layoutPr></cx:series></cx:plotAreaRegion><cx:axis id="0"><cx:catScaling gapWidth="1"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea></cx:chart></cx:chartSpace>"#
    );
    crate::xml::parse(x.as_bytes()).unwrap()
}

fn texts(items: &[GraphicItem]) -> Vec<(String, f32, bool, Rgb)> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Text { text, size, bold, color, .. } => Some((text.clone(), *size, *bold, *color)),
            _ => None,
        })
        .collect()
}

/// Stroke-only outlines (paths or shapes) as (colour, width, [x0, y0, x1, y1]).
fn outlines(items: &[GraphicItem]) -> Vec<(Rgb, f32, [f32; 4])> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Shape { rect, fill: None, stroke: Some(c), stroke_width, .. } => {
                Some((*c, *stroke_width, [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]]))
            }
            GraphicItem::Path { segs, fill: None, stroke: Some(c), stroke_width } => {
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
                Some((*c, *stroke_width, [x0, y0, x1, y1]))
            }
            _ => None,
        })
        .collect()
}

/// Outlines round the whole chart frame (within a point of [0, 0, W, H]).
fn frame_borders(items: &[GraphicItem]) -> Vec<(Rgb, f32)> {
    outlines(items)
        .into_iter()
        .filter(|(_, _, b)| b[0].abs() <= 1.0 && b[1].abs() <= 1.0 && (b[2] - W).abs() <= 1.0 && (b[3] - H).abs() <= 1.0)
        .map(|(c, w, _)| (c, w))
        .collect()
}

#[test]
fn chart_title_is_regular_weight_14_pt_grey() {
    let items = chartex_items(&waterfall(""), &THEME_COLORS, W, H).expect("waterfall is drawn");
    let t = texts(&items);
    let title = t.iter().find(|x| x.0 == "Chart Title").unwrap_or_else(|| panic!("title in {t:?}"));
    assert!(!title.2, "title not bold: {title:?}");
    assert!((title.1 - 14.0).abs() < 0.01, "title 14 pt: {title:?}");
    assert_eq!(title.3, Rgb(0x59, 0x59, 0x59), "title colour: {title:?}");
}

#[test]
fn chart_space_without_a_line_gets_a_light_grey_border() {
    for sppr in ["", r#"<cx:spPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></cx:spPr>"#] {
        let items = chartex_items(&waterfall(sppr), &THEME_COLORS, W, H).expect("waterfall is drawn");
        let b = frame_borders(&items);
        assert_eq!(b.len(), 1, "one border round the frame with spPr {sppr:?}: {b:?}");
        assert_eq!(b[0].0, Rgb(0xD9, 0xD9, 0xD9), "{b:?}");
        assert!((b[0].1 - 0.75).abs() < 0.01, "{b:?}");
    }
}

#[test]
fn chart_space_line_no_fill_or_colour_is_followed() {
    let none = r#"<cx:spPr><a:ln><a:noFill/></a:ln></cx:spPr>"#;
    let items = chartex_items(&waterfall(none), &THEME_COLORS, W, H).expect("waterfall is drawn");
    assert!(frame_borders(&items).is_empty(), "no border with a:ln/a:noFill: {:?}", frame_borders(&items));

    let red = r#"<cx:spPr><a:ln w="25400"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln></cx:spPr>"#;
    let items = chartex_items(&waterfall(red), &THEME_COLORS, W, H).expect("waterfall is drawn");
    let b = frame_borders(&items);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, Rgb(0xFF, 0x00, 0x00), "{b:?}");
    assert!((b[0].1 - 2.0).abs() < 0.01, "25400 EMU = 2 pt: {b:?}");
}

#[test]
fn waterfall_value_axis_runs_0_to_180_in_steps_of_20() {
    let items = chartex_items(&waterfall(""), &THEME_COLORS, W, H).expect("waterfall is drawn");
    let t: Vec<String> = texts(&items).into_iter().map(|x| x.0).collect();
    for want in ["0", "20", "40", "60", "80", "100", "120", "140", "160", "180"] {
        assert!(t.iter().any(|s| s == want), "tick {want} in {t:?}");
    }
    for not in ["25", "200"] {
        assert!(!t.iter().any(|s| s == not), "no tick {not} in {t:?}");
    }
}

#[test]
fn box_whisker_value_axis_runs_minus_100_to_150_in_steps_of_50() {
    let items = chartex_items(&box_whisker(), &THEME_COLORS, W, H).expect("box & whisker is drawn");
    let t: Vec<String> = texts(&items).into_iter().map(|x| x.0).collect();
    for want in ["-100", "-50", "0", "50", "100", "150"] {
        assert!(t.iter().any(|s| s == want), "tick {want} in {t:?}");
    }
    assert!(!t.iter().any(|s| s == "200" || s == "-150"), "{t:?}");
}
