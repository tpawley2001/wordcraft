//! Layout › Arrange › Align with several objects: Align Selected Objects lines them up on their
//! bounding box, and Distribute Horizontally / Vertically spaces them evenly.
use super::*;
use wordcraft_doc::para::Anchor;

/// Floating 20 × 20 squares with their top-left at each (x, y) from the margins' corner, the
/// last inserted one selected and the others added to the selection. Returns them in the order
/// given, and the margins' corner on the page.
fn squares(at: &[(f32, f32)]) -> (Session, Vec<Pos>, (f32, f32)) {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({"text": "one\ntwo\nthree\nfour\nfive\nsix"})).unwrap();
    let sect = crate::cmd::page::sect(&s);
    let (ml, mt) = (sect.margin_left, sect.margin_top);
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
    for (i, (x, y)) in at.iter().enumerate() {
        if i > 0 {
            s.run("select.collapse", &json!({"end": true})).unwrap();
        }
        s.run("insert.shape", &json!({"kind": "rectangle", "width": 20, "height": 20})).unwrap();
        s.run("arrange.bounds", &json!({"page": 0, "x": ml + x, "y": mt + y})).unwrap();
    }
    // Each move re-anchors; find them by where they are.
    let list = all_objects(&s);
    let mut found = Vec::new();
    for (x, y) in at {
        let pos = list.iter().map(|(p, _)| p.clone()).find(|p| {
            let r = s.layout().object(p, 0).unwrap().rect;
            (r.x - (ml + x)).abs() < 0.5 && (r.y - (mt + y)).abs() < 0.5
        });
        found.push(pos.expect("square laid out where it was put"));
    }
    for i in 0..list.len() {
        s.run("select.addObject", &json!({"index": i})).unwrap();
    }
    assert_eq!(picked_objects(&s).len(), at.len());
    (s, found, (ml, mt))
}

fn rect(s: &mut Session, pos: &Pos) -> (f32, f32) {
    let r = s.layout().object(pos, 0).unwrap().rect;
    (r.x, r.y)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn align_selected_objects_left_and_bottom() {
    let (mut s, pos, (ml, mt)) = squares(&[(100.0, 40.0), (200.0, 120.0)]);
    // Two objects: relative to each other by default.
    s.run("arrange.align", &json!({"value": "left"})).unwrap();
    let (a, b) = (rect(&mut s, &pos[0]), rect(&mut s, &pos[1]));
    assert!(close(a.0, ml + 100.0) && close(b.0, ml + 100.0), "{a:?} {b:?}");
    assert!(close(a.1, mt + 40.0) && close(b.1, mt + 120.0), "left leaves the tops: {a:?} {b:?}");
    s.run("arrange.align", &json!({"value": "bottom", "relativeTo": "selected"})).unwrap();
    let (a, b) = (rect(&mut s, &pos[0]), rect(&mut s, &pos[1]));
    assert!(close(a.1, mt + 120.0) && close(b.1, mt + 120.0), "{a:?} {b:?}");
    // One undo step takes back the whole align.
    s.run("edit.undo", &json!({})).unwrap();
    let (a, b) = (rect(&mut s, &pos[0]), rect(&mut s, &pos[1]));
    assert!(close(a.1, mt + 40.0) && close(b.1, mt + 120.0), "{a:?} {b:?}");
}

#[test]
fn distribute_selected_keeps_the_ends_and_evens_the_gaps() {
    let (mut s, pos, (ml, _)) = squares(&[(0.0, 0.0), (50.0, 60.0), (300.0, 120.0)]);
    s.run("arrange.distribute", &json!({"direction": "horizontal"})).unwrap();
    let xs: Vec<f32> = pos.iter().map(|p| rect(&mut s, p).0 - ml).collect();
    // 0..20 and 300..320 stay; the gaps are (320 - 60) / 2 = 130 each.
    assert!(close(xs[0], 0.0) && close(xs[1], 150.0) && close(xs[2], 300.0), "{xs:?}");
}

#[test]
fn distribute_across_the_margin() {
    let (mut s, pos, _) = squares(&[(10.0, 0.0), (50.0, 60.0), (300.0, 120.0)]);
    let tw = crate::cmd::page::sect(&s).text_width();
    assert!(close(tw, 468.0));
    s.run("arrange.distribute", &json!({"direction": "horizontal", "relativeTo": "margin"})).unwrap();
    for (p, want) in pos.iter().zip([0.0, 224.0, 448.0]) {
        let o = s.doc.para_at(p).and_then(|q| q.object_at(p.off)).unwrap();
        let f = o.frame().unwrap().2;
        assert_eq!(f.h_rel, Anchor::Margin);
        assert!(close(f.x, want), "x {} want {want}", f.x);
    }
}

#[test]
fn distribute_vertically_and_needs_three() {
    let (mut s, pos, (_, mt)) = squares(&[(0.0, 0.0), (100.0, 30.0), (200.0, 200.0)]);
    s.run("arrange.distribute", &json!({"direction": "vertical"})).unwrap();
    let ys: Vec<f32> = pos.iter().map(|p| rect(&mut s, p).1 - mt).collect();
    assert!(close(ys[0], 0.0) && close(ys[1], 100.0) && close(ys[2], 200.0), "{ys:?}");
    let (mut s, _, _) = squares(&[(0.0, 0.0), (100.0, 30.0)]);
    assert!(s.run("arrange.distribute", &json!({"direction": "horizontal"})).is_err());
    assert!(s.run("arrange.distribute", &json!({"direction": "sideways"})).is_err());
}
