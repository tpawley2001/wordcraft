//! Chart Design › Add Chart Element › Axes › More Axis Options (Format Axis): the value axis's
//! Minimum, Maximum, Major unit and number format for WordCraft's own charts (`chart.axis`),
//! drawn, saved to .docx (`c:valAx`) and read back as the same editable chart.

use super::*;
use wordcraft_doc::graphic::GraphicItem;
use wordcraft_doc::para::InlineObject;

fn first_chart(doc: &wordcraft_doc::Document) -> Option<wordcraft_doc::chart::ChartSpec> {
    let p = doc.para_at(&Pos::body(0, 0))?;
    p.object_offsets().into_iter().find_map(|off| match p.object_at(off) {
        Some(InlineObject::Graphic { graphic, .. }) => graphic.chart.as_deref().cloned(),
        _ => None,
    })
}

/// The texts of the first chart as drawn that hold a digit: the value axis's tick labels, in
/// ascending order (the test data has no data labels, no legend and letter categories).
fn tick_labels(doc: &wordcraft_doc::Document) -> Vec<String> {
    let p = doc.para_at(&Pos::body(0, 0)).unwrap();
    let items = p
        .object_offsets()
        .into_iter()
        .find_map(|off| match p.object_at(off) {
            Some(InlineObject::Graphic { graphic, .. }) if graphic.chart.is_some() => Some(graphic.items.clone()),
            _ => None,
        })
        .expect("a chart");
    fn walk(items: &[GraphicItem], out: &mut Vec<String>) {
        for i in items {
            match i {
                GraphicItem::Text { text, .. } if text.chars().any(|c| c.is_ascii_digit()) => out.push(text.trim().to_string()),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&items, &mut out);
    let key = |t: &String| t.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-')).collect::<String>().parse::<f64>().unwrap_or(f64::NAN);
    out.sort_by(|a, b| key(a).total_cmp(&key(b)));
    out
}

fn column_chart(values: [f64; 3]) -> Session {
    let mut s = s();
    let data = json!({"categories": ["A", "B", "C"], "series": [{"name": "Sales", "values": values}]});
    run(&mut s, "insert.chart", json!({"type": "column", "data": data, "legend": "none"}));
    s
}

const AUTO: fn() -> serde_json::Value = || json!({"min": null, "max": null, "majorUnit": null, "numberFormat": null});

#[test]
fn without_settings_it_reports_the_automatic_axis() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    assert_eq!(run(&mut s, "chart.axis", json!({})), AUTO());
    // The automatic scale for 3..37 is unchanged (0 to 40 in steps of 10).
    assert_eq!(tick_labels(&s.doc), ["0", "10", "20", "30", "40"]);
}

#[test]
fn minimum_maximum_and_major_unit_set_the_ticks() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    let r = run(&mut s, "chart.axis", json!({"min": 0, "max": 60, "majorUnit": 20}));
    assert_eq!(r, json!({"min": 0.0, "max": 60.0, "majorUnit": 20.0, "numberFormat": null}));
    assert_eq!(tick_labels(&s.doc), ["0", "20", "40", "60"]);
    // A minimum away from zero: the axis starts there.
    run(&mut s, "chart.axis", json!({"min": 10, "max": 40, "majorUnit": 10}));
    assert_eq!(tick_labels(&s.doc), ["10", "20", "30", "40"]);
}

#[test]
fn a_major_unit_alone_rounds_the_automatic_ends_to_it() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    run(&mut s, "chart.axis", json!({"majorUnit": 5}));
    assert_eq!(tick_labels(&s.doc), ["0", "5", "10", "15", "20", "25", "30", "35", "40"]);
    run(&mut s, "chart.axis", json!({"majorUnit": 15}));
    assert_eq!(tick_labels(&s.doc), ["0", "15", "30", "45"]);
    // Only the maximum: the automatic step for 0..100.
    run(&mut s, "chart.axis", json!({"majorUnit": null, "max": 100}));
    assert_eq!(run(&mut s, "chart.axis", json!({})), json!({"min": null, "max": 100.0, "majorUnit": null, "numberFormat": null}));
    assert_eq!(tick_labels(&s.doc), ["0", "20", "40", "60", "80", "100"]);
}

#[test]
fn null_puts_a_setting_back_to_auto_and_unnamed_settings_stay() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    run(&mut s, "chart.axis", json!({"min": 0, "max": 60, "majorUnit": 20}));
    run(&mut s, "chart.axis", json!({"max": null}));
    assert_eq!(run(&mut s, "chart.axis", json!({})), json!({"min": 0.0, "max": null, "majorUnit": 20.0, "numberFormat": null}));
    assert_eq!(tick_labels(&s.doc), ["0", "20", "40"]);
    run(&mut s, "chart.axis", json!({"min": null, "majorUnit": null}));
    assert_eq!(run(&mut s, "chart.axis", json!({})), AUTO());
}

#[test]
fn the_number_format_formats_the_tick_labels() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    run(&mut s, "chart.axis", json!({"min": 0, "max": 40, "majorUnit": 20, "numberFormat": "0.0"}));
    assert_eq!(tick_labels(&s.doc), ["0.0", "20.0", "40.0"]);
    run(&mut s, "chart.axis", json!({"numberFormat": "\"€\"0"}));
    assert_eq!(tick_labels(&s.doc), ["€0", "€20", "€40"]);
    // An empty format or null: General again.
    run(&mut s, "chart.axis", json!({"numberFormat": ""}));
    assert_eq!(run(&mut s, "chart.axis", json!({}))["numberFormat"], json!(null));
    assert_eq!(tick_labels(&s.doc), ["0", "20", "40"]);
}

#[test]
fn bad_settings_are_refused_and_change_nothing() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    run(&mut s, "chart.axis", json!({"min": 0, "max": 60, "majorUnit": 20}));
    let before = first_chart(&s.doc);
    for bad in [
        json!({"min": 60}),                             // min must stay below max
        json!({"max": 0}),                              // max must stay above min
        json!({"min": 5, "max": 5}),                    // equal ends
        json!({"majorUnit": 0}),                        // a step must be > 0
        json!({"majorUnit": -10}),
        json!({"min": "zero"}),                         // not a number
        json!({"majorUnit": true}),
        json!({"min": 0, "max": 1e9, "majorUnit": 1}),  // more than 1000 steps
        json!({"numberFormat": 5}),
        json!({"numberFormat": "0".repeat(256)}),       // longer than 255 characters
    ] {
        assert!(s.run("chart.axis", &bad).is_err(), "{bad} is refused");
        assert_eq!(first_chart(&s.doc), before, "{bad} changes nothing");
    }
}

#[test]
fn pie_and_doughnut_charts_have_no_value_axis() {
    for kind in ["pie", "doughnut"] {
        let mut s = s();
        run(&mut s, "insert.chart", json!({"type": kind}));
        let e = s.run("chart.axis", &json!({"min": 0})).unwrap_err().to_string();
        assert!(e.contains("no value axis"), "{kind}: {e}");
    }
    // Nothing selected: the command is unavailable.
    let mut s = s();
    assert!(s.run("chart.axis", &json!({})).is_err());
}

#[test]
fn the_axis_is_saved_to_docx_and_reads_back_as_the_same_editable_chart() {
    for kind in ["column", "barStacked", "line", "area", "scatter"] {
        let mut s = s();
        let data = json!({"categories": ["1", "2", "3"], "series": [{"name": "S", "values": [3, 17, 37]}]});
        run(&mut s, "insert.chart", json!({"type": kind, "data": data}));
        run(&mut s, "chart.axis", json!({"min": -10, "max": 50, "majorUnit": 5, "numberFormat": "0.0%"}));
        let spec = first_chart(&s.doc).unwrap();
        let bytes = wordcraft_docx::write(&s.doc).unwrap();
        let back = wordcraft_docx::read(&bytes).unwrap();
        assert_eq!(first_chart(&back), Some(spec), "{kind}: same editable chart after save and open");
    }
}

#[test]
fn the_docx_value_axis_carries_word_s_elements_in_schema_order() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    run(&mut s, "chart.axis", json!({"min": 0, "max": 60, "majorUnit": 20, "numberFormat": "0.0"}));
    let spec = first_chart(&s.doc).unwrap();
    let xml = wordcraft_docx::chart_xml(&spec);
    let val_ax = &xml[xml.find("<c:valAx>").unwrap()..xml.find("</c:valAx>").unwrap()];
    let at = |needle: &str| val_ax.find(needle).unwrap_or_else(|| panic!("{needle} in {val_ax}"));
    // CT_Scaling: orientation, max, min. CT_ValAx: ... numFmt before majorTickMark,
    // crossBetween before majorUnit.
    assert!(at(r#"<c:orientation val="minMax"/>"#) < at(r#"<c:max val="60"/>"#));
    assert!(at(r#"<c:max val="60"/>"#) < at(r#"<c:min val="0"/>"#));
    assert!(at(r#"<c:numFmt formatCode="0.0" sourceLinked="0"/>"#) < at("<c:majorTickMark"));
    assert!(at("<c:crossBetween") < at(r#"<c:majorUnit val="20"/>"#));
    // An automatic axis writes none of them.
    let plain = wordcraft_docx::chart_xml(&first_chart(&column_chart([1.0, 2.0, 3.0]).doc).unwrap());
    for el in ["<c:max ", "<c:min ", "<c:majorUnit", "<c:numFmt"] {
        assert!(!plain.contains(el), "{el} in an automatic axis");
    }
}

#[test]
fn each_axis_change_is_one_undo_step() {
    let mut s = column_chart([3.0, 17.0, 37.0]);
    let before = first_chart(&s.doc);
    run(&mut s, "chart.axis", json!({"min": 0, "max": 60, "majorUnit": 20}));
    run(&mut s, "chart.axis", json!({"numberFormat": "0.0"}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "chart.axis", json!({})), json!({"min": 0.0, "max": 60.0, "majorUnit": 20.0, "numberFormat": null}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(first_chart(&s.doc), before);
}
