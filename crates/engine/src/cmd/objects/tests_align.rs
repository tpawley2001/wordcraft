//! Layout › Arrange › Align: Top / Middle / Bottom and Align to Page for one floating object,
//! as Word places it (the edge goes to the margin's or the page's edge).
use super::*;
use wordcraft_doc::para::Anchor;

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

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn top_middle_bottom_of_the_margin() {
    let th = crate::cmd::page::sect(&rectangle()).text_height();
    for (value, y) in [("top", 0.0), ("middle", (th - 50.0) / 2.0), ("bottom", th - 50.0)] {
        let mut s = rectangle();
        s.run("arrange.align", &json!({"value": value})).unwrap();
        let f = float(&s);
        assert_eq!(f.v_rel, Anchor::Margin, "{value}");
        assert_eq!(f.v_align, None, "{value}");
        assert!(close(f.y, y), "{value}: y {} want {y}", f.y);
    }
}

#[test]
fn align_to_page_measures_from_the_page_edge() {
    let sect = crate::cmd::page::sect(&rectangle());
    let (pw, ph) = (sect.page_w, sect.page_h);
    let mut s = rectangle();
    s.run("arrange.align", &json!({"value": "right", "relativeTo": "page"})).unwrap();
    let f = float(&s);
    assert_eq!(f.h_rel, Anchor::Page);
    assert!(close(f.x, pw - 100.0), "x {} want {}", f.x, pw - 100.0);
    s.run("arrange.align", &json!({"value": "bottom", "relativeTo": "page"})).unwrap();
    let f = float(&s);
    assert_eq!(f.v_rel, Anchor::Page);
    assert!(close(f.y, ph - 50.0), "y {} want {}", f.y, ph - 50.0);
    assert!(close(f.x, pw - 100.0), "a vertical align leaves x alone");
}

#[test]
fn horizontal_align_to_the_margin_is_unchanged() {
    let tw = crate::cmd::page::sect(&rectangle()).text_width();
    let mut s = rectangle();
    s.run("arrange.align", &json!({"value": "right"})).unwrap();
    let f = float(&s);
    assert_eq!(f.h_rel, Anchor::Margin);
    assert!(close(f.x, tw - 100.0));
}

#[test]
fn distribute_needs_three_selected_objects() {
    let mut s = rectangle();
    assert!(s.run("arrange.distribute", &json!({"direction": "horizontal", "relativeTo": "selected"})).is_err());
}
