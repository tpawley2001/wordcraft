//! Sentence boundaries as Word finds them (Select Sentence, F8, Delete Previous Sentence): a
//! sentence also ends after a closing `”`, `)` or `]` that follows its `.`, `!` or `?` (even with no
//! space after the closer), and tabs and line breaks after the end belong to it like spaces.
//! Measured in Microsoft 365 Word 16.0 with `Range.Sentences`, 2026-11-14.

use serde_json::json;
use wordcraft_doc::para::LINE_BREAK;
use wordcraft_doc::{CharProps, Document, Paragraph, StoryRef};

use crate::Session;

/// The paragraph's sentences, walked from the start with `sentence_at`.
fn sentences(text: &str) -> Vec<String> {
    let p = Paragraph::with_text(text, CharProps::default());
    let mut out = Vec::new();
    let mut off = 0;
    while off < text.len() && out.len() <= text.len() {
        let (a, b) = p.sentence_at(off);
        assert_eq!(a, off, "{text:?}: the sentence at {off} starts at {a}");
        assert!(b > a, "{text:?}: empty sentence at {off}");
        out.push(text[a..b].to_string());
        off = b;
    }
    out
}

#[test]
fn closing_quotes_and_brackets_end_the_sentence() {
    for (text, want) in [
        ("He said \u{201C}Stop.\u{201D} Then left.", vec!["He said \u{201C}Stop.\u{201D} ", "Then left."]),
        ("(See this.) Next one.", vec!["(See this.) ", "Next one."]),
        ("A [ref.] here. Z.", vec!["A [ref.] ", "here. ", "Z."]),
    ] {
        assert_eq!(sentences(text), want, "{text:?}");
    }
}

#[test]
fn a_closer_ends_the_sentence_even_without_a_space() {
    assert_eq!(sentences("Odd.)Next one."), ["Odd.)", "Next one."]);
    // No closer and no space: one sentence, as before.
    assert_eq!(sentences("End.Next starts here."), ["End.Next starts here."]);
}

#[test]
fn tabs_and_line_breaks_after_the_end_belong_to_the_sentence() {
    assert_eq!(sentences("One.\tTab next."), ["One.\t", "Tab next."]);
    let lb = format!("One.{LINE_BREAK}Line next.");
    assert_eq!(sentences(&lb), [format!("One.{LINE_BREAK}"), "Line next.".to_string()]);
    assert_eq!(sentences("End.  Two spaces."), ["End.  ", "Two spaces."]);
}

#[test]
fn terminator_runs_abbreviations_and_quotes_as_word() {
    for (text, want) in [
        // A run of terminators is one end (ours split "Go?" / "! ").
        ("Go?! Now.", vec!["Go?! ", "Now."]),
        // A straight single quote after the period is not a closer in Word.
        ("It's 'done.' Next.", vec!["It's 'done.' Next."]),
        // Word has no abbreviation list.
        ("Fig. 3 shows it. Ok.", vec!["Fig. ", "3 shows it. ", "Ok."]),
        ("Mr. Smith went home. He slept.", vec!["Mr. ", "Smith went home. ", "He slept."]),
        ("Is it 3.5? Maybe.", vec!["Is it 3.5? ", "Maybe."]),
    ] {
        assert_eq!(sentences(text), want, "{text:?}");
    }
}

#[test]
fn a_caret_inside_any_part_of_a_sentence_finds_it() {
    let text = "He said \u{201C}Stop.\u{201D} Then left.";
    let p = Paragraph::with_text(text, CharProps::default());
    let then = text.find("Then").unwrap();
    // On the period, the closing quote and the space after it: the first sentence.
    for off in [text.find('.').unwrap(), text.find('\u{201D}').unwrap(), then - 1] {
        assert_eq!(p.sentence_at(off), (0, then), "at {off}");
    }
    assert_eq!(p.sentence_at(then + 2), (then, text.len()));
}

fn session(text: &str, caret: usize) -> Session {
    let mut s = Session::new(Document::new());
    s.run("text.insert", &json!({ "text": text, "raw": true })).unwrap();
    let mut pos = s.sel.focus.clone();
    pos.off = caret;
    s.sel = crate::Selection::caret(pos);
    s
}

#[test]
fn select_sentence_stops_after_the_closing_bracket() {
    let text = "(See this.) Next one.";
    let mut s = session(text, text.find("Next").unwrap() + 2);
    s.run("select.sentence", &json!({})).unwrap();
    let (a, b) = s.sel.ordered();
    assert_eq!((a.off, b.off), (text.find("Next").unwrap(), text.len()));
    let mut s = session(text, 3);
    s.run("select.sentence", &json!({})).unwrap();
    let (a, b) = s.sel.ordered();
    assert_eq!((a.off, b.off), (0, text.find("Next").unwrap()));
}

#[test]
fn delete_previous_sentence_stops_after_the_closing_quote() {
    let text = "He said \u{201C}Stop.\u{201D} Then left.";
    let mut s = session(text, text.len());
    s.run("text.deleteSentenceBack", &json!({})).unwrap();
    let path = s.doc.para_paths(StoryRef::Body).remove(0);
    assert_eq!(s.doc.para(StoryRef::Body, &path).unwrap().text, "He said \u{201C}Stop.\u{201D} ");
}
