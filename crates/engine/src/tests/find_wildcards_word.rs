//! Find and Replace with "Use wildcards" as Word does it: `@` is lazy while `{n,}` and `{n,m}`
//! are greedy, `{,n}` is not a valid count, and `?`, `*` and `[!…]` match a paragraph mark, so a
//! match can run into the next paragraph. Expected results measured in Microsoft 365 Word
//! 16.0.20527 (Find.Execute with MatchWildcards, Replace All with `[^&]`), 2026-11-14.

use serde_json::json;

use crate::Session;

/// The body's paragraphs after Replace All of `pattern` with `[^&]` in `text` (`\n` = new paragraph).
fn bracket(text: &str, pattern: &str) -> Vec<String> {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({ "text": text })).unwrap();
    s.run("edit.replaceAll", &json!({ "text": pattern, "with": "[^&]", "wildcards": true })).unwrap();
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect()
}

#[test]
fn at_sign_takes_as_few_as_it_can() {
    assert_eq!(bracket("ab\ncd\nxay aaab", "a@a"), ["ab", "cd", "xay [aa]ab"]);
    assert_eq!(bracket("aa bb ab", "b@"), ["aa [b][b] a[b]"]);
    assert_eq!(bracket("aa a-b", "[a-]@"), ["[a][a] [a][-]b"]);
    assert_eq!(bracket("abc", "?@"), ["[a][b][c]"]);
    assert_eq!(bracket("The lot loot lt", "lo@t"), ["The [lot] [loot] lt"]);
}

#[test]
fn counted_repeats_stay_greedy() {
    assert_eq!(bracket("ab\ncd\nxay aaab", "a{1,}"), ["[a]b", "cd", "x[a]y [aaa]b"]);
    assert_eq!(bracket("loot", "o{1,2}"), ["l[oo]t"]);
}

#[test]
fn count_without_a_lower_bound_is_refused() {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({ "text": "seek seeeek" })).unwrap();
    assert!(s.run("edit.replaceAll", &json!({ "text": "e{,2}k", "with": "[^&]", "wildcards": true })).is_err());
    assert!(s.run("edit.find", &json!({ "text": "e{,2}k", "wildcards": true })).is_err());
    let texts: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
    assert_eq!(texts, ["seek seeeek"]);
}

#[test]
fn any_character_star_and_not_classes_match_a_paragraph_mark() {
    for pattern in ["b?c", "b[!x]c", "b*c"] {
        assert_eq!(bracket("ab\ncd\nxay aaab", pattern), ["a[b", "c]d", "xay aaab"], "{pattern}");
    }
    assert_eq!(bracket("ab\ncd\nxay aaab", "b@c"), ["ab", "cd", "xay aaab"]);
}
