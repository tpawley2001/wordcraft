//! Compatibility mode 14 and older with `w:characterSpacingControl` other than `doNotCompress`
//! (ECMA-376 §17.15.1.18, `Settings::char_spacing`), and documents opened from RTF
//! (`Settings::rtf_justification`): Word never ends a line after a hyphen-minus that sits between two
//! letters; the whole hyphenated word moves to the next line. Mode 15, and mode 11 without the
//! setting, break after the hyphen as before.
//!
//! Measured in Microsoft 365 Word on synthetic files (Letter page, margins 144 pt left and
//! 84.25 pt right): a paragraph of about 400 hyphen-rich words in Times New Roman 12 pt has 21 lines
//! ending in "x-" in mode 11 or 15 without the setting and in mode 15 with it, and none in mode 11
//! with `compressPunctuation` or when opened from RTF (Times New Roman, Arial and Courier New alike).
//! In Courier New 12 pt (53 characters fit), a line whose text up to and including "p-" is exactly
//! 53 characters wraps the whole "p…p-ssssssssssss" word, for prefixes of 1 to 12 letters.

use super::*;
use wordcraft_doc::{CharProps, CharSpacing, Run};

const PAGE_W: f32 = 612.0;
const LEFT: f32 = 144.0;
const RIGHT: f32 = 84.25;

#[derive(Clone, Copy, Debug)]
struct Mode {
    compat_mode: u32,
    char_spacing: CharSpacing,
    rtf: bool,
}

const fn mode(compat_mode: u32, char_spacing: CharSpacing, rtf: bool) -> Mode {
    Mode { compat_mode, char_spacing, rtf }
}

/// Word keeps hyphenated words whole in these.
const NO_HYPHEN_BREAK: &[Mode] = &[
    mode(11, CharSpacing::CompressPunctuation, false),
    mode(12, CharSpacing::CompressPunctuation, false),
    mode(14, CharSpacing::CompressPunctuationAndJapaneseKana, false),
    mode(11, CharSpacing::DoNotCompress, true),
];

/// Word breaks after the hyphen in these.
const HYPHEN_BREAK: &[Mode] =
    &[mode(15, CharSpacing::CompressPunctuation, false), mode(15, CharSpacing::DoNotCompress, false), mode(11, CharSpacing::DoNotCompress, false)];

fn props(font: &str, size: f32) -> CharProps {
    CharProps { font: Some(font.into()), size: Some(size), ..Default::default() }
}

/// Lays `text` out as one left-aligned paragraph; returns the text of each line on the first page
/// (trailing spaces left out).
fn lines(font: &str, size: f32, text: &str, m: Mode) -> Vec<String> {
    let mut para = wordcraft_doc::Paragraph::with_text(text, props(font, size));
    para.runs = vec![Run { len: text.len(), props: props(font, size) }];
    para.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.last_section.page_w = PAGE_W;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = RIGHT;
    d.settings.rtf_justification = m.rtf;
    d.settings.compat_mode = m.compat_mode;
    d.settings.char_spacing = m.char_spacing;
    d.body = vec![wordcraft_doc::para_block(para)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let para = l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap();
    para.lines.iter().map(|line| text[line.start..line.stop].trim_end().to_string()).collect()
}

/// The Courier New probe line for a hyphen prefix of `n` letters: filler words, then
/// "p…p-ssssssssssss", where the text through the hyphen is exactly 53 characters.
fn courier_probe(n: usize) -> (String, String) {
    let mut rest = 53 - (n + 1) - 1;
    let mut words = Vec::new();
    while rest > 0 {
        let w = if rest == 10 { 8 } else { rest.min(9) };
        words.push("f".repeat(w));
        rest -= w.min(rest);
        rest = rest.saturating_sub(1);
    }
    let filler = words.join(" ");
    let line = format!("{filler} {}-{}", "p".repeat(n), "s".repeat(12));
    (filler, line)
}

#[test]
fn courier_probe_lines_are_53_characters_through_the_hyphen() {
    for n in 1..=12 {
        let (filler, line) = courier_probe(n);
        assert_eq!(line.find('-').unwrap() + 1, 53, "prefix {n}: {line}");
        assert_eq!(filler.len() + 1 + n + 1, 53, "prefix {n}");
    }
}

#[test]
fn compressing_below_mode_15_and_rtf_keep_the_hyphenated_word_whole() {
    crate::test_fonts();
    for m in NO_HYPHEN_BREAK {
        for n in 1..=12 {
            let (filler, line) = courier_probe(n);
            let text = format!("{line} cd cd cd cd cd end");
            let got = lines("Courier New", 12.0, &text, *m);
            assert_eq!(got[0], filler, "{m:?}, prefix {n}: the first line keeps \"{}-…\" for the next line", "p".repeat(n));
        }
    }
}

#[test]
fn mode_15_and_no_compression_still_break_after_the_hyphen() {
    crate::test_fonts();
    for m in HYPHEN_BREAK {
        for n in [1, 4, 12] {
            let (_, line) = courier_probe(n);
            let text = format!("{line} cd cd cd cd cd end");
            let got = lines("Courier New", 12.0, &text, *m);
            assert_eq!(got[0], &line[..53], "{m:?}, prefix {n}: the first line ends after the hyphen");
        }
    }
}

/// About 150 words drawn from a fixed list of hyphenated and plain words (deterministic).
fn hyphen_rich_text() -> String {
    const WORDS: &[&str] = &[
        "well-known",
        "end-points",
        "so-called",
        "long-term",
        "follow-up",
        "co-operate",
        "first-line",
        "re-entry",
        "self-evident",
        "up-to-date",
        "x-ray",
        "e-mail",
        "anti-inflammatory",
        "non-trivial",
        "the",
        "and",
        "of",
        "results",
        "patients",
        "drug",
        "study",
    ];
    let mut seed: u32 = 12345;
    (0..150)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            WORDS[(seed >> 16) as usize % WORDS.len()]
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ends_after_hyphen(line: &str) -> bool {
    let mut it = line.chars().rev();
    it.next() == Some('-') && it.next().is_some_and(char::is_alphabetic)
}

#[test]
fn proportional_text_never_ends_a_line_after_a_hyphen_when_compressing() {
    crate::test_fonts();
    let text = hyphen_rich_text();
    for m in NO_HYPHEN_BREAK {
        let got = lines("Times New Roman", 12.0, &text, *m);
        assert!(got.len() > 10, "{m:?}: {} lines", got.len());
        let split: Vec<_> = got.iter().filter(|l| ends_after_hyphen(l)).collect();
        assert!(split.is_empty(), "{m:?}: lines ending after a hyphen: {split:?}");
    }
}

#[test]
fn proportional_text_breaks_after_hyphens_otherwise() {
    crate::test_fonts();
    let text = hyphen_rich_text();
    for m in HYPHEN_BREAK {
        let got = lines("Times New Roman", 12.0, &text, *m);
        let split = got.iter().filter(|l| ends_after_hyphen(l)).count();
        assert!(split >= 3, "{m:?}: only {split} of {} lines end after a hyphen", got.len());
    }
}
