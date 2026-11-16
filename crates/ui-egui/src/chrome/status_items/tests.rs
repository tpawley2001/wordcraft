use super::*;

fn app(text: &str) -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text(text)), crate::Services::default())
}

/// One status-bar frame, so the document is laid out as on screen.
fn frame(a: &mut WordApp) {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    for i in 0..2 {
        let input = egui::RawInput { time: Some(f64::from(i) / 60.0), predicted_dt: 1.0 / 60.0, ..Default::default() };
        ctx.run_ui(input, |ui| crate::chrome::status_bar(a, ui)).drop_without_applying_deltas();
    }
}

#[test]
fn the_list_is_in_words_order_with_todays_items_on() {
    let labels: Vec<&str> = StatusItem::ALL.iter().map(|i| i.label()).collect();
    assert_eq!(
        labels,
        [
            "Formatted Page Number",
            "Section",
            "Page Number",
            "Vertical Page Position",
            "Line Number",
            "Column",
            "Word Count",
            "Character Count (with spaces)",
            "Language",
            "Track Changes",
            "Caps Lock",
            "Overtype",
            "Selection Mode",
            "View Shortcuts",
            "Zoom Slider",
            "Zoom",
        ]
    );
    let on: Vec<StatusItem> = StatusItem::ALL.into_iter().filter(|i| i.default_on()).collect();
    use StatusItem::*;
    assert_eq!(on, [PageNumber, WordCount, Language, TrackChanges, Overtype, ViewShortcuts, ZoomSlider, Zoom]);
    let a = app("x");
    for i in StatusItem::ALL {
        assert_eq!(status_item_shown(&a, i), i.default_on(), "{i:?}");
    }
}

#[test]
fn toggling_shows_and_hides() {
    let mut a = app("x");
    toggle_status_item(&mut a, StatusItem::Section);
    assert!(status_item_shown(&a, StatusItem::Section));
    toggle_status_item(&mut a, StatusItem::WordCount);
    assert!(!status_item_shown(&a, StatusItem::WordCount));
    toggle_status_item(&mut a, StatusItem::Section);
    assert!(!status_item_shown(&a, StatusItem::Section));
}

#[test]
fn caret_position_values() {
    let mut a = app("One\nTwo");
    frame(&mut a);
    a.run("caret.docEnd", serde_json::json!({})).unwrap();
    frame(&mut a);
    assert_eq!(status_item_value(&a, StatusItem::Section), "1");
    assert_eq!(status_item_value(&a, StatusItem::LineNumber), "2");
    assert_eq!(status_item_value(&a, StatusItem::Column), "4");
    assert_eq!(status_item_value(&a, StatusItem::PageNumber), "Page 1 of 1");
    assert_eq!(status_item_value(&a, StatusItem::FormattedPageNumber), "1");
    assert!(status_item_value(&a, StatusItem::Zoom).ends_with('%'));
    a.run("caret.docStart", serde_json::json!({})).unwrap();
    frame(&mut a);
    assert_eq!(status_item_value(&a, StatusItem::LineNumber), "1");
    assert_eq!(status_item_value(&a, StatusItem::Column), "1");
}
