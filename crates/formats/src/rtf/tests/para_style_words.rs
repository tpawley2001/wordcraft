//! RTF save: a paragraph in one of the document's own paragraph styles states the style's formatting
//! (RTF 1.9.1, "Style sheet": a paragraph's `\sN` names its style; the formatting is the words the
//! paragraph and its text state). Word draws only what is stated, so a paragraph written as
//! `\pard\plain\sN text` shows as plain text there. The exporter writes the effective values: the
//! paragraph's own where it sets them, else its style chain's.

use super::*;
use wordcraft_doc::props::{Align, TextColor};
use wordcraft_doc::{Document, Paragraph, Rgb};

/// Centred, 12 pt before, 6 pt after, 0.5" left, 0.25" right, 0.25" hanging, keep with next;
/// bold, italic, 20 pt, red.
const LOUD: &str = r"{\s15\sbasedon0\snext15\qc\sb240\sa120\li720\ri360\fi-360\keepn\b\i\fs40\cf1 Loud;}";
/// Based on Loud: right-aligned and underlined, the rest inherited.
const LOUD_KID: &str = r"{\s16\sbasedon15\snext16\qr\ul Loud Kid;}";

fn open(stylesheet: &str, body: &str) -> Document {
    let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}{\colortbl;\red255\green0\blue0;}";
    import(format!(r"{head}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}").as_bytes()).unwrap()
}

/// The body paragraph whose text contains `text`.
fn para<'a>(d: &'a Document, text: &str) -> &'a Paragraph {
    d.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains(text)).unwrap_or_else(|| panic!("no paragraph with {text}"))
}

/// The exported line of the paragraph whose text contains `text`.
fn line<'a>(rtf: &'a str, text: &str) -> &'a str {
    rtf.lines().find(|l| l.starts_with(r"\pard") && l.contains(text)).unwrap_or_else(|| panic!("no paragraph line with {text} in {rtf}"))
}

/// How often control word `word` (with its number, e.g. `\sb240`) occurs in `line` as a whole word.
fn count(line: &str, word: &str) -> usize {
    line.match_indices(word).filter(|(i, _)| !line[i + word.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')).count()
}

/// The effective character formatting of the run of `p` that holds `text`.
fn chars(d: &Document, p: &Paragraph, text: &str) -> wordcraft_doc::resolve::ResolvedChar {
    let at = p.text.find(text).unwrap_or_else(|| panic!("no {text} in {:?}", p.text));
    let (_, c) = p.run_ranges().find(|(r, _)| r.contains(&at)).expect("a run at the text");
    d.styles.resolve_char(p.props.style.as_deref(), c)
}

/// What the tests compare of a paragraph's effective formatting and of the run holding `text`.
fn look(d: &Document, text: &str) -> (Option<String>, Align, [i32; 5], bool, bool, bool, i32, TextColor) {
    let p = para(d, text);
    let r = d.styles.resolve_para(&p.props);
    let c = chars(d, p, text);
    let tw = |v: f32| (v * 20.0).round() as i32;
    let spacing = [tw(r.space_before), tw(r.space_after), tw(r.indent_left), tw(r.indent_right), tw(r.indent_first)];
    (p.props.style.clone(), r.align, spacing, r.keep_next, c.bold, c.italic, (c.size * 2.0).round() as i32, c.color)
}

const RED: TextColor = TextColor::Rgb(Rgb(255, 0, 0));

#[test]
fn the_fixture_paragraph_has_only_its_style() {
    // What the export tests start from: the style gives the look, the paragraph sets nothing.
    let d = open(LOUD, r"\pard\plain\s15 Shouted\par\pard\plain Plain\par");
    let p = para(&d, "Shouted");
    assert_eq!(p.props.style.as_deref(), Some("Loud"));
    assert_eq!(look(&d, "Shouted"), (Some("Loud".to_string()), Align::Center, [240, 120, 720, 360, -360], true, true, true, 40, RED));
}

#[test]
fn a_styled_paragraph_states_the_styles_paragraph_words() {
    let d = open(LOUD, r"\pard\plain\s15 Shouted\par\pard\plain Plain\par");
    let rtf = export(&d);
    let l = line(&rtf, "Shouted");
    for word in [r"\qc", r"\sb240", r"\sa120", r"\li720", r"\ri360", r"\fi-360", r"\keepn"] {
        assert_eq!(count(l, word), 1, "{word} once in {l}");
    }
    assert_eq!(count(l, r"\ql"), 0, "no left alignment on a centred style: {l}");
}

#[test]
fn a_styled_paragraph_states_the_styles_character_words() {
    let d = open(LOUD, r"\pard\plain\s15 Shouted\par\pard\plain Plain\par");
    let rtf = export(&d);
    let l = line(&rtf, "Shouted");
    for word in [r"\b", r"\i", r"\fs40"] {
        assert_eq!(count(l, word), 1, "{word} once in {l}");
    }
    // The colour is whichever colour-table entry is red: read it back.
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(chars(&back, para(&back, "Shouted"), "Shouted").color, RED, "{l}");
    assert!(l.contains(r"\cf"), "a colour word in {l}");
}

#[test]
fn a_plain_paragraph_gains_nothing() {
    let d = open(LOUD, r"\pard\plain\s15 Shouted\par\pard\plain Plain\par");
    let rtf = export(&d);
    let l = line(&rtf, "Plain");
    for word in [r"\qc", r"\sb240", r"\sa120", r"\li720", r"\keepn", r"\b", r"\i", r"\fs40"] {
        assert_eq!(count(l, word), 0, "no {word} in {l}");
    }
    assert_eq!(count(l, r"\ql"), 1, "{l}");
}

#[test]
fn a_based_on_chain_gives_the_inherited_words_too() {
    let sheet = format!("{LOUD}{LOUD_KID}");
    let d = open(&sheet, r"\pard\plain\s16 Child\par");
    assert_eq!(para(&d, "Child").props.style.as_deref(), Some("LoudKid"));
    let rtf = export(&d);
    let l = line(&rtf, "Child");
    for word in [r"\qr", r"\sb240", r"\sa120", r"\li720", r"\keepn", r"\b", r"\i", r"\fs40", r"\ul"] {
        assert_eq!(count(l, word), 1, "{word} once in {l}");
    }
    assert_eq!(count(l, r"\qc"), 0, "the child's alignment replaces its base's: {l}");
    assert_eq!(count(l, r"\ql"), 0, "{l}");
}

#[test]
fn the_paragraphs_own_settings_win_over_the_style() {
    // Direct left alignment and no space before on a centred style; the first word's run is set
    // not bold directly (built here: the reader has no "not bold" for a run in a bold style).
    let mut d = open(LOUD, r"\pard\plain\s15\ql\sb0 Quiet still loud\par");
    let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(&mut d.body[0]) else { panic!("a paragraph first") };
    assert_eq!((p.text.as_str(), p.runs.len()), ("Quiet still loud", 1), "fixture: one run");
    let all = p.runs[0].clone();
    let mut quiet = all.clone();
    quiet.len = "Quiet".len();
    quiet.props.bold = Some(false);
    let mut rest = all;
    rest.len -= quiet.len;
    p.runs = vec![quiet, rest];
    let before = look(&d, "Quiet");
    assert_eq!((before.1, before.2[0], before.4), (Align::Left, 0, false), "fixture: direct values beat the style");
    assert!(chars(&d, para(&d, "still loud"), "still loud").bold, "fixture: the rest stays bold");
    let rtf = export(&d);
    let l = line(&rtf, "Quiet");
    assert_eq!(count(l, r"\ql"), 1, "{l}");
    assert_eq!(count(l, r"\qc"), 0, "{l}");
    assert_eq!(count(l, r"\sb240"), 0, "{l}");
    assert_eq!(count(l, r"\sa120"), 1, "{l}");
    assert_eq!(count(l, r"\b0"), 1, "the run that is not bold says so: {l}");
    assert!(l.find(r"\b0").unwrap() < l.find("Quiet").unwrap() && l.find("Quiet").unwrap() < l.find("still loud").unwrap(), "{l}");
    // Paragraph settings and the bold text come back (the not-bold run is the reader's to keep: not checked here).
    let back = import(rtf.as_bytes()).unwrap();
    let after = look(&back, "still loud");
    assert_eq!((after.0.as_deref(), after.1, after.2, after.3, after.4), (Some("Loud"), Align::Left, [0, 120, 720, 360, -360], true, true));
}

#[test]
fn the_look_survives_one_and_two_round_trips_with_each_word_stated_once() {
    let sheet = format!("{LOUD}{LOUD_KID}");
    let d = open(&sheet, r"\pard\plain\s15 Shouted\par\pard\plain\s16 Child\par\pard\plain Plain\par");
    let first = export(&d);
    let once = import(first.as_bytes()).unwrap();
    let second = export(&once);
    let twice = import(second.as_bytes()).unwrap();
    for text in ["Shouted", "Child", "Plain"] {
        assert_eq!(look(&once, text), look(&d, text), "{text} after one round trip");
        assert_eq!(look(&twice, text), look(&d, text), "{text} after two round trips");
    }
    for (text, align) in [("Shouted", r"\qc"), ("Child", r"\qr")] {
        let l = line(&second, text);
        for word in [align, r"\sb240", r"\sa120", r"\li720", r"\keepn", r"\b", r"\i", r"\fs40"] {
            assert_eq!(count(l, word), 1, "{word} once in the second save: {l}");
        }
    }
}
