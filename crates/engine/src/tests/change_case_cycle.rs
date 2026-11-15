//! Shift+F3 (Change Case with no mode) as Microsoft 365 Word does it (Range.Case = wdNextCase on
//! the same text, 2026-11-14): the next case depends on the selected text, its first cased letter
//! and whether it holds `.`, `!` or `?`.

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

fn body(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
}

fn session(text: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": text, "raw": true}));
    s
}

/// The paragraph after each of `n` Shift+F3 presses with `select` (or everything) selected.
fn cycle(text: &str, select: Option<&str>, n: usize) -> Vec<String> {
    let mut s = session(text);
    match select {
        Some(t) => run(&mut s, "select.text", json!({"text": t})),
        None => run(&mut s, "select.all", json!({})),
    }
    (0..n)
        .map(|_| {
            run(&mut s, "format.changeCase", json!({}));
            body(&s)
        })
        .collect()
}

const PUNCT: &str = "hello world. next one? yes! i'm o'neil-smith from mcdonald's, e.g. test";
const MIXED: &str = "HELLO wORLD. tHE iPhone 3D and NASA";

#[test]
fn shift_f3_picks_the_next_case_from_the_selected_text() {
    // Each row: the text, then the paragraph after 1, 2, 3 and 4 presses.
    let rows: [(&str, [&str; 4]); 13] = [
        ("hello world", ["Hello World", "HELLO WORLD", "hello world", "Hello World"]),
        ("Hello World", ["HELLO WORLD", "hello world", "Hello World", "HELLO WORLD"]),
        ("HELLO WORLD", ["hello world", "Hello World", "HELLO WORLD", "hello world"]),
        ("Hello world", ["HELLO WORLD", "hello world", "Hello World", "HELLO WORLD"]),
        ("hello", ["Hello", "HELLO", "hello", "Hello"]),
        ("hello world.", ["Hello world.", "HELLO WORLD.", "hello world.", "Hello world."]),
        ("Hello world.", ["HELLO WORLD.", "hello world.", "Hello world.", "HELLO WORLD."]),
        ("one. two", ["One. Two", "ONE. TWO", "one. two", "One. Two"]),
        ("x", ["X", "x", "X", "x"]),
        ("hello world!", ["Hello world!", "HELLO WORLD!", "hello world!", "Hello world!"]),
        ("hello world?", ["Hello world?", "HELLO WORLD?", "hello world?", "Hello world?"]),
        ("mixed Case words", ["MIXED CASE WORDS", "mixed case words", "Mixed Case Words", "MIXED CASE WORDS"]),
        ("hello wORLD.", ["Hello world.", "HELLO WORLD.", "hello world.", "Hello world."]),
    ];
    for (input, want) in rows {
        assert_eq!(cycle(input, None, 4), want, "{input}");
    }
}

#[test]
fn shift_f3_cycles_longer_text_with_sentences() {
    assert_eq!(
        cycle(PUNCT, None, 4),
        [
            "Hello world. Next one? Yes! I'm o'neil-smith from mcdonald's, e.g. Test",
            "HELLO WORLD. NEXT ONE? YES! I'M O'NEIL-SMITH FROM MCDONALD'S, E.G. TEST",
            PUNCT,
            "Hello world. Next one? Yes! I'm o'neil-smith from mcdonald's, e.g. Test",
        ]
    );
    assert_eq!(
        cycle(MIXED, None, 4),
        [
            "HELLO WORLD. THE IPHONE 3D AND NASA",
            "hello world. the iphone 3d and nasa",
            "Hello world. The iphone 3d and nasa",
            "HELLO WORLD. THE IPHONE 3D AND NASA",
        ]
    );
}

#[test]
fn shift_f3_on_part_of_a_sentence_looks_only_at_the_selection() {
    // The selection holds no . ! or ?, so lowercase text goes to Capitalize Each Word.
    assert_eq!(
        cycle("the quick brown fox. jumps", Some("quick brown"), 4),
        [
            "the Quick Brown fox. jumps",
            "the QUICK BROWN fox. jumps",
            "the quick brown fox. jumps",
            "the Quick Brown fox. jumps",
        ]
    );
}
