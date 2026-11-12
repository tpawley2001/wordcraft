//! The number's own size and underline (`para.defineNumber`, `list.define`): Word draws a list
//! number at its level's `w:sz` and with its level's `w:u`, whatever the paragraph's text looks
//! like (ECMA-376 Part 1 §17.9.24, §17.3.2.38, §17.3.2.40). Sizes run from 1 to 1638 pt in half
//! points, as Word's font sizes do.

use serde_json::{Value, json};
use wordcraft_doc::numbering::Level;
use wordcraft_doc::props::Underline;
use wordcraft_doc::{Document, Pos};

use crate::cmd::lists::level_from;
use crate::{CmdError, Session};

fn session() -> Session {
    let mut s = Session::new(Document::from_text("First\nSecond\nThird"));
    s.run("select.all", &json!({})).unwrap();
    s
}

/// `para.defineNumber` with `extra` beside a decimal `%1.`: the new list's first level.
fn defined(extra: Value) -> Level {
    let mut params = json!({"format": "decimal", "text": "%1."});
    for (k, v) in extra.as_object().unwrap() {
        params[k.as_str()] = v.clone();
    }
    let mut s = session();
    let r = s.run("para.defineNumber", &params).unwrap();
    let num = r["num"].as_u64().expect("the result names the new list") as u32;
    s.doc.numbering.level(num, 0).expect("the new list has a first level").clone()
}

#[test]
fn a_number_without_the_keys_has_no_size_and_no_underline() {
    let level = defined(json!({}));
    assert_eq!((level.chr.size, level.chr.underline), (None, None));
    let level = defined(json!({"size": null, "underline": null}));
    assert_eq!((level.chr.size, level.chr.underline), (None, None));
}

#[test]
fn size_is_stored_in_points_rounded_to_half_points() {
    let cases = [(json!(20), 20.0), (json!(10.5), 10.5), (json!(10.3), 10.5), (json!(11.2), 11.0), (json!(1), 1.0), (json!(1638), 1638.0)];
    for (given, want) in cases {
        let level = defined(json!({"size": given}));
        assert_eq!(level.chr.size, Some(want), "size {given}");
        assert_eq!(level.chr.underline, None, "size {given} sets no underline");
    }
}

#[test]
fn underline_is_stored_by_its_w_u_name() {
    for u in Underline::ALL {
        let level = defined(json!({"underline": u.ooxml()}));
        assert_eq!(level.chr.underline, Some(u), "underline {}", u.ooxml());
        assert_eq!(level.chr.size, None, "underline {} sets no size", u.ooxml());
    }
    // (none) is a setting of its own: it switches a line off.
    assert_eq!(defined(json!({"underline": "none"})).chr.underline, Some(Underline::None));
    assert_eq!(defined(json!({"underline": "wavyDouble"})).chr.underline, Some(Underline::DoubleWave));
}

#[test]
fn size_and_underline_sit_beside_the_number_font() {
    let level = defined(json!({"size": 20, "underline": "double", "bold": true, "font": "Arial"}));
    assert_eq!(level.chr.size, Some(20.0));
    assert_eq!(level.chr.underline, Some(Underline::Double));
    assert_eq!(level.chr.bold, Some(true));
    assert_eq!(level.chr.font.as_deref(), Some("Arial"));
    assert_eq!(level.chr.italic, None);
}

#[test]
fn bad_sizes_and_underlines_are_refused_and_add_no_list() {
    let cases = [
        (json!({"size": 0}), "size"),
        (json!({"size": 0.4}), "size"),
        (json!({"size": 1639}), "size"),
        (json!({"size": -3}), "size"),
        (json!({"size": "12"}), "size"),
        (json!({"size": true}), "size"),
        (json!({"underline": "dashed"}), "underline"),
        (json!({"underline": "Single"}), "underline"),
        (json!({"underline": 3}), "underline"),
        (json!({"size": 12, "underline": "thickDash"}), "underline"),
    ];
    for (extra, word) in cases {
        let mut params = json!({"format": "decimal", "text": "%1."});
        for (k, v) in extra.as_object().unwrap() {
            params[k.as_str()] = v.clone();
        }
        let mut s = session();
        let lists = (s.doc.numbering.nums.len(), s.doc.numbering.abstracts.len());
        match s.run("para.defineNumber", &params) {
            Err(CmdError::Params(message)) => assert!(message.contains(word), "{params}: `{message}` should mention `{word}`"),
            other => panic!("{params}: expected a parameter error, got {other:?}"),
        }
        assert_eq!((s.doc.numbering.nums.len(), s.doc.numbering.abstracts.len()), lists, "{params}: no list is added");
        let first = s.doc.para_at(&Pos::body(0, 0)).unwrap();
        assert_eq!(first.props.numbering, None, "{params}: the paragraphs stay unnumbered");
    }
}

/// `list.define` reads its levels with the same function: set, keep, clear.
#[test]
fn a_level_takes_keeps_and_clears_its_size_and_underline() {
    let set = level_from(Level::default(), 0, &json!({"size": 9, "underline": "dotted"}));
    assert_eq!((set.chr.size, set.chr.underline), (Some(9.0), Some(Underline::Dotted)));
    // Missing keys, other types and unknown names leave the level as it is.
    let kept = level_from(set.clone(), 0, &json!({"bold": true}));
    assert_eq!((kept.chr.size, kept.chr.underline, kept.chr.bold), (Some(9.0), Some(Underline::Dotted), Some(true)));
    let kept = level_from(set.clone(), 0, &json!({"size": "big", "underline": "dashed"}));
    assert_eq!((kept.chr.size, kept.chr.underline), (Some(9.0), Some(Underline::Dotted)));
    // Sizes outside Word's range are pulled into it.
    assert_eq!(level_from(set.clone(), 0, &json!({"size": 5000})).chr.size, Some(1638.0));
    assert_eq!(level_from(set.clone(), 0, &json!({"size": 0.2})).chr.size, Some(1.0));
    assert_eq!(level_from(set.clone(), 0, &json!({"size": 12.74})).chr.size, Some(12.5));
    let cleared = level_from(set, 0, &json!({"size": null, "underline": null}));
    assert_eq!((cleared.chr.size, cleared.chr.underline), (None, None));
}
