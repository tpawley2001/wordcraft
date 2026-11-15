//! GREETINGLINE and ADDRESSBLOCK follow their switches during a merge (ECMA-376 Part 1 §17.16.5.24 and
//! §17.16.5.1). Expected values are what a mail merge in Microsoft 365 Word 16 produced for the same
//! field codes and records (postal codes left out of the data on purpose).

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;

const HEAD: &str = "Title,First Name,Last Name,Suffix,Company,Address 1,Address 2,City,State,Country";
const R1: &str = "Mr,John,Smith,Jr,Acme Pty Ltd,1 Main St,Suite 2,Sydney,NSW,Australia";
const R2: &str = "Ms,Jane,Doe,,,2 High St,,Auckland,,New Zealand";
const R3: &str = ",Pat,Lee,,Beta Inc,3 Low Rd,,Perth,WA,Australia";
const R4: &str = ",Sam,,,,4 Side Ln,,Hobart,TAS,Australia";
const R5: &str = ",,,,Gamma Ltd,5 Far Rd,,Darwin,NT,Australia";

const G_TITLE_LAST: &str = r#"GREETINGLINE \f "<<_BEFORE_ Dear >><<_TITLE0_ >><<_LAST0_>><<_AFTER_ ,>>" \l 1033 \e "Dear Sir or Madam,""#;
const G_FIRST: &str = r#"GREETINGLINE \f "<<_BEFORE_ Dear >><<_FIRST0_>><<_AFTER_ ,>>" \l 1033 \e "Dear friend,""#;
const G_NOT_A_FORMAT: &str =
    r#"GREETINGLINE \f "<<_BEFORE_ To >><<_TITLE0_ >><<_FIRST0_>><< _LAST0_>><<_AFTER_ :>>" \l 1033 \e "To whom it may concern:""#;
const A_FULL: &str = r#"\f "<<_TITLE0_ >><<_FIRST0_>><< _LAST0_>><< _SUFFIX0_>>\n<<_COMPANY_\n>><<_STREET1_\n>><<_STREET2_\n>><<_CITY_>><<, _STATE_>><< _POSTAL_>><<\n_COUNTRY_>>" \l 1033"#;

fn run(s: &mut Session, id: &str, params: Value) {
    s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"));
}

/// Merges one record into a directory with `insert` building the document; the body text with line
/// breaks and paragraph marks as `\n`, trailing ones dropped.
fn merge_with(record: &str, insert: impl FnOnce(&mut Session)) -> String {
    let mut s = Session::new(Document::new());
    run(&mut s, "mailings.recipients", json!({"csv": format!("{HEAD}\n{record}")}));
    run(&mut s, "mailings.start", json!({"kind": "directory"}));
    insert(&mut s);
    run(&mut s, "mailings.finish", json!({}));
    let t = s.doc.plain_text(StoryRef::Body).replace(['\u{b}', '\r', '\u{2028}'], "\n");
    t.trim_end_matches('\n').to_string()
}

fn merge(record: &str, instr: &str) -> String {
    merge_with(record, |s| run(s, "insert.field", json!({"instr": instr, "result": ""})))
}

/// The result with its line breaks taken out (where Word broke the lines doesn't matter for these cases).
fn flat(t: &str) -> String {
    t.replace('\n', "")
}

#[test]
fn greeting_title_last_uses_title_and_last_name() {
    assert_eq!(merge(R1, G_TITLE_LAST), "Dear Mr Smith,");
    assert_eq!(merge(R2, G_TITLE_LAST), "Dear Ms Doe,");
}

#[test]
fn greeting_title_last_without_a_title_uses_first_and_last_name() {
    assert_eq!(merge(R3, G_TITLE_LAST), "Dear Pat Lee,");
    assert_eq!(merge(R4, G_TITLE_LAST), "Dear Sam,");
}

#[test]
fn greeting_without_any_name_is_the_alternate_text() {
    assert_eq!(merge(R5, G_TITLE_LAST), "Dear Sir or Madam,");
    assert_eq!(merge(R5, G_FIRST), "Dear friend,");
}

#[test]
fn greeting_first_name_format() {
    assert_eq!(merge(R1, G_FIRST), "Dear John,");
    assert_eq!(merge(R2, G_FIRST), "Dear Jane,");
    assert_eq!(merge(R4, G_FIRST), "Dear Sam,");
}

#[test]
fn greeting_format_that_is_not_one_of_words_is_always_the_alternate_text() {
    for r in [R1, R2, R3, R4] {
        assert_eq!(merge(r, G_NOT_A_FORMAT), "To whom it may concern:", "{r}");
    }
}

#[test]
fn greeting_without_a_format_switch_is_empty() {
    assert_eq!(merge(R1, "GREETINGLINE"), "");
}

#[test]
fn greeting_line_command_inserts_words_default_switches() {
    let t = merge_with(R1, |s| run(s, "mailings.greetingLine", json!({})));
    assert_eq!(t, "Dear Mr Smith,");
    let t = merge_with(R5, |s| run(s, "mailings.greetingLine", json!({})));
    assert_eq!(t, "Dear Sir or Madam,");
    let t = merge_with(R2, |s| run(s, "mailings.greetingLine", json!({"format": "<<_BEFORE_ Hello >><<_FIRST0_>><<_AFTER_ !>>", "alt": "Hello!"})));
    assert_eq!(t, "Hello Jane!");
}

#[test]
fn address_backslash_n_in_a_format_is_the_letter_n() {
    let f = r#"ADDRESSBLOCK \f "<<_FIRST0_>><< _LAST0_>>\n<<_STREET1_\n>><<_CITY_>>" \l 1033"#;
    assert_eq!(merge(R1, f), "John Smithn1 Main StnSydney");
    assert_eq!(merge(R2, f), "Jane Doen2 High StnAuckland");
    assert_eq!(merge(R4, f), "Samn4 Side LnnHobart");
}

#[test]
fn address_groups_drop_with_their_empty_token() {
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 1 \e "Australia""#);
    assert_eq!(flat(&merge(R1, &f)), "Mr John Smith JrnAcme Pty Ltdn1 Main StnSuite 2nSydney, NSWnAustralia");
    assert_eq!(flat(&merge(R2, &f)), "Ms Jane Doen2 High StnAucklandnNew Zealand");
    assert_eq!(flat(&merge(R3, &f)), "Pat LeenBeta Incn3 Low RdnPerth, WAnAustralia");
    assert_eq!(flat(&merge(R4, &f)), "Samn4 Side LnnHobart, TASnAustralia");
}

#[test]
fn address_country_switch_zero_keeps_a_country_the_format_names() {
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 0 \e "Australia""#);
    assert_eq!(flat(&merge(R3, &f)), "Pat LeenBeta Incn3 Low RdnPerth, WAnAustralia");
}

#[test]
fn address_country_switch_two_drops_the_excluded_country() {
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 2 \e "Australia" \d"#);
    assert_eq!(flat(&merge(R1, &f)), "Mr John Smith JrnAcme Pty Ltdn1 Main StnSuite 2nSydney, NSW");
    assert_eq!(flat(&merge(R3, &f)), "Pat LeenBeta Incn3 Low RdnPerth, WA");
    assert_eq!(flat(&merge(R4, &f)), "Samn4 Side LnnHobart, TAS");
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 2 \e "australia""#);
    assert_eq!(flat(&merge(R4, &f)), "Samn4 Side LnnHobart, TAS");
}

#[test]
fn address_only_the_rightmost_excluded_country_counts() {
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 2 \e "New Zealand" \e "Australia""#);
    assert_eq!(flat(&merge(R1, &f)), "Mr John Smith JrnAcme Pty Ltdn1 Main StnSuite 2nSydney, NSW");
    let f = format!(r#"ADDRESSBLOCK {A_FULL} \c 2 \e "Australia" \e "New Zealand""#);
    assert_eq!(flat(&merge(R1, &f)), "Mr John Smith JrnAcme Pty Ltdn1 Main StnSuite 2nSydney, NSWnAustralia");
}

#[test]
fn address_block_command_writes_its_switches() {
    let t = merge_with(R4, |s| {
        run(s, "mailings.addressBlock", json!({"format": "<<_FIRST0_>><<, _CITY_>><< _COUNTRY_>>", "country": 2, "exclude": "Australia"}))
    });
    assert_eq!(t, "Sam, Hobart");
    let t = merge_with(R2, |s| {
        run(s, "mailings.addressBlock", json!({"format": "<<_FIRST0_>><<, _CITY_>><< _COUNTRY_>>", "country": 2, "exclude": "Australia"}))
    });
    assert_eq!(t, "Jane, Auckland New Zealand");
}

#[test]
fn short_placeholders_read_the_same_columns() {
    let f = r#"GREETINGLINE \f "<</g Dear >><</t0 >><</l0>><</x ,>>" \e "Dear Sir or Madam,""#;
    assert_eq!(merge(R1, f), "Dear Mr Smith,");
}
