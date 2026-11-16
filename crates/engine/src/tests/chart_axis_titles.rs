//! Chart Design › Add Chart Element › Axis Titles and Gridlines for WordCraft's own charts
//! (`chart.axisTitle`, `chart.gridlines`), drawn, saved to .docx (`c:catAx`/`c:valAx`) and read
//! back as the same editable chart.

use super::*;
use wordcraft_doc::graphic::GraphicItem;
use wordcraft_doc::para::InlineObject;

fn chart_json(doc: &wordcraft_doc::Document) -> serde_json::Value {
    let p = doc.para_at(&Pos::body(0, 0)).expect("a paragraph");
    p.object_offsets()
        .into_iter()
        .find_map(|off| match p.object_at(off) {
            Some(InlineObject::Graphic { graphic, .. }) => graphic.chart.as_deref().map(|c| serde_json::to_value(c).unwrap()),
            _ => None,
        })
        .expect("a chart")
}

fn items(doc: &wordcraft_doc::Document) -> Vec<GraphicItem> {
    let p = doc.para_at(&Pos::body(0, 0)).unwrap();
    p.object_offsets()
        .into_iter()
        .find_map(|off| match p.object_at(off) {
            Some(InlineObject::Graphic { graphic, .. }) if graphic.chart.is_some() => Some(graphic.items.clone()),
            _ => None,
        })
        .expect("a chart")
}

/// The drawn text item holding exactly `t`, as JSON (so the test reads its `rotation`).
fn text_item(doc: &wordcraft_doc::Document, t: &str) -> Option<serde_json::Value> {
    items(doc).into_iter().find(|i| matches!(i, GraphicItem::Text { text, .. } if text.trim() == t)).map(|i| serde_json::to_value(i).unwrap())
}

fn rotation(v: &serde_json::Value) -> f64 {
    fn find(v: &serde_json::Value) -> Option<f64> {
        match v {
            serde_json::Value::Object(m) => m.get("rotation").and_then(|r| r.as_f64()).or_else(|| m.values().find_map(find)),
            _ => None,
        }
    }
    find(v).unwrap_or(0.0)
}

/// Horizontal gridline-like strokes: two-point paths with equal y, counted.
fn horizontal_lines(doc: &wordcraft_doc::Document) -> usize {
    use wordcraft_doc::graphic::PathSeg;
    items(doc)
        .iter()
        .filter(|i| match i {
            GraphicItem::Path { segs, fill: None, .. } => {
                let pts: Vec<(f32, f32)> = segs
                    .iter()
                    .filter_map(|s| match s {
                        PathSeg::Move(x, y) | PathSeg::Line(x, y) => Some((*x, *y)),
                        _ => None,
                    })
                    .collect();
                pts.len() == 2 && (pts[0].1 - pts[1].1).abs() < 0.01 && (pts[0].0 - pts[1].0).abs() > 1.0
            }
            _ => false,
        })
        .count()
}

fn chart(kind: &str) -> Session {
    let mut s = s();
    let data = json!({"categories": ["A", "B", "C"], "series": [{"name": "Sales", "values": [3, 17, 37]}]});
    run(&mut s, "insert.chart", json!({"type": kind, "data": data, "legend": "none"}));
    s
}

const NO_TITLES: fn() -> serde_json::Value = || json!({"horizontal": null, "vertical": null});

#[test]
fn axis_title_toggles_word_s_placeholder_and_undoes_in_one_step() {
    let mut s = chart("column");
    assert_eq!(run(&mut s, "chart.axisTitle", json!({"axis": "vertical"})), json!({"horizontal": null, "vertical": "Axis Title"}));
    assert!(text_item(&s.doc, "Axis Title").is_some(), "the placeholder title is drawn");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": null})), NO_TITLES());
    run(&mut s, "chart.axisTitle", json!({"axis": "vertical"}));
    assert_eq!(run(&mut s, "chart.axisTitle", json!({"axis": "vertical"})), NO_TITLES(), "toggling again removes it");
}

#[test]
fn a_set_title_is_kept_trimmed_to_255_characters_and_empty_removes_it() {
    let mut s = chart("column");
    let r = run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": "Quarter"}));
    assert_eq!(r, json!({"horizontal": "Quarter", "vertical": null}));
    let long = "x".repeat(300);
    let r = run(&mut s, "chart.axisTitle", json!({"axis": "vertical", "text": long}));
    assert_eq!(r["vertical"].as_str().unwrap().chars().count(), 255);
    let r = run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": "   "}));
    assert_eq!(r["horizontal"], serde_json::Value::Null);
}

#[test]
fn the_vertical_title_is_turned_and_takes_room_from_the_plot_area() {
    let mut s = chart("column");
    let plain = horizontal_lines(&s.doc);
    run(&mut s, "chart.axisTitle", json!({"axis": "vertical", "text": "Revenue"}));
    run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": "Quarter"}));
    let v = text_item(&s.doc, "Revenue").expect("vertical title drawn");
    assert_eq!(rotation(&v), 270.0, "reads bottom to top: {v}");
    let h = text_item(&s.doc, "Quarter").expect("horizontal title drawn");
    assert_eq!(rotation(&h), 0.0);
    assert_eq!(horizontal_lines(&s.doc), plain, "titles add no gridlines");
    // The vertical title sits left of every tick label.
    let rect_x = |v: &serde_json::Value| -> f64 {
        fn find(v: &serde_json::Value) -> Option<f64> {
            match v {
                serde_json::Value::Object(m) => m.get("rect").and_then(|r| r[0].as_f64()).or_else(|| m.values().find_map(find)),
                _ => None,
            }
        }
        find(v).unwrap()
    };
    let title_x = rect_x(&v);
    for t in ["0", "10", "20", "30", "40"] {
        let tick = text_item(&s.doc, t).unwrap_or_else(|| panic!("tick {t}"));
        assert!(title_x < rect_x(&tick), "title left of tick label {t}");
    }
}

#[test]
fn gridlines_toggle_and_minor_ones_add_lines() {
    let mut s = chart("column");
    let major = horizontal_lines(&s.doc);
    let r = run(&mut s, "chart.gridlines", json!({"kind": "minorHorizontal"}));
    assert_eq!(r, json!({"majorHorizontal": true, "majorVertical": false, "minorHorizontal": true, "minorVertical": false}));
    assert!(horizontal_lines(&s.doc) > major, "minor gridlines drawn");
    let r = run(&mut s, "chart.gridlines", json!({"kind": "majorHorizontal", "on": false}));
    assert_eq!(r["majorHorizontal"], json!(false));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "chart.gridlines", json!({"kind": "majorVertical", "on": false}))["majorHorizontal"], json!(true));
}

#[test]
fn bad_parameters_and_pie_charts_change_nothing() {
    let mut s = chart("column");
    let before = chart_json(&s.doc);
    for (id, v) in [
        ("chart.axisTitle", json!({"axis": "diagonal"})),
        ("chart.axisTitle", json!({"axis": "vertical", "text": 5})),
        ("chart.gridlines", json!({"kind": "major"})),
        ("chart.gridlines", json!({"kind": "minorVertical", "on": "yes"})),
    ] {
        assert!(s.run(id, &v).is_err(), "{id} {v}");
    }
    assert_eq!(chart_json(&s.doc), before);
    let mut p = chart("pie");
    let before = chart_json(&p.doc);
    let e = p.run("chart.axisTitle", &json!({"axis": "vertical"})).unwrap_err().to_string();
    assert!(e.contains("no axes"), "{e}");
    let e = p.run("chart.gridlines", &json!({"kind": "majorHorizontal"})).unwrap_err().to_string();
    assert!(e.contains("no axes"), "{e}");
    assert_eq!(chart_json(&p.doc), before);
}

#[test]
fn an_old_spec_without_the_new_keys_keeps_today_s_look() {
    let mut s = chart("column");
    let mut old = chart_json(&s.doc);
    let m = old.as_object_mut().unwrap();
    m.remove("axisTitles");
    m.remove("gridlines");
    let spec: wordcraft_doc::chart::ChartSpec = serde_json::from_value(old).unwrap();
    let back = serde_json::to_value(&spec).unwrap();
    assert_eq!(back["gridlines"], json!({"majorHorizontal": true, "majorVertical": false, "minorHorizontal": false, "minorVertical": false}));
    assert_eq!(back["axisTitles"], NO_TITLES());
    assert_eq!(run(&mut s, "chart.axisTitle", json!({"axis": "vertical", "text": null})), NO_TITLES());
}

#[test]
fn titles_and_gridlines_are_saved_in_schema_order_on_the_right_axis() {
    let mut s = chart("column");
    run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": "Quarter & year"}));
    run(&mut s, "chart.axisTitle", json!({"axis": "vertical", "text": "Revenue"}));
    run(&mut s, "chart.gridlines", json!({"kind": "minorHorizontal", "on": true}));
    let spec: wordcraft_doc::chart::ChartSpec = serde_json::from_value(chart_json(&s.doc)).unwrap();
    let xml = wordcraft_docx::chart_xml(&spec);
    let cat = &xml[xml.find("<c:catAx>").unwrap()..xml.find("</c:catAx>").unwrap()];
    let val = &xml[xml.find("<c:valAx>").unwrap()..xml.find("</c:valAx>").unwrap()];
    assert!(cat.contains("Quarter &amp; year"), "{cat}");
    assert!(!cat.contains("Gridlines"), "column chart: horizontal gridlines belong to the value axis: {cat}");
    assert!(val.contains("Revenue") && val.contains(r#"rot="-5400000""#), "{val}");
    let at = |ax: &str, n: &str| ax.find(n).unwrap_or_else(|| panic!("{n} in {ax}"));
    assert!(at(val, "<c:axPos") < at(val, "<c:majorGridlines"));
    assert!(at(val, "<c:majorGridlines") < at(val, "<c:minorGridlines"));
    assert!(at(val, "<c:minorGridlines") < at(val, "<c:title>"));
    assert!(at(val, "<c:title>") < at(val, "<c:majorTickMark"));
    assert!(at(cat, "<c:axPos") < at(cat, "<c:title>") && at(cat, "<c:title>") < at(cat, "<c:majorTickMark"));
}

#[test]
fn bar_charts_put_vertical_gridlines_on_the_value_axis() {
    let mut s = chart("bar");
    run(&mut s, "chart.gridlines", json!({"kind": "majorVertical", "on": true}));
    run(&mut s, "chart.gridlines", json!({"kind": "majorHorizontal", "on": false}));
    let spec: wordcraft_doc::chart::ChartSpec = serde_json::from_value(chart_json(&s.doc)).unwrap();
    let xml = wordcraft_docx::chart_xml(&spec);
    let val = &xml[xml.find("<c:valAx>").unwrap()..xml.find("</c:valAx>").unwrap()];
    let cat = &xml[xml.find("<c:catAx>").unwrap()..xml.find("</c:catAx>").unwrap()];
    assert!(val.contains("<c:majorGridlines"), "{val}");
    assert!(!cat.contains("<c:majorGridlines"), "{cat}");
}

#[test]
fn titles_and_gridlines_read_back_as_the_same_editable_chart() {
    for kind in ["column", "bar", "line", "area", "scatter"] {
        let mut s = chart(kind);
        run(&mut s, "chart.axisTitle", json!({"axis": "horizontal", "text": "X"}));
        run(&mut s, "chart.axisTitle", json!({"axis": "vertical", "text": "Y"}));
        for k in ["majorVertical", "minorHorizontal", "minorVertical"] {
            run(&mut s, "chart.gridlines", json!({"kind": k, "on": true}));
        }
        let spec = chart_json(&s.doc);
        let bytes = wordcraft_docx::write(&s.doc).unwrap();
        let back = wordcraft_docx::read(&bytes).unwrap();
        assert_eq!(chart_json(&back), spec, "{kind}: same editable chart after save and open");
        assert!(text_item(&back, "Y").is_some(), "{kind}: the title is drawn after open");
    }
}
