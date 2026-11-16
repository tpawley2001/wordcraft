//! Readability Statistics: Passive Sentences, the share of sentences in the passive voice, as
//! Word reports it (#4152). Every sentence below was measured one per document in Microsoft 365
//! Word (en-US): `PASSIVE` ones read 100%, `ACTIVE` ones 0%.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

fn stats(text: &str) -> Value {
    let mut s = Session::new(Document::from_text(text));
    s.run("review.readability", &json!({})).unwrap_or_else(|e| panic!("review.readability: {e}"))
}

fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or_else(|| panic!("{k} missing or not a whole number in {v}"))
}

const PASSIVE: &[&str] = &[
    "The ball was thrown by the boy.",
    "The cake was eaten.",
    "The letters were being written.",
    "The work has been done.",
    "She got fired yesterday.",
    "The house was built in 1990.",
    "They will be paid tomorrow.",
    "The song was sung well.",
    "The window was broken.",
    "The door is closed.",
    "The report is written in English.",
    "Mistakes were made.",
    "The file is saved automatically.",
    "The car needs to be washed.",
    "Being chosen was an honour.",
    "The cat was seen and the dog barked.",
    "The book's been read.",
    "It is said that he left.",
    "It was decided by the board.",
    "The man was bored.",
    "The man was killed.",
    "The letter was not sent.",
    "The letter was quickly sent.",
    "The letter was never really sent.",
    "He is called John.",
    "He was given a book.",
    "He got hurt.",
    "The match is being played.",
    "The task had been completed.",
    "The man was hit.",
    "The cars were put away.",
    "The window was opened.",
    "The shop was closed.",
    "The text was formatted.",
    "the ball was thrown.",
    "The data were analysed.",
    "The documents are attached.",
    "It is required.",
    "This is done.",
    "The meeting is scheduled for Monday.",
];

const ACTIVE: &[&str] = &[
    "The boy threw the ball.",
    "He was happy.",
    "It is known that cats sleep.",
    "He was running home.",
    "The man was tired.",
    "I am interested in art.",
    "He has gone home.",
    "Was the letter sent?",
    "Were the letters sent?",
    "The fact is known.",
    "She was excited.",
    "They were worried.",
    "The man was interested.",
    "They are known as the best.",
    "The cat got wet.",
    "He became known.",
    "He seems tired.",
    "The sea was blue.",
    "The room was clean.",
    "The glass is empty.",
    "He was married.",
    "He is gone.",
    "The tired man was sleeping.",
];

#[test]
fn passive_sentences_read_100_percent() {
    let wrong: Vec<_> = PASSIVE
        .iter()
        .filter(|t| {
            let v = stats(t);
            (n(&v, "passiveCount"), n(&v, "passiveSentences")) != (1, 100)
        })
        .collect();
    assert!(wrong.is_empty(), "Word counts these as passive: {wrong:?}");
}

#[test]
fn active_sentences_read_0_percent() {
    let wrong: Vec<_> = ACTIVE
        .iter()
        .filter(|t| {
            let v = stats(t);
            (n(&v, "passiveCount"), n(&v, "passiveSentences")) != (0, 0)
        })
        .collect();
    assert!(wrong.is_empty(), "Word doesn't count these as passive: {wrong:?}");
}

#[test]
fn the_percentage_is_of_the_counted_sentences_rounded() {
    let v = stats("The boy threw the ball. The girl caught it. The ball was dropped. The game ended.");
    assert_eq!((n(&v, "sentences"), n(&v, "passiveCount"), n(&v, "passiveSentences")), (4, 1, 25));
    let v = stats("The boy threw the ball. The ball was caught. The ball was dropped.");
    assert_eq!((n(&v, "sentences"), n(&v, "passiveCount"), n(&v, "passiveSentences")), (3, 2, 67));
    let v = stats("The ball was thrown.\nThe boy ran.");
    assert_eq!((n(&v, "sentences"), n(&v, "passiveCount"), n(&v, "passiveSentences")), (2, 1, 50));
}

#[test]
fn no_sentences_is_zero_percent() {
    let v = stats("");
    assert_eq!((n(&v, "passiveCount"), n(&v, "passiveSentences")), (0, 0));
}
