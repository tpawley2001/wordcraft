//! A .doc paragraph names its style by istd ([MS-DOC] §2.4.6.5), not by name, so two styles whose
//! names differ only in letter case ("Body Text" and "Body text") stay two styles, each with its
//! own formatting, as in Word.

use super::{RichSpec, cfb_file, grpprl, read, rich_doc, std_bytes};

const HPS: u16 = 0x4A43; // sprmCHps, half-points

fn size(hps: u16) -> Vec<u8> {
    grpprl(&[(HPS, &hps.to_le_bytes())])
}

/// The style ids of the document's first two paragraphs.
fn para_styles(doc: &wordcraft_doc::Document) -> Vec<String> {
    doc.body.iter().filter_map(|b| b.as_para()).take(2).map(|p| p.props.style.clone().expect("paragraph has a style")).collect()
}

fn style<'a>(doc: &'a wordcraft_doc::Document, id: &str) -> &'a wordcraft_doc::Style {
    doc.styles.get(id).unwrap_or_else(|| panic!("no style with id {id:?}"))
}

/// Normal (istd 0); "Body Text" (istd 1, 7 pt); "Body text" (istd 2, based on istd 1, 10 pt).
/// "one" is in istd 1, "two" in istd 2.
fn two_cases() -> wordcraft_doc::Document {
    let spec = RichSpec {
        text: "one\rtwo\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "Body Text", Vec::new(), size(14)),
            (1, 1, "Body text", Vec::new(), size(20)),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(3, 1, Vec::new()), (7, 2, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

#[test]
fn styles_differing_only_in_case_get_their_own_ids() {
    let d = two_cases();
    let ids = para_styles(&d);
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1], "both paragraphs point at one style: {ids:?}");
}

#[test]
fn each_paragraph_keeps_its_own_style_formatting() {
    let d = two_cases();
    let ids = para_styles(&d);
    let built_in = style(&d, &ids[0]);
    let user = style(&d, &ids[1]);
    assert_eq!(built_in.chr.size, Some(7.0));
    assert_eq!(user.chr.size, Some(10.0));
    assert_eq!(built_in.name, "Body Text");
    assert_eq!(user.name, "Body text");
}

#[test]
fn the_lower_case_style_is_based_on_the_other() {
    let d = two_cases();
    let ids = para_styles(&d);
    assert_eq!(style(&d, &ids[1]).based_on.as_deref(), Some(ids[0].as_str()));
}

/// Guard: one "Body Text" style alone keeps working (passes before the change).
#[test]
fn a_single_style_is_unchanged() {
    let spec = RichSpec {
        text: "one\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body Text", Vec::new(), size(14))],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(3, 1, Vec::new())],
    };
    let d = read(&rich_doc(&spec)).expect("opens");
    let ids = para_styles(&d);
    assert_eq!(ids.len(), 1);
    assert_eq!(style(&d, &ids[0]).chr.size, Some(7.0));
}

// Word's style ids and built-in styles (carried from #4484).

/// [`rich_doc`] with each style's built-in index (`sti`, [MS-DOC] §2.9.260), parallel to
/// `spec.styles`; the plain builder marks every style 0x0FFF.
fn rich_doc_sti(spec: &RichSpec, sti: &[u16]) -> Vec<u8> {
    use std::io::Read;
    let doc = rich_doc(spec);
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(doc)).expect("cfb");
    let mut stream = |name: &str| {
        let mut v = Vec::new();
        comp.open_stream(name).expect("stream").read_to_end(&mut v).expect("read");
        v
    };
    let word = stream("/WordDocument");
    let mut table = stream("/0Table");
    for (s, sti) in spec.styles.iter().zip(sti) {
        let std = std_bytes(s.0, s.1, s.2, &s.3, &s.4);
        let at = table.windows(std.len()).position(|w| w == std.as_slice()).expect("the style's STD");
        // After the record's cbStd: sti(12) and flags.
        table[at + 2..at + 4].copy_from_slice(&sti.to_le_bytes());
    }
    cfb_file(&[("WordDocument", &word), ("0Table", &table)])
}

/// Word's own heading 1 (`sti` 1) takes Word's id, which Home › Styles › Heading 1 (Ctrl+Alt+1)
/// applies; with `Heading 1` the command failed with "no style `Heading1`" in .doc files.
#[test]
fn a_built_in_heading_takes_words_id() {
    let spec = RichSpec {
        text: "centered\rplain\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "heading 1", grpprl(&[(0x2461, &[0x01, 0x00])]), Vec::new()), // centered
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new()), (9, 0, Vec::new())],
    };
    let doc = read(&rich_doc_sti(&spec, &[0, 1])).expect("opens");
    let heading = doc.styles.styles.iter().find(|s| s.name == "Heading 1").expect("style");
    assert_eq!(heading.id, "Heading1");
    assert_eq!(heading.para.align, Some(wordcraft_doc::props::Align::Center));
    assert_eq!(heading.para.outline_level, Some(0));
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    assert_eq!(p0.props.style.as_deref(), Some("Heading1"));
}

/// A style based on itself (a corrupt `istdBase`) keeps no base.
#[test]
fn a_style_based_on_itself_keeps_no_base() {
    let spec = RichSpec {
        text: "Text\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 1, "Self", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new())],
    };
    let doc = read(&rich_doc(&spec)).expect("opens");
    let own = doc.styles.styles.iter().find(|s| s.name == "Self").expect("style");
    assert_eq!(own.based_on, None, "based on itself");
}

/// The TOC level styles get Word's ids (`TOC1`), the ones References › Update Table gives its
/// entries; with `TOC 1` the rebuilt entries lost the document's TOC formatting.
#[test]
fn toc_styles_take_words_ids() {
    let spec = RichSpec {
        text: "Intro\rPart\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "toc 1", grpprl(&[(0x2461, &[0x01, 0x00])]), Vec::new()),
            (1, 1, "toc 2", Vec::new(), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new()), (6, 2, Vec::new())],
    };
    let doc = read(&rich_doc(&spec)).expect("opens");
    let ids: Vec<(&str, &str, Option<&str>)> =
        doc.styles.styles.iter().filter(|s| s.name.starts_with("TOC")).map(|s| (s.id.as_str(), s.name.as_str(), s.based_on.as_deref())).collect();
    assert_eq!(ids, [("TOC1", "TOC 1", Some("Normal")), ("TOC2", "TOC 2", Some("TOC1"))]);
    let styles: Vec<Option<&str>> = doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.style.as_deref()).collect();
    assert_eq!(styles, [Some("TOC1"), Some("TOC2")]);
}

/// A list style numbers its paragraphs without a list of their own. Its `sprmPIlfo` was read
/// only from paragraphs, so the numbers and bullets of "Numbered Paragraph"-style documents were
/// lost. Ending the list (in a based-on style or a paragraph) is `num: 0`; `ilfo 0` on a
/// paragraph whose style has no list keeps nothing, so its indents stay.
#[test]
fn list_styles_number_their_paragraphs() {
    use wordcraft_doc::props::NumRef;
    let ilfo = |n: i16| n.to_le_bytes();
    let spec = RichSpec {
        text: "a\rb\rc\rd\re\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "List Number", grpprl(&[(0x460B, &ilfo(1)), (0x260A, &[0])]), Vec::new()),
            (1, 1, "List Number Extra", grpprl(&[(0x460B, &ilfo(0))]), Vec::new()),
            (1, 0, "Body", Vec::new(), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![
            (0, 1, Vec::new()),
            (2, 2, Vec::new()),
            (4, 1, grpprl(&[(0x460B, &ilfo(0))])),
            (6, 3, grpprl(&[(0x460B, &ilfo(0))])),
            (8, 1, grpprl(&[(0x260A, &[2])])),
        ],
    };
    let doc = read(&rich_doc(&spec)).expect("opens");
    let style = |id: &str| doc.styles.get(id).and_then(|s| s.para.numbering);
    assert_eq!(style("List Number"), Some(NumRef { num: 1, level: 0 }));
    assert_eq!(style("List Number Extra"), Some(NumRef { num: 0, level: 0 }), "ends the based-on list");
    let paras: Vec<_> = doc.body.iter().filter_map(|b| b.as_para()).collect();
    let own: Vec<Option<NumRef>> = paras.iter().map(|p| p.props.numbering).collect();
    assert_eq!(own, [None, None, Some(NumRef { num: 0, level: 0 }), None, Some(NumRef { num: 1, level: 2 })]);
    let shown: Vec<Option<u32>> =
        paras.iter().map(|p| p.props.numbering.or(doc.styles.resolve_para(&p.props).numbering).filter(|n| n.num != 0).map(|n| n.num)).collect();
    assert_eq!(shown, [Some(1), None, None, None, Some(1)]);
}

/// Word's built-in styles (`sti` below istiUser) are built in, and with no quick-style flag
/// read the Home › Styles gallery shows Word's usual set, as for a DOCX file without
/// `w:qFormat`. The gallery was empty for every .doc file.
#[test]
fn built_in_styles_fill_the_styles_gallery() {
    let spec = RichSpec {
        text: "a\r",
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "heading 1", Vec::new(), Vec::new()),
            (1, 0, "Report Body", Vec::new(), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 0, Vec::new())],
    };
    let sti: Vec<u16> = vec![0, 1, 0x0FFE];
    let doc = read(&rich_doc_sti(&spec, &sti)).expect("opens");
    let st = |id: &str| doc.styles.get(id).cloned().expect(id);
    assert!(st("Normal").builtin && st("Heading1").builtin && !st("Report Body").builtin);
    assert!(st("Normal").quick && st("Heading1").quick, "Word's usual gallery");
    assert!(!st("Report Body").quick);
}

/// Built-in styles are known by their `sti`, which is the same in every language: German
/// Word's "Überschrift 1" and "Verzeichnis 1" are heading 1 and toc 1. By name alone they got
/// ids no command looks for.
#[test]
fn localized_built_in_styles_take_words_ids() {
    let spec = RichSpec {
        text: "a\rb\r",
        styles: vec![
            (1, 0x0FFF, "Standard", Vec::new(), Vec::new()),
            (1, 0, "\u{DC}berschrift 1", Vec::new(), Vec::new()),
            (1, 0, "Verzeichnis 1", Vec::new(), Vec::new()),
            (1, 0, "Eigene", Vec::new(), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new()), (2, 2, Vec::new())],
    };
    let sti: Vec<u16> = vec![0, 1, 19, 0x0FFE];
    let doc = read(&rich_doc_sti(&spec, &sti)).expect("opens");
    let own: Vec<(&str, &str, Option<&str>)> =
        doc.styles.styles.iter().take(4).map(|s| (s.id.as_str(), s.name.as_str(), s.based_on.as_deref())).collect();
    assert_eq!(
        own,
        [
            ("Normal", "Normal", None),
            ("Heading1", "Heading 1", Some("Normal")),
            ("TOC1", "TOC 1", Some("Normal")),
            ("Eigene", "Eigene", Some("Normal"))
        ]
    );
    assert_eq!(doc.styles.get("Heading1").and_then(|s| s.para.outline_level), Some(0));
    let styles: Vec<Option<&str>> = doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.style.as_deref()).collect();
    assert_eq!(styles, [Some("Heading1"), Some("TOC1")]);
}

/// A user style named like a built-in one gets its own id: "Heading 1" in a German document
/// took `Heading1` from "Überschrift 1", so both paragraphs showed one style (review of #4484).
#[test]
fn user_styles_named_like_built_in_ones_keep_their_own_id() {
    let spec = RichSpec {
        text: "a\rb\rc\r",
        styles: vec![
            (1, 0x0FFF, "Standard", Vec::new(), Vec::new()),
            (1, 0, "\u{DC}berschrift 1", Vec::new(), Vec::new()),
            (1, 0, "Heading 1", Vec::new(), Vec::new()),
            (1, 2, "Eigene", Vec::new(), Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new()), (2, 2, Vec::new()), (4, 3, Vec::new())],
    };
    let sti: Vec<u16> = vec![0, 1, 0x0FFE, 0x0FFE];
    let doc = read(&rich_doc_sti(&spec, &sti)).expect("opens");
    let own: Vec<(&str, Option<&str>, bool)> = doc.styles.styles.iter().take(4).map(|s| (s.id.as_str(), s.based_on.as_deref(), s.builtin)).collect();
    assert_eq!(
        own,
        [("Normal", None, true), ("Heading1", Some("Normal"), true), ("Heading1_2", Some("Normal"), false), ("Eigene", Some("Heading1_2"), false)]
    );
    let styles: Vec<Option<&str>> = doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.style.as_deref()).collect();
    assert_eq!(styles, [Some("Heading1"), Some("Heading1_2"), Some("Eigene")]);
    let mut ids: Vec<&str> = doc.styles.styles.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    assert!(ids.windows(2).all(|w| w.first() != w.get(1)), "{ids:?}");
}
