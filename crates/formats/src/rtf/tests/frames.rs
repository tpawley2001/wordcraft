//! Positioned frames (`\absw`, `\absh`, `\posx…`, `\posy…`, `\ph…`, `\pv…`, `\dxfrtext`, `\wrap…`)
//! and drop caps (`\dropcapli`, `\dropcapt`), RTF 1.9.1: read into `ParaProps::frame` and the
//! drop cap fields, reset by `\pard`, and written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{ParaProps, TextFrame};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<ParaProps> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.clone()).collect()
}

fn frame(d: &Document, i: usize) -> TextFrame {
    paras(d)[i].frame.clone().expect("paragraph should be framed")
}

#[test]
fn frame_size_position_and_page_anchors_are_read() {
    let d = open(r"\pard\absw2880\absh-720\phpg\pvpg\posx1440\posy2160\dxfrtext180 Framed\par\pard Body\par");
    let f = frame(&d, 0);
    assert_eq!(f.w, Some(144.0));
    assert_eq!(f.h, Some(36.0));
    assert!(f.h_exact);
    assert_eq!((f.x, f.y), (72.0, 108.0));
    assert_eq!((f.x_align.as_deref(), f.y_align.as_deref()), (None, None));
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("page", "page"));
    assert_eq!((f.h_space, f.v_space), (9.0, 9.0));
    assert_eq!(f.wrap, "auto");
    assert_eq!(paras(&d)[1].frame, None, "\\pard ends the frame");
}

#[test]
fn frame_defaults_are_column_and_margin_anchored_auto_height() {
    let d = open(r"\pard\absw1440 x\par");
    let f = frame(&d, 0);
    assert_eq!(f.w, Some(72.0));
    assert_eq!(f.h, None);
    assert!(!f.h_exact);
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("text", "margin"));
    assert_eq!(f.wrap, "auto");
    assert_eq!(f.anchor_lock, None);
    assert_eq!((f.h_space, f.v_space), (0.0, 0.0));
}

#[test]
fn aligned_positions_and_anchor_words() {
    let cases: [(&str, Option<&str>, Option<&str>, &str, &str); 5] = [
        (r"\phmrg\pvmrg\posxr\posyb", Some("right"), Some("bottom"), "margin", "margin"),
        (r"\phcol\pvpara\posxc\posyc", Some("center"), Some("center"), "text", "text"),
        (r"\phpg\pvpg\posxi\posyt", Some("inside"), Some("top"), "page", "page"),
        (r"\phmrg\pvpg\posxo\posyout", Some("outside"), Some("outside"), "margin", "page"),
        (r"\phmrg\pvmrg\posxl\posyin", Some("left"), Some("inside"), "margin", "margin"),
    ];
    for (words, xa, ya, ha, va) in cases {
        let d = open(&format!(r"\pard\absw1440{words} x\par"));
        let f = frame(&d, 0);
        assert_eq!((f.x_align.as_deref(), f.y_align.as_deref()), (xa, ya), "{words}");
        assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), (ha, va), "{words}");
    }
}

#[test]
fn wrap_lock_negative_offsets_and_separate_distances() {
    let cases =
        [(r"\wrapdefault", "auto"), (r"\wraparound", "around"), (r"\wraptight", "tight"), (r"\wrapthrough", "through"), (r"\nowrap", "notBeside")];
    for (word, wrap) in cases {
        let d = open(&format!(r"\pard\absw1440{word} x\par"));
        assert_eq!(frame(&d, 0).wrap, wrap, "{word}");
    }
    let d = open(r"\pard\absh360\abslock\phpg\pvpg\posnegx-720\posnegy-240\dfrmtxtx360\dfrmtxty120 x\par");
    let f = frame(&d, 0);
    assert_eq!(f.w, None);
    assert_eq!(f.h, Some(18.0));
    assert!(!f.h_exact);
    assert_eq!(f.anchor_lock, Some(true));
    assert_eq!((f.x, f.y), (-36.0, -12.0));
    assert_eq!((f.h_space, f.v_space), (18.0, 6.0));
}

#[test]
fn drop_caps_are_read_and_are_not_frames() {
    let d = open(r"\pard\dropcapli3\dropcapt1\dxfrtext72 T\par\pard\dropcapli2\dropcapt2 M\par\pard plain\par");
    let p = paras(&d);
    assert_eq!((p[0].drop_cap, p[0].drop_cap_margin, p[0].drop_cap_distance), (Some(3), None, Some(3.6)));
    assert_eq!(p[0].frame, None);
    assert_eq!((p[1].drop_cap, p[1].drop_cap_margin, p[1].drop_cap_distance), (Some(2), Some(true), None));
    assert_eq!(p[1].frame, None);
    assert_eq!((p[2].drop_cap, p[2].frame.clone()), (None, None), "\\pard clears the drop cap");
}

#[test]
fn frames_and_drop_caps_round_trip() {
    let d = open(concat!(
        r"\pard\absw2880\absh-720\phpg\pvpg\posx1440\posy2160\dxfrtext180\wraparound A\par",
        r"\pard\absh360\abslock\phmrg\pvpara\posxc\posnegy-240\dfrmtxtx360\dfrmtxty120\wraptight B\par",
        r"\pard\absw1440\phcol\pvmrg\posxo\posyb\nowrap C\par",
        r"\pard\dropcapli3\dropcapt1\dxfrtext72 D\par",
        r"\pard\dropcapli2\dropcapt2 E\par",
        r"\pard plain\par",
    ));
    let back = import(export(&d).as_bytes()).unwrap();
    let (a, b) = (paras(&d), paras(&back));
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x.frame, y.frame, "paragraph {i} frame");
        assert_eq!(
            (x.drop_cap, x.drop_cap_margin, x.drop_cap_distance),
            (y.drop_cap, y.drop_cap_margin, y.drop_cap_distance),
            "paragraph {i} drop cap"
        );
    }
    assert!(b[0].frame.is_some() && b[3].drop_cap == Some(3) && b[5].frame.is_none());
}
