//! Word Count's Paragraphs figure as Microsoft 365 Word computes it (2026-11-14, Range.ComputeStatistics
//! wdStatisticParagraphs): only paragraphs holding at least one word count.
use super::*;

fn paras(texts: &[&str]) -> u64 {
    let mut s = s();
    for (i, t) in texts.iter().enumerate() {
        if i > 0 {
            run(&mut s, "text.newParagraph", json!({}));
        }
        if !t.is_empty() {
            run(&mut s, "text.insert", json!({"text": t}));
        }
    }
    run(&mut s, "caret.set", json!({"pos": Pos::body(0, 0)}));
    run(&mut s, "review.wordCount", json!({}))["paragraphs"].as_u64().unwrap()
}

#[test]
fn paragraphs_with_words_count() {
    for (texts, want) in [
        (&["a"][..], 1),
        (&["a", "b"][..], 2),
        (&["a", "", "b"][..], 2),
        (&["a", " ", "b"][..], 2),
        (&["a", "\t", "b"][..], 2),
        (&["a", ".", "b"][..], 3),
        (&["a", "", ""][..], 1),
        (&["", "", ""][..], 0),
    ] {
        assert_eq!(paras(texts), want, "{texts:?}");
    }
}

#[test]
fn paragraphs_without_a_word_do_not_count() {
    for (texts, want) in [
        (&["a", "\u{00A0}", "b"][..], 2),
        (&["a", "\u{200B}", "b"][..], 2),
        (&["a", "\u{2014}", "b"][..], 2),
    ] {
        assert_eq!(paras(texts), want, "{texts:?}");
    }
}

fn paras_with_break(before: &str, after: &str) -> u64 {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "a"}));
    run(&mut s, "text.newParagraph", json!({}));
    if !before.is_empty() {
        run(&mut s, "text.insert", json!({"text": before}));
    }
    run(&mut s, "text.lineBreak", json!({}));
    if !after.is_empty() {
        run(&mut s, "text.insert", json!({"text": after}));
    }
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "b"}));
    run(&mut s, "caret.set", json!({"pos": Pos::body(0, 0)}));
    run(&mut s, "review.wordCount", json!({}))["paragraphs"].as_u64().unwrap()
}

#[test]
fn a_line_break_does_not_start_a_paragraph() {
    // Word: a | <line break only> | b = 2 paragraphs (measured); a | x<line break>y | b = 3 (one paragraph holds both lines).
    assert_eq!(paras_with_break("", ""), 2);
    assert_eq!(paras_with_break("x", "y"), 3);
}
