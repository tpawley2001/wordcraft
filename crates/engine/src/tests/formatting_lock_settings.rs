//! Restrict Editing › Formatting restrictions › Settings, as in Word's Formatting Restrictions
//! dialog: "Block Theme or Scheme switching" (`w:styleLockTheme`) and "Block Quick Style Set
//! switching" (`w:styleLockQFSet`) refuse those Design commands while formatting is limited and
//! the protection is enforced; "Allow AutoFormat to override formatting restrictions"
//! (`w:autoFormatOverride`) is kept but, as in Word, changes nothing. All three survive DOCX and
//! RTF saves.
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{CmdError, Session};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::from_text("Some text."))
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn disabled(s: &mut Session, id: &str, v: Value) -> String {
    match s.run(id, &v) {
        Err(CmdError::Disabled(m)) => m,
        other => panic!("{id} should be refused as disabled, got {other:?}"),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-fmtlock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn flags(state: &Value) -> (Value, Value, Value) {
    (state["blockTheme"].clone(), state["blockStyleSet"].clone(), state["autoFormatOverride"].clone())
}

#[test]
fn the_three_settings_are_off_by_default_and_reported() {
    let mut s = s();
    let st = run(&mut s, "review.restrict", json!({}));
    assert_eq!(flags(&st), (json!(false), json!(false), json!(false)));
    let st = run(&mut s, "review.restrict", json!({"blockTheme": true, "blockStyleSet": true, "autoFormatOverride": true}));
    assert_eq!(flags(&st), (json!(true), json!(true), json!(true)));
    let st = run(&mut s, "review.restrict", json!({"blockStyleSet": false}));
    assert_eq!(flags(&st), (json!(true), json!(false), json!(true)));
}

#[test]
fn block_theme_refuses_theme_colours_and_fonts_while_formatting_is_limited() {
    let mut s = s();
    run(&mut s, "review.restrict", json!({"mode": "none", "formatting": true, "blockTheme": true}));
    for (id, v) in [
        ("design.theme", json!({"name": "Craft"})),
        ("design.themeColors", json!({"name": "Craft"})),
        ("design.themeFonts", json!({"heading": "Georgia", "body": "Georgia"})),
    ] {
        let m = disabled(&mut s, id, v);
        assert!(m.contains("theme switching is blocked"), "{id}: {m}");
    }
    // The style set isn't blocked by this one.
    run(&mut s, "design.styleSet", json!({"name": "basic"}));
    // Stopped: the theme can change again.
    run(&mut s, "review.restrict", json!({"stop": true}));
    run(&mut s, "design.theme", json!({"name": "Craft"}));
}

#[test]
fn block_style_set_refuses_only_the_style_set() {
    let mut s = s();
    run(&mut s, "review.restrict", json!({"blockStyleSet": true}));
    run(&mut s, "review.restrict", json!({"mode": "none", "formatting": true}));
    let m = disabled(&mut s, "design.styleSet", json!({"name": "basic"}));
    assert!(m.contains("style set switching is blocked"), "{m}");
    run(&mut s, "design.theme", json!({"name": "Craft"}));
}

#[test]
fn the_blocks_do_nothing_until_formatting_is_limited_and_enforced() {
    let mut s = s();
    run(&mut s, "review.restrict", json!({"blockTheme": true, "blockStyleSet": true}));
    run(&mut s, "design.theme", json!({"name": "Craft"}));
    run(&mut s, "design.styleSet", json!({"name": "basic"}));
}

#[test]
fn the_settings_cant_change_while_protection_is_enforced() {
    let mut s = s();
    run(&mut s, "review.restrict", json!({"mode": "none", "formatting": true}));
    let m = disabled(&mut s, "review.restrict", json!({"blockTheme": true}));
    assert!(m.contains("stop protection"), "{m}");
    let st = run(&mut s, "review.restrict", json!({}));
    assert_eq!(st["blockTheme"], json!(false));
}

fn round_trip(ext: &str) {
    let dir = temp_dir(ext);
    let path = dir.join(format!("locked.{ext}")).to_string_lossy().into_owned();
    let mut a = s();
    run(&mut a, "review.restrict", json!({"blockTheme": true, "blockStyleSet": true, "autoFormatOverride": true}));
    run(&mut a, "review.restrict", json!({"mode": "none", "formatting": true}));
    run(&mut a, "file.save", json!({"path": path}));
    let mut b = s();
    run(&mut b, "file.open", json!({"path": path}));
    let st = run(&mut b, "review.restrict", json!({}));
    assert_eq!(st["formatting"], json!(true), "{ext}");
    assert_eq!(flags(&st), (json!(true), json!(true), json!(true)), "{ext}");
    let m = disabled(&mut b, "design.theme", json!({"name": "Craft"}));
    assert!(m.contains("theme switching is blocked"), "{ext}: {m}");
    // Off settings are not written: a plain document reopens with all three off.
    let plain = dir.join(format!("plain.{ext}")).to_string_lossy().into_owned();
    let mut c = s();
    run(&mut c, "file.save", json!({"path": plain}));
    let mut d = s();
    run(&mut d, "file.open", json!({"path": plain}));
    let st = run(&mut d, "review.restrict", json!({}));
    assert_eq!(flags(&st), (json!(false), json!(false), json!(false)), "{ext}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn docx_keeps_the_settings() {
    round_trip("docx");
}

#[test]
fn rtf_keeps_the_settings() {
    round_trip("rtf");
}
