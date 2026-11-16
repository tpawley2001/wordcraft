//! English passive-sentence word patterns for Readability Statistics.

// Word-measured adjective exceptions after be/get (Microsoft 365 Word, en-US).
const PASSIVE_ADJECTIVES: &[&str] =
    &["known", "gone", "tired", "interested", "excited", "worried", "surprised", "married", "wet", "clean", "empty", "blue", "happy"];

const IRREGULAR_PARTICIPLES: &[&str] = &[
    "arisen",
    "awoken",
    "been",
    "beaten",
    "become",
    "begun",
    "bent",
    "bet",
    "bitten",
    "bled",
    "blown",
    "broken",
    "brought",
    "built",
    "burnt",
    "burst",
    "bought",
    "caught",
    "chosen",
    "come",
    "cost",
    "crept",
    "cut",
    "dealt",
    "done",
    "drawn",
    "dreamt",
    "driven",
    "drunk",
    "eaten",
    "fallen",
    "fed",
    "felt",
    "fought",
    "found",
    "fled",
    "flown",
    "forbidden",
    "forgotten",
    "forgiven",
    "frozen",
    "given",
    "gone",
    "grown",
    "hung",
    "had",
    "heard",
    "hidden",
    "hit",
    "held",
    "hurt",
    "kept",
    "knelt",
    "known",
    "laid",
    "led",
    "left",
    "lent",
    "let",
    "lain",
    "lit",
    "lost",
    "made",
    "meant",
    "met",
    "paid",
    "put",
    "quit",
    "read",
    "ridden",
    "rung",
    "risen",
    "run",
    "said",
    "seen",
    "sought",
    "sold",
    "sent",
    "set",
    "shaken",
    "shed",
    "shone",
    "shot",
    "shown",
    "shut",
    "sung",
    "sunk",
    "sat",
    "slain",
    "slept",
    "slid",
    "spoken",
    "spent",
    "spun",
    "split",
    "spread",
    "stood",
    "stolen",
    "stuck",
    "stung",
    "struck",
    "sworn",
    "swept",
    "swum",
    "swung",
    "taken",
    "taught",
    "torn",
    "told",
    "thought",
    "thrown",
    "understood",
    "woken",
    "worn",
    "woven",
    "wept",
    "won",
    "wound",
    "written",
];

fn participle(word: &str) -> bool {
    IRREGULAR_PARTICIPLES.contains(&word) || (word.chars().take(4).count() == 4 && word.ends_with("ed") && !word.ends_with("eed"))
}

fn passive_skip(word: &str) -> bool {
    matches!(word, "not" | "never" | "really" | "quickly" | "also" | "just" | "already" | "being" | "been" | "be")
        || word.ends_with("ly")
        || word.ends_with("n't")
        || word.ends_with("n’t")
}

/// Check one sentence's existing whitespace tokens, preserving punctuation for questions.
pub(super) fn is_passive(sentence_tokens: &[&str]) -> bool {
    let words: Vec<String> = sentence_tokens.iter().map(|token| token.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase()).collect();
    if words.first().is_some_and(|w| matches!(w.as_str(), "am" | "is" | "are" | "was" | "were"))
        && sentence_tokens.last().is_some_and(|t| t.trim_end_matches(super::readability::is_closer).ends_with('?'))
    {
        return false;
    }
    for (i, word) in words.iter().enumerate() {
        let following = words.iter().skip(i.saturating_add(1));
        let contracted = ["'s", "'re", "'m", "’s", "’re", "’m"].iter().any(|suffix| word.ends_with(suffix))
            && following.clone().next().is_some_and(|w| w == "been" || participle(w));
        let auxiliary = word.strip_suffix("n't").or_else(|| word.strip_suffix("n’t")).unwrap_or(word);
        if !contracted
            && !matches!(auxiliary, "am" | "is" | "are" | "was" | "were" | "be" | "been" | "being" | "get" | "gets" | "got" | "gotten" | "getting")
        {
            continue;
        }
        // At most three adverb-like words may intervene before the participle.
        for candidate in following.take(4) {
            if participle(candidate) && !PASSIVE_ADJECTIVES.contains(&candidate.as_str()) {
                return true;
            }
            if !passive_skip(candidate) {
                break;
            }
        }
    }
    false
}
