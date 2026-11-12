//! Text in one of the document's own styles shows only the formatting its own words state, as
//! Word opens the file: the stylesheet entry defines the style, it does not format the text.
//! (Microsoft 365 Word's PDF output of small RTF files; see the table in the docs row.)

use super::*;
use wordcraft_doc::props::{Align, TextColor, Underline, VertAlign};
use wordcraft_doc::{Document, Paragraph};

/// `\s20` sets every character and paragraph property the tests look at; `\s21` the rest of the
/// character ones; `\s22` is based on `\s20`; `\cs10` is a character style.
const SHEET: &str = concat!(
    r"{\s20\b\i\ul\strike\caps\cf1\fs40\f1\sb480\sa240\li1440\fi720\ri720\qc\sl480\slmult1\brdrb\brdrs\brdrw30\brdrcf1 XAll;}",
    r"{\s21\scaps\super\highlight3 XMore;}",
    r"{\s22\sbasedon20\scaps XDerived;}",
    r"{\*\cs10 \additive\b\i\ul\super\highlight3\cf1\fs40 XChar;}"
);
/// What a paragraph that repeats `\s20`'s words states.
const ALL: &str = r"\s20\b\i\ul\strike\caps\cf1\fs40\f1\sb480\sa240\li1440\fi720\ri720\qc\sl480\slmult1\brdrb\brdrs\brdrw30\brdrcf1";
const PLAIN: &str = r"\pard\plain\f0\fs24 Plain line.\par";

fn open_from(generator: &str, body: &str) -> Document {
    let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}{\f1 Arial;}}{\colortbl;\red255\green0\blue0;\red0\green0\blue255;\red255\green255\blue0;}";
    import(format!(r"{head}{generator}{{\stylesheet{{\s0 Normal;}}{SHEET}}}{PLAIN}{body}}}").as_bytes()).unwrap()
}

/// A file that names no program, as the Word probes were.
fn open(body: &str) -> Document {
    open_from("", body)
}

fn para<'a>(d: &'a Document, text: &str) -> &'a Paragraph {
    d.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains(text)).unwrap_or_else(|| panic!("no paragraph with {text}"))
}

/// The look of the text `text`: font, size, bold, italic, underlined, struck, all caps, small
/// caps, hidden, raised or lowered, colour, highlighted.
type Look = (String, f32, bool, bool, bool, bool, bool, bool, bool, VertAlign, TextColor, bool);

fn look(d: &Document, text: &str) -> Look {
    let p = para(d, text);
    let at = p.text.find(text).unwrap_or_else(|| panic!("no {text}"));
    let (_, c) = p.run_ranges().find(|(r, _)| r.contains(&at)).expect("a run at the text");
    let r = d.styles.resolve_char(p.props.style.as_deref(), c);
    let underlined = r.underline != Underline::None;
    (r.font.clone(), r.size, r.bold, r.italic, underlined, r.strike, r.caps, r.small_caps, r.hidden, r.vert_align, r.color, r.highlight.is_some())
}

/// The paragraph formatting of the paragraph holding `text`: alignment, left, first-line and
/// right indent, space before and after, line spacing, whether it has a visible border.
type ParaLook = (Align, f32, f32, f32, f32, f32, String, bool);

fn para_look(d: &Document, text: &str) -> ParaLook {
    let p = para(d, text);
    let r = d.styles.resolve_para(&p.props);
    let bordered = r.borders.is_some_and(|b| b.any_visible());
    (r.align, r.indent_left, r.indent_first, r.indent_right, r.space_before, r.space_after, format!("{:?}", r.line_spacing), bordered)
}

/// The look of the plain line every test document starts with.
fn plain(d: &Document) -> Look {
    look(d, "Plain line")
}

/// The style's own look, which a paragraph that repeats every word of `\s20` has: Arial 20 pt,
/// bold, italic, underlined, struck, all caps, red.
fn all_on(d: &Document) -> Look {
    let p = plain(d);
    ("Arial".to_string(), 20.0, true, true, true, true, true, p.7, p.8, p.9, TextColor::Rgb(wordcraft_doc::Rgb(255, 0, 0)), p.11)
}

// ---- text that repeats its style's words keeps the look (as today) ----

#[test]
fn text_that_repeats_its_styles_words_has_the_styles_look() {
    let d = open(&format!(r"\pard\plain{ALL} Restated words.\par"));
    assert_eq!(look(&d, "Restated"), all_on(&d));
    let p = para_look(&d, "Restated");
    assert_eq!((p.0, p.1, p.2, p.3, p.4, p.5, p.7), (Align::Center, 72.0, 36.0, 36.0, 24.0, 12.0, true));
    assert_eq!(para(&d, "Restated").props.style.as_deref(), Some("XAll"));
}

#[test]
fn a_character_style_whose_words_are_repeated_has_its_look() {
    let d = open(r"\pard\plain\f0\fs24 Start {\cs10\b\i\ul\super\highlight3\cf1\fs40 restatedrun} end.\par");
    let l = look(&d, "restatedrun");
    assert_eq!((l.1, l.2, l.3, l.4, l.9, l.11), (20.0, true, true, true, VertAlign::Superscript, true));
    assert_eq!(look(&d, "Start"), plain(&d));
}

#[test]
fn formatting_carried_over_in_the_rtf_state_still_applies() {
    // Word: the second paragraph is bold (the state still holds `\b`), and has none of its
    // style's small caps, superscript or highlight, which it does not state.
    let d = open(r"\pard\plain\f0\fs24\b Bold direct line.\par\pard\s21 Carried words.\par");
    let (l, p) = (look(&d, "Carried"), plain(&d));
    assert!(l.2, "bold is carried over");
    assert_eq!((l.7, l.9, l.11), (p.7, p.9, p.11));
}

// ---- rule 1: a run turns off what its style turns on (every file) ----

/// The look of `run` inside a paragraph that repeats all of `\s20`'s words.
fn turned_off(generator: &str, run: &str) -> (Look, Look) {
    let d = open_from(generator, &format!(r"\pard\plain{ALL} Restated words {{{run} offrun}} and on again.\par"));
    assert_eq!(look(&d, "on again"), all_on(&d), "the text after the group is the style's look again");
    (look(&d, "offrun"), all_on(&d))
}

#[test]
fn bold_off_in_a_bold_style_is_not_bold() {
    let (l, on) = turned_off("", r"\b0");
    assert_eq!(l, (on.0, on.1, false, on.3, on.4, on.5, on.6, on.7, on.8, on.9, on.10, on.11));
}

#[test]
fn italic_off_in_an_italic_style_is_not_italic() {
    let (l, on) = turned_off("", r"\i0");
    assert_eq!(l, (on.0, on.1, on.2, false, on.4, on.5, on.6, on.7, on.8, on.9, on.10, on.11));
}

#[test]
fn underline_off_in_an_underlined_style_is_not_underlined() {
    for word in [r"\ulnone", r"\ul0"] {
        let (l, on) = turned_off("", word);
        assert_eq!(l, (on.0, on.1, on.2, on.3, false, on.5, on.6, on.7, on.8, on.9, on.10, on.11), "{word}");
    }
}

#[test]
fn strikethrough_off_in_a_struck_style_is_not_struck() {
    let (l, on) = turned_off("", r"\strike0");
    assert_eq!(l, (on.0, on.1, on.2, on.3, on.4, false, on.6, on.7, on.8, on.9, on.10, on.11));
}

#[test]
fn caps_off_in_an_all_caps_style_is_as_typed() {
    let (l, on) = turned_off("", r"\caps0");
    assert_eq!(l, (on.0, on.1, on.2, on.3, on.4, on.5, false, on.7, on.8, on.9, on.10, on.11));
}

#[test]
fn the_automatic_colour_in_a_coloured_style_is_automatic() {
    let (l, on) = turned_off("", r"\cf0");
    assert_eq!(l, (on.0, on.1, on.2, on.3, on.4, on.5, on.6, on.7, on.8, on.9, TextColor::Auto, on.11));
}

#[test]
fn a_size_in_a_sized_style_is_that_size() {
    let (l, on) = turned_off("", r"\fs24");
    assert_eq!(l, (on.0, 12.0, on.2, on.3, on.4, on.5, on.6, on.7, on.8, on.9, on.10, on.11));
}

#[test]
fn plain_in_a_styled_paragraph_is_plain() {
    let d = open(&format!(r"\pard\plain{ALL} Restated words {{\plain\f0\fs24 offrun}} and on again.\par"));
    assert_eq!(look(&d, "offrun"), plain(&d));
    assert_eq!(look(&d, "on again"), all_on(&d));
}

#[test]
fn small_caps_superscript_and_highlight_turned_off_are_off() {
    let d = open(r"\pard\plain\s21\scaps\super\highlight3\f0\fs24 Restated more {\scaps0 nosmall} {\nosupersub nosuper} {\highlight0 nomarker} end.\par");
    let on = look(&d, "Restated more");
    assert_eq!((on.7, on.9, on.11), (true, VertAlign::Superscript, true), "the restated text has the style's look");
    let (a, b, c) = (look(&d, "nosmall"), look(&d, "nosuper"), look(&d, "nomarker"));
    assert_eq!((a.7, a.9, a.11), (false, VertAlign::Superscript, true), r"\scaps0");
    assert_eq!((b.7, b.9, b.11), (true, VertAlign::Baseline, true), r"\nosupersub");
    assert_eq!((c.7, c.9, c.11), (true, VertAlign::Superscript, false), r"\highlight0");
}

#[test]
fn a_run_turning_off_its_character_styles_words_is_off() {
    let d = open(r"\pard\plain\f0\fs24 Start {\cs10\b\i\ul\super\highlight3\cf1\fs40 re{\nosupersub\highlight0 offpart}stated} end.\par");
    let l = look(&d, "offpart");
    assert_eq!((l.2, l.3, l.4, l.9, l.11), (true, true, true, VertAlign::Baseline, false));
}

#[test]
fn stated_off_words_count_in_a_file_wordcraft_wrote() {
    let (l, on) = turned_off(r"{\*\generator WordCraft;}", r"\b0\i0\ulnone\strike0\caps0\cf0");
    assert_eq!(l, (on.0, on.1, false, false, false, false, false, on.7, on.8, on.9, TextColor::Auto, on.11));
}

// ---- rule 2: text that does not repeat its style's words (files WordCraft did not write) ----

const UNSTATED: &str = r"\pard\plain\s20\f0\fs24 Unstated words.\par";

#[test]
fn text_that_does_not_repeat_its_paragraph_style_is_plain() {
    for generator in ["", r"{\*\generator Microsoft Word 15.0.5000;}"] {
        let d = open_from(generator, UNSTATED);
        assert_eq!(look(&d, "Unstated"), plain(&d), "{generator}");
        assert_eq!(para_look(&d, "Unstated"), para_look(&d, "Plain line"), "{generator}");
    }
}

#[test]
fn the_paragraph_keeps_its_style_and_the_style_its_definition() {
    let d = open(UNSTATED);
    assert_eq!(para(&d, "Unstated").props.style.as_deref(), Some("XAll"));
    let s = d.styles.get("XAll").expect("style XAll");
    assert_eq!((s.chr.bold, s.chr.italic, s.chr.caps, s.chr.size), (Some(true), Some(true), Some(true), Some(20.0)));
    assert_eq!((s.para.space_before, s.para.indent_left, s.para.align), (Some(24.0), Some(72.0), Some(Align::Center)));
}

#[test]
fn text_with_no_words_at_all_has_the_rtf_defaults() {
    // No `\f`, no `\fs`: the `\deff` font at 12 pt.
    let d = open(r"\pard\plain\s20 Bare words.\par");
    assert_eq!(look(&d, "Bare"), plain(&d));
}

#[test]
fn a_paragraph_without_plain_takes_the_state_it_finds() {
    let d = open(r"\pard\s20 Noplain words.\par");
    assert_eq!(look(&d, "Noplain"), plain(&d));
    assert_eq!(para_look(&d, "Noplain"), para_look(&d, "Plain line"));
}

#[test]
fn a_based_on_style_shows_neither_its_own_nor_its_bases_words() {
    let d = open(r"\pard\plain\s22\f0\fs24 Derived words.\par");
    assert_eq!(look(&d, "Derived"), plain(&d));
    assert_eq!(para_look(&d, "Derived"), para_look(&d, "Plain line"));
}

#[test]
fn text_that_does_not_repeat_its_character_style_is_plain() {
    let d = open(r"\pard\plain\f0\fs24 Start {\cs10 unstatedrun} end.\par");
    assert_eq!(look(&d, "unstatedrun"), plain(&d));
}

#[test]
fn a_file_wordcraft_wrote_keeps_the_styles_look_on_text_that_states_nothing() {
    let d = open_from(r"{\*\generator WordCraft;}", UNSTATED);
    let (l, p) = (look(&d, "Unstated"), plain(&d));
    assert_eq!((l.2, l.3, l.4, l.5, l.6), (true, true, true, true, true));
    assert_ne!(l, p);
    assert_eq!(para_look(&d, "Unstated").4, 24.0);
}

#[test]
fn restated_text_survives_a_save_and_reopen() {
    let d = open(&format!(r"\pard\plain{ALL} Restated words.\par"));
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(look(&back, "Restated"), look(&d, "Restated"));
    assert_eq!(para_look(&back, "Restated"), para_look(&d, "Restated"));
}
