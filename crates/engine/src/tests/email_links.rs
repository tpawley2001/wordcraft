//! Insert Link › E-mail Address, as in Word's Insert Hyperlink dialog: `insert.link` with `email`
//! and `subject` links to a `mailto:` URI (RFC 6068, the subject percent-encoded as UTF-8), and
//! `link.targets` lists the document's e-mail links for "Recently used e-mail addresses".

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Every (target, text) linked run in the body, in order.
fn links(s: &Session) -> Vec<(String, String)> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.run_ranges().filter_map(|(r, c)| Some((c.link.clone()?, p.text.get(r)?.to_string()))).collect::<Vec<_>>())
        .collect()
}

/// The link target `insert.link` makes from `params` at the end of an empty document.
fn target(params: Value) -> String {
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    run(&mut s, "insert.link", params);
    let l = links(&s);
    assert_eq!(l.len(), 1, "one link: {l:?}");
    l[0].0.clone()
}

#[test]
fn email_link_targets_are_mailto_uris_with_an_encoded_subject() {
    let cases = [
        (json!({"email": "ann@example.com"}), "mailto:ann@example.com"),
        (json!({"email": "ann@example.com", "subject": "Hello there"}), "mailto:ann@example.com?subject=Hello%20there"),
        (json!({"email": "  ann@example.com  "}), "mailto:ann@example.com"),
        (json!({"email": "MAILTO:ann@example.com"}), "mailto:ann@example.com"),
        (json!({"email": "ann@example.com", "subject": ""}), "mailto:ann@example.com"),
        (json!({"email": "a@b.c", "subject": "Q&A: 50% off? #1"}), "mailto:a@b.c?subject=Q%26A%3A%2050%25%20off%3F%20%231"),
        (json!({"email": "a@b.c", "subject": "Café"}), "mailto:a@b.c?subject=Caf%C3%A9"),
    ];
    for (params, want) in cases {
        assert_eq!(target(params.clone()), want, "{params}");
    }
}

#[test]
fn email_link_text_defaults_to_the_target_and_takes_text_to_display() {
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    run(&mut s, "insert.link", json!({"email": "ann@example.com", "subject": "Hi"}));
    assert_eq!(links(&s), vec![("mailto:ann@example.com?subject=Hi".to_string(), "mailto:ann@example.com?subject=Hi".to_string())]);
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    run(&mut s, "insert.link", json!({"email": "ann@example.com", "text": "Write to Ann"}));
    assert_eq!(links(&s), vec![("mailto:ann@example.com".to_string(), "Write to Ann".to_string())]);
}

#[test]
fn a_url_keeps_priority_over_an_email() {
    assert_eq!(target(json!({"url": "https://example.com", "email": "ann@example.com"})), "https://example.com");
}

#[test]
fn a_blank_email_address_is_refused_and_changes_nothing() {
    for blank in ["", "   "] {
        let mut s = Session::new(wordcraft_doc::Document::from_text("Body"));
        run(&mut s, "caret.docEnd", json!({}));
        let before = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<Vec<_>>();
        assert!(s.run("insert.link", &json!({"email": blank, "subject": "x"})).is_err(), "{blank:?} accepted");
        let after = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<Vec<_>>();
        assert_eq!(before, after);
        assert!(links(&s).is_empty());
    }
}

#[test]
fn link_targets_lists_the_documents_email_links_once_in_order() {
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    assert_eq!(run(&mut s, "link.targets", json!({}))["emails"], json!([]));
    for params in [
        json!({"email": "a@b.c", "subject": "Hi there", "text": "one"}),
        json!({"url": "https://x.org", "text": " two "}),
        json!({"email": "z@y.x", "text": " three "}),
        json!({"email": "a@b.c", "subject": "Hi there", "text": " four"}),
    ] {
        run(&mut s, "insert.link", params);
    }
    assert_eq!(
        run(&mut s, "link.targets", json!({}))["emails"],
        json!([{"address": "a@b.c", "subject": "Hi there"}, {"address": "z@y.x", "subject": ""}])
    );
}

#[test]
fn link_targets_decodes_subjects_and_ignores_other_headers() {
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    for (url, text) in [
        ("mailto:p@q.r?cc=s@t.u&subject=Caf%C3%A9%20menu", "one"),
        ("mailto:m@n.o?subject=a+b%ZZ", " two"),
        ("mailto:k@l.m?body=Hello", " three"),
    ] {
        run(&mut s, "insert.link", json!({"url": url, "text": text}));
    }
    assert_eq!(
        run(&mut s, "link.targets", json!({}))["emails"],
        json!([
            {"address": "p@q.r", "subject": "Café menu"},
            {"address": "m@n.o", "subject": "a+b%ZZ"},
            {"address": "k@l.m", "subject": ""}
        ])
    );
}

#[test]
fn email_links_survive_a_docx_round_trip() {
    let mut s = Session::new(wordcraft_doc::Document::from_text(""));
    run(&mut s, "insert.link", json!({"email": "ann@example.com", "subject": "Q&A: 50% off", "text": "Ann"}));
    let back = Session::new(wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap()).unwrap());
    assert_eq!(links(&back), vec![("mailto:ann@example.com?subject=Q%26A%3A%2050%25%20off".to_string(), "Ann".to_string())]);
}
