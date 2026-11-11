//! Edge cases of the information group's dates and version and of `{\*\userprops …}`: `\uN`
//! escapes, dates kept apart, a huge `\version`, and a group's text staying out of the body.

use super::*;
use wordcraft_doc::{Document, StoryRef};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

#[test]
fn unicode_escapes_in_user_properties_decode() {
    // The escapes are built from parts so no tool reads them as its own escapes.
    let value = format!(r"\u{}?\u{}?", 20013, 25991);
    let d = doc(&format!(r"{{\rtf1\ansi\uc1{{\*\userprops {{\propname Caf\u233?}}\proptype30{{\staticval {value}}}}}\pard Body\par}}"));
    assert_eq!(d.custom_props.len(), 1);
    assert_eq!(d.custom_prop("café"), Some("中文"));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
}

#[test]
fn each_date_keeps_its_own_words() {
    // `\revtim` after `\printim` (and `\sec`) never leaks into `\creatim`.
    let d = doc(
        r"{\rtf1\ansi{\info{\printim\yr2023\mo2\dy3\hr4\min5\sec6}{\creatim\yr2020\mo1\dy2}{\revtim\yr2024\mo3\dy5}{\buptim\yr1999}}\pard x\par}",
    );
    assert_eq!(d.core.created, "2020-01-02T00:00:00Z");
    assert_eq!(d.core.modified, "2024-03-05T00:00:00Z");
    assert_eq!(d.core.last_printed, "2023-02-03T04:05:00Z");
}

#[test]
fn huge_version_does_not_wrap_negative() {
    let d = doc(r"{\rtf1\ansi{\info{\version99999999999}}\pard x\par}");
    assert!(d.core.revision <= i32::MAX as u32);
}

#[test]
fn user_properties_outside_their_group_are_body_words() {
    // `\propname` outside `\userprops` is not a property.
    let d = doc(r"{\rtf1\ansi\pard {\*\propname Lost}Body\par}");
    assert!(d.custom_props.is_empty());
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
}

#[test]
fn a_property_value_cut_at_the_limit_keeps_whole_characters() {
    let long = "é".repeat(40_000);
    let d = doc(&format!(r"{{\rtf1\ansi{{\*\userprops {{\propname L}}\proptype30{{\staticval {long}}}}}\pard x\par}}"));
    let v = d.custom_prop("L").unwrap_or_default();
    assert!(!v.is_empty() && v.len() <= 64 * 1024);
}
