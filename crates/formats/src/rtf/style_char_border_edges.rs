//! A style-sheet entry's character border (`\chbrdr`), underline and strikethrough: read into the
//! style, written back, and never taken from a paragraph border.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{Border, BorderStyle, CharProps, Rgb, Underline};
use wordcraft_doc::styles::StyleKind;

const SHEET: &str = concat!(
    r"{\*\cs10 \additive \chbrdr\brdrs\brdrw10\brdrcf1 Boxed;}",
    r"{\s15 \chbrdr\brdrdb\brdrw30 Para Frame;}",
    r"{\s16 \brdrt\brdrs\brdrw10 Top Rule;}",
    r"{\s17 \chbrdr\brdrs\brdrw10\brdrt\brdrdb\brdrw30 Mixed;}",
    r"{\*\cs11 \additive \b Plain Bold;}",
    r"{\*\cs12 \additive \ul Under;}",
    r"{\*\cs13 \additive \uldb\strike Two;}",
    r"{\*\cs14 \additive \ulnone\strike0\striked1 Off;}",
);

fn open(stylesheet: &str, body: &str) -> Document {
    let rtf = format!(
        r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\colortbl;\red255\green0\blue0;}}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}"
    );
    import(rtf.as_bytes()).unwrap()
}

fn chr(d: &Document, id: &str) -> CharProps {
    d.styles.get(id).unwrap_or_else(|| panic!("no style {id}")).chr.clone()
}

fn border(style: BorderStyle, width: f32, color: Option<Rgb>) -> Option<Border> {
    Some(Border { style, width, color, space: 0.0 })
}

/// What each entry of [`SHEET`] states, checked on `d`.
fn check_sheet(d: &Document) {
    assert_eq!(d.styles.get("Boxed").map(|s| s.kind), Some(StyleKind::Character));
    assert_eq!(chr(d, "Boxed").border, border(BorderStyle::Single, 0.5, Some(Rgb(255, 0, 0))), "Boxed");
    assert_eq!(d.styles.get("ParaFrame").map(|s| s.kind), Some(StyleKind::Paragraph));
    assert_eq!(chr(d, "ParaFrame").border, border(BorderStyle::Double, 1.5, None), "Para Frame");
    assert_eq!(d.styles.get("ParaFrame").and_then(|s| s.para.borders), None, "Para Frame: no paragraph border");
    assert_eq!(chr(d, "TopRule").border, None, "Top Rule: a paragraph border is not the character border");
    assert_eq!(chr(d, "Mixed").border, border(BorderStyle::Single, 0.5, None), "Mixed");
    let plain = chr(d, "PlainBold");
    assert_eq!(plain.bold, Some(true));
    assert_eq!((plain.border, plain.underline, plain.strike, plain.double_strike), (None, None, None, None), "Plain Bold");
    assert_eq!(chr(d, "Under").underline, Some(Underline::Single), "Under");
    assert_eq!(chr(d, "Under").strike, None, "Under");
    let two = chr(d, "Two");
    assert_eq!((two.underline, two.strike, two.double_strike), (Some(Underline::Double), Some(true), None), "Two");
    let off = chr(d, "Off");
    assert_eq!((off.underline, off.strike, off.double_strike), (Some(Underline::None), Some(false), Some(true)), "Off");
}

#[test]
fn a_style_entry_keeps_its_character_border_underline_and_strike() {
    let d = open(SHEET, r"\pard\plain\f0\fs24 Text.\par");
    check_sheet(&d);
}

#[test]
fn the_style_formatting_round_trips() {
    let d = open(SHEET, r"\pard\plain\f0\fs24 Text.\par");
    let rtf = export(&d);
    let entry = |name: &str| {
        let end = rtf.find(&format!(" {name};}}")).unwrap_or_else(|| panic!("no entry {name}: {rtf}"));
        let start = rtf[..end].rfind('{').expect("entry start");
        rtf[start..end].to_string()
    };
    let boxed = entry("Boxed");
    assert!(boxed.contains(r"\chbrdr\brdrs\brdrw10") && boxed.contains(r"\brdrcf"), "{boxed}");
    assert!(entry("Para Frame").contains(r"\chbrdr\brdrdb\brdrw30"), "{}", entry("Para Frame"));
    assert!(!entry("Top Rule").contains(r"\chbrdr"), "{}", entry("Top Rule"));
    assert!(!entry("Plain Bold").contains(r"\chbrdr") && !entry("Plain Bold").contains(r"\ul"), "{}", entry("Plain Bold"));
    assert!(entry("Under").contains(r"\ul") && !entry("Under").contains(r"\ulnone"), "{}", entry("Under"));
    let off = entry("Off");
    assert!(off.contains(r"\ulnone") && off.contains(r"\strike0") && off.contains(r"\striked1"), "{off}");
    check_sheet(&import(rtf.as_bytes()).unwrap());
}

/// A stated "no border" (`\chbrdr\brdrnone`) is kept as one: it undoes a base style's border.
#[test]
fn a_stated_no_border_round_trips() {
    let d = open(
        r"{\*\cs10 \additive \chbrdr\brdrs\brdrw10 Boxed;}{\*\cs11 \additive \sbasedon10 \chbrdr\brdrnone Unboxed;}",
        r"\pard\plain\f0\fs24 Text.\par",
    );
    let stated = |d: &Document| chr(d, "Unboxed").border;
    assert!(stated(&d).is_some_and(|b| !b.is_visible()), "{:?}", stated(&d));
    let rtf = export(&d);
    let end = rtf.find(" Unboxed;}").expect("entry");
    let start = rtf[..end].rfind('{').expect("entry start");
    assert!(rtf[start..end].contains(r"\chbrdr\brdrnone"), "{}", &rtf[start..end]);
    let back = import(rtf.as_bytes()).unwrap();
    assert!(stated(&back).is_some_and(|b| !b.is_visible()), "{:?}", stated(&back));
    assert_eq!(chr(&back, "Boxed").border, border(BorderStyle::Single, 0.5, None));
}

/// The colour table may follow the style sheet: the border's colour is looked up when the
/// style is made, like `\cf`.
#[test]
fn a_border_colour_is_found_in_a_colour_table_after_the_style_sheet() {
    let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}{\stylesheet{\s0 Normal;}{\*\cs10 \additive \chbrdr\brdrs\brdrw20\brdrcf2 Late;}}{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}\pard\plain\f0\fs24 Text.\par}";
    let d = import(rtf.as_bytes()).unwrap();
    assert_eq!(chr(&d, "Late").border, border(BorderStyle::Single, 1.0, Some(Rgb(0, 0, 255))));
}

/// Text keeps what it states itself; text without the style has no border.
#[test]
fn text_is_not_changed() {
    let d = open(SHEET, r"\pard\plain\f0\fs24 Plain {\cs10\chbrdr\brdrs\brdrw10\brdrcf1 styled} tail.\par");
    let p = d.body[0].as_para().unwrap();
    let at = |needle: &str| {
        let i = p.text[..p.text.find(needle).expect("text")].chars().count();
        p.run_ranges().find(|(r, _)| r.contains(&i)).and_then(|(_, c)| c.border)
    };
    assert_eq!(at("Plain"), None);
    assert_eq!(at("tail"), None);
    let i = p.text[..p.text.find("styled").expect("text")].chars().count();
    let resolved = d.styles.resolve_char(p.props.style.as_deref(), p.props_of_char(i));
    assert_eq!(resolved.border, border(BorderStyle::Single, 0.5, Some(Rgb(255, 0, 0))));
}

#[test]
fn table_and_hostile_entries_do_not_panic() {
    let d = open(r"{\*\ts20\tsrowd \chbrdr\brdrs\brdrw10 Grid Like;}", r"\pard\plain\f0\fs24 Text.\par");
    assert!(d.styles.get("GridLike").is_none_or(|s| s.chr.border.is_none()));
    for sheet in [
        r"{\*\cs10 \additive \chbrdr\brdrs\brdrw-2147483648\brdrcf99999\brsp2147483647 A;}",
        r"{\*\cs10 \additive \chbrdr B;}",
        r"{\s15 \brdrw10\brdrcf1\chbrdr\chbrdr\brdrnone\brdrs C;}",
        r"{\*\cs10 \additive \ul-5\ul2147483647\striked-1\strike99 D;}",
    ] {
        let d = open(sheet, r"\pard\plain\s15\f0\fs24 a {\cs10 b}\par");
        let back = import(export(&d).as_bytes()).unwrap();
        assert_eq!(back.body.len(), d.body.len());
    }
}
