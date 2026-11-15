//! `\* format` switches on formula fields give Word's results: non-whole numbers round before the
//! format, a number a format can't write shows Word's error text, 0 in roman/alphabetic is a space.
//! Expected results measured in Microsoft 365 Word (Fields.Add, Update, Field.Result.Text), 2026-11-14.

const ERR: &str = "Error! Number cannot be represented in specified format.";

fn eval(code: &str) -> String {
    let doc = wordcraft_doc::Document::new();
    crate::cmd::table_formula::evaluate_text_field(&doc, None, code).unwrap_or_else(|e| panic!("{code}: {e}"))
}

fn check(rows: &[(&str, &str)]) {
    let wrong: Vec<String> = rows
        .iter()
        .filter_map(|(code, word)| {
            let ours = eval(code);
            (ours != *word).then(|| format!("{code}: Word {word:?}, ours {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn roman_and_alphabetic_round_and_show_a_space_for_zero() {
    check(&[
        (r"= 0 \* roman", " "),
        (r"= 4000 \* roman", "mmmm"),
        (r"= 3999 \* ROMAN", "MMMCMXCIX"),
        (r"= 12.7 \* roman", "xiii"),
        (r"= -4 \* roman", ERR),
        (r"= 27 \* ALPHABETIC", "AA"),
        (r"= 0 \* alphabetic", " "),
        (r"= 703 \* alphabetic", "aaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        (r"= 780 \* alphabetic", "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"),
        (r"= 12.7 \* alphabetic", "m"),
    ]);
}

#[test]
fn card_ord_and_dollar_text_round_and_stop_at_a_million() {
    check(&[
        (r"= 0 \* CardText", "zero"),
        (r"= -5 \* CardText", ERR),
        (r"= 1000000 \* CardText", ERR),
        (r"= 999999 \* CardText", "nine hundred ninety-nine thousand nine hundred ninety-nine"),
        (r"= 1001 \* CardText", "one thousand one"),
        (r"= 3.5 \* CardText", "four"),
        (r"= 115 \* CardText", "one hundred fifteen"),
        (r"= 12.5 \* DollarText", "twelve and 50/100"),
        (r"= 0.07 \* DollarText", "zero and 07/100"),
        (r"= 1.999 \* DollarText", "one and 00/100"),
        (r"= -12.5 \* DollarText", ERR),
        (r"= 1000000 \* DollarText", ERR),
        (r"= 21 \* OrdText", "twenty-first"),
        (r"= 100 \* OrdText", "one hundredth"),
        (r"= 0 \* OrdText", "zeroth"),
        (r"= 1000000 \* OrdText", ERR),
        (r"= 12.7 \* OrdText", "thirteenth"),
        (r"= 12.5 \* DollarText \* Upper", "TWELVE AND 50/100"),
        (r"= 21 \* OrdText \* Caps", "Twenty-First"),
    ]);
}

#[test]
fn ordinal_hex_and_arabic_round_and_keep_the_sign() {
    check(&[
        (r"= 0 \* Ordinal", "0th"),
        (r"= 11 \* Ordinal", "11th"),
        (r"= 112 \* Ordinal", "112th"),
        (r"= -3 \* Ordinal", "-3rd"),
        (r"= 12.7 \* Ordinal", "13th"),
        (r"= 1001 \* Ordinal", "1001st"),
        (r"= 255 \* Hex", "FF"),
        (r"= -1 \* Hex", ERR),
        (r"= 12.7 \* Hex", "D"),
        (r"= 0 \* Hex", "0"),
        (r"= 5 \* ArabicDash", "- 5 -"),
        (r"= 2.5 \* Arabic", "3"),
    ]);
}

#[test]
fn case_switches_after_the_number_format() {
    check(&[
        (r"= 7 \* roman \* Upper", "VII"),
    ]);
}
