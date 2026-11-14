//! Word Count as Microsoft 365 Word counts it: measured 2026-11-14 with `Range.ComputeStatistics`
//! (words, characters without spaces, characters with spaces) on one-paragraph documents.

use crate::{count_chars, count_chars_no_spaces, count_words};

/// (text, words, characters without spaces, characters with spaces) as Word reports them.
const WORD: &[(&str, usize, usize, usize)] = &[
    ("alfa \u{A7} beta", 3, 9, 11),
    ("alfa \u{A7}\u{A7} beta", 3, 10, 12),
    ("alfa (\u{A7}\u{A7} beta", 3, 11, 13),
    ("alfa & beta", 3, 9, 11),
    ("alfa - beta", 3, 9, 11),
    ("alfa -- beta", 3, 10, 12),
    ("alfa ... beta", 3, 11, 13),
    ("alfa \u{2014} beta", 2, 9, 11),
    ("alfa \u{2013} beta", 2, 9, 11),
    ("alfa \u{2022} beta", 2, 9, 11),
    ("alfa \u{2014}\u{2014} beta", 2, 10, 12),
    ("alfa \u{2014}\u{2013} beta", 2, 10, 12),
    ("alfa \u{2022}\u{2022} beta", 2, 10, 12),
    ("alfa \u{2010} beta", 3, 9, 11),
    ("alfa \u{2011} beta", 3, 9, 11),
    ("alfa \u{2012} beta", 3, 9, 11),
    ("alfa \u{2015} beta", 3, 9, 11),
    ("alfa \u{B7} beta", 3, 9, 11),
    ("alfa \u{B6} beta", 3, 9, 11),
    ("alfa / beta", 3, 9, 11),
    ("alfa * beta", 3, 9, 11),
    ("alfa , beta", 3, 9, 11),
    ("alfa . beta", 3, 9, 11),
    ("alfa ! beta", 3, 9, 11),
    ("alfa ? beta", 3, 9, 11),
    ("alfa $ beta", 3, 9, 11),
    ("alfa % beta", 3, 9, 11),
    ("alfa # beta", 3, 9, 11),
    ("alfa @ beta", 3, 9, 11),
    ("alfa + beta", 3, 9, 11),
    ("alfa = beta", 3, 9, 11),
    ("alfa \u{AB} beta", 3, 9, 11),
    ("alfa \u{201C}\u{201D} beta", 3, 10, 12),
    ("alfa \u{27} beta", 3, 9, 11),
    ("alfa \u{2026} beta", 3, 9, 11),
    ("alfa \u{25E6} beta", 3, 9, 11),
    ("alfa \u{25AA} beta", 3, 9, 11),
    ("alfa \u{2219} beta", 3, 9, 11),
    ("alfa \u{2023} beta", 3, 9, 11),
    ("alfa \u{2212} beta", 3, 9, 11),
    ("alfa _ beta", 3, 9, 11),
    ("alfa ~ beta", 3, 9, 11),
    ("alfa | beta", 3, 9, 11),
    ("alfa \u{20AC} beta", 3, 9, 11),
    ("alfa \u{A9} beta", 3, 9, 11),
    ("alfa ( beta", 3, 9, 11),
    ("alfa ) beta", 3, 9, 11),
    ("alfa [] beta", 3, 10, 12),
    ("alfa : beta", 3, 9, 11),
    ("alfa ; beta", 3, 9, 11),
    ("a\u{2014}b", 2, 3, 3),
    ("a-b", 1, 3, 3),
    ("a\u{2013}b", 2, 3, 3),
    ("\u{2014} a", 1, 2, 3),
    ("a \u{2014}", 1, 2, 3),
    ("\u{2022} a", 1, 2, 3),
    ("a\u{9}b", 2, 2, 3),
    ("a\u{A0}b", 2, 2, 3),
    ("a\u{2002}b", 1, 3, 3),
    ("a\u{200B}b", 1, 2, 2),
    ("a\u{3000}b", 2, 2, 3),
    ("e.g.", 1, 4, 4),
    ("1,000", 1, 5, 5),
    ("$5", 1, 2, 2),
    ("x \u{2014}\u{2022} y", 2, 4, 6),
    ("x \u{2013}- y", 3, 4, 6),
    ("x -\u{2014} y", 3, 4, 6),
    ("x \u{2022}- y", 3, 4, 6),
    ("a\u{2003}b", 1, 3, 3),
    ("a\u{2009}b", 1, 3, 3),
    ("a\u{2007}b", 1, 3, 3),
    ("a\u{202F}b", 1, 3, 3),
    ("a\u{205F}b", 1, 3, 3),
    ("a\u{2004}b", 1, 3, 3),
    ("a\u{2005}b", 2, 2, 3),
    ("a\u{2006}b", 1, 3, 3),
    ("a\u{2008}b", 1, 3, 3),
    ("a\u{200A}b", 1, 3, 3),
    ("a\u{B}b", 2, 2, 3),
    ("a\u{AD}b", 1, 3, 3),
    ("a\u{200D}b", 1, 2, 2),
    ("a\u{2015}b", 1, 3, 3),
    ("a\u{2012}b", 1, 3, 3),
    ("x \u{2014} \u{2013} \u{2022} y", 2, 5, 9),
    ("a\u{2022}b", 2, 3, 3),
    ("a\u{2014}\u{2014}b", 2, 4, 4),
    ("\u{2014}", 0, 1, 1),
    ("x \u{2014}a y", 3, 4, 6),
];

#[test]
fn words_count_as_in_word() {
    for (text, words, _, _) in WORD {
        assert_eq!(count_words(text), *words, "words in {text:?}");
    }
}

#[test]
fn characters_without_spaces_count_as_in_word() {
    for (text, _, chars, _) in WORD {
        assert_eq!(count_chars_no_spaces(text), *chars, "characters (no spaces) in {text:?}");
    }
}

#[test]
fn characters_with_spaces_count_as_in_word() {
    for (text, _, _, with_spaces) in WORD {
        assert_eq!(count_chars(text), *with_spaces, "characters (with spaces) in {text:?}");
    }
}

#[test]
fn empty_and_cjk_counts_are_unchanged() {
    assert_eq!(count_words(""), 0);
    assert_eq!(count_chars(""), 0);
    assert_eq!(count_chars_no_spaces(""), 0);
    // Each ideograph is a word; full-width punctuation between them is not (#4547).
    assert_eq!(count_words("WordCraft是一个文字处理器，很好用！"), 1 + 11);
    assert_eq!(count_words("我们今天去公园。"), 7);
    // Invisible zero-width characters alone are no word.
    assert_eq!(count_words("\u{200B}"), 0);
}
