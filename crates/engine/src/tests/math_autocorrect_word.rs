//! Math AutoCorrect as Microsoft 365 Word (English, US) has it, read from `Application.OMathAutoCorrect` on
//! 2026-11-15: Replace text as you type on, Use Math AutoCorrect rules outside of math regions off, 780 built-in
//! entries (`crates/doc/src/math_autocorrect_entries.tsv`) and 55 recognized functions.

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};

use crate::Session;

/// Word's recognized functions, in Word's order.
const WORD_FUNCTIONS: [&str; 55] = [
    "acos", "acosh", "acot", "acoth", "acsc", "acsch", "arccos", "arccosh", "arccot", "arccoth", "arccsc", "arccsch", "arcsec",
    "arcsech", "arcsin", "arcsinh", "arctan", "arctanh", "arg", "asec", "asech", "asin", "asinh", "atan", "atanh", "cos", "cosh",
    "cot", "coth", "csc", "csch", "def", "deg", "det", "dim", "erf", "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "ln", "log",
    "max", "min", "pr", "sec", "sech", "sin", "sinh", "sup", "tan", "tanh",
];

fn options(s: &mut Session, v: Value) -> Value {
    s.run("tools.autocorrect", &v).unwrap()
}

/// Type `keys` one character at a time into a new equation and return its linear text.
fn in_equation(s: &mut Session, keys: &str) -> String {
    s.run("insert.equation", &json!({})).unwrap();
    for c in keys.chars() {
        s.run("equation.type", &json!({"text": c.to_string()})).unwrap();
    }
    s.run("equation.get", &json!({})).unwrap()["linear"].as_str().unwrap().trim().to_string()
}

fn eq(keys: &str) -> String {
    in_equation(&mut Session::new(Document::new()), keys)
}

/// Type `keys` into the body of `s` one character at a time and return the body text.
fn in_body(s: &mut Session, keys: &str) -> String {
    for c in keys.chars() {
        s.run("text.insert", &json!({"text": c.to_string()})).unwrap();
    }
    s.doc.plain_text(StoryRef::Body)
}

#[test]
fn defaults_are_words() {
    let mut s = Session::new(Document::new());
    let r = options(&mut s, json!({}));
    assert_eq!(r["mathReplaceText"], json!(true));
    assert_eq!(r["mathOutside"], json!(false));
    assert_eq!(r["mathEntryCount"], json!(780));
    assert_eq!(r["functions"], json!(WORD_FUNCTIONS.to_vec()));
}

#[test]
fn the_built_in_table_is_words() {
    let tsv = include_str!("../../../doc/src/math_autocorrect_entries.tsv");
    assert_eq!(tsv.lines().count(), 780);
}

#[test]
fn word_entries_replace_in_equations() {
    for (keys, want) in [
        ("\\alpha ", "\u{3B1}"),
        ("\\doubleR ", "\u{211D}"),
        ("\\Rrightarrow ", "\u{21DB}"),
        ("\\Union ", "\u{22C3}"),
        ("\\degc ", "\u{2103}"),
        ("\\langle ", "\u{2329}"),
        ("\\implies ", "\u{2192}"),
        ("\\doubleA ", "\u{1D538}"),
        ("a-> ", "a\u{2192}"),
        ("x<= ", "x\u{2264}"),
        ("!! ", "\u{203C}"),
    ] {
        assert_eq!(eq(keys), want, "typed {keys:?}");
    }
}

#[test]
fn a_formula_entry_builds_up_like_typed_text() {
    let typed = eq("x=(-b\u{B1}\u{221A}(b^2-4ac))/2a ");
    assert_eq!(eq("\\quadratic "), typed);
}

#[test]
fn replace_text_off_replaces_nothing_in_equations() {
    let mut s = Session::new(Document::new());
    options(&mut s, json!({"mathReplaceText": false}));
    assert_eq!(in_equation(&mut s, "\\alpha "), "\\alpha");
}

#[test]
fn own_entries_add_replace_and_delete() {
    let mut s = Session::new(Document::new());
    let r = options(&mut s, json!({"mathAdd": {"from": "\\RR", "to": "\u{211D}"}}));
    assert_eq!(r["mathEntryCount"], json!(781));
    assert_eq!(in_equation(&mut s, "\\RR "), "\u{211D}");

    let mut s = Session::new(Document::new());
    options(&mut s, json!({"mathAdd": {"from": "\\alpha", "to": "A"}}));
    assert_eq!(in_equation(&mut s, "\\alpha "), "A", "an added entry replaces the built-in one");

    let mut s = Session::new(Document::new());
    let r = options(&mut s, json!({"mathDelete": "\\alpha"}));
    assert_eq!(r["mathEntryCount"], json!(779));
    assert_eq!(in_equation(&mut s, "\\alpha "), "\\alpha");
    assert!(s.run("tools.autocorrect", &json!({"mathDelete": "\\nosuchentry"})).is_err());
}

#[test]
fn recognized_functions_add_and_delete() {
    let mut s = Session::new(Document::new());
    let r = options(&mut s, json!({"addFunction": "foo"}));
    let mut want: Vec<&str> = WORD_FUNCTIONS.to_vec();
    want.push("foo");
    assert_eq!(r["functions"], json!(want));
    // An added name builds the same function as a built-in one does.
    assert_eq!(in_equation(&mut s, "foo x "), eq("sin x ").replace("sin", "foo"));

    let mut s = Session::new(Document::new());
    let r = options(&mut s, json!({"deleteFunction": "sin"}));
    assert!(!r["functions"].as_array().unwrap().contains(&json!("sin")));
    assert_ne!(in_equation(&mut s, "sin x "), eq("sin x "), "a deleted name is no longer a function");
}

#[test]
fn outside_math_regions() {
    let mut s = Session::new(Document::new());
    assert_eq!(in_body(&mut s, "so \\alpha "), "So \\alpha ", "off by default");

    let mut s = Session::new(Document::new());
    options(&mut s, json!({"mathOutside": true}));
    assert_eq!(in_body(&mut s, "so \\alpha "), "So \u{3B1} ");

    // Ordinary AutoCorrect wins where both have the entry: "->" is an ordinary AutoCorrect entry too.
    let mut s = Session::new(Document::new());
    let ordinary = in_body(&mut s, "so a -> b ");
    let mut s = Session::new(Document::new());
    options(&mut s, json!({"mathOutside": true}));
    assert_eq!(in_body(&mut s, "so a -> b "), ordinary);
}
