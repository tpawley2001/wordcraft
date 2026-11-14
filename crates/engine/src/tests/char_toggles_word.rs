//! Underline shortcuts and exclusive character effects as in Word: Ctrl+U on any other underline
//! style makes it single, Ctrl+Shift+D / Ctrl+Shift+W toggle their own style off again, and All
//! Caps/Small Caps, Strikethrough/Double strikethrough and Emboss/Engrave/Outline/Shadow switch
//! each other off. Measured in Microsoft 365 Word 16.0.20527 (Windows, en-US) through
//! `Application.FindKey(..).Execute()` and the Font object, 2026-11-14.

use serde_json::json;
use wordcraft_doc::props::{CharProps, Underline};
use wordcraft_doc::{Document, Pos};

use crate::Session;

fn session(set: &dyn Fn(&mut CharProps)) -> Session {
    let mut doc = Document::from_text("abcde fghij");
    let end = doc.para_at(&Pos::body(0, 0)).unwrap().len();
    doc.format_range(&Pos::body(0, 0), &Pos::body(0, end), set).unwrap();
    let mut s = Session::new(doc);
    s.run("select.all", &json!({})).unwrap();
    s
}

fn resolved(s: &Session) -> Vec<wordcraft_doc::resolve::ResolvedChar> {
    let p = s.doc.para_at(&Pos::body(0, 0)).unwrap();
    (0..p.len()).map(|i| s.doc.styles.resolve_char(p.props.style.as_deref(), p.props_of_char(i))).collect()
}

fn underline_of(s: &Session) -> Underline {
    let all = resolved(s);
    let u = all[0].underline;
    assert!(all.iter().all(|r| r.underline == u), "the whole paragraph should have one underline: {all:?}");
    u
}

/// (start, Ctrl+U, Ctrl+U again, Ctrl+Shift+D, again, Ctrl+Shift+W, again) as Word gives them.
const UNDERLINE: &[[Underline; 7]] = {
    use Underline::*;
    &[
        [None, Single, None, Double, None, Words, None],
        [Single, None, Single, Double, None, Words, None],
        [Double, Single, None, None, Double, Words, None],
        [Words, Single, None, Double, None, None, Words],
        [Dotted, Single, None, Double, None, Words, None],
        [Wave, Single, None, Double, None, Words, None],
    ]
};

#[test]
fn underline_shortcuts_as_word() {
    for row in UNDERLINE {
        let start = row[0];
        for (i, command) in ["format.underline", "format.doubleUnderline", "format.wordUnderline"].iter().enumerate() {
            let mut s = session(&|c| c.underline = Some(start));
            s.run(command, &json!({})).unwrap();
            assert_eq!(underline_of(&s), row[1 + 2 * i], "{command} on {start:?}");
            s.run("select.all", &json!({})).unwrap();
            s.run(command, &json!({})).unwrap();
            assert_eq!(underline_of(&s), row[2 + 2 * i], "{command} twice on {start:?}");
        }
    }
}

#[test]
fn underline_gallery_style_still_sets() {
    // The Underline gallery passes a style: it sets that style, it does not toggle.
    let mut s = session(&|c| c.underline = Some(Underline::Dotted));
    s.run("format.underline", &json!({ "style": "dotted" })).unwrap();
    assert_eq!(underline_of(&s), Underline::Dotted);
    s.run("format.underline", &json!({ "value": false })).unwrap();
    assert_eq!(underline_of(&s), Underline::None);
}

/// The effects compared, in this order.
const NAMES: [&str; 8] = ["caps", "smallCaps", "strike", "doubleStrike", "emboss", "engrave", "outline", "shadow"];

fn effects(s: &Session) -> [bool; 8] {
    let r = &resolved(s)[0];
    [r.caps, r.small_caps, r.strike, r.double_strike, r.emboss, r.engrave, r.outline, r.shadow]
}

fn set_effect(c: &mut CharProps, name: &str) {
    match name {
        "caps" => c.caps = Some(true),
        "smallCaps" => c.small_caps = Some(true),
        "strike" => c.strike = Some(true),
        "doubleStrike" => c.double_strike = Some(true),
        "emboss" => c.emboss = Some(true),
        "engrave" => c.engrave = Some(true),
        "outline" => c.outline = Some(true),
        _ => {
            assert_eq!(name, "shadow");
            c.shadow = Some(true);
        }
    }
}

fn command_for(name: &str) -> &'static str {
    match name {
        "caps" => "format.allCaps",
        "smallCaps" => "format.smallCaps",
        "strike" => "format.strikethrough",
        "doubleStrike" => "format.doubleStrikethrough",
        "emboss" => "format.emboss",
        "engrave" => "format.engrave",
        "outline" => "format.outline",
        _ => {
            assert_eq!(name, "shadow");
            "format.shadow"
        }
    }
}

fn only(names: &[&str]) -> [bool; 8] {
    NAMES.map(|n| names.contains(&n))
}

/// (effect already on, effect toggled, effects on afterwards) as Word gives them.
const EXCLUSIVE: &[(&str, &str, &[&str])] = &[
    ("smallCaps", "caps", &["caps"]),
    ("caps", "smallCaps", &["smallCaps"]),
    ("caps", "caps", &[]),
    ("smallCaps", "smallCaps", &[]),
    ("strike", "doubleStrike", &["doubleStrike"]),
    ("doubleStrike", "strike", &["strike"]),
    ("emboss", "engrave", &["engrave"]),
    ("engrave", "emboss", &["emboss"]),
    ("outline", "emboss", &["emboss"]),
    ("shadow", "emboss", &["emboss"]),
    ("outline", "engrave", &["engrave"]),
    ("shadow", "engrave", &["engrave"]),
    ("emboss", "outline", &["outline"]),
    ("emboss", "shadow", &["shadow"]),
    ("engrave", "outline", &["outline"]),
    ("engrave", "shadow", &["shadow"]),
    ("outline", "shadow", &["outline", "shadow"]),
    ("shadow", "outline", &["outline", "shadow"]),
];

#[test]
fn exclusive_effects_as_word() {
    for (start, toggled, after) in EXCLUSIVE {
        let mut s = session(&|c| set_effect(c, start));
        s.run(command_for(toggled), &json!({})).unwrap();
        assert_eq!(effects(&s), only(after), "{toggled} on text with {start}: {NAMES:?}");
    }
}

#[test]
fn explicit_on_also_turns_partners_off() {
    let mut s = session(&|c| set_effect(c, "smallCaps"));
    s.run("format.allCaps", &json!({ "value": true })).unwrap();
    assert_eq!(effects(&s), only(&["caps"]));
}

#[test]
fn turning_an_effect_off_leaves_its_partners() {
    let mut s = session(&|c| {
        c.outline = Some(true);
        c.shadow = Some(true);
    });
    s.run("format.outline", &json!({})).unwrap();
    assert_eq!(effects(&s), only(&["shadow"]));
}

#[test]
fn superscript_over_subscript_still_switches() {
    let mut s = session(&|c| c.vert_align = Some(wordcraft_doc::props::VertAlign::Subscript));
    s.run("format.superscript", &json!({})).unwrap();
    assert_eq!(resolved(&s)[0].vert_align, wordcraft_doc::props::VertAlign::Superscript);
}
