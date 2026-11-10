//! File › Print › Document Info › Key Assignments: `print::document_info(s, "keys")` lists the
//! keyboard customizations (commands and styles), and a print job's `what: "keys"` prints them.

use serde_json::{Value, json};

use crate::Session;
use crate::print::{DOCUMENT_INFO, document_info};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "tools.customizeKeyboard", json!({"reset": true}));
    s
}

fn keys(s: &Session) -> Vec<String> {
    document_info(s, "keys").unwrap_or_else(|e| panic!("keys: {e}"))
}

#[test]
fn no_customizations_is_an_empty_report_that_does_not_print() {
    let s = session();
    assert_eq!(keys(&s), Vec::<String>::new());
    assert!(s.print_pdf(&json!({"what": "keys"})).is_err(), "nothing to print");
}

#[test]
fn commands_and_styles_are_listed_by_name_with_their_key() {
    let mut s = session();
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"command": "format.italic", "key": "Alt+Shift+F11"}}));
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"style": "Heading 2", "key": "Alt+Shift+F12"}}));
    assert_eq!(keys(&s), vec!["Heading 2\tAlt+Shift+F12".to_string(), "Italic\tAlt+Shift+F11".to_string()]);
}

#[test]
fn a_command_with_two_keys_has_a_line_per_key_in_key_order() {
    let mut s = session();
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"command": "format.bold", "key": "Alt+Shift+F9"}}));
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"command": "format.bold", "key": "Alt+Shift+F10"}}));
    assert_eq!(keys(&s), vec!["Bold\tAlt+Shift+F10".to_string(), "Bold\tAlt+Shift+F9".to_string()]);
}

#[test]
fn reset_leaves_nothing_to_list() {
    let mut s = session();
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"command": "format.italic", "key": "Alt+Shift+F11"}}));
    run(&mut s, "tools.customizeKeyboard", json!({"reset": true}));
    assert_eq!(keys(&s), Vec::<String>::new());
}

#[test]
fn key_assignments_is_a_document_info_report_that_prints() {
    assert!(DOCUMENT_INFO.contains(&"keys"), "{DOCUMENT_INFO:?}");
    let mut s = session();
    run(&mut s, "tools.customizeKeyboard", json!({"assign": {"command": "format.italic", "key": "Alt+Shift+F11"}}));
    let (pdf, plan) = s.print_pdf(&json!({"what": "keys"})).expect("key assignments print");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(plan.chosen, 1);
}
