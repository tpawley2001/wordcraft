//! File › Options › Display › Printing options: Print background colors and images (off) and
//! Print drawings created in Word (on). Per-user preferences; print jobs follow them (job params
//! `background` and `drawings` override), ordinary PDF export and the screen don't.
//! Sources: Microsoft support "Print a background color or image"; Word's Options help for
//! "Print drawings created in Word" (drawing objects print as blank space when it is off).

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_layout::display::{DisplayOptions, Draw, page_display};

use crate::Session;
use crate::prefs::Prefs;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn defaults_are_words() {
    let p = Prefs::default();
    assert!(!p.print_background, "Print background colors and images is off by default");
    assert!(p.print_drawings, "Print drawings created in Word is on by default");
    // A settings file saved before these options existed loads Word's defaults.
    let old: Prefs = serde_json::from_value(json!({})).expect("empty prefs load");
    assert!(!old.print_background);
    assert!(old.print_drawings);
}

#[test]
fn file_options_reads_and_sets_both() {
    let mut s = Session::new(Document::from_text("Body"));
    let r = run(&mut s, "file.options", json!({"printHiddenText": false}));
    assert_eq!(r["printBackground"], json!(false));
    assert_eq!(r["printDrawings"], json!(true));
    let r = run(&mut s, "file.options", json!({"printBackground": true, "printDrawings": false}));
    assert_eq!((r["printBackground"].clone(), r["printDrawings"].clone()), (json!(true), json!(false)));
    assert!(s.prefs.print_background);
    assert!(!s.prefs.print_drawings);
    for bad in [json!("yes"), json!(1), json!([true])] {
        assert!(s.run("file.options", &json!({"printBackground": bad})).is_err(), "printBackground {bad}");
        assert!(s.run("file.options", &json!({"printDrawings": bad})).is_err(), "printDrawings {bad}");
    }
    assert!(s.prefs.print_background && !s.prefs.print_drawings, "a rejected call changes nothing");
}

#[test]
fn print_jobs_follow_the_preferences_and_job_params_override() {
    let mut s = Session::new(Document::from_text("Body"));
    assert_eq!(s.print_background_and_drawings(&json!({})).unwrap(), (false, true));
    s.prefs.print_background = true;
    s.prefs.print_drawings = false;
    assert_eq!(s.print_background_and_drawings(&json!({})).unwrap(), (true, false));
    assert_eq!(s.print_background_and_drawings(&json!({"background": false})).unwrap(), (false, false));
    assert_eq!(s.print_background_and_drawings(&json!({"drawings": true})).unwrap(), (true, true));
    assert!(s.print_background_and_drawings(&json!({"background": "no"})).is_err());
    assert!(s.print_background_and_drawings(&json!({"drawings": 0})).is_err());
}

/// Every glyph draw's text, joined, and the (glyph id, x, y) of the glyphs of draws whose text
/// contains `of`, looking inside turned, rotated, clipped and figure draws.
fn walk(draws: &[Draw], of: &str, text: &mut String, at: &mut Vec<(u32, f32, f32)>, shapes: &mut usize) {
    for d in draws {
        match d {
            Draw::Glyphs { text: t, glyphs, .. } => {
                text.push_str(t);
                text.push('|');
                if t.contains(of) {
                    at.extend(glyphs.iter().copied());
                }
            }
            Draw::Shape { .. } => *shapes += 1,
            Draw::Turned { items, .. } | Draw::Rotated { items, .. } | Draw::Clip { items, .. } => walk(items, of, text, at, shapes),
            Draw::Figure { draws, .. } => walk(draws, of, text, at, shapes),
            _ => {}
        }
    }
}

/// "Body words" with a floating rectangle and a text box reading "Boxed".
fn with_drawings() -> Session {
    let mut s = Session::new(Document::from_text("Body words"));
    run(&mut s, "insert.shape", json!({"kind": "rectangle", "width": 100, "height": 50, "fill": "FF0000"}));
    run(&mut s, "insert.textBox", json!({"text": "Boxed"}));
    s
}

fn page(s: &Session, hide_drawings: bool) -> (String, Vec<(u32, f32, f32)>, usize) {
    let lay = s.export_layout();
    let opts = DisplayOptions { hide_drawings, ..DisplayOptions::default() };
    let draws = page_display(&s.doc, &lay.pages[0], &opts);
    let (mut text, mut at, mut shapes) = (String::new(), Vec::new(), 0);
    walk(&draws, "Body", &mut text, &mut at, &mut shapes);
    (text, at, shapes)
}

#[test]
fn hide_drawings_draws_no_shapes_or_text_boxes_and_moves_nothing() {
    let s = with_drawings();
    let (text, body_at, shapes) = page(&s, false);
    assert!(text.contains("Boxed"), "the text box's text draws normally: {text:?}");
    assert!(text.contains("Body"), "{text:?}");
    assert!(shapes >= 1, "the rectangle draws normally");
    let (text, hidden_body_at, shapes) = page(&s, true);
    assert!(!text.contains("Boxed"), "a text box's text is a drawing: {text:?}");
    assert!(text.contains("Body"), "body text still prints: {text:?}");
    assert_eq!(shapes, 0, "no shape draws");
    assert_eq!(hidden_body_at, body_at, "the body text stays where it was: layout is unchanged");
}

#[test]
fn the_print_layout_is_the_same_either_way() {
    let mut s = with_drawings();
    let pages = |s: &Session| s.print_layout(&json!({})).pages.len();
    let on = pages(&s);
    s.prefs.print_drawings = false;
    s.prefs.print_background = true;
    assert_eq!(pages(&s), on);
}
