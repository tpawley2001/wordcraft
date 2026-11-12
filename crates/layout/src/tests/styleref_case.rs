//! STYLEREF with the capitalization switches (`\* Upper`, `\* Lower`, `\* Caps`, `\* FirstCap`) and
//! `\p` in a page header, as Word shows them (measured with synthetic documents): a result takes
//! the switch, an error message never does, and `\p` in a header shows the text.

use crate::*;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{PartKind, Style, StyleKind, para_block};

const HEADING: &str = "wEEKLY rEPORT on sales";

/// Word's "not defined" message with no name in it: two spaces between `apply` and `to`.
const EMPTY: &str = "Error! Use the Home tab to apply  to the text that you want to appear here.";
const NO_TEXT: &str = "Error! No text of specified style in document.";
const NO_STYLE_NAME: &str = "Error! No style name given.";

fn not_defined(name: &str) -> String {
    format!("Error! Use the Home tab to apply {name} to the text that you want to appear here.")
}

fn field(instr: &str) -> InlineObject {
    InlineObject::Field {
        instr: instr.into(),
        result: "stale".into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    }
}

fn style(id: &str, name: &str) -> Style {
    Style { id: id.into(), name: name.into(), kind: StyleKind::Paragraph, based_on: Some("Normal".into()), ..Style::default() }
}

/// The probe document: a `heading 1` paragraph in mixed case, a `heading 2` paragraph, a `My Own`
/// paragraph in lowercase and a `Normal` paragraph. The style `Unused` has no paragraph.
fn probe() -> Document {
    let mut d = Document::from_text(&format!("{HEADING}\nsecond LEVEL text\nplain words here\nBody text."));
    let styles = Arc::make_mut(&mut d.styles);
    styles.styles.push(style("MyOwn", "My Own"));
    styles.styles.push(style("Unused", "Unused"));
    for (i, id) in ["Heading1", "Heading2", "MyOwn", "Normal"].into_iter().enumerate() {
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().props.style = Some(id.to_string());
    }
    d
}

/// What each STYLEREF code shows in the probe document's page header.
fn header_results(codes: &[&str]) -> Vec<String> {
    crate::test_fonts();
    let mut d = probe();
    let blocks = codes
        .iter()
        .map(|c| {
            let mut p = Paragraph::new();
            p.insert_object(0, field(c), &CharProps::default()).unwrap();
            para_block(p)
        })
        .collect();
    let id = d.add_part(PartKind::Header, blocks);
    d.last_section.headers.default = Some(id);
    let mut c = LayoutCache::new();
    let out = layout(&d, &mut c, &LayoutOptions::default());
    let page = out.pages.first().expect("a page");
    page.header
        .iter()
        .flat_map(|it| if let Placed::Lines { para, .. } = it { para.shown.iter().map(|s| s.1.clone()).collect() } else { vec![] })
        .collect()
}

/// What a body STYLEREF in the last paragraph evaluates to (the path Update Field takes).
fn body_result(code: &str) -> String {
    let d = probe();
    styleref::StyleIndex::build(&d).eval(code, styleref::At::Body(3))
}

#[test]
fn results_take_the_capitalization_switches() {
    let got = header_results(&[
        r#"STYLEREF "Heading 1""#,
        r#"STYLEREF "Heading 1" \* Upper"#,
        r#"STYLEREF "Heading 1" \* Lower"#,
        r#"STYLEREF "Heading 1" \* FirstCap"#,
        r#"STYLEREF "Heading 1" \* Upper \* MERGEFORMAT"#,
        r#"STYLEREF "Heading 2" \* Upper \* Lower"#,
        r#"STYLEREF "My Own" \* Caps"#,
    ]);
    let want = [
        HEADING,
        "WEEKLY REPORT ON SALES",
        "weekly report on sales",
        "WEEKLY rEPORT on sales",
        "WEEKLY REPORT ON SALES",
        "second level text",
        "Plain Words Here",
    ];
    assert_eq!(got, want);
}

#[test]
fn error_results_keep_their_capitalization_in_a_header() {
    let got = header_results(&[
        r#"STYLEREF "Nope" \* Upper"#,
        r#"STYLEREF "Nope" \* Lower"#,
        r"STYLEREF Heading 1 \* Upper",
        r"STYLEREF \* Upper",
        r#"STYLEREF "Unused" \* Upper"#,
        r#"STYLEREF "Unused" \* Lower"#,
        r#"STYLEREF "Unused" \* Caps"#,
    ]);
    let want = [
        not_defined("Nope"),
        not_defined("Nope"),
        EMPTY.to_string(),
        NO_STYLE_NAME.to_string(),
        NO_TEXT.to_string(),
        NO_TEXT.to_string(),
        NO_TEXT.to_string(),
    ];
    assert_eq!(got, want);
}

/// `eval` itself applies the switches, so Update Field in the body shows what a header shows.
#[test]
fn eval_applies_the_switches_to_results_only() {
    assert_eq!(body_result(r#"STYLEREF "Heading 1" \* Upper"#), "WEEKLY REPORT ON SALES");
    assert_eq!(body_result(r#"STYLEREF "Heading 1" \* Lower"#), "weekly report on sales");
    assert_eq!(body_result(r#"STYLEREF "Heading 1""#), HEADING);
    assert_eq!(body_result(r#"STYLEREF "Nope" \* Upper"#), not_defined("Nope"));
    assert_eq!(body_result(r#"STYLEREF "Unused" \* Lower"#), NO_TEXT);
    assert_eq!(body_result(r"STYLEREF \* Upper"), NO_STYLE_NAME);
}

#[test]
fn position_switch_in_a_header_shows_the_text() {
    let got = header_results(&[r#"STYLEREF "Heading 1" \p"#, r#"STYLEREF "Heading 1" \p \* Lower"#, r#"STYLEREF "Heading 1" \p \* Upper"#]);
    assert_eq!(got, [HEADING, "weekly report on sales", "WEEKLY REPORT ON SALES"]);
}

/// In the body `\p` still shows where the paragraph is.
#[test]
fn position_switch_in_the_body_is_unchanged() {
    assert_eq!(body_result(r#"STYLEREF "Heading 1" \p"#), "above");
}
