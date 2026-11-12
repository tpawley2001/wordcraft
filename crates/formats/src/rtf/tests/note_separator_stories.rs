//! A document's own note separators (RTF 1.9.1 "Footnotes": `{\ftnsep}`, `{\ftnsepc}`, `{\ftncn}`,
//! `{\aftnsep}`, `{\aftnsepc}`, `{\aftncn}`, with or without `\*`) are kept in
//! `Document::footnote_separators` / `Document::endnote_separators` on open and written back on
//! save. Expected values are what Microsoft 365 Word draws above the notes of each file.

use super::*;
use wordcraft_doc::{Align, Block, Blocks, InlineObject, NoteSeparators, Paragraph, PartKind};

const HEAD: &str = r"{\rtf1\ansi\deff0\fet2\ftnbj\aenddoc{\fonttbl{\f0 Arial;}}";
const NOTE: &str = r"{\footnote \pard\plain \chftn  note}";
const ENDNOTE: &str = r"{\footnote\ftnalt \pard\plain \chftn  enote}";

fn open(body: &str) -> Document {
    import(format!("{HEAD}{body}}}").as_bytes()).unwrap()
}

fn body_texts(d: &Document) -> Vec<String> {
    doc_paras(d).iter().map(|p| p.plain_text().trim().to_string()).collect()
}

fn note_parts(d: &Document) -> usize {
    d.parts.values().filter(|p| matches!(p.kind, PartKind::Footnote | PartKind::Endnote)).count()
}

fn paras(story: &Option<Blocks>) -> Vec<&Paragraph> {
    story.iter().flatten().filter_map(|b| if let Block::Para(p) = &**b { Some(p) } else { None }).collect()
}

/// A separator story's paragraphs as text; `None` when the document has none of its own.
fn texts(story: &Option<Blocks>) -> Option<Vec<String>> {
    story.as_ref().map(|_| paras(story).iter().map(|p| p.plain_text().trim().to_string()).collect())
}

/// The separator lines in a story, paragraph by paragraph (`true`: the continuation line).
fn lines(story: &Option<Blocks>) -> Vec<Vec<bool>> {
    paras(story).iter().map(|p| p.objects.iter().filter_map(InlineObject::as_note_separator).collect()).collect()
}

fn one(text: &str) -> Option<Vec<String>> {
    Some(vec![text.to_string()])
}

/// Everything the tests compare of the two sets of separators.
fn shape(s: &NoteSeparators) -> Vec<(Option<Vec<String>>, Vec<Vec<bool>>)> {
    [&s.separator, &s.continuation_separator, &s.continuation_notice].into_iter().map(|b| (texts(b), lines(b))).collect()
}

/// How many `{\*\word` groups `rtf` holds (`\ftnsep` is not counted inside `\ftnsepc`).
fn groups(rtf: &str, word: &str) -> usize {
    let open = format!(r"{{\*\{word}");
    rtf.match_indices(&open).filter(|(i, _)| !rtf[i + open.len()..].starts_with(|c: char| c.is_ascii_alphabetic())).count()
}

#[test]
fn the_three_footnote_stories_are_kept() {
    // Word draws SEPTEXT above the note; the body is "Body".
    let d = open(&format!(
        r"{{\ftnsep \pard\plain SEPTEXT\par}}{{\ftnsepc \pard\plain CONTSEP\par}}{{\ftncn \pard\plain NOTICE\par}}\pard\plain Body{NOTE}\par"
    ));
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(note_parts(&d), 1);
    assert_eq!(texts(&d.footnote_separators.separator), one("SEPTEXT"));
    assert_eq!(texts(&d.footnote_separators.continuation_separator), one("CONTSEP"));
    assert_eq!(texts(&d.footnote_separators.continuation_notice), one("NOTICE"));
    assert!(d.endnote_separators.is_empty());
}

#[test]
fn starred_groups_are_kept_the_same_way() {
    let d = open(&format!(
        r"{{\*\ftnsep \pard\plain SEPTEXT\par}}{{\*\ftnsepc \pard\plain CONTSEP\par}}{{\*\ftncn \pard\plain NOTICE\par}}\pard\plain Body{NOTE}\par"
    ));
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(texts(&d.footnote_separators.separator), one("SEPTEXT"));
    assert_eq!(texts(&d.footnote_separators.continuation_separator), one("CONTSEP"));
    assert_eq!(texts(&d.footnote_separators.continuation_notice), one("NOTICE"));
}

#[test]
fn each_word_fills_its_own_field() {
    for (word, starred) in [("ftnsep", false), ("ftnsepc", true), ("ftncn", false), ("aftnsep", true), ("aftnsepc", false), ("aftncn", true)] {
        let star = if starred { r"\*" } else { "" };
        let d = open(&format!(r"{{{star}\{word} \pard\plain ZZ{word}ZZ\par}}\pard\plain Body{NOTE} more{ENDNOTE}\par"));
        let f = &d.footnote_separators;
        let e = &d.endnote_separators;
        let got = [&f.separator, &f.continuation_separator, &f.continuation_notice, &e.separator, &e.continuation_separator, &e.continuation_notice]
            .map(texts);
        let want = ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"].map(|w| if w == word { one(&format!("ZZ{word}ZZ")) } else { None });
        assert_eq!(got, want, "\\{word}");
        assert_eq!(body_texts(&d), ["Body more"], "\\{word}");
        assert_eq!(note_parts(&d), 2, "\\{word}");
    }
}

#[test]
fn footnote_and_endnote_separators_side_by_side() {
    // Word: ASEP above the endnote, FSEP above the footnote.
    let d = open(&format!(r"{{\ftnsep \pard\plain FSEP\par}}{{\aftnsep \pard\plain ASEP\par}}\pard\plain Body{NOTE} more{ENDNOTE}\par"));
    assert_eq!(texts(&d.footnote_separators.separator), one("FSEP"));
    assert_eq!(texts(&d.endnote_separators.separator), one("ASEP"));
}

#[test]
fn the_line_characters_become_separator_lines_in_the_story() {
    // Word: a 2-inch line, then MIXED after it on the same line.
    let d = open(&format!(r"{{\ftnsep \pard\plain \chftnsep MIXED\par}}{{\ftnsepc \pard\plain \chftnsepc CONT\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(texts(&d.footnote_separators.separator), one("MIXED"));
    assert_eq!(lines(&d.footnote_separators.separator), [vec![false]]);
    assert_eq!(texts(&d.footnote_separators.continuation_separator), one("CONT"));
    assert_eq!(lines(&d.footnote_separators.continuation_separator), [vec![true]]);
    // The line comes before the text.
    let sep = paras(&d.footnote_separators.separator);
    let p = sep.first().expect("separator paragraph");
    assert_eq!(p.objects.len(), 1);
    assert!(!p.text.starts_with('M'), "{:?}", p.text);
    assert!(p.text.trim_end().ends_with("MIXED"), "{:?}", p.text);
}

#[test]
fn an_empty_separator_paragraph_is_kept_as_no_line() {
    // Word: nothing above the note but an empty line's height.
    let d = open(&format!(r"{{\ftnsep \pard\plain \par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(texts(&d.footnote_separators.separator), one(""));
    assert_eq!(lines(&d.footnote_separators.separator), [Vec::<bool>::new()]);
}

#[test]
fn the_last_group_of_a_kind_wins() {
    // Word draws SECOND.
    let d = open(&format!(r"{{\ftnsep \pard\plain FIRST\par}}{{\ftnsep \pard\plain SECOND\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(texts(&d.footnote_separators.separator), one("SECOND"));
}

#[test]
fn a_group_after_body_text_counts() {
    // Word draws LATE; its text is "Body", "Second".
    let d = open(&format!(r"\pard\plain Body{NOTE}\par{{\ftnsep \pard\plain LATE\par}}\pard\plain Second\par"));
    assert_eq!(body_texts(&d), ["Body", "Second"]);
    assert_eq!(texts(&d.footnote_separators.separator), one("LATE"));
}

#[test]
fn paragraphs_keep_their_own_formatting() {
    let d = open(&format!(r"{{\ftnsep \pard\qc\plain\b ONE\par\pard\plain TWO\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(texts(&d.footnote_separators.separator), Some(vec!["ONE".to_string(), "TWO".to_string()]));
    let sep = paras(&d.footnote_separators.separator);
    let (one, two) = (sep.first().expect("first paragraph"), sep.get(1).expect("second paragraph"));
    assert_eq!(one.props.align, Some(Align::Center));
    assert_eq!(one.props_of_char(0).bold, Some(true));
    assert_ne!(two.props.align, Some(Align::Center));
    assert_ne!(two.props_of_char(0).bold, Some(true));
    // None of it reaches the body.
    let body = doc_paras(&d);
    let p = body.first().expect("body paragraph");
    assert_eq!(body_texts(&d), ["Body"]);
    assert_ne!(p.props.align, Some(Align::Center));
    assert_ne!(p.props_of_char(0).bold, Some(true));
}

#[test]
fn default_groups_store_nothing_but_a_formatted_line_is_the_documents_own() {
    // What Word 97 wrote for the built-in separators: the defaults stay the defaults.
    let d = open(&format!(
        r"{{\ftnsep \pard\plain \chftnsep\par}}{{\ftnsepc \pard\plain \chftnsepc\par}}{{\aftnsep \pard\plain \chftnsep\par}}{{\aftnsepc \pard\plain \chftnsepc\par}}\pard\plain Body{NOTE}\par"
    ));
    assert!(d.footnote_separators.is_empty(), "{:?}", d.footnote_separators);
    assert!(d.endnote_separators.is_empty(), "{:?}", d.endnote_separators);
    assert_eq!(body_texts(&d), ["Body"]);
    // A centred line is not the default.
    let d = open(&format!(r"{{\ftnsep \pard\plain\qc \chftnsep\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(lines(&d.footnote_separators.separator), [vec![false]]);
    let sep = paras(&d.footnote_separators.separator);
    assert_eq!(sep.first().map(|p| p.props.align), Some(Some(Align::Center)));
}

#[test]
fn a_group_in_a_cell_or_a_note_and_an_empty_group_store_nothing() {
    for body in [
        r"\trowd\cellx3000\pard\intbl {\ftnsep \pard\plain ZZSEPZZ\par}Cell\cell\row\pard\plain After\par".to_string(),
        r"\pard\plain Body{\footnote \pard\plain \chftn  {\ftnsep ZZSEPZZ}note}\par".to_string(),
        format!(r"{{\ftnsep}}{{\*\aftnsep}}\pard\plain Body{NOTE}\par"),
    ] {
        let d = open(&body);
        assert!(d.footnote_separators.is_empty(), "{body}");
        assert!(d.endnote_separators.is_empty(), "{body}");
    }
}

#[test]
fn a_note_inside_a_separator_group_is_no_note() {
    let d = open(&format!(r"{{\ftnsep \pard\plain A{{\footnote \pard\plain \chftn  inner}}B\sect\sectd\par}}\pard\plain Body{NOTE}\par"));
    assert_eq!(note_parts(&d), 1);
    assert_eq!(body_texts(&d), ["Body"]);
    assert_eq!(d.sections().len(), 1);
    let sep = texts(&d.footnote_separators.separator).expect("separator kept");
    assert!(sep.first().is_some_and(|t| t.starts_with('A')), "{sep:?}");
    assert!(!format!("{:?}", d.parts).contains("inner"));
}

#[test]
fn save_writes_each_story_once_and_reopens_the_same() {
    let d = open(&format!(
        r"{{\ftnsep \pard\plain \chftnsep MIXED\par}}{{\ftnsepc \pard\plain \chftnsepc\par\pard\plain CONT2\par}}{{\ftncn \pard\plain NOTICE\par}}{{\aftnsep \pard\qc\plain\b ASEP\par}}{{\aftnsepc \pard\plain \par}}{{\aftncn \pard\plain ANOTICE\par}}\pard\plain Body{NOTE} more{ENDNOTE}\par"
    ));
    let out = export(&d);
    for word in ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"] {
        assert_eq!(groups(&out, word), 1, "\\{word} in {out}");
    }
    assert_eq!(out.matches(r"\chftnsep").count(), 2, "{out}");
    assert_eq!(out.matches(r"\chftnsepc").count(), 1, "{out}");
    let back = import(out.as_bytes()).unwrap();
    assert_eq!(shape(&back.footnote_separators), shape(&d.footnote_separators));
    assert_eq!(shape(&back.endnote_separators), shape(&d.endnote_separators));
    assert_eq!(texts(&back.footnote_separators.separator), one("MIXED"));
    assert_eq!(texts(&back.footnote_separators.continuation_separator), Some(vec![String::new(), "CONT2".to_string()]));
    assert_eq!(lines(&back.footnote_separators.continuation_separator), [vec![true], vec![]]);
    assert_eq!(texts(&back.endnote_separators.continuation_separator), one(""));
    assert_eq!(texts(&back.endnote_separators.continuation_notice), one("ANOTICE"));
    let sep = paras(&back.endnote_separators.separator);
    let p = sep.first().expect("endnote separator paragraph");
    assert_eq!(p.props.align, Some(Align::Center));
    assert_eq!(p.props_of_char(0).bold, Some(true));
    assert_eq!(body_texts(&back), ["Body more"]);
    assert_eq!(note_parts(&back), 2);
}

#[test]
fn every_saved_separator_paragraph_ends_with_par() {
    let d = open(&format!(r"{{\ftnsep \pard\plain ONE\par\pard\plain TWO\par}}\pard\plain Body{NOTE}\par"));
    let out = export(&d);
    let at = out.find(r"{\*\ftnsep").expect("separator group written");
    let group = &out[at..];
    let (one, two) = (group.find("ONE").expect("ONE"), group.find("TWO").expect("TWO"));
    assert!(group[one..two].contains(r"\par"), "{group}");
    let rest = &group[two..];
    let close = rest.find('}').expect("group closes");
    assert!(rest[..close].contains(r"\par"), "{group}");
}

#[test]
fn a_document_without_its_own_separators_writes_none() {
    let d = open(&format!(r"\pard\plain Body{NOTE} more{ENDNOTE}\par"));
    let out = export(&d);
    for word in ["ftnsep", "ftnsepc", "ftncn", "aftnsep", "aftnsepc", "aftncn"] {
        assert!(!out.contains(&format!(r"\{word}")), "\\{word} in {out}");
    }
    assert!(!out.contains(r"\chftnsep"), "{out}");
}
