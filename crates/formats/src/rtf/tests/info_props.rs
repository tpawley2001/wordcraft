//! More of the information group (`\operator`, `\category`, `\version`, `\revtim`, `\printim`) and
//! user properties (`{\*\userprops {\propname …}\proptypeN{\staticval …}{\linkval …}}`), RTF 1.9.1:
//! the RTF form of DOCX `cp:lastModifiedBy`, `cp:category`, `cp:revision`, `dcterms:modified`,
//! `cp:lastPrinted` and the custom properties part.

use super::*;
use wordcraft_doc::{CustomProp, StoryRef};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn round_trip(d: &Document) -> Document {
    doc(&export(d))
}

fn with_info(info: &str) -> String {
    format!(r"{{\rtf1\ansi{{\info{{\title T}}{info}}}\pard Body\par}}")
}

fn with_props(props: &str) -> String {
    format!(r"{{\rtf1\ansi{{\info{{\title T}}}}{{\*\userprops {props}}}\pard Body\par}}")
}

/// (name, kind, value, link target).
fn props(d: &Document) -> Vec<(String, String, String, Option<String>)> {
    d.custom_props.iter().map(|p| (p.name.clone(), p.kind.clone(), p.value.clone(), p.link_target.clone())).collect()
}

fn p(name: &str, kind: &str, value: &str) -> (String, String, String, Option<String>) {
    (name.into(), kind.into(), value.into(), None)
}

#[test]
fn info_fields_are_read() {
    let d = doc(&with_info(
        r"{\operator Bob Lee}{\category Reports}{\version7}{\creatim\yr2020\mo1\dy2\hr3\min4}{\revtim\yr2024\mo3\dy5\hr14\min7}{\printim\yr2023\mo12\dy31\hr0\min0}",
    ));
    assert_eq!(d.core.last_modified_by, "Bob Lee");
    assert_eq!(d.core.category, "Reports");
    assert_eq!(d.core.revision, 7);
    assert_eq!(d.core.created, "2020-01-02T03:04:00Z");
    assert_eq!(d.core.modified, "2024-03-05T14:07:00Z");
    assert_eq!(d.core.last_printed, "2023-12-31T00:00:00Z");
    assert_eq!(d.core.title, "T");
}

#[test]
fn missing_info_fields_keep_the_defaults() {
    let d = doc(&with_info(""));
    assert_eq!(d.core.last_modified_by, "");
    assert_eq!(d.core.category, "");
    assert_eq!(d.core.revision, 0);
    assert_eq!(d.core.modified, "");
    assert_eq!(d.core.last_printed, "");
    assert!(d.custom_props.is_empty());
}

#[test]
fn info_text_never_reaches_the_body() {
    let d = doc(&with_info(r"{\operator Bob}{\category Cat}{\version3}{\revtim\yr2024\mo3\dy5\hr1\min2\sec30}"));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
    let d = doc(&with_props(r"{\propname Client}\proptype30{\staticval Acme}"));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
}

#[test]
fn bad_version_numbers_do_not_panic() {
    assert_eq!(doc(&with_info(r"{\version-3}")).core.revision, 0);
    let _ = doc(&with_info(r"{\version99999999999}"));
}

#[test]
fn info_fields_round_trip() {
    let mut d = Document::new();
    d.core.title = "T".into();
    d.core.last_modified_by = "Zoë {Ops}".into();
    d.core.category = "Reports\\2024".into();
    d.core.revision = 12;
    d.core.modified = "2024-03-05T14:07:00Z".into();
    d.core.last_printed = "2023-12-31T08:30:00Z".into();
    let out = export(&d);
    assert!(out.contains(r"\operator"), "{out}");
    assert!(out.contains(r"{\version12}") || out.contains(r"{\version 12}"), "{out}");
    let r = round_trip(&d);
    assert_eq!(r.core.last_modified_by, "Zoë {Ops}");
    assert_eq!(r.core.category, "Reports\\2024");
    assert_eq!(r.core.revision, 12);
    assert_eq!(r.core.modified, "2024-03-05T14:07:00Z");
    assert_eq!(r.core.last_printed, "2023-12-31T08:30:00Z");
}

#[test]
fn empty_info_fields_are_not_written() {
    let out = export(&Document::new());
    for w in [r"\operator", r"\category", r"\version", r"\revtim", r"\printim", r"\userprops"] {
        assert!(!out.contains(w), "{w} in {out}");
    }
}

#[test]
fn user_properties_are_read_by_type() {
    let d = doc(&with_props(concat!(
        r"{\propname Client}\proptype30{\staticval Acme Ltd}",
        r"{\propname Pages}\proptype3{\staticval -5}",
        r"{\propname Ratio}\proptype5{\staticval 2.5}",
        r"{\propname Checked}\proptype11{\staticval 1}",
        r"{\propname Off}\proptype11{\staticval FALSE}",
        r"{\propname Due}\proptype64{\staticval 2024-03-05}",
        r"{\propname Odd}\proptype99{\staticval x}",
    )));
    assert_eq!(
        props(&d),
        vec![
            p("Client", "lpwstr", "Acme Ltd"),
            p("Pages", "i4", "-5"),
            p("Ratio", "r8", "2.5"),
            p("Checked", "bool", "true"),
            p("Off", "bool", "false"),
            p("Due", "filetime", "2024-03-05T00:00:00Z"),
            p("Odd", "lpwstr", "x"),
        ]
    );
}

#[test]
fn values_that_do_not_parse_are_kept_as_text() {
    let d = doc(&with_props(concat!(
        r"{\propname N}\proptype3{\staticval twelve}",
        r"{\propname R}\proptype5{\staticval 1,5x}",
        r"{\propname D}\proptype64{\staticval next week}",
    )));
    assert_eq!(props(&d), vec![p("N", "lpwstr", "twelve"), p("R", "lpwstr", "1,5x"), p("D", "lpwstr", "next week")]);
}

#[test]
fn linked_property_keeps_its_bookmark() {
    let d = doc(&with_props(r"{\propname Total}\proptype30{\staticval 9}{\linkval Sum1}"));
    assert_eq!(props(&d), vec![("Total".into(), "lpwstr".into(), "9".into(), Some("Sum1".into()))]);
}

#[test]
fn empty_names_are_dropped_and_duplicates_keep_the_last() {
    let d = doc(&with_props(concat!(
        r"{\propname }\proptype30{\staticval lost}",
        r"{\propname Ver}\proptype30{\staticval a}",
        r"{\propname VER}\proptype30{\staticval b}",
    )));
    assert_eq!(d.custom_props.len(), 1);
    assert_eq!(d.custom_prop("ver"), Some("b"));
}

#[test]
fn unicode_names_and_values_decode() {
    let d = doc(&with_props(r"{\propname Caf\u233?}\proptype30{\staticval  3?▙1?}"));
    assert_eq!(props(&d), vec![p("Café", "lpwstr", "中文")]);
}

#[test]
fn too_many_properties_are_capped() {
    let many: String = (0..1500).map(|i| format!(r"{{\propname P{i}}}\proptype30{{\staticval {i}}}")).collect();
    let d = doc(&with_props(&many));
    assert_eq!(d.custom_props.len(), 1000);
    let long = "x".repeat(100_000);
    let d = doc(&with_props(&format!(r"{{\propname L}}\proptype30{{\staticval {long}}}")));
    assert!(d.custom_prop("L").is_some_and(|v| v.len() <= 64 * 1024));
}

#[test]
fn user_properties_round_trip() {
    let mut d = Document::new();
    d.set_custom_prop_typed("Client", "lpwstr", "Acme {Ltd}");
    d.set_custom_prop_typed("Pages", "i4", "42");
    d.set_custom_prop_typed("Ratio", "r8", "0.25");
    d.set_custom_prop_typed("Checked", "bool", "true");
    d.set_custom_prop_typed("Off", "bool", "false");
    d.set_custom_prop_typed("Due", "filetime", "2024-03-05T09:15:00Z");
    d.custom_props.push(CustomProp {
        name: "Total".into(),
        kind: "lpwstr".into(),
        value: "9".into(),
        link_target: Some("Sum1".into()),
        ..Default::default()
    });
    let out = export(&d);
    assert!(out.contains(r"\proptype11{\staticval 1}"), "{out}");
    assert!(out.contains(r"\proptype3{\staticval 42}"), "{out}");
    assert!(out.contains(r"\proptype64"), "{out}");
    assert_eq!(props(&round_trip(&d)), props(&d));
}

#[test]
fn raw_properties_are_not_written() {
    let mut d = Document::new();
    d.custom_props.push(CustomProp { name: "Blob".into(), kind: "raw".into(), value: "<vt:vector/>".into(), ..Default::default() });
    let out = export(&d);
    assert!(!out.contains("Blob"), "{out}");
    assert!(!out.contains(r"\userprops"), "{out}");
}
