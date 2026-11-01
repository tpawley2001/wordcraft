//! Read Mode › View › Layout (`view.readLayout`): Column Layout reflows the text into reading
//! columns; Paper Layout shows the document's printed pages, as in Word.
use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_engine::Session;
use wordcraft_engine::cmd::view::ReadLayout;

fn session() -> Session {
    let text = (0..60).map(|i| format!("Paragraph {i} of a document long enough to fill more than one printed page.")).collect::<Vec<_>>();
    Session::new(Document::from_text(&text.join("\n")))
}

fn page_sizes(s: &mut Session) -> Vec<(f32, f32)> {
    s.layout().pages.iter().map(|p| (p.w, p.h)).collect()
}

#[test]
fn read_layout_command_sets_and_reports_the_choice() {
    let mut s = session();
    s.run("view.readMode", &json!({})).unwrap();
    assert_eq!(s.view.read_layout, ReadLayout::Column);
    assert_eq!(s.run("view.readLayout", &json!({"value": "paper"})).unwrap()["layout"], "paper");
    assert_eq!(s.view.read_layout, ReadLayout::Paper);
    assert_eq!(s.run("view.readLayout", &json!({"value": "column"})).unwrap()["layout"], "column");
    assert_eq!(s.view.read_layout, ReadLayout::Column);
    s.run("view.readLayout", &json!({"value": "paper"})).unwrap();
    assert!(s.run("view.readLayout", &json!({"value": "wide"})).is_err());
    assert!(s.run("view.readLayout", &json!({})).is_err());
    assert_eq!(s.view.read_layout, ReadLayout::Paper, "a refused value leaves the choice alone");
}

#[test]
fn paper_layout_shows_the_printed_pages() {
    let mut s = session();
    let print = page_sizes(&mut s);
    assert!(print.len() > 1, "the fixture must fill more than one page");
    s.run("view.readMode", &json!({})).unwrap();
    s.run("view.readLayout", &json!({"value": "paper"})).unwrap();
    assert_eq!(page_sizes(&mut s), print, "Paper Layout pages are Print Layout's pages");
}

#[test]
fn column_layout_reflows_and_keeps_its_options_across_paper() {
    let mut s = session();
    s.run("view.readMode", &json!({})).unwrap();
    let column_w = s.view.read_width;
    assert_eq!(s.layout().pages[0].w, column_w, "Column Layout is the default");
    s.run("view.readLineSpacing", &json!({"value": 1.5})).unwrap();
    s.run("view.readLayout", &json!({"value": "paper"})).unwrap();
    assert_ne!(s.layout().pages[0].w, column_w);
    s.run("view.readLayout", &json!({"value": "column"})).unwrap();
    assert_eq!(s.layout().pages[0].w, column_w);
    assert_eq!(s.view.read_line_spacing, Some(1.5), "line spacing is kept through Paper Layout");
}

#[test]
fn entering_read_mode_again_starts_in_column_layout() {
    let mut s = session();
    s.run("view.readMode", &json!({})).unwrap();
    s.run("view.readLayout", &json!({"value": "paper"})).unwrap();
    s.run("view.printLayout", &json!({})).unwrap();
    s.run("view.readMode", &json!({})).unwrap();
    assert_eq!(s.view.read_layout, ReadLayout::Column);
}
