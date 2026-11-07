//! 3-D bar, line, area and pie charts and pie-of-pie / bar-of-pie charts (ECMA-376 Part 1
//! §21.2.2.16 bar3DChart, §21.2.2.96 line3DChart, §21.2.2.4 area3DChart, §21.2.2.140 pie3DChart,
//! §21.2.2.126 ofPieChart, §21.2.2.228 view3D) are drawn from their cached data, not a placeholder.

use super::*;
use wordcraft_doc::THEME_COLORS;

const NS: &str =
    r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

const CATS: [&str; 3] = ["Q1", "Q2", "Q3"];
const BLUE: Rgb = Rgb(0x44, 0x72, 0xC4);
const ORANGE: Rgb = Rgb(0xED, 0x7D, 0x31);

fn draw(view3d: &str, plot: &str) -> Vec<GraphicItem> {
    let x = format!(
        r#"<c:chartSpace {NS}><c:chart>{view3d}<c:plotArea>{plot}</c:plotArea><c:legend><c:legendPos val="r"/></c:legend></c:chart></c:chartSpace>"#
    );
    let space = crate::xml::parse(x.as_bytes()).unwrap();
    chart_items(&space, &THEME_COLORS, 400.0, 300.0)
}

/// What Word writes for a default 3-D chart (the corpus charts carry only perspective 30).
const VIEW_DEFAULT: &str = r#"<c:view3D><c:perspective val="30"/></c:view3D>"#;
const VIEW_FLAT: &str = r#"<c:view3D><c:rotX val="0"/><c:rotY val="0"/><c:rAngAx val="1"/></c:view3D>"#;

fn ser(idx: usize, name: &str, cats: &[&str], vals: &[f64], fill: Option<&str>) -> String {
    let cat_pts: String = cats.iter().enumerate().map(|(k, c)| format!(r#"<c:pt idx="{k}"><c:v>{c}</c:v></c:pt>"#)).collect();
    let val_pts: String = vals.iter().enumerate().map(|(k, v)| format!(r#"<c:pt idx="{k}"><c:v>{v}</c:v></c:pt>"#)).collect();
    let sppr = fill.map(|h| format!(r#"<c:spPr><a:solidFill><a:srgbClr val="{h}"/></a:solidFill></c:spPr>"#)).unwrap_or_default();
    format!(
        r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/><c:tx><c:strRef><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>{sppr}<c:cat><c:strRef><c:strCache><c:ptCount val="{nc}"/>{cat_pts}</c:strCache></c:strRef></c:cat><c:val><c:numRef><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{nv}"/>{val_pts}</c:numCache></c:numRef></c:val></c:ser>"#,
        nc = cats.len(),
        nv = vals.len()
    )
}

fn two_series() -> String {
    format!("{}{}", ser(0, "North", &CATS, &[10.0, 20.0, 30.0], Some("4472C4")), ser(1, "South", &CATS, &[15.0, 5.0, 25.0], Some("ED7D31")))
}

const AXES: &str = r#"<c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/><c:crossAx val="1"/></c:valAx><c:serAx><c:axId val="3"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:serAx>"#;

fn bar3d(dir: &str, grouping: &str) -> String {
    format!(
        r#"<c:bar3DChart><c:barDir val="{dir}"/><c:grouping val="{grouping}"/><c:varyColors val="0"/>{}<c:gapWidth val="150"/><c:shape val="box"/><c:axId val="1"/><c:axId val="2"/><c:axId val="3"/></c:bar3DChart>{AXES}"#,
        two_series()
    )
}

fn fill_colours(items: &[GraphicItem]) -> Vec<Rgb> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Path { fill: Some(c), .. } => Some(*c),
            _ => None,
        })
        .collect()
}

fn texts(items: &[GraphicItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|it| match it {
            GraphicItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn no_placeholder(items: &[GraphicItem]) {
    let t = texts(items);
    assert!(!t.iter().any(|s| s.contains("not supported")), "drawn, not a placeholder: {t:?}");
}

/// Same hue darker: every channel at most the front colour's, and not equal to it.
fn darker(c: Rgb, of: Rgb) -> bool {
    c != of && c.0 <= of.0 && c.1 <= of.1 && c.2 <= of.2
}

fn faces(items: &[GraphicItem], front: Rgb) -> (usize, usize) {
    let fills = fill_colours(items);
    (fills.iter().filter(|c| **c == front).count(), fills.iter().filter(|c| darker(**c, front)).count())
}

#[test]
fn clustered_3d_columns_draw_a_front_top_and_side_per_bar() {
    let items = draw(VIEW_DEFAULT, &bar3d("col", "clustered"));
    no_placeholder(&items);
    for colour in [BLUE, ORANGE] {
        let (front, shaded) = faces(&items, colour);
        // One front face per point in the series colour (the legend swatch adds at most one).
        assert!(front == 3 || front == 4, "{colour:?}: {front} front faces");
        assert!(shaded >= 6, "{colour:?}: a darker top and side per bar, got {shaded}");
    }
    let t = texts(&items);
    for s in ["North", "South", "Q1", "Q2", "Q3"] {
        assert!(t.iter().any(|x| x == s), "{s} in {t:?}");
    }
}

#[test]
fn stacked_3d_bars_draw_every_point() {
    let items = draw(VIEW_DEFAULT, &bar3d("bar", "stacked"));
    no_placeholder(&items);
    for colour in [BLUE, ORANGE] {
        let (front, shaded) = faces(&items, colour);
        assert!(front == 3 || front == 4, "{colour:?}: {front} front faces");
        assert!(shaded >= 3, "{colour:?}: shaded faces, got {shaded}");
    }
}

#[test]
fn zero_rotation_still_shows_depth() {
    let items = draw(VIEW_FLAT, &bar3d("col", "clustered"));
    no_placeholder(&items);
    let (_, shaded) = faces(&items, BLUE);
    assert!(shaded >= 3, "rotX = rotY = 0 still draws some depth: {shaded}");
}

#[test]
fn line_and_stacked_area_3d_charts_draw_their_series() {
    let line = format!(
        r#"<c:line3DChart><c:grouping val="standard"/><c:varyColors val="0"/>{}<c:gapDepth val="150"/><c:axId val="1"/><c:axId val="2"/><c:axId val="3"/></c:line3DChart>{AXES}"#,
        two_series()
    );
    let area = format!(
        r#"<c:area3DChart><c:grouping val="stacked"/><c:varyColors val="0"/>{}<c:axId val="1"/><c:axId val="2"/><c:axId val="3"/></c:area3DChart>{AXES}"#,
        two_series()
    );
    for plot in [line, area] {
        let items = draw(VIEW_DEFAULT, &plot);
        no_placeholder(&items);
        let fills = fill_colours(&items);
        for colour in [BLUE, ORANGE] {
            // Ribbons and areas are filled in the series colour (front) and a darker shade (depth).
            assert!(fills.iter().any(|c| *c == colour), "{colour:?} in {fills:?}");
            assert!(fills.iter().any(|c| darker(*c, colour)), "{colour:?} shaded in {fills:?}");
        }
    }
}

#[test]
fn pie_3d_draws_a_slice_per_point() {
    let plot =
        format!(r#"<c:pie3DChart><c:varyColors val="1"/>{}</c:pie3DChart>"#, ser(0, "Share", &["A", "B", "C", "D"], &[40.0, 30.0, 20.0, 10.0], None));
    let items = draw(r#"<c:view3D><c:rotX val="30"/><c:rotY val="0"/><c:rAngAx val="0"/></c:view3D>"#, &plot);
    no_placeholder(&items);
    let fills = fill_colours(&items);
    // varyColors: accents 1..4 of the theme, one per slice, each with a darker side wall.
    for k in 0..4 {
        let accent = THEME_COLORS[4 + k];
        assert!(fills.iter().any(|c| *c == accent), "slice {k} {accent:?} in {fills:?}");
    }
    assert!(THEME_COLORS[4..8].iter().any(|a| fills.iter().any(|c| darker(*c, *a))), "a visible side wall: {fills:?}");
}

/// Which points go to the second plot of a pie-of-pie / bar-of-pie (§21.2.2.191 splitType,
/// §21.2.2.190 splitPos, §21.2.2.35 custSplit).
#[test]
fn of_pie_split_rules() {
    let vals = [50.0, 30.0, 5.0, 8.0, 7.0];
    // pos: the last splitPos points.
    assert_eq!(of_pie_second(&vals, "pos", Some(2.0), &[]), vec![3, 4]);
    // val: the points below splitPos.
    assert_eq!(of_pie_second(&vals, "val", Some(10.0), &[]), vec![2, 3, 4]);
    // percent: the points below splitPos percent of the total (100 here).
    assert_eq!(of_pie_second(&vals, "percent", Some(7.5), &[]), vec![2, 4]);
    // cust: the c:secondPiePt indices.
    assert_eq!(of_pie_second(&vals, "cust", None, &[1, 3]), vec![1, 3]);
    // Never every point: the first pie keeps at least one.
    assert_eq!(of_pie_second(&vals, "pos", Some(9.0), &[]), vec![1, 2, 3, 4]);
}

fn of_pie(kind: &str) -> String {
    format!(
        r#"<c:ofPieChart><c:ofPieType val="{kind}"/><c:varyColors val="1"/>{}<c:gapWidth val="150"/><c:splitType val="pos"/><c:splitPos val="2"/><c:secondPieSize val="75"/><c:serLines/></c:ofPieChart>"#,
        ser(0, "Share", &["A", "B", "C", "D", "E"], &[50.0, 30.0, 5.0, 8.0, 7.0], None)
    )
}

#[test]
fn pie_of_pie_and_bar_of_pie_draw_both_plots() {
    for kind in ["pie", "bar"] {
        let items = draw("", &of_pie(kind));
        no_placeholder(&items);
        let paths = items.iter().filter(|it| matches!(it, GraphicItem::Path { fill: Some(_), .. })).count();
        // First pie: A, B, C and Other (4); second plot: D and E (2); the legend may add swatches.
        assert!(paths >= 6, "{kind}: {paths} filled paths");
        let lines = items.iter().filter(|it| matches!(it, GraphicItem::Path { fill: None, stroke: Some(_), .. })).count();
        assert!(lines >= 2, "{kind}: two series lines join the plots, got {lines}");
    }
}
