//! Note separator groups (`\ftnsep`, `\ftnsepc`, `\ftncn`, `\aftnsep`, `\aftnsepc`, `\aftncn`,
//! RTF 1.9.1 "Footnotes") are destinations with or without `\*`: Microsoft 365 Word shows none of
//! their text in the body. A tracked section formatting change (`{\*\oldsprops …}`) keeps
//! Suppress endnotes (`\endnhere`) like the section itself. Custom note marks stay 1 to 10
//! characters however much text the groups before them hold.

use super::*;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::props::PropChange;
use wordcraft_doc::section::NotePos;
use wordcraft_doc::{Align, PartKind, Revision, RevisionKind, SectionProps};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0\ftnbj{{\fonttbl{{\f0 Arial;}}}}{body}}}").as_bytes()).unwrap()
}

/// The body paragraphs' text.
fn body_texts(d: &Document) -> Vec<String> {
    doc_paras(d).iter().map(|p| p.plain_text().trim().to_string()).collect()
}

fn note_parts(d: &Document) -> usize {
    d.parts.values().filter(|p| matches!(p.kind, PartKind::Footnote | PartKind::Endnote)).count()
}

const NOTE: &str = r"{\footnote \pard\plain \chftn  note}";

#[test]
fn separator_groups_without_a_star_are_not_body_text() {
    // Word: one paragraph, "Body".
    let d = open(&format!(
        r"{{\ftnsep \pard\plain SEPTEXT\par}}{{\ftnsepc \pard\plain CONTSEP\par}}{{\ftncn \pard\plain NOTICE\par}}{{\aftnsep \pard\plain ASEP\par}}{{\aftnsepc \pard\plain ACONT\par}}{{\aftncn \pard\plain ANOTICE\par}}\pard\plain Body{NOTE}\par"
    ));
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(note_parts(&d), 1);
}

#[test]
fn each_separator_word_is_a_destination_on_its_own() {
    for word in ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"] {
        let d = open(&format!(r"{{\{word} \pard\plain ZZSEPZZ\par}}\pard\plain Body\par"));
        assert_eq!(body_texts(&d), ["Body"], "\\{word}");
        assert!(!format!("{:?}{:?}", body_texts(&d), d.parts).contains("ZZSEPZZ"), "\\{word}");
    }
}

#[test]
fn default_separator_groups_add_no_empty_paragraphs() {
    // What Word 97 wrote for the default separators. Word: two paragraphs, "Body" and "Second".
    let d = open(&format!(r"{{\ftnsep \pard\plain \chftnsep\par}}{{\ftnsepc \pard\plain \chftnsepc\par}}\pard\plain Body{NOTE}\par Second\par"));
    assert_eq!(body_texts(&d), ["Body", "Second"]);
    assert_eq!(note_parts(&d), 1);
}

#[test]
fn starred_separator_groups_stay_skipped() {
    let d = open(&format!(r"{{\*\ftnsep \pard\plain SEPTEXT\par}}{{\*\aftnsep \pard\plain ASEP\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(note_parts(&d), 1);
}

#[test]
fn separator_characters_in_body_text_change_nothing_around_them() {
    // Word's text: "Body X Y".
    let d = open(r"\pard\plain Body \chftnsep X \chftnsepc Y\par");
    assert_eq!(body_texts(&d), ["Body X Y"]);
}

#[test]
fn formatting_and_breaks_inside_a_separator_group_do_not_reach_the_body() {
    let d = open(r"\pard\plain {\ftnsep \pard\qc\plain\b ZZSEPZZ\par\sect\sectd}After\par");
    assert_eq!(body_texts(&d), ["After"]);
    assert_eq!(d.sections().len(), 1);
    let paras = doc_paras(&d);
    let p = paras.first().expect("body paragraph");
    assert_ne!(p.props.align, Some(Align::Center));
    assert_ne!(p.props_of_char(0).bold, Some(true));
    assert!(!format!("{:?}{:?}", body_texts(&d), d.parts).contains("ZZSEPZZ"));
}

#[test]
fn a_separator_group_in_a_table_cell_or_a_note_is_skipped_too() {
    let d = open(r"\trowd\cellx3000\pard\intbl {\ftnsep \pard\plain ZZSEPZZ\par}Cell\cell\row\pard\plain After\par");
    let dump = format!("{d:?}");
    assert!(!dump.contains("ZZSEPZZ"), "{dump}");
    assert!(dump.contains("Cell") && dump.contains("After"), "{dump}");
    let d = open(r"\pard\plain Body{\footnote \pard\plain \chftn  {\ftnsep ZZSEPZZ}note}\par");
    let dump = format!("{d:?}");
    assert!(!dump.contains("ZZSEPZZ"), "{dump}");
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(note_parts(&d), 1);
}

/// The sections of a file whose document-level note words are `doc_words`: each section's own
/// Suppress endnotes and the one in its formatting before the tracked change.
fn suppress(doc_words: &str, sections: &str) -> Vec<(Option<bool>, Option<bool>)> {
    let rtf = format!(r"{{\rtf1\ansi{doc_words}{{\*\revtbl {{Unknown;}}{{Alice;}}}}{sections}}}");
    let d = import(rtf.as_bytes()).unwrap();
    d.sections().into_iter().map(|(_, s)| (s.no_endnote, s.fmt_change.as_deref().expect("section change read").old.no_endnote)).collect()
}

#[test]
fn oldsprops_reads_suppress_endnotes_when_endnotes_are_at_section_ends() {
    // One section (the document's last).
    let one = |now: &str, old: &str| format!(r"\sectd\marglsxn1800{now}{{\*\oldsprops \sectd\marglsxn1440{old}}}\pard one\par");
    assert_eq!(suppress(r"\aendnotes", &one(r"\endnhere", r"\endnhere")), [(Some(false), Some(false))]);
    assert_eq!(suppress(r"\aendnotes", &one(r"\endnhere", "")), [(Some(false), Some(true))]);
    assert_eq!(suppress(r"\aendnotes", &one("", r"\endnhere")), [(Some(true), Some(false))]);
    assert_eq!(suppress(r"\aendnotes", &one("", "")), [(Some(true), Some(true))]);
}

#[test]
fn oldsprops_suppress_endnotes_in_a_section_that_ends_with_sect() {
    let two = r"\sectd\marglsxn1800\endnhere{\*\oldsprops \sectd\marglsxn1440}\pard one\par\sect\sectd\marglsxn1800{\*\oldsprops \sectd\marglsxn1440\endnhere}\pard two\par";
    assert_eq!(suppress(r"\aendnotes", two), [(Some(false), Some(true)), (Some(true), Some(false))]);
}

#[test]
fn oldsprops_has_no_suppress_endnotes_when_endnotes_are_at_the_document_end() {
    let one = r"\sectd\marglsxn1800\endnhere{\*\oldsprops \sectd\marglsxn1440\endnhere}\pard one\par";
    assert_eq!(suppress(r"\aenddoc", one), [(None, None)]);
    assert_eq!(suppress("", one), [(None, None)]);
}

/// A one-section document whose section's left margin was 72 pt before a tracked change, with
/// endnotes at `pos`, and the given Suppress endnotes now and before the change.
fn changed(pos: Option<NotePos>, now: Option<bool>, old: Option<bool>) -> Document {
    let mut d = Document::from_text("one");
    d.settings.endnote_pr.pos = pos;
    let before = SectionProps { margin_left: 72.0, no_endnote: old, ..Default::default() };
    d.last_section = SectionProps { margin_left: 90.0, no_endnote: now, fmt_change: PropChange::boxed(0, before), ..Default::default() };
    d.revisions = vec![Revision::new(RevisionKind::Format, "Alice", "2024-03-15T10:30:00Z")];
    d
}

/// The `{\*\oldsprops …}` group of `rtf`.
fn old_group(rtf: &str) -> &str {
    let at = rtf.find("{\\*\\oldsprops").expect("an \\oldsprops group");
    let rest = rtf.get(at..).unwrap_or_default();
    rest.get(..rest.find('}').map_or(rest.len(), |i| i + 1)).unwrap_or_default()
}

#[test]
fn oldsprops_is_written_with_endnhere_unless_the_old_section_suppressed() {
    for (old, here) in [(Some(true), false), (Some(false), true), (None, true)] {
        // The section itself suppresses, so the only `\endnhere` can be the old properties'.
        let rtf = export(&changed(Some(NotePos::SectEnd), Some(true), old));
        assert_eq!(old_group(&rtf).contains("\\endnhere"), here, "old {old:?}: {rtf}");
        assert_eq!(rtf.matches("\\endnhere").count(), usize::from(here), "old {old:?}: {rtf}");
    }
}

#[test]
fn oldsprops_has_no_endnhere_when_endnotes_are_at_the_document_end() {
    for pos in [None, Some(NotePos::DocEnd)] {
        let rtf = export(&changed(pos, None, None));
        assert!(!rtf.contains("\\endnhere"), "{pos:?}: {rtf}");
    }
}

#[test]
fn suppress_endnotes_before_and_after_a_tracked_change_round_trips() {
    for (now, old) in [(Some(true), Some(false)), (Some(false), Some(true)), (Some(true), Some(true)), (Some(false), Some(false))] {
        let rtf = export(&changed(Some(NotePos::SectEnd), now, old));
        let back = import(rtf.as_bytes()).unwrap();
        let secs = back.sections();
        let (_, s) = secs.first().copied().expect("a section");
        assert_eq!(s.no_endnote, now, "{rtf}");
        let ch = s.fmt_change.as_deref().expect("section change read back");
        assert_eq!(ch.old.no_endnote, old, "{rtf}");
        assert!((ch.old.margin_left - 72.0).abs() < 0.1, "{rtf}");
    }
}

/// The body's first paragraph's note references as `(kind, custom mark)`.
fn marks(d: &Document) -> Vec<(NoteKind, String)> {
    doc_paras(d)
        .iter()
        .flat_map(|p| p.objects.iter())
        .filter_map(|o| if let InlineObject::NoteRef { kind, custom, .. } = o { Some((*kind, custom.clone())) } else { None })
        .collect()
}

#[test]
fn a_ten_character_group_is_a_custom_mark_and_eleven_is_not() {
    let d = open(r"\pard Ten{\super abcdefghij}{\footnote\pard x}\par");
    assert_eq!(marks(&d), [(NoteKind::Footnote, "abcdefghij".to_string())]);
    assert_eq!(body_texts(&d), ["Ten"]);
    let d = open(r"\pard Eleven{\super abcdefghijk}{\footnote\pard x}\par");
    assert_eq!(marks(&d), [(NoteKind::Footnote, String::new())]);
    assert_eq!(body_texts(&d), ["Elevenabcdefghijk"]);
}

#[test]
fn ten_four_byte_characters_are_still_a_custom_mark() {
    // U+1F600 ten times: 40 bytes of UTF-8.
    let mark = r"\u-10179?\u-8704?".repeat(10);
    let d = open(&format!(r"\pard Faces{{\super {mark}}}{{\footnote\pard x}}\par"));
    assert_eq!(marks(&d), [(NoteKind::Footnote, "\u{1F600}".repeat(10))]);
}

#[test]
fn a_long_paragraph_in_nested_groups_keeps_its_text_and_the_mark_after_it() {
    let long = "A".repeat(200_000);
    let body = format!(r"\pard {}{long}{}{{\super *}}{{\footnote\pard {{\super *}} Star note.}}\par", "{".repeat(60), "}".repeat(60));
    let d = open(&body);
    assert_eq!(marks(&d), [(NoteKind::Footnote, "*".to_string())]);
    assert_eq!(body_texts(&d), [long]);
}

#[test]
fn a_page_break_or_note_number_inside_a_separator_story_adds_nothing() {
    let d = open(&format!(r"{{\ftnsep \pard\plain A\page B\chftn C\par}}\pard\plain Body{NOTE}\par"));
    let sep = d.footnote_separators.separator.as_ref().expect("separator kept");
    let text: String = sep.iter().filter_map(|b| if let wordcraft_doc::Block::Para(p) = &**b { Some(p.plain_text()) } else { None }).collect();
    assert_eq!(text.trim(), "ABC");
    assert_eq!(body_texts(&d), ["Body"]);
}

#[test]
fn a_separator_group_in_shape_text_is_skipped() {
    let d = open(r"\pard\plain {\shp{\*\shpinst\shpleft0\shpright2000\shptop0\shpbottom500{\shptxt {\ftnsep ZZSEPZZ\par}Box\par}}}Body\par");
    assert!(d.footnote_separators.is_empty());
    assert!(!format!("{d:?}").contains("ZZSEPZZ"));
}

#[test]
fn hostile_separator_groups_never_panic_and_stay_bounded() {
    // Many groups of every kind, unclosed ones, huge text and stray line words.
    let mut rtf = String::from(r"{\rtf1\ansi\ftnbj ");
    for i in 0..2_000 {
        let word = ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"][i % 6];
        rtf.push_str(&format!(r"{{\{word} \pard\chftnsep\chftnsepc x{i}\par\sect\page{{\footnote y}}}}"));
    }
    rtf.push_str(r"{\ftnsep ");
    rtf.push_str(&"z".repeat(200_000));
    rtf.push_str(r"\par{\ftnsep {\aftnsep \chftnsep");
    let d = import(rtf.as_bytes()).unwrap();
    assert!(d.footnote_separators.separator.is_some());
    assert!(d.endnote_separators.continuation_notice.is_some());
    assert!(d.parts.is_empty());
    let out = export(&d);
    assert!(import(out.as_bytes()).is_ok());
}

#[test]
fn docx_default_separators_saved_as_rtf_read_back_as_the_defaults() {
    use wordcraft_doc::{Blocks, InlineObject, Paragraph};
    let line = |c: bool| -> Option<Blocks> {
        let mut p = Paragraph::default();
        p.insert_object(0, InlineObject::note_separator(c), &Default::default()).unwrap();
        Some(vec![std::sync::Arc::new(wordcraft_doc::Block::Para(p))])
    };
    let mut d = open(&format!(r"\pard\plain Body{NOTE}\par"));
    d.footnote_separators.separator = line(false);
    d.footnote_separators.continuation_separator = line(true);
    let out = export(&d);
    assert!(out.contains(r"{\*\ftnsep ") && out.contains(r"{\*\ftnsepc "), "{out}");
    let back = import(out.as_bytes()).unwrap();
    assert!(back.footnote_separators.is_empty(), "{:?}", back.footnote_separators);
    assert_eq!(body_texts(&back), ["Body"]);
}
