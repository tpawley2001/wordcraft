//! Formula fields show their result as Word does: as many decimals as the numbers written in the
//! formula (two when none has decimals), halves rounded away from zero, trailing zeros dropped; INT
//! and MOD truncate toward zero. Expected results measured in Microsoft 365 Word 16.0.20527
//! (Fields.Add, Update, Field.Result.Text), 2026-11-14.

fn eval(code: &str) -> String {
    let doc = wordcraft_doc::Document::new();
    crate::cmd::table_formula::evaluate_text_field(&doc, None, code).unwrap_or_else(|e| panic!("{code}: {e}"))
}

fn check(rows: &[(&str, &str)]) {
    let wrong: Vec<String> = rows
        .iter()
        .filter_map(|(code, word)| {
            let ours = eval(code);
            (ours != *word).then(|| format!("{code}: Word {word}, ours {ours}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn decimals_follow_the_numbers_written_and_halves_round_away_from_zero() {
    check(&[
        ("= 1/8", "0.13"),
        ("= -1/8", "-0.13"),
        ("= 5/1000", "0.01"),
        ("= 2/3", "0.67"),
        ("= 10/4", "2.5"),
        ("= AVERAGE(1,2,4)", "2.33"),
        ("= 1/400", "0.0"),
        ("= 0.625*1", "0.625"),
        ("= 0.005*1", "0.005"),
        ("= 1.005*1", "1.005"),
        ("= 2.675*1", "2.675"),
        ("= 0.015*1", "0.015"),
        ("= 1.41421356*1", "1.41421356"),
        ("= 2^0.5", "1.4"),
        ("= 3^0.5", "1.7"),
        ("= 2^1.5", "2.8"),
        ("= 2^0.25", "1.19"),
        ("= 10^-1", "0.1"),
        ("= 2^-1", "0.5"),
        ("= ROUND(2.345,2)", "2.35"),
        ("= ABS(-3.5)", "3.5"),
        ("= MOD(7.5,2)", "1.5"),
        ("= 1/3*3", "1"),
    ]);
}

#[test]
fn int_and_mod_truncate_toward_zero() {
    check(&[
        ("= INT(2.7)", "2"),
        ("= INT(-2.5)", "-2"),
        ("= INT(-0.5)", "0"),
        ("= INT(-3)", "-3"),
        ("= MOD(-7,3)", "-1"),
        ("= MOD(7,-3)", "1"),
        ("= MOD(-7,-3)", "-1"),
        ("= MOD(-7.5,2)", "-1.5"),
        ("= MOD(7,3)", "1"),
    ]);
}
