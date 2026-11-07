//! A table of figures links its entries to their captions, as Word's Insert Table of Figures does
//! with "Use hyperlinks instead of page numbers" (on by default): `TOC \h \z \c "Figure"`, each
//! entry a link to a hidden `_Toc` bookmark around its caption, the page a `PAGEREF … \h` field
//! (#5783).

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;
use wordcraft_doc::para::{InlineObject, Paragraph};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

/// "Kiln" captioned "Figure 1: The kiln", "Wheel" captioned "Figure 2: The wheel", then the
/// table of figures with `params` at the end.
fn with_figures(params: Value) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("Kiln\nWheel\n"));
    for (what, text) in [("Kiln", "The kiln"), ("Wheel", "The wheel")] {
        run(&mut s, "select.text", json!({"text": what}));
        run(&mut s, "references.caption", json!({"label": "Figure", "text": text}));
    }
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "references.tableOfFigures", params);
    s
}

fn entries(s: &Session) -> Vec<&Paragraph> {
    s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| p.props.style.as_deref() == Some("ListEntry")).collect()
}

fn link(p: &Paragraph) -> Option<String> {
    p.props_of_char(0).link.clone()
}

fn fields(p: &Paragraph) -> Vec<(String, String)> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, .. } => Some((instr.trim().to_string(), result.clone())),
            _ => None,
        })
        .collect()
}

fn list_instr(s: &Session) -> String {
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, .. } = o
                && instr.trim_start().starts_with("TOC")
            {
                return instr.trim().to_string();
            }
        }
    }
    panic!("no TOC field")
}

fn toc_bookmarks(s: &Session) -> Vec<String> {
    let mut v: Vec<String> = s.doc.bookmarks().into_iter().map(|(n, _)| n).filter(|n| n.starts_with("_Toc")).collect();
    v.sort();
    v
}

#[test]
fn the_default_list_is_written_with_hyperlinks() {
    let s = with_figures(json!({}));
    assert_eq!(list_instr(&s), r#"TOC \h \z \c "Figure""#);
    assert_eq!(crate::cmd::citations::figures_instr(&json!({"hyperlinks": false})), r#"TOC \c "Figure""#);
    assert_eq!(crate::cmd::citations::figures_instr(&json!({"label": "Table", "includeLabel": false})), r#"TOC \h \z \a "Table""#);
    assert_eq!(crate::cmd::citations::figures_instr(&json!({"pageNumbers": false})), r#"TOC \h \z \c "Figure" \n"#);
    assert_eq!(crate::cmd::citations::figures_instr(&json!({"rightAlign": false})), r#"TOC \h \z \c "Figure" \p " ""#);
}

#[test]
fn each_entry_links_to_a_bookmark_around_its_caption() {
    let s = with_figures(json!({}));
    let e = entries(&s);
    assert_eq!(e.iter().map(|p| p.plain_text()).collect::<Vec<_>>(), ["Figure 1: The kiln\t1", "Figure 2: The wheel\t1"]);
    let marks = toc_bookmarks(&s);
    assert_eq!(marks.len(), 2, "one _Toc bookmark per caption: {marks:?}");
    for (p, caption) in e.iter().zip(["Figure 1: The kiln", "Figure 2: The wheel"]) {
        let target = link(p).unwrap_or_else(|| panic!("entry {:?} has no link", p.plain_text()));
        let name = target.strip_prefix('#').expect("an in-document link");
        assert!(name.starts_with("_Toc"), "{name}");
        assert_eq!(crate::cmd::references::bookmark_text(&s.doc, name).as_deref(), Some(caption));
        // The page is a PAGEREF to the same bookmark, and is linked too.
        assert_eq!(fields(p), [(format!("PAGEREF {name} \\h"), "1".to_string())]);
        let last = p.len().saturating_sub(1);
        assert_eq!(p.props_of_char(last).link.as_deref(), Some(target.as_str()), "the page number links too");
    }
}

#[test]
fn updating_again_reuses_the_bookmarks() {
    let mut s = with_figures(json!({}));
    let before = toc_bookmarks(&s);
    assert_eq!(before.len(), 2, "{before:?}");
    let links: Vec<_> = entries(&s).iter().map(|p| link(p)).collect();
    run(&mut s, "references.updateFigures", json!({}));
    run(&mut s, "references.updateFigures", json!({}));
    assert_eq!(toc_bookmarks(&s), before);
    assert_eq!(entries(&s).iter().map(|p| link(p)).collect::<Vec<_>>(), links);
}

#[test]
fn without_page_numbers_entries_still_link() {
    let s = with_figures(json!({"pageNumbers": false}));
    let e = entries(&s);
    assert_eq!(e.iter().map(|p| p.plain_text()).collect::<Vec<_>>(), ["Figure 1: The kiln", "Figure 2: The wheel"]);
    for p in e {
        assert!(link(p).is_some_and(|l| l.starts_with("#_Toc")), "{:?}", p.plain_text());
        assert!(fields(p).is_empty(), "no PAGEREF without page numbers");
    }
}

#[test]
fn without_hyperlinks_the_list_is_plain_text_and_captions_get_no_bookmarks() {
    let s = with_figures(json!({"hyperlinks": false}));
    assert_eq!(list_instr(&s), r#"TOC \c "Figure""#);
    let e = entries(&s);
    assert_eq!(e.iter().map(|p| p.plain_text()).collect::<Vec<_>>(), ["Figure 1: The kiln\t1", "Figure 2: The wheel\t1"]);
    assert!(e.iter().all(|p| link(p).is_none()));
    assert!(toc_bookmarks(&s).is_empty(), "{:?}", toc_bookmarks(&s));
}
