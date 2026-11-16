//! Chart Design › Data › Switch Row/Column (`chart.switchRowColumn`): the chart's series become
//! its categories and its categories its series, as Word does for a chart's data.

use super::*;
use wordcraft_doc::chart::{ChartSeries, ChartSpec, ChartType, LegendPos};
use wordcraft_doc::para::InlineObject;

fn first_chart(doc: &wordcraft_doc::Document) -> Option<ChartSpec> {
    let p = doc.para_at(&Pos::body(0, 0))?;
    p.object_offsets().into_iter().find_map(|off| match p.object_at(off) {
        Some(InlineObject::Graphic { graphic, .. }) => graphic.chart.as_deref().cloned(),
        _ => None,
    })
}

fn ser(name: &str, values: &[Option<f64>]) -> ChartSeries {
    ChartSeries { name: name.into(), values: values.to_vec() }
}

fn chart(kind: &str) -> Session {
    let mut s = s();
    let data = json!({"categories": ["Q1", "Q2"], "series": [
        {"name": "North", "values": [1, 2]},
        {"name": "East", "values": [3, null]},
        {"name": "South", "values": [5, 6.5]},
    ]});
    run(&mut s, "insert.chart", json!({"type": kind, "data": data, "title": "Sales", "legend": "right", "dataLabels": true}));
    s
}

#[test]
fn series_become_categories_and_categories_series() {
    let mut s = chart("column");
    let r = run(&mut s, "chart.switchRowColumn", json!({}));
    assert_eq!(r, json!({"categories": 3, "series": 2}));
    let c = first_chart(&s.doc).unwrap();
    assert_eq!(c.categories, ["North", "East", "South"]);
    assert_eq!(c.series, [ser("Q1", &[Some(1.0), Some(3.0), Some(5.0)]), ser("Q2", &[Some(2.0), None, Some(6.5)])]);
}

#[test]
fn everything_else_about_the_chart_stays() {
    let mut s = chart("barStacked");
    run(&mut s, "chart.switchRowColumn", json!({}));
    let c = first_chart(&s.doc).unwrap();
    assert_eq!((c.kind, c.title.as_deref(), c.legend, c.data_labels), (ChartType::BarStacked, Some("Sales"), Some(LegendPos::Right), true));
}

#[test]
fn switching_twice_gives_the_chart_back_and_each_switch_is_one_undo_step() {
    let mut s = chart("line");
    let before = first_chart(&s.doc).unwrap();
    run(&mut s, "chart.switchRowColumn", json!({}));
    let once = first_chart(&s.doc).unwrap();
    assert_ne!(once, before);
    run(&mut s, "chart.switchRowColumn", json!({}));
    assert_eq!(first_chart(&s.doc).unwrap(), before);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(first_chart(&s.doc).unwrap(), once);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(first_chart(&s.doc).unwrap(), before);
}

#[test]
fn a_pie_chart_switches_too() {
    // A pie draws its first series: after the switch that is the first category across the series.
    let mut s = chart("pie");
    run(&mut s, "chart.switchRowColumn", json!({}));
    let c = first_chart(&s.doc).unwrap();
    assert_eq!(c.kind, ChartType::Pie);
    assert_eq!(c.series[0], ser("Q1", &[Some(1.0), Some(3.0), Some(5.0)]));
}

#[test]
fn ragged_and_empty_data() {
    let mut s = s();
    // A series with fewer values than categories: the missing ones are gaps.
    let data = json!({"categories": ["a", "b", "c"], "series": [{"name": "S", "values": [7]}]});
    run(&mut s, "insert.chart", json!({"type": "column", "data": data}));
    run(&mut s, "chart.switchRowColumn", json!({}));
    let c = first_chart(&s.doc).unwrap();
    assert_eq!(c.categories, ["S"]);
    assert_eq!(c.series, [ser("a", &[Some(7.0)]), ser("b", &[None]), ser("c", &[None])]);
}

#[test]
fn refused_cases_change_nothing() {
    // Scatter charts plot x values, not categories: Switch Row/Column is not offered.
    let mut s = chart("scatter");
    let before = first_chart(&s.doc);
    let e = s.run("chart.switchRowColumn", &json!({})).unwrap_err().to_string();
    assert!(e.contains("scatter"), "{e}");
    assert_eq!(first_chart(&s.doc), before);
    // More categories than a chart can have series (255).
    let mut s = self::s();
    let cats: Vec<String> = (0..300).map(|i| format!("c{i}")).collect();
    let vals: Vec<f64> = (0..300).map(f64::from).collect();
    run(&mut s, "insert.chart", json!({"type": "column", "data": {"categories": cats, "series": [{"name": "S", "values": vals}]}}));
    let before = first_chart(&s.doc);
    let e = s.run("chart.switchRowColumn", &json!({})).unwrap_err().to_string();
    assert!(e.contains("255"), "{e}");
    assert_eq!(first_chart(&s.doc), before);
    // Nothing selected.
    let mut s = self::s();
    assert!(s.run("chart.switchRowColumn", &json!({})).is_err());
}

#[test]
fn the_switched_chart_saves_and_reads_back_editable() {
    let mut s = chart("column");
    run(&mut s, "chart.switchRowColumn", json!({}));
    let spec = first_chart(&s.doc).unwrap();
    let back = wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(first_chart(&back), Some(spec));
}

#[test]
fn it_is_on_the_chart_design_tab() {
    let spec = crate::cmd::chart::specs().into_iter().find(|c| c.id == "chart.switchRowColumn").expect("registered");
    assert_eq!(spec.location, "Chart Design › Data");
    assert_eq!(spec.label, "Switch Row/Column");
}
