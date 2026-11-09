//! Review › Proofing › Readability Statistics: the counts, averages and Flesch scores Word shows
//! after a spelling check when File › Options › Proofing › "Show readability statistics" is on.
//! Passive sentences aren't reported (#4152).

use super::{CmdResult, Session, Value, json};

/// Closing punctuation that may follow a sentence's `.`, `!` or `?` (`"Stop!"`, `(Yes.)`).
fn is_closer(c: char) -> bool {
    matches!(c, '"' | '\'' | ')' | ']' | '}' | '\u{201D}' | '\u{2019}' | '\u{00BB}' | '\u{203A}')
}

/// A token (text between white space) that ends a sentence: `.`, `!` or `?` last, optionally
/// followed by closing quotes or brackets. A period inside a token (`3.14`) doesn't count.
fn ends_sentence(token: &str) -> bool {
    token.trim_end_matches(is_closer).ends_with(['.', '!', '?'])
}

/// Syllables in one word, estimated from its letters: groups of consecutive vowels (`y`
/// included), less a final silent `e` (not `-le`), at least one. A token without a letter has none.
pub(crate) fn syllables(word: &str) -> usize {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
    if letters.is_empty() {
        return 0;
    }
    let vowel = |c: &char| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
    let mut groups = 0usize;
    let mut in_group = false;
    for c in &letters {
        let v = vowel(c);
        if v && !in_group {
            groups = groups.saturating_add(1);
        }
        in_group = v;
    }
    let silent_e = letters.last() == Some(&'e') && !letters.ends_with(&['l', 'e']) && groups > 1;
    if silent_e {
        groups -= 1;
    }
    groups.max(1)
}

/// Sentences and syllables in `text` (paragraphs separated by `\n`), and the paragraphs that
/// hold at least one word. A paragraph's last words without terminal punctuation are a sentence.
fn sentences_syllables_paragraphs(text: &str) -> (usize, usize, usize) {
    let (mut sentences, mut syl, mut paragraphs) = (0usize, 0usize, 0usize);
    for para in text.split('\n') {
        if wordcraft_doc::count_words(para) == 0 {
            continue;
        }
        paragraphs = paragraphs.saturating_add(1);
        let mut open = false;
        for token in para.split(|c: char| c.is_whitespace() || c == wordcraft_doc::para::OBJ).filter(|t| !t.is_empty()) {
            if wordcraft_doc::count_words(token) > 0 {
                open = true;
                syl = syl.saturating_add(syllables(token));
            }
            if open && ends_sentence(token) {
                sentences = sentences.saturating_add(1);
                open = false;
            }
        }
        if open {
            sentences = sentences.saturating_add(1);
        }
    }
    (sentences, syl, paragraphs)
}

/// `a / b`, or 0 when `b` is 0.
fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

pub(super) fn readability(s: &mut Session, _: &Value) -> CmdResult {
    // As Word Count: a selected picture, shape or text box isn't a text selection.
    let whole = s.sel.is_collapsed() || crate::cmd::objects::object_selection(s).is_some();
    let text = if whole { s.doc.plain_text(wordcraft_doc::StoryRef::Body) } else { s.selected_text() };
    Ok(readability_json(&text))
}

/// The statistics for `text` (paragraphs separated by `\n`).
fn readability_json(text: &str) -> Value {
    let words = wordcraft_doc::count_words(text);
    let characters = wordcraft_doc::count_chars_no_spaces(text);
    let (sentences, syllables, paragraphs) = sentences_syllables_paragraphs(text);
    let wps = ratio(words, sentences);
    let spw = ratio(syllables, words);
    let (ease, grade) = if words == 0 || sentences == 0 {
        (0.0, 0.0)
    } else {
        ((206.835 - 1.015 * wps - 84.6 * spw).clamp(0.0, 100.0), (0.39 * wps + 11.8 * spw - 15.59).max(0.0))
    };
    json!({
        "words": words, "characters": characters, "paragraphs": paragraphs, "sentences": sentences, "syllables": syllables,
        "sentencesPerParagraph": ratio(sentences, paragraphs), "wordsPerSentence": wps, "charactersPerWord": ratio(characters, words),
        "fleschReadingEase": ease, "fleschKincaidGrade": grade,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllable_rules() {
        for (w, n) in [("the", 1), ("make", 1), ("table", 2), ("elephant", 3), ("Tomato,", 3), ("rhythm", 1), ("3.14", 0), ("x", 1)] {
            assert_eq!(syllables(w), n, "{w}");
        }
    }

    #[test]
    fn closing_quotes_after_the_stop_end_a_sentence() {
        assert_eq!(sentences_syllables_paragraphs("He said \"Stop.\" Then (he left.) e.g.x is fine").0, 3);
    }
}
