//! Mailings › Labels › New Label…: `mailings.labels` with `custom` sheet measurements (points).

use serde_json::{Value, json};
use wordcraft_doc::{Block, Document};
use wordcraft_engine::Session;

/// 3.5" x 2" labels, 2 across and 5 down on Letter, 18 pt side margin, 0.25" gap between columns.
fn letter_custom() -> Value {
    json!({"name": "Name badge", "top": 36.0, "side": 18.0, "width": 252.0, "height": 144.0,
           "hPitch": 270.0, "vPitch": 144.0, "cols": 2, "rows": 5})
}

fn table(s: &Session) -> &wordcraft_doc::Table {
    match s.doc.body.first().map(|b| &**b) {
        Some(Block::Table(t)) => t,
        _ => panic!("labels start with a table"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

#[test]
fn custom_letter_sheet_is_laid_out_from_its_measurements() {
    let mut s = Session::new(Document::new());
    let r = s.run("mailings.labels", &json!({"text": "Ada", "custom": letter_custom()})).unwrap();
    assert_eq!(r["labels"], 10);
    let sec = &s.doc.last_section;
    assert!(close(sec.page_w, 612.0) && close(sec.page_h, 792.0), "Letter by default");
    assert!(close(sec.margin_top, 36.0) && close(sec.margin_left, 18.0));
    // 612 - (18 + 270 + 252) = 72
    assert!(close(sec.margin_right, 72.0), "right margin {}", sec.margin_right);
    let t = table(&s);
    assert_eq!(t.grid.len(), 3, "label, 18 pt gap, label");
    assert!(close(t.grid[0], 252.0) && close(t.grid[1], 18.0) && close(t.grid[2], 252.0), "{:?}", t.grid);
    assert_eq!(t.rows.len(), 5, "no spacer rows when the vertical pitch equals the height");
    assert!(t.rows.iter().all(|r| r.props.height.is_some_and(|h| close(h, 144.0))));
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body).matches("Ada").count(), 10);
}

#[test]
fn custom_a4_sheet_uses_its_page_size_and_spacer_columns() {
    let mm = 72.0 / 25.4;
    let custom = json!({"top": 15.15 * mm, "side": 7.25 * mm, "width": 63.5 * mm, "height": 38.1 * mm,
                        "hPitch": 66.0 * mm, "vPitch": 38.1 * mm, "cols": 3, "rows": 7,
                        "pageWidth": 595.3, "pageHeight": 841.9});
    let mut s = Session::new(Document::new());
    let r = s.run("mailings.labels", &json!({"text": "Bo", "custom": custom})).unwrap();
    assert_eq!(r["labels"], 21);
    assert!(close(s.doc.last_section.page_w, 595.3) && close(s.doc.last_section.page_h, 841.9));
    let t = table(&s);
    assert_eq!((t.rows.len(), t.grid.len()), (7, 5));
    assert!(close(t.grid[0], 180.0) && close(t.grid[1], 7.087), "{:?}", t.grid);
}

#[test]
fn custom_wins_over_product_and_single_label_uses_custom_rows() {
    let mut s = Session::new(Document::new());
    let r = s
        .run("mailings.labels", &json!({"text": "Cy", "product": "letter-3x10", "custom": letter_custom(), "single": {"row": 5, "col": 2}}))
        .unwrap();
    assert_eq!(r["filled"], 1);
    let t = table(&s);
    assert_eq!((t.rows.len(), t.grid.len()), (5, 3));
    let cell = t.rows.get(4).and_then(|r| r.cells.get(2)).and_then(|c| c.blocks.first());
    assert!(matches!(cell.map(|b| &**b), Some(Block::Para(p)) if p.text == "Cy"));
}

#[test]
fn default_pitch_is_the_label_size() {
    let mut c = letter_custom();
    c.as_object_mut().unwrap().remove("hPitch");
    c.as_object_mut().unwrap().remove("vPitch");
    let mut s = Session::new(Document::new());
    s.run("mailings.labels", &json!({"text": "Di", "custom": c})).unwrap();
    assert_eq!(table(&s).grid.len(), 2, "touching labels: no spacer column");
}

#[test]
fn impossible_sheets_are_refused() {
    let bad = |edit: &dyn Fn(&mut serde_json::Map<String, Value>)| {
        let mut c = letter_custom();
        edit(c.as_object_mut().unwrap());
        let mut s = Session::new(Document::from_text("keep"));
        let r = s.run("mailings.labels", &json!({"text": "x", "custom": c}));
        assert!(r.is_err(), "{c}");
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body).trim(), "keep", "nothing created");
    };
    bad(&|c| {
        c.insert("cols".into(), json!(3)); // 18 + 2*270 + 252 = 810 > 612
    });
    bad(&|c| {
        c.insert("rows".into(), json!(6)); // 36 + 5*144 + 144 = 900 > 792
    });
    bad(&|c| {
        c.insert("hPitch".into(), json!(200.0)); // pitch smaller than the 252 pt label
    });
    bad(&|c| {
        c.insert("vPitch".into(), json!(100.0));
    });
    bad(&|c| {
        c.insert("width".into(), json!(0.0));
    });
    bad(&|c| {
        c.insert("top".into(), json!(-1.0));
    });
    bad(&|c| {
        c.insert("cols".into(), json!(0));
    });
    bad(&|c| {
        c.remove("height");
    });
    bad(&|c| {
        c.insert("width".into(), json!("wide"));
    });
}

#[test]
fn products_still_work_without_custom() {
    let mut s = Session::new(Document::new());
    let r = s.run("mailings.labels", &json!({"text": "Ed", "product": "letter-3x10"})).unwrap();
    assert_eq!(r["labels"], 30);
}
