use super::*;

/// #3300: Replace All, as in Word, replaces in the main text, headers, footers, footnotes,
/// endnotes and text boxes, wherever the caret is. Comments keep their words.
#[test]
fn replace_all_replaces_in_every_story() {
    use wordcraft_doc::{CharProps, Paragraph, PartKind, StoryRef, para_block};
    let story_text = |t: &Session, story: StoryRef| {
        let first = wordcraft_doc::Pos::new(story, wordcraft_doc::Path::top(0), 0);
        if story == StoryRef::Body { t.doc.plain_text(story) } else { t.doc.para_at(&first).map(|p| p.text.clone()).unwrap_or_default() }
    };
    let build = || {
        let mut t = s();
        run(&mut t, "document.setText", json!({"text": "Body: [NAME] and [NAME]."}));
        // A text box the body shows (one nothing shows is pruned with the next command).
        let boxed = run(&mut t, "insert.textBox", json!({"text": "Text box: [NAME]"}))["story"].as_u64().unwrap() as u32;
        let mut add = |kind, text: &str| t.doc.add_part(kind, vec![para_block(Paragraph::with_text(text, CharProps::default()))]);
        let ids = [
            add(PartKind::Header, "Header: [NAME]"),
            add(PartKind::Footer, "Footer: [NAME]"),
            add(PartKind::Footnote, " Footnote: [NAME]"),
            add(PartKind::Endnote, " Endnote: [NAME]"),
            boxed,
            add(PartKind::Comment, "Comment: [NAME]"),
        ];
        (t, ids)
    };
    let (mut t, ids) = build();
    let r = run(&mut t, "edit.replaceAll", json!({"text": "[NAME]", "with": "Jane"}));
    assert_eq!(r["replaced"], 7, "{r}");
    assert_eq!(story_text(&t, StoryRef::Body), "Body: Jane and Jane.");
    let texts: Vec<String> = ids.iter().map(|id| story_text(&t, StoryRef::Part(*id))).collect();
    assert_eq!(texts, ["Header: Jane", "Footer: Jane", " Footnote: Jane", " Endnote: Jane", "Text box: Jane", "Comment: [NAME]"]);
    // One undo step brings every story back.
    run(&mut t, "edit.undo", json!({}));
    assert_eq!(story_text(&t, StoryRef::Body), "Body: [NAME] and [NAME].");
    assert_eq!(story_text(&t, StoryRef::Part(ids[0])), "Header: [NAME]");
    assert_eq!(story_text(&t, StoryRef::Part(ids[2])), " Footnote: [NAME]");
    // From the caret in a footnote the body is replaced too.
    let (mut t, ids) = build();
    t.sel = crate::session::Selection::caret(wordcraft_doc::Pos::new(StoryRef::Part(ids[2]), wordcraft_doc::Path::top(0), 0));
    let r = run(&mut t, "edit.replaceAll", json!({"text": "[NAME]", "with": "Jane"}));
    assert_eq!(r["replaced"], 7, "{r}");
    assert_eq!(story_text(&t, StoryRef::Body), "Body: Jane and Jane.");
    // Under Track Changes the replacements are revisions in every story, and nothing breaks.
    let (mut t, ids) = build();
    run(&mut t, "review.trackChanges", json!({"enabled": true}));
    let r = run(&mut t, "edit.replaceAll", json!({"text": "[NAME]", "with": "Jane"}));
    assert_eq!(r["replaced"], 7, "{r}");
    assert!(story_text(&t, StoryRef::Part(ids[0])).contains("Jane"));
}
