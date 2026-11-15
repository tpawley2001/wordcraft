//! Words in capitals take hyphens where the same word in lower case does, unless the document
//! turns off Hyphenate words in CAPS (`w:doNotHyphenateCaps`, ECMA-376 Part 1 §17.15.1.37).
//! Measured in Microsoft 365 Word's PDFs: `INTERNATIONALI-`, `INTERNA-`, `INTER-`, `ACCOM-` and
//! `AC-` at the right margin with the option on, never a hyphen in a capitals word with it off.

use super::*;

const WORDS: [&str; 2] = ["internationalization", "accompany"];

/// The hyphen points of each word of `text` (byte offsets from the word's start), from the
/// laid-out paragraph's hyphenation points.
fn points(caps: bool, text: &str) -> Vec<(String, Vec<usize>)> {
    crate::test_fonts();
    let mut d = Document::from_text(text);
    d.settings.auto_hyphenation = true;
    d.settings.hyphenate_caps = caps;
    let l = lay(&d);
    let para = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, .. } => Some(para.clone()),
            _ => None,
        })
        .expect("a body paragraph");
    let ends: Vec<usize> = para.hyph_after.iter().map(|k| para.clusters[*k as usize].end).collect();
    let mut out = Vec::new();
    let mut at = 0;
    for w in text.split(' ') {
        let (a, b) = (at, at + w.len());
        out.push((w.to_string(), ends.iter().filter(|&&e| e > a && e < b).map(|e| e - a).collect()));
        at = b + 1;
    }
    out
}

fn text() -> String {
    let lower = WORDS.join(" ");
    format!("{lower} {}", lower.to_uppercase())
}

#[test]
fn lower_case_words_have_points() {
    // Guard: the dictionary is there, or the capitals tests prove nothing.
    for (w, p) in points(true, &text()).into_iter().take(WORDS.len()) {
        assert!(!p.is_empty(), "no hyphen points in {w}");
    }
}

#[test]
fn capitals_words_hyphenate_like_their_lower_case_forms() {
    let p = points(true, &text());
    for i in 0..WORDS.len() {
        assert_eq!(p[i + WORDS.len()].1, p[i].1, "{} vs {}", p[i + WORDS.len()].0, p[i].0);
    }
}

#[test]
fn capitals_words_take_no_hyphen_with_hyphenate_caps_off() {
    let p = points(false, &text());
    for (w, pts) in &p[WORDS.len()..] {
        assert!(pts.is_empty(), "{w}: {pts:?}");
    }
}

#[test]
fn lower_case_words_are_the_same_either_way() {
    let (on, off) = (points(true, &text()), points(false, &text()));
    assert_eq!(on[..WORDS.len()], off[..WORDS.len()]);
}
