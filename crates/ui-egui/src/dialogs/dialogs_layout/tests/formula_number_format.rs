//! The table Formula dialog's Number format box (#1201): Word's seven formats, free text, and
//! the `\#` switch the chosen picture adds to the field.

use serde_json::json;

use super::{FORMULA_NUMBER_FORMATS, FormulaForm, LayoutDialog, fields, form, table_app};

/// A form over a two-column table whose last cell has 3 and 4 above it.
fn formula_form() -> (crate::WordApp, FormulaForm) {
    let mut a = table_app(2, &["Item", "Cost", "Tea", "3", "Jam", "4", "Total", ""]);
    a.run("table.formula", json!({})).unwrap();
    let LayoutDialog::Formula(f) = form(&mut a) else { panic!("formula") };
    (a, f)
}

/// What OK sends for a formula and a Number format box text.
fn sent(formula: &str, number_format: &str) -> String {
    let (_, mut f) = formula_form();
    f.formula = formula.into();
    f.number_format = number_format.into();
    f.params()["formula"].as_str().expect("a formula").to_string()
}

/// The list holds Word's seven formats in Word's order.
#[test]
fn number_format_lists_words_seven_formats() {
    assert_eq!(FORMULA_NUMBER_FORMATS, ["#,##0", "#,##0.00", "$#,##0.00;($#,##0.00)", "0", "0%", "0.00", "0.00%"]);
}

/// The box opens empty, and an empty or blank box adds no switch.
#[test]
fn number_format_opens_empty_and_adds_nothing() {
    let (_, f) = formula_form();
    assert_eq!(f.number_format, "");
    assert_eq!(f.params(), json!({"formula": "=SUM(ABOVE)"}));
    assert_eq!(serde_json::to_value(&f).unwrap()["numberFormat"], json!(""));
    assert_eq!(sent("=SUM(ABOVE)", "   "), "=SUM(ABOVE)");
    assert_eq!(sent("=SUM(ABOVE)", "\"\""), "=SUM(ABOVE)", "only quotes: no picture");
}

/// Picking from the list replaces the box's text.
#[test]
fn picking_a_number_format_fills_the_box() {
    let (_, mut f) = formula_form();
    f.pick_number_format(FORMULA_NUMBER_FORMATS[1]);
    assert_eq!(f.number_format, "#,##0.00");
    f.pick_number_format(FORMULA_NUMBER_FORMATS[5]);
    assert_eq!(f.number_format, "0.00", "the pick replaces the text, it is not added to it");
    assert_eq!(f.formula, "=SUM(ABOVE)", "the formula is left alone");
}

/// A picture goes after the formula as a quoted `\#` switch.
#[test]
fn number_format_adds_the_picture_switch() {
    assert_eq!(sent("=SUM(ABOVE)", "#,##0.00"), "=SUM(ABOVE) \\# \"#,##0.00\"");
    assert_eq!(sent(" =SUM(ABOVE) ", "  0.00  "), "=SUM(ABOVE) \\# \"0.00\"");
    assert_eq!(sent("=SUM(ABOVE)", "$#,##0.00;($#,##0.00)"), "=SUM(ABOVE) \\# \"$#,##0.00;($#,##0.00)\"");
    assert_eq!(sent("=A1*2", "0.00 'each'"), "=A1*2 \\# \"0.00 'each'\"", "free text is allowed");
    for picture in FORMULA_NUMBER_FORMATS {
        assert_eq!(sent("=B2+B3", picture), format!("=B2+B3 \\# \"{picture}\""));
    }
}

/// Double quotes typed in the box are dropped, so the switch stays one quoted picture.
#[test]
fn number_format_drops_typed_double_quotes() {
    assert_eq!(sent("=SUM(ABOVE)", "\"0.00\""), "=SUM(ABOVE) \\# \"0.00\"");
    assert_eq!(sent("=SUM(ABOVE)", "0\".\"00"), "=SUM(ABOVE) \\# \"0.00\"");
}

/// A formula typed with its own `\#` switch is sent as typed.
#[test]
fn a_typed_picture_switch_wins_over_the_box() {
    assert_eq!(sent("=SUM(ABOVE) \\# \"0\"", "0.00"), "=SUM(ABOVE) \\# \"0\"");
}

/// OK inserts the field with the switch in its code and the formatted result (Word: 7.00).
#[test]
fn ok_inserts_the_field_with_its_number_format() {
    let (mut a, mut f) = formula_form();
    f.pick_number_format("0.00");
    a.run("table.formula", f.params()).unwrap();
    assert_eq!(fields(&a), [("=SUM(ABOVE) \\# \"0.00\"".to_string(), "7.00".to_string())]);
    assert!(a.dialog.is_none());
}
