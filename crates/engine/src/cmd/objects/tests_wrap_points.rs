//! Edit Wrap Points (`arrange.wrapPoints`): reads and edits the outline Tight and Through
//! wrapping follow (`Float::wrap_polygon`, ECMA-376 Part 1 §20.4.2.16), as Word's Layout ›
//! Wrap Text › Edit Wrap Points does. Coordinates are in the wrap space (21600 = the object's
//! width or height).
use super::*;

fn rectangle() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("insert.shape", &json!({"kind": "rectangle"})).unwrap();
    s.run("picture.size", &json!({"width": 100, "height": 50})).unwrap();
    s.run("arrange.wrap", &json!({"wrap": "square"})).unwrap();
    s
}

fn float(s: &Session) -> Float {
    let (_, o) = selected(s).unwrap();
    o.frame().unwrap().2.clone()
}

fn points(s: &Session) -> Vec<[i32; 2]> {
    float(s).wrap_polygon.map(|p| p.points.clone()).unwrap_or_default()
}

fn wp(s: &mut Session, v: serde_json::Value) -> Result<serde_json::Value, CmdError> {
    s.run("arrange.wrapPoints", &v)
}

#[test]
fn moving_a_point_switches_square_to_tight_and_stores_an_edited_polygon() {
    let mut s = rectangle();
    assert_eq!(float(&s).wrap, Wrap::Square);
    wp(&mut s, json!({"op": "move", "index": 0, "x": 1000, "y": 2000})).unwrap();
    let f = float(&s);
    assert_eq!(f.wrap, Wrap::Tight);
    let poly = f.wrap_polygon.expect("an edited polygon is stored");
    assert!(poly.edited);
    assert!(poly.points.len() >= 3);
    assert_eq!(poly.points[0], [1000, 2000]);
}

#[test]
fn get_reports_the_outline_without_changing_the_object() {
    let mut s = rectangle();
    let v = wp(&mut s, json!({"op": "get"})).unwrap();
    assert_eq!(float(&s).wrap, Wrap::Square, "get changes nothing");
    assert!(float(&s).wrap_polygon.is_none());
    let pts = v["points"].as_array().expect("points array");
    assert!(pts.len() >= 3, "{v}");
    assert_eq!(v["edited"], json!(false));
}

#[test]
fn set_replaces_the_points_and_get_returns_them() {
    let mut s = rectangle();
    let tri = json!([[0, 0], [21600, 10800], [0, 21600]]);
    wp(&mut s, json!({"op": "set", "points": tri})).unwrap();
    assert_eq!(points(&s), vec![[0, 0], [21600, 10800], [0, 21600]]);
    let v = wp(&mut s, json!({"op": "get"})).unwrap();
    assert_eq!(v["points"], tri);
    assert_eq!(v["edited"], json!(true));
    assert_eq!(v["wrap"], json!("tight"));
}

#[test]
fn add_inserts_after_the_index_and_remove_takes_it_out() {
    let mut s = rectangle();
    wp(&mut s, json!({"op": "set", "points": [[0, 0], [21600, 0], [21600, 21600], [0, 21600]]})).unwrap();
    wp(&mut s, json!({"op": "add", "after": 0, "x": 10800, "y": 5000})).unwrap();
    assert_eq!(points(&s), vec![[0, 0], [10800, 5000], [21600, 0], [21600, 21600], [0, 21600]]);
    wp(&mut s, json!({"op": "add", "after": 4, "x": -500, "y": 10800})).unwrap();
    assert_eq!(points(&s).last(), Some(&[-500, 10800]), "after the last index appends");
    wp(&mut s, json!({"op": "remove", "index": 1})).unwrap();
    assert_eq!(points(&s), vec![[0, 0], [21600, 0], [21600, 21600], [0, 21600], [-500, 10800]]);
}

#[test]
fn an_outline_keeps_at_least_three_points() {
    let mut s = rectangle();
    assert!(wp(&mut s, json!({"op": "set", "points": [[0, 0], [21600, 0]]})).is_err());
    assert!(float(&s).wrap_polygon.is_none(), "a refused set changes nothing");
    assert_eq!(float(&s).wrap, Wrap::Square);
    wp(&mut s, json!({"op": "set", "points": [[0, 0], [21600, 0], [0, 21600]]})).unwrap();
    assert!(wp(&mut s, json!({"op": "remove", "index": 0})).is_err());
    assert_eq!(points(&s).len(), 3);
}

#[test]
fn an_index_out_of_range_is_an_error() {
    let mut s = rectangle();
    wp(&mut s, json!({"op": "set", "points": [[0, 0], [21600, 0], [0, 21600]]})).unwrap();
    assert!(wp(&mut s, json!({"op": "move", "index": 3, "x": 1, "y": 1})).is_err());
    assert!(wp(&mut s, json!({"op": "add", "after": 3, "x": 1, "y": 1})).is_err());
    assert!(wp(&mut s, json!({"op": "remove", "index": 7})).is_err());
    assert_eq!(points(&s), vec![[0, 0], [21600, 0], [0, 21600]]);
}

#[test]
fn reset_goes_back_to_the_derived_outline() {
    let mut s = rectangle();
    wp(&mut s, json!({"op": "move", "index": 0, "x": 1000, "y": 2000})).unwrap();
    wp(&mut s, json!({"op": "reset"})).unwrap();
    assert!(float(&s).wrap_polygon.is_none());
    assert_eq!(float(&s).wrap, Wrap::Tight, "reset keeps the wrap style");
}

#[test]
fn one_edit_is_one_undo_step() {
    let mut s = rectangle();
    wp(&mut s, json!({"op": "move", "index": 0, "x": 1000, "y": 2000})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    let f = float(&s);
    assert!(f.wrap_polygon.is_none(), "undo removes the polygon");
    assert_eq!(f.wrap, Wrap::Square, "and the switch to Tight, in the same step");
}
