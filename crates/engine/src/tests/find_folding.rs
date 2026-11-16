use crate::Session;
use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};

fn s() -> Session {
    Session::new(Document::new())
}

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.run(command, &params).unwrap()
}

fn text(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

fn loose() -> Value {
    json!({"matchDiacritics": false})
}

fn fold_count(s: &mut Session, query: &str, options: Value) -> u64 {
    let mut params = json!({"text": query, "matchDiacritics": true, "matchKashida": true, "matchAlefHamza": true,
        "matchCase": false, "wholeWord": false, "regex": false, "wildcards": false, "ignoreSpace": false, "special": true});
    for (key, value) in options.as_object().unwrap() {
        params[key] = value.clone();
    }
    run(s, "edit.find", params)["count"].as_u64().unwrap()
}

#[test]
fn find_folding_acceptance_ranges_and_exact_defaults() {
    for (source, query, options) in [
        ("cafe\u{301} au lait", "cafe", loose()),
        ("שָׁלוֹם", "שלום", json!({"matchDiacritics": false, "wholeWord": true})),
        ("كَتَبَ", "كتب", loose()),
        ("كـــتاب", "كتاب", json!({"matchKashida": false})),
        ("أحمد", "احمد", json!({"matchAlefHamza": false})),
        ("إبراهيم", "ابراهيم", json!({"matchAlefHamza": false})),
        ("آمنة", "امنة", json!({"matchAlefHamza": false})),
        ("ٱمنة", "امنة", json!({"matchAlefHamza": false})),
    ] {
        let mut s = s();
        run(&mut s, "document.setText", json!({"text": source}));
        // Default searches retain exact matching, including the trailing accent.
        assert_eq!(fold_count(&mut s, query, json!({"wholeWord": true})), 0, "{source}");
        assert_eq!(fold_count(&mut s, query, options), 1, "{source}");
        let expected = source.split(' ').next().unwrap();
        assert_eq!(s.find.results.len(), 1);
        let (a, b) = &s.find.results[0];
        assert_eq!((a.off, b.off), (0, expected.len()), "{source}");
        let r = run(&mut s, "edit.replaceAll", json!({"with": "X"}));
        assert_eq!(r["replaced"], 1);
        assert_eq!(text(&s), source.replacen(expected, "X", 1));
    }
    let mut s = s();
    run(&mut s, "document.setText", json!({"text": "أحمد إبراهيم آمنة"}));
    for query in ["احمد", "ابراهيم", "امنة"] {
        assert_eq!(fold_count(&mut s, query, json!({"matchAlefHamza": false})), 1);
    }
    run(&mut s, "document.setText", json!({"text": "café"}));
    for mode in ["wildcards", "regex"] {
        let mut options = loose();
        options[mode] = json!(true);
        assert_eq!(fold_count(&mut s, "cafe", options), 0);
    }
}

#[test]
fn find_folding_long_paragraph_is_linear() {
    let mut s = s();
    let source = format!("{}كَت", "كَتَبَ ".repeat(28_571));
    assert_eq!(source.chars().count(), 200_000);
    run(&mut s, "document.setText", json!({"text": source}));
    let started = std::time::Instant::now();
    assert_eq!(fold_count(&mut s, "كتب", loose()), 28_571);
    let elapsed = started.elapsed();
    assert!(elapsed < std::time::Duration::from_secs(2), "folded search took {elapsed:?}");
    eprintln!("200,000-character folded search: {elapsed:?}");
    let (a, b) = s.find.results.last().unwrap();
    assert_eq!((a.off, b.off), (28_570 * "كَتَبَ ".len(), 28_570 * "كَتَبَ ".len() + "كَتَبَ".len()));
}

#[test]
fn find_folding_preserves_format_codes_and_smart_apostrophes() {
    let mut s = s();
    run(&mut s, "document.setText", json!({"text": "cafe\u{301} l’été"}));
    run(&mut s, "select.range", json!({"anchor": {"block": 0, "off": 0}, "focus": {"block": 0, "off": "cafe\u{301}".len()}}));
    run(&mut s, "format.bold", json!({}));
    assert_eq!(fold_count(&mut s, "cafe", json!({"matchDiacritics": false, "format": {"bold": true}})), 1);
    assert_eq!(fold_count(&mut s, "cafe", json!({"matchDiacritics": false, "format": {"bold": false}})), 0);
    assert_eq!(fold_count(&mut s, "l'ete", json!({"matchDiacritics": false, "format": null})), 1);
    assert_eq!(fold_count(&mut s, "l^u8217ete", loose()), 1);
    assert_eq!(fold_count(&mut s, "l^u39ete", loose()), 0);
    // Replacement's ^& omits the final synthetic story mark after mapping folded offsets.
    run(&mut s, "document.setText", json!({"text": "cafe\u{301}"}));
    let r = run(&mut s, "edit.replaceAll", json!({"text": "cafe^p", "with": "[^&]", "matchDiacritics": false}));
    assert_eq!(r["replaced"], 1);
    assert_eq!(text(&s), "[cafe\u{301}]");
}
