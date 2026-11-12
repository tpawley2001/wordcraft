//! STYLEREF's style-name argument, as Word reads it (measured with synthetic documents): one
//! argument, found by the style's name or one of its aliases, never by its internal id.

use crate::*;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{PartKind, Style, StyleKind, para_block};

/// Word's "not defined" message with no name in it: two spaces between `apply` and `to`.
const EMPTY: &str = "Error! Use the Home tab to apply  to the text that you want to appear here.";

fn not_defined(name: &str) -> String {
    format!("Error! Use the Home tab to apply {name} to the text that you want to appear here.")
}

fn field(instr: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: "stale".into(), locked: false, dirty: false, code: false, form: String::new() }
}

fn style(id: &str, name: &str, aliases: Option<&str>) -> Style {
    Style {
        id: id.into(),
        name: name.into(),
        kind: StyleKind::Paragraph,
        based_on: Some("Normal".into()),
        aliases: aliases.map(str::to_string),
        ..Style::default()
    }
}

/// The probe document: one paragraph each of `heading 1` (id `Heading1`), `heading 2` (id
/// `Heading2`), `My Own` (id `MyOwn`), `My`, `Second Style` (id `SecondStyle`, alias `ss`) and
/// `Third Style,ts` (id `ThirdStyle`, the alias inside the name), then a `Normal` paragraph.
fn probe() -> Document {
    let mut d = Document::from_text("One text\nTwo text\nOwn text\nMy text\nAlias text\nThird text\nBody text.");
    let styles = Arc::make_mut(&mut d.styles);
    styles.styles.push(style("MyOwn", "My Own", None));
    styles.styles.push(style("My", "My", None));
    styles.styles.push(style("SecondStyle", "Second Style", Some("ss")));
    styles.styles.push(style("ThirdStyle", "Third Style,ts", None));
    for (i, id) in ["Heading1", "Heading2", "MyOwn", "My", "SecondStyle", "ThirdStyle", "Normal"].into_iter().enumerate() {
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

/// What a body STYLEREF at the last paragraph evaluates to (the path Update Field takes).
fn body_result(code: &str) -> String {
    let d = probe();
    styleref::StyleIndex::build(&d).eval(code, styleref::At::Body(6))
}

#[test]
fn one_quoted_or_one_word_name_keeps_working() {
    let got = header_results(&[
        r#"STYLEREF "Heading 2""#,
        "STYLEREF My",
        r#"STYLEREF \l "Heading 2""#,
        r#"STYLEREF "heading 2"\l"#,
        r#"styleref "heading 2""#,
        "STYLEREF 2",
        r#"STYLEREF "2""#,
        r#"STYLEREF "MY OWN""#,
        r#"STYLEREF "Heading 2" \* MERGEFORMAT"#,
    ]);
    assert_eq!(got, ["Two text", "My text", "Two text", "Two text", "Two text", "Two text", "Two text", "Own text", "Two text"]);
}

#[test]
fn an_unquoted_name_of_several_words_is_several_arguments() {
    let got = header_results(&["STYLEREF Heading 2", r"STYLEREF heading 2 \l", "STYLEREF My Own", "STYLEREF My Own Extra"]);
    assert_eq!(got, [EMPTY, EMPTY, EMPTY, EMPTY]);
}

#[test]
fn a_general_switch_takes_its_word_but_not_the_name_s_second_word() {
    assert_eq!(header_results(&[r"STYLEREF Heading 2 \* MERGEFORMAT"]), [EMPTY]);
    // `\* MERGEFORMAT` after one name is not a second argument.
    assert_eq!(header_results(&[r"STYLEREF My \* MERGEFORMAT"]), ["My text"]);
}

#[test]
fn text_after_the_name_is_a_second_argument() {
    let got = header_results(&[r#"STYLEREF "Heading 2" junk"#, r#"STYLEREF "Heading 2" "junk""#, r#"STYLEREF "Heading 2" \l junk"#]);
    assert_eq!(got, [EMPTY, EMPTY, EMPTY]);
}

#[test]
fn a_quoted_switch_is_an_argument() {
    assert_eq!(header_results(&[r#"STYLEREF "Heading 2" "\l""#]), [EMPTY]);
}

#[test]
fn a_style_id_that_is_not_a_name_is_not_found() {
    let got = header_results(&["STYLEREF MyOwn", "STYLEREF Heading1", "STYLEREF SecondStyle"]);
    assert_eq!(got, [not_defined("MyOwn"), not_defined("Heading1"), not_defined("SecondStyle")]);
}

#[test]
fn an_alias_finds_its_style() {
    let got = header_results(&["STYLEREF ss", "STYLEREF SS", r#"STYLEREF "Second Style""#, r#"STYLEREF "Second Style,ss""#]);
    assert_eq!(got, ["Alias text", "Alias text", "Alias text", "Alias text"]);
}

#[test]
fn an_alias_inside_the_style_name_finds_its_style() {
    let got = header_results(&["STYLEREF ts", r#"STYLEREF "Third Style""#, r#"STYLEREF "Third Style,ts""#]);
    assert_eq!(got, ["Third text", "Third text", "Third text"]);
}

#[test]
fn body_fields_read_the_name_the_same_way() {
    assert_eq!(body_result(r#"STYLEREF "Heading 2""#), "Two text");
    assert_eq!(body_result("STYLEREF Heading 2"), EMPTY);
    assert_eq!(body_result(r#"STYLEREF "Heading 2" junk"#), EMPTY);
    assert_eq!(body_result("STYLEREF MyOwn"), not_defined("MyOwn"));
    assert_eq!(body_result("STYLEREF ss"), "Alias text");
}

#[test]
fn heading_levels_still_find_the_heading_styles() {
    assert_eq!(header_results(&["STYLEREF 1", "STYLEREF 2"]), ["One text", "Two text"]);
}

#[test]
fn no_name_still_says_so() {
    assert_eq!(header_results(&["STYLEREF", r"STYLEREF \l"]), ["Error! No style name given.", "Error! No style name given."]);
}
