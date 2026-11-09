//! Readability Statistics (Word's box after a spelling check with "Show readability statistics"
//! on): counts, averages, Flesch Reading Ease and Flesch-Kincaid Grade Level.

use serde_json::{Value, json};
use wordcraft_doc::{Document, Pos};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn stats(text: &str) -> Value {
    let mut s = Session::new(Document::from_text(text));
    run(&mut s, "review.readability", json!({}))
}

fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("{k} missing in {v}"))
}

fn close(v: &Value, k: &str, want: f64) {
    let got = f(v, k);
    assert!((got - want).abs() < 1e-6, "{k}: got {got}, want {want}");
}

#[test]
fn counts_averages_and_scores() {
    // 12 words, 2 sentences, 20 syllables (elephant, banana, animal, tomato: 3 each), 48
    // characters without spaces.
    let v = stats("The big elephant had a banana. His animal got a red tomato.\n");
    assert_eq!(
        (v["words"].as_u64(), v["sentences"].as_u64(), v["paragraphs"].as_u64(), v["syllables"].as_u64(), v["characters"].as_u64()),
        (Some(12), Some(2), Some(1), Some(20), Some(48))
    );
    close(&v, "sentencesPerParagraph", 2.0);
    close(&v, "wordsPerSentence", 6.0);
    close(&v, "charactersPerWord", 4.0);
    // 206.835 - 1.015 * 6 - 84.6 * 20/12
    close(&v, "fleschReadingEase", 59.745);
    // 0.39 * 6 + 11.8 * 20/12 - 15.59
    close(&v, "fleschKincaidGrade", 6.416_666_666_666_667);
}

#[test]
fn scores_are_clamped_like_words() {
    // Raw: reading ease 116.145, grade -1.45.
    let v = stats("The cat sat on the mat.\n");
    assert_eq!(v["syllables"].as_u64(), Some(6));
    close(&v, "fleschReadingEase", 100.0);
    close(&v, "fleschKincaidGrade", 0.0);
}

#[test]
fn sentences_end_at_terminal_punctuation_or_the_paragraph_end() {
    let v = stats("One two. Three four five!\nSix\n");
    assert_eq!((v["words"].as_u64(), v["sentences"].as_u64(), v["paragraphs"].as_u64()), (Some(6), Some(3), Some(2)));
    close(&v, "sentencesPerParagraph", 1.5);
    close(&v, "wordsPerSentence", 2.0);
}

#[test]
fn a_period_inside_a_number_does_not_end_a_sentence() {
    let v = stats("Pi is 3.14 today. Yes?!\n");
    assert_eq!((v["words"].as_u64(), v["sentences"].as_u64()), (Some(5), Some(2)));
}

#[test]
fn an_empty_document_gives_zeros_not_nan() {
    let v = stats("");
    for k in ["words", "sentences", "paragraphs", "syllables"] {
        assert_eq!(v[k].as_u64(), Some(0), "{k}");
    }
    for k in ["sentencesPerParagraph", "wordsPerSentence", "charactersPerWord", "fleschReadingEase", "fleschKincaidGrade"] {
        close(&v, k, 0.0);
    }
}

#[test]
fn a_selection_limits_the_statistics() {
    let mut s = Session::new(Document::from_text("The cat sat on the mat.\nThe big elephant had a banana.\n"));
    s.sel = Selection { anchor: Pos::body(1, 0), focus: Pos::body(1, 30) };
    let v = run(&mut s, "review.readability", json!({}));
    assert_eq!((v["words"].as_u64(), v["sentences"].as_u64(), v["syllables"].as_u64()), (Some(6), Some(1), Some(10)));
    // A caret means the whole text.
    s.sel = Selection::caret(Pos::body(0, 0));
    let v = run(&mut s, "review.readability", json!({}));
    assert_eq!(v["words"].as_u64(), Some(12));
}

#[test]
fn show_readability_option_toggles_and_sticks() {
    let mut s = Session::new(Document::from_text("Hi.\n"));
    assert_eq!(run(&mut s, "review.showReadability", json!({"value": true}))["value"], json!(true));
    assert!(s.prefs.show_readability);
    assert_eq!(run(&mut s, "review.showReadability", json!({}))["value"], json!(false));
    assert!(!s.prefs.show_readability);
}
