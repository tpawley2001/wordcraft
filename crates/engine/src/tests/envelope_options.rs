//! Envelope Options as in Word (Mailings › Envelopes › Options…): delivery and return address
//! positions (From left / From top) and fonts, Omit return address, and Word's built-in Envelope
//! Address / Envelope Return paragraph styles. A positioned address is a page-anchored text frame
//! (ECMA-376 Part 1 §17.3.1.11).

use serde_json::{Value, json};
use wordcraft_doc::{Block, Document, Paragraph};

use crate::Session;

fn run(s: &mut Session, v: Value) -> Value {
    s.run("mailings.envelopes", &v).unwrap_or_else(|e| panic!("mailings.envelopes: {e}"))
}

fn para<'a>(s: &'a Session, text: &str) -> &'a Paragraph {
    s.doc
        .body
        .iter()
        .find_map(|b| match &**b {
            Block::Para(p) if p.text == text => Some(p),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no paragraph `{text}`"))
}

fn has_para(s: &Session, text: &str) -> bool {
    s.doc.body.iter().any(|b| matches!(&**b, Block::Para(p) if p.text == text))
}

#[test]
fn addresses_use_the_envelope_styles() {
    let mut s = Session::new(Document::new());
    run(&mut s, json!({"delivery": "Jo Doe\n1 Main St", "return": "Me\nHome"}));
    for t in ["Jo Doe", "1 Main St"] {
        assert_eq!(para(&s, t).props.style.as_deref(), Some("EnvelopeAddress"), "{t}");
    }
    for t in ["Me", "Home"] {
        let p = para(&s, t);
        assert_eq!(p.props.style.as_deref(), Some("EnvelopeReturn"), "{t}");
    }
    let addr = s.doc.styles.get("EnvelopeAddress").expect("EnvelopeAddress style added");
    assert_eq!(addr.name, "envelope address");
    assert_eq!(addr.chr.size, Some(12.0));
    let ret = s.doc.styles.get("EnvelopeReturn").expect("EnvelopeReturn style added");
    assert_eq!(ret.name, "envelope return");
    assert_eq!(ret.chr.size, Some(10.0));
}

#[test]
fn auto_position_keeps_the_indent_and_no_frame() {
    let mut s = Session::new(Document::new());
    let r = run(&mut s, json!({"delivery": "Jo Doe"}));
    let w = r["width"].as_f64().unwrap() as f32;
    let p = para(&s, "Jo Doe");
    assert!(p.props.frame.is_none(), "Auto is not a frame");
    assert_eq!(p.props.indent_left, Some(w * 0.42));
}

#[test]
fn omit_drops_the_return_address() {
    let mut s = Session::new(Document::new());
    run(&mut s, json!({"delivery": "Jo Doe", "return": "Me", "omitReturn": true}));
    assert!(!has_para(&s, "Me"), "return address omitted");
    assert!(has_para(&s, "Jo Doe"));
}

#[test]
fn delivery_from_left_and_top_is_a_page_anchored_frame_shared_by_its_lines() {
    let mut s = Session::new(Document::new());
    run(&mut s, json!({"delivery": "Jo Doe\n1 Main St\nTown", "deliveryLeft": 288.0, "deliveryTop": 144.0}));
    let frames: Vec<_> = ["Jo Doe", "1 Main St", "Town"].iter().map(|t| para(&s, t).props.frame.clone().expect("framed")).collect();
    let f = &frames[0];
    assert_eq!((f.x, f.y), (288.0, 144.0));
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("page", "page"));
    assert!(frames.iter().all(|g| g == f), "identical frame properties: one frame");
    assert_eq!(para(&s, "Jo Doe").props.indent_left, None, "a framed address has no indent");
}

#[test]
fn return_from_left_and_top_and_a_single_coordinate_default() {
    let mut s = Session::new(Document::new());
    run(&mut s, json!({"delivery": "Jo", "return": "Me", "returnLeft": 36.0, "returnTop": 30.0}));
    let f = para(&s, "Me").props.frame.clone().expect("framed return address");
    assert_eq!((f.x, f.y, f.h_anchor.as_str(), f.v_anchor.as_str()), (36.0, 30.0, "page", "page"));
    assert!(para(&s, "Jo").props.frame.is_none(), "delivery stays Auto");

    let mut s = Session::new(Document::new());
    let r = run(&mut s, json!({"delivery": "Jo", "deliveryTop": 150.0, "size": "DL"}));
    let w = r["width"].as_f64().unwrap() as f32;
    let f = para(&s, "Jo").props.frame.clone().expect("framed");
    assert_eq!((f.x, f.y), (w * 0.42, 150.0), "the missing From left defaults to the Auto column");
}

#[test]
fn fonts_change_the_styles() {
    let mut s = Session::new(Document::new());
    run(&mut s, json!({"delivery": "Jo", "return": "Me", "deliveryFont": {"name": "Courier New", "size": 14.0}, "returnFont": {"size": 8.0}}));
    let addr = s.doc.styles.get("EnvelopeAddress").unwrap();
    assert_eq!((addr.chr.font.as_deref(), addr.chr.size), (Some("Courier New"), Some(14.0)));
    let ret = s.doc.styles.get("EnvelopeReturn").unwrap();
    assert_eq!(ret.chr.size, Some(8.0));
    assert_eq!(para(&s, "Jo").props_at(0).size, None, "the size comes from the style, not direct formatting");
}

#[test]
fn added_envelope_puts_the_styles_in_this_document() {
    let mut s = Session::new(Document::from_text("Dear Jo,"));
    run(&mut s, json!({"delivery": "Jo Doe", "add": true, "deliveryFont": {"size": 13.0}}));
    assert_eq!(s.doc.styles.get("EnvelopeAddress").and_then(|st| st.chr.size), Some(13.0));
    assert_eq!(para(&s, "Jo Doe").props.style.as_deref(), Some("EnvelopeAddress"));
    assert!(has_para(&s, "Dear Jo,"));
}

#[test]
fn out_of_range_positions_and_fonts_are_refused_and_change_nothing() {
    let mut s = Session::new(Document::from_text("Dear Jo,"));
    let before = s.doc.body.len();
    for bad in [
        json!({"delivery": "Jo", "add": true, "deliveryLeft": -1.0}),
        json!({"delivery": "Jo", "add": true, "deliveryTop": 5000.0}),
        json!({"delivery": "Jo", "add": true, "returnLeft": 700.0}),
        json!({"delivery": "Jo", "add": true, "deliveryFont": {"size": 0.0}}),
        json!({"delivery": "Jo", "add": true, "deliveryFont": {"size": 2000.0}}),
        json!({"delivery": "Jo", "add": true, "returnFont": {"name": ""}}),
        json!({"delivery": "Jo", "add": true, "returnFont": {"name": "A font name longer than thirty-one"}}),
    ] {
        assert!(s.run("mailings.envelopes", &bad).is_err(), "{bad}");
        assert_eq!(s.doc.body.len(), before, "{bad}: nothing inserted");
    }
}
