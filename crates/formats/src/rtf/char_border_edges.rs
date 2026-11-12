//! Character borders (`\chbrdr`) in tracked formatting changes, and a character style's border
//! in the stylesheet.

use super::*;
use wordcraft_doc::props::{Border, BorderStyle, Borders, Rgb};
use wordcraft_doc::{Document, Paragraph};

const HEAD: &str = r"{\fonttbl{\f0 Times New Roman;}}{\colortbl;\red255\green0\blue0;}";

fn paras(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

fn border_at(d: &Document, needle: &str) -> Option<Border> {
    for p in paras(d) {
        let Some(byte) = p.text.find(needle) else { continue };
        let at = p.text[..byte].chars().count();
        return p.run_ranges().find(|(r, _)| r.contains(&at)).and_then(|(_, c)| c.border);
    }
    panic!("no paragraph holds {needle:?}");
}

fn para_borders(d: &Document, i: usize) -> Borders {
    paras(d).get(i).and_then(|p| p.props.borders).unwrap_or_default()
}

fn single() -> Option<Border> {
    Some(Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 })
}

/// A run with a tracked formatting change (`\crauthN\crdateN` and `{\*\oldcprops …}`), read
/// from `run` with the revision table naming Alice.
fn open_tracked(run: &str) -> Document {
    let rtf = format!(r"{{\rtf1\ansi\deff0{HEAD}{{\*\revtbl {{Unknown;}}{{Alice;}}}}\pard\plain\f0\fs24 Plain {run} after.\par}}");
    import(rtf.as_bytes()).unwrap()
}

fn old_border(d: &Document, needle: &str) -> Option<Border> {
    for p in paras(d) {
        let Some(byte) = p.text.find(needle) else { continue };
        let at = p.text[..byte].chars().count();
        let c = p.props_of_char(at);
        return c.fmt_change.as_deref().expect("a tracked formatting change").old.border;
    }
    panic!("no paragraph holds {needle:?}");
}

#[test]
fn a_border_added_by_a_tracked_change_round_trips() {
    let d = open_tracked(r"{\chbrdr\brdrs\brdrw10\crauth1\crdate-1480361314{\*\oldcprops \chbrdr\brdrnone}boxed}");
    assert_eq!(border_at(&d, "boxed"), single());
    assert_eq!(old_border(&d, "boxed"), None);
    let out = export(&d);
    assert!(out.contains(r"{\*\oldcprops \chbrdr\brdrnone\crauth"), "{out}");
    let back = import(out.as_bytes()).unwrap();
    assert_eq!(border_at(&back, "boxed"), single());
    assert_eq!(old_border(&back, "boxed"), None);
    assert_eq!(border_at(&back, "after"), None);
    assert_eq!(para_borders(&back, 0), Borders::default());
}

#[test]
fn a_border_removed_by_a_tracked_change_round_trips() {
    let d = open_tracked(r"{\crauth1\crdate-1480361314{\*\oldcprops \chbrdr\brdrdb\brdrw30\brdrcf1}unboxed}");
    let was = Some(Border { style: BorderStyle::Double, width: 1.5, color: Some(Rgb(255, 0, 0)), space: 0.0 });
    assert_eq!(border_at(&d, "unboxed"), None);
    assert_eq!(old_border(&d, "unboxed"), was);
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(border_at(&back, "unboxed"), None);
    assert_eq!(old_border(&back, "unboxed"), was);
    assert_eq!(para_borders(&back, 0), Borders::default());
}

/// Stylesheet entries keep only the formatting the stylesheet reader knows; a character
/// style's `\chbrdr` is dropped there and never reaches the text or the paragraph.
#[test]
fn a_character_style_border_does_not_leak() {
    let rtf = format!(
        r"{{\rtf1\ansi\deff0{HEAD}{{\stylesheet{{\s0 Normal;}}{{\*\cs10 \additive \chbrdr\brdrs\brdrw10 Boxed;}}}}\pard\plain\f0\fs24 Text.\par}}"
    );
    let d = import(rtf.as_bytes()).unwrap();
    assert_eq!(border_at(&d, "Text."), None);
    assert_eq!(para_borders(&d, 0), Borders::default());
}
