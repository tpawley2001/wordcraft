//! View › Reveal Formatting pane: `view.revealFormattingPane` shows or hides the pane that lists
//! `format.reveal`'s groups, and Shift+F1 opens it, as in Word (the Style Inspector keeps its
//! command and pane button but no key).

use serde_json::json;
use wordcraft_doc::Document;

use crate::{KeyMap, Session, cmd::registry};

#[test]
fn the_pane_command_toggles_and_returns_the_reveal_groups() {
    let mut s = Session::new(Document::from_text("Body text"));
    let r = s.run("view.revealFormattingPane", &json!({})).unwrap();
    assert_eq!(r["pane"], true);
    assert_eq!(r["groups"], s.run("format.reveal", &json!({})).unwrap()["groups"]);
    let r = s.run("view.revealFormattingPane", &json!({})).unwrap();
    assert_eq!(r["pane"], false, "a second run hides it");
    let r = s.run("view.revealFormattingPane", &json!({"value": true})).unwrap();
    assert_eq!(r["pane"], true);
    let r = s.run("view.revealFormattingPane", &json!({"value": true})).unwrap();
    assert_eq!(r["pane"], true, "value sets, it doesn't toggle");
    let r = s.run("view.revealFormattingPane", &json!({"value": false})).unwrap();
    assert_eq!(r["pane"], false);
}

#[test]
fn shift_f1_opens_reveal_formatting_as_in_word() {
    let reg = registry();
    let map = KeyMap::default();
    assert_eq!(map.resolve(&reg, "Shift+F1").map(|c| c.id), Some("view.revealFormattingPane"));
    assert!(map.keys_for(&reg, "styles.inspector").is_empty(), "Style Inspector has no key in Word");
    assert!(reg.get("styles.inspector").is_some(), "the Style Inspector command stays");
}
