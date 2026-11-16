//! Axis titles (`c:title` in `c:catAx` / `c:valAx`) and major and minor gridlines of charts from
//! Word.

use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str =
    r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

const SER: &str = r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:cat><c:strLit><c:ptCount val="3"/><c:pt idx="0"><c:v>A</c:v></c:pt><c:pt idx="1"><c:v>B</c:v></c:pt><c:pt idx="2"><c:v>C</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:ptCount val="3"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>17</c:v></c:pt><c:pt idx="2"><c:v>37</c:v></c:pt></c:numLit></c:val></c:ser>"#;

/// A column chart (`bar` = bars across) with `cat` and `val` inside its category and value axes.
fn chart(bar: bool, cat: &str, val: &str) -> Vec<GraphicItem> {
    let (dir, cpos, vpos) = if bar { ("bar", "l", "b") } else { ("col", "b", "l") };
    let x = format!(
        r#"<c:chartSpace {NS}><c:chart><c:plotArea><c:barChart><c:barDir val="{dir}"/><c:grouping val="clustered"/>{SER}<c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:delete val="0"/><c:axPos val="{cpos}"/>{cat}<c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:delete val="0"/><c:axPos val="{vpos}"/>{val}<c:crossAx val="1"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#
    );
    chart_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, 400.0, 250.0)
}

/// Two-point unfilled strokes: (horizontal, colour, width) of each.
fn lines(items: &[GraphicItem]) -> Vec<(bool, Rgb, f32)> {
    items
        .iter()
        .filter_map(|i| match i {
            GraphicItem::Path { segs, fill: None, stroke: Some(c), stroke_width } => match segs.as_slice() {
                [PathSeg::Move(x0, y0), PathSeg::Line(x1, y1)] if (x0 - x1).abs() > 1.0 || (y0 - y1).abs() > 1.0 => {
                    Some(((y0 - y1).abs() < 0.01, *c, *stroke_width))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn count(items: &[GraphicItem], horizontal: bool, color: Rgb) -> usize {
    lines(items).iter().filter(|(h, c, _)| *h == horizontal && *c == color).count()
}

fn title<'a>(items: &'a [GraphicItem], t: &str) -> Option<(&'a [f32; 4], f32, f32, bool, f32)> {
    items.iter().find_map(|i| match i {
        GraphicItem::Text { rect, text, size, bold, rotation, .. } if text == t => Some((rect, *size, *rotation, *bold, rect[0])),
        _ => None,
    })
}

/// Left edge of the leftmost bar.
fn bars_left(items: &[GraphicItem]) -> f32 {
    items
        .iter()
        .filter_map(|i| match i {
            GraphicItem::Path { segs, fill: Some(_), .. } => segs.iter().find_map(|s| match s {
                PathSeg::Move(x, _) => Some(*x),
                _ => None,
            }),
            _ => None,
        })
        .fold(f32::INFINITY, f32::min)
}

const REVENUE: &str = r#"<c:title><c:tx><c:rich><a:bodyPr rot="-5400000" vert="horz"/><a:p><a:r><a:rPr sz="1200" b="1"/><a:t>Revenue</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title>"#;

#[test]
fn a_value_axis_title_is_drawn_turned_in_its_own_size() {
    let items = chart(false, "", &format!("<c:majorGridlines/>{REVENUE}"));
    let (_, size, rotation, bold, _) = title(&items, "Revenue").expect("the title is drawn");
    assert_eq!((size, rotation, bold), (12.0, 270.0, true));
    // Category axis titles under a column chart read across, in the default look.
    let items = chart(false, r#"<c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>Quarter</a:t></a:r></a:p></c:rich></c:tx></c:title>"#, "");
    let (_, size, rotation, bold, _) = title(&items, "Quarter").expect("the title is drawn");
    assert_eq!((size, rotation, bold), (10.0, 0.0, false));
}

#[test]
fn a_title_without_text_is_word_s_placeholder() {
    let items = chart(false, "<c:title/>", "");
    assert!(title(&items, "Axis Title").is_some());
}

#[test]
fn a_vertical_title_takes_room_from_the_plot_area() {
    let plain = chart(false, "", "<c:majorGridlines/>");
    let titled = chart(false, "", &format!("<c:majorGridlines/>{REVENUE}"));
    assert!(bars_left(&titled) > bars_left(&plain) + 10.0, "{} vs {}", bars_left(&titled), bars_left(&plain));
    let (rect, ..) = title(&titled, "Revenue").unwrap();
    // The turned line's centre sits left of the tick labels.
    let tick = titled.iter().find_map(|i| match i {
        GraphicItem::Text { rect, text, .. } if text == "40" => Some(rect[0]),
        _ => None,
    });
    assert!(rect[0] + rect[2] / 2.0 < tick.unwrap());
}

#[test]
fn minor_gridlines_add_lines_between_the_major_ones() {
    let major = chart(false, "", "<c:majorGridlines/>");
    let both = chart(false, "", "<c:majorGridlines/><c:minorGridlines/>");
    let (m, b) = (count(&major, true, GRID), count(&both, true, GRID));
    let minor = count(&both, true, Rgb(0xF2, 0xF2, 0xF2));
    // Scale 0..40 by 10: 5 major lines, 4 minor between each pair.
    assert_eq!((m, b, minor), (5, 5, 16));
}

#[test]
fn category_gridlines_sit_at_the_category_boundaries_and_take_their_line() {
    let items = chart(
        false,
        r#"<c:majorGridlines><c:spPr><a:ln w="25400"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln></c:spPr></c:majorGridlines><c:minorGridlines/>"#,
        "",
    );
    let red: Vec<_> = lines(&items).into_iter().filter(|(_, c, _)| *c == Rgb(0xFF, 0, 0)).collect();
    assert_eq!(red.len(), 4, "3 categories, 4 boundaries");
    assert!(red.iter().all(|(h, _, w)| !h && *w == 2.0));
    assert_eq!(count(&items, false, Rgb(0xF2, 0xF2, 0xF2)), 3, "one minor line per category centre");
    // Bars across: the category axis is vertical, its gridlines horizontal.
    let items = chart(true, "<c:majorGridlines/>", "");
    assert_eq!(count(&items, true, GRID), 4);
}

#[test]
fn a_deleted_axis_keeps_its_gridlines_and_title() {
    let x = format!(
        r#"<c:chartSpace {NS}><c:chart><c:plotArea><c:barChart><c:barDir val="col"/>{SER}<c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/></c:catAx><c:valAx><c:axId val="2"/><c:delete val="1"/><c:majorGridlines/><c:minorGridlines/>{REVENUE}</c:valAx></c:plotArea></c:chart></c:chartSpace>"#
    );
    let items = chart_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, 400.0, 250.0);
    assert_eq!(count(&items, true, Rgb(0xF2, 0xF2, 0xF2)), 16);
    assert!(title(&items, "Revenue").is_some());
    assert!(!items.iter().any(|i| matches!(i, GraphicItem::Text { text, .. } if text == "40")), "no tick labels");
}

#[test]
fn hostile_axis_titles_and_gridlines_never_panic() {
    let long = "x".repeat(10_000);
    for (cat, val) in [
        (
            format!("<c:title><c:tx><c:rich><a:p><a:r><a:rPr sz=\"-5\" b=\"maybe\"/><a:t>{long}</a:t></a:r></a:p></c:rich></c:tx></c:title>"),
            String::new(),
        ),
        (String::new(), r#"<c:minorGridlines><c:spPr><a:ln w="-99999999999"/></c:spPr></c:minorGridlines><c:majorUnit val="1e-300"/>"#.to_string()),
        (
            r#"<c:title><c:tx/></c:title><c:majorGridlines/><c:minorGridlines/>"#.to_string(),
            r#"<c:title><c:tx><c:v></c:v></c:tx></c:title>"#.to_string(),
        ),
    ] {
        for bar in [false, true] {
            let _ = chart(bar, &cat, &val);
        }
    }
    // Tiny charts: the titles take at most half the room.
    let x = format!(
        r#"<c:chartSpace {NS}><c:chart><c:plotArea><c:barChart>{SER}<c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/>{REVENUE}</c:catAx><c:valAx><c:axId val="2"/>{REVENUE}</c:valAx></c:plotArea></c:chart></c:chartSpace>"#
    );
    for (w, h) in [(5.0, 5.0), (30.0, 1.0), (1.0, 300.0)] {
        let _ = chart_items(&crate::xml::parse(x.as_bytes()).unwrap(), &THEME_COLORS, w, h);
    }
}
