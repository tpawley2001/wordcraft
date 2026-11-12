//! Update Field on a `DOCPROPERTY` field that names one of Word's built-in properties shows the
//! document's real value, as measured in Word: the stored extended properties (Company, Manager,
//! HyperlinkBase), the core dates in local time (CreateTime, LastSavedTime, LastPrinted) and
//! counts of the body (Words, Characters, CharactersWithSpaces, Paragraphs, Pages). A field with
//! no name shows `Error! No property name supplied.`, and names Word's field does not know
//! (Creator, Description, Revision ...) show the unknown-name error.

use std::sync::Arc;

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::para::InlineObject;
use wordcraft_engine::Session;

const UNKNOWN: &str = "Error! Unknown document property name.";
const NO_NAME: &str = "Error! No property name supplied.";

/// A session whose body holds one field per code, each with the stored result `stored`.
fn session_with(codes: &[&str], stored: &str) -> Session {
    let mut s = Session::new(Document::new());
    for code in codes {
        s.run("insert.field", &json!({"instr": code, "result": stored})).unwrap();
        s.run("text.insert", &json!({"text": " "})).unwrap();
    }
    s
}

/// The displayed results of the fields in the body, in order.
fn results(s: &Session) -> Vec<String> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn update(s: &mut Session) -> Vec<String> {
    s.run("references.updateFields", &json!({})).unwrap();
    results(s)
}

/// The body Word measured: `One two three four five.` and `Six seven.`, then one field with an
/// empty stored result right after the last word, so the field adds nothing to any count.
fn counted(code: &str) -> String {
    let mut s = Session::new(Document::new());
    s.run("text.insert", &json!({"text": "One two three four five."})).unwrap();
    s.run("text.newParagraph", &json!({})).unwrap();
    s.run("text.insert", &json!({"text": "Six seven."})).unwrap();
    s.run("insert.field", &json!({"instr": code, "result": ""})).unwrap();
    let got = update(&mut s);
    assert_eq!(got.len(), 1, "{code}");
    got[0].clone()
}

fn keep_app_properties(s: &mut Session, lines: &str) {
    s.doc.passthrough.insert(wordcraft_doc::APP_KEPT.into(), Arc::new(lines.as_bytes().to_vec()));
}

/// `value` (an RFC 3339 instant) in the local time zone under a Word date picture.
fn local(value: &str, picture: &str) -> String {
    let unix = u64::try_from(chrono::DateTime::parse_from_rfc3339(value).unwrap().timestamp()).unwrap();
    wordcraft_engine::cmd::insert::format_date_at(picture, unix, wordcraft_engine::cmd::local_offset_secs(unix))
}

#[test]
fn a_field_with_no_name_shows_words_error() {
    let mut s = session_with(&["DOCPROPERTY", r#"DOCPROPERTY """#, r"DOCPROPERTY \* Upper", r"DOCPROPERTY \* MERGEFORMAT"], "stale");
    assert_eq!(update(&mut s), [NO_NAME; 4]);
}

#[test]
fn names_words_field_does_not_know_show_the_unknown_name_error() {
    let codes = [
        "DOCPROPERTY Creator",
        "DOCPROPERTY Description",
        "DOCPROPERTY LastModifiedBy",
        r#"DOCPROPERTY "Last saved by""#,
        r#"DOCPROPERTY "Last modified by""#,
        "DOCPROPERTY Revision",
        r#"DOCPROPERTY "Revision number""#,
    ];
    let mut s = session_with(&codes, "stale");
    s.doc.core.creator = "Ada".into();
    s.doc.core.description = "Notes".into();
    s.doc.core.last_modified_by = "Grace".into();
    s.doc.core.revision = 42;
    assert_eq!(update(&mut s), [UNKNOWN; 7]);
}

#[test]
fn the_names_word_knows_still_show_the_core_values() {
    let codes = ["DOCPROPERTY Author", "DOCPROPERTY Comments", "DOCPROPERTY LastSavedBy", "DOCPROPERTY RevisionNumber", "DOCPROPERTY revisionnumber"];
    let mut s = session_with(&codes, "stale");
    s.doc.core.creator = "Ada".into();
    s.doc.core.description = "Notes".into();
    s.doc.core.last_modified_by = "Grace".into();
    s.doc.core.revision = 42;
    assert_eq!(update(&mut s), ["Ada", "Notes", "Grace", "42", "42"]);
}

#[test]
fn a_custom_property_named_like_a_dropped_alias_still_wins() {
    let mut s = session_with(&["DOCPROPERTY Revision"], "stale");
    s.run("file.properties", &json!({"custom": {"Revision": "B"}})).unwrap();
    assert_eq!(update(&mut s), ["B"]);
}

#[test]
fn company_manager_and_hyperlink_base_show_the_stored_values() {
    let codes = [
        "DOCPROPERTY Company",
        "DOCPROPERTY company",
        r#"DOCPROPERTY "Manager" \* MERGEFORMAT"#,
        "DOCPROPERTY HyperlinkBase",
        r"DOCPROPERTY Company \* Upper",
    ];
    let mut s = session_with(&codes, "stale");
    keep_app_properties(&mut s, "Company\tAcme Stored\nManager\tMona Manager\nHyperlinkBase\thttps://example.com/base/\n");
    assert_eq!(update(&mut s), ["Acme Stored", "Acme Stored", "Mona Manager", "https://example.com/base/", "ACME STORED"]);
}

#[test]
fn company_manager_and_hyperlink_base_are_empty_when_the_file_has_none() {
    let mut s = session_with(&["DOCPROPERTY Company", "DOCPROPERTY Manager", "DOCPROPERTY HyperlinkBase"], "stale");
    assert_eq!(update(&mut s), ["", "", ""]);
    // Only one of the three stored: the others are still empty.
    let mut s = session_with(&["DOCPROPERTY Company", "DOCPROPERTY Manager"], "stale");
    keep_app_properties(&mut s, "Manager\tMona Manager\n");
    assert_eq!(update(&mut s), ["", "Mona Manager"]);
}

#[test]
fn the_dates_show_in_local_time_with_words_default_picture() {
    let mut s = session_with(&["DOCPROPERTY CreateTime", "DOCPROPERTY LastSavedTime", "DOCPROPERTY LastPrinted", "DOCPROPERTY createtime"], "stale");
    s.doc.core.created = "2002-03-04T05:06:00Z".into();
    s.doc.core.modified = "2003-04-05T06:07:00Z".into();
    s.doc.core.last_printed = "2001-02-03T04:05:00Z".into();
    let picture = "M/d/yyyy h:mm AM/PM";
    let created = local("2002-03-04T05:06:00Z", picture);
    assert_eq!(update(&mut s), [created.clone(), local("2003-04-05T06:07:00Z", picture), local("2001-02-03T04:05:00Z", picture), created.clone()]);
    // The shape Word printed (`3/3/2002 9:06 PM` in UTC-8), whatever zone the test runs in.
    assert!(created.ends_with(" AM") || created.ends_with(" PM"), "{created}");
    assert!(created.contains("/2002 "), "{created}");
}

#[test]
fn a_date_picture_and_a_capitalization_switch_apply_to_the_dates() {
    // Mid-year, midday: the same year and month in every time zone.
    let mut s = session_with(
        &[r#"DOCPROPERTY CreateTime \@ "yyyy-MM""#, r#"DOCPROPERTY LastSavedTime \@ "MMMM yyyy" \* Upper"#, r#"DOCPROPERTY LastPrinted \@ "yyyy""#],
        "stale",
    );
    s.doc.core.created = "2002-06-15T12:00:00Z".into();
    s.doc.core.modified = "2003-06-15T12:00:00Z".into();
    s.doc.core.last_printed = "2001-06-15T12:00:00Z".into();
    assert_eq!(update(&mut s), ["2002-06", "JUNE 2003", "2001"]);
}

#[test]
fn a_missing_date_keeps_the_stored_result_except_last_printed() {
    let mut s = session_with(&["DOCPROPERTY CreateTime", "DOCPROPERTY LastSavedTime", "DOCPROPERTY LastPrinted"], "kept");
    s.doc.core.created = String::new();
    s.doc.core.modified = String::new();
    s.doc.core.last_printed = String::new();
    assert_eq!(update(&mut s), ["kept", "kept", ""]);
}

/// The result Insert › Field gives the field in [`counted`]'s body, before any Update Field.
fn inserted(code: &str) -> String {
    let mut s = Session::new(Document::new());
    s.run("text.insert", &json!({"text": "One two three four five."})).unwrap();
    s.run("text.newParagraph", &json!({})).unwrap();
    s.run("text.insert", &json!({"text": "Six seven."})).unwrap();
    s.run("insert.field", &json!({"instr": code, "result": ""})).unwrap();
    let got = results(&s);
    assert_eq!(got.len(), 1, "{code}");
    got[0].clone()
}

/// Insert › Field shows the count at once: the new field has no result yet, so it adds nothing.
#[test]
fn the_counts_are_taken_from_the_body() {
    assert_eq!(inserted("DOCPROPERTY Words"), "7");
    assert_eq!(inserted("DOCPROPERTY Characters"), "29");
    assert_eq!(inserted("DOCPROPERTY CharactersWithSpaces"), "34");
    assert_eq!(inserted("DOCPROPERTY Paragraphs"), "2");
    assert_eq!(inserted("DOCPROPERTY words"), "7");
    assert_eq!(inserted(r#"DOCPROPERTY "Characters" \* MERGEFORMAT"#), "29");
}

/// Update Field counts the body as shown, the count field's own digits included, as Word does:
/// `29` adds two characters (29 + 2, 34 + 2) and joins the last word.
#[test]
fn update_field_counts_the_count_fields_own_result() {
    assert_eq!(counted("DOCPROPERTY Words"), "7");
    assert_eq!(counted("DOCPROPERTY Characters"), "31");
    assert_eq!(counted("DOCPROPERTY CharactersWithSpaces"), "36");
    assert_eq!(counted("DOCPROPERTY Paragraphs"), "2");
}

#[test]
fn pages_is_the_laid_out_page_count() {
    assert_eq!(counted("DOCPROPERTY Pages"), "1");
}

#[test]
fn number_format_switches_apply_to_the_counts() {
    assert_eq!(counted(r"DOCPROPERTY Pages \* Roman"), "I");
    assert_eq!(counted(r"DOCPROPERTY Words \* Ordinal"), "7th");
    assert_eq!(counted(r"DOCPROPERTY Words \* CardText"), "seven");
}

#[test]
fn a_numeric_picture_applies_to_the_counts() {
    assert_eq!(counted(r#"DOCPROPERTY Words \# "0000""#), "0007");
}
