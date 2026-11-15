//! Character "snap to grid" (`\cgridN`, RTF 1.9.1), the RTF form of DOCX run `w:snapToGrid`: `\cgrid0`
//! turns it off and is kept on open and save; `\cgrid` / `\cgrid1` are the default and stored as
//! nothing; `\plain` resets it; a character style carries it. As Word reads and writes these files.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// The properties of the run holding `needle` in paragraph `para`.
fn run(d: &Document, para: usize, needle: &str) -> CharProps {
    let p = d.body[para].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    for r in &p.runs {
        let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

const ROW1: &str = r"\pard\plain A{\cgrid0 OFF}B{\cgrid ON1}C{\cgrid1 ON2}\par";

#[test]
fn cgrid0_turns_snap_off_and_on_is_stored_as_nothing() {
    let d = open(ROW1);
    assert_eq!(run(&d, 0, "OFF").snap_to_grid, Some(false));
    for t in ["A", "ON1", "ON2"] {
        assert_eq!(run(&d, 0, t).snap_to_grid, None, "{t}");
    }
}

#[test]
fn plain_resets_it() {
    let d = open(r"\pard\plain\cgrid0 PARA{\plain Z}\par");
    assert_eq!(run(&d, 0, "PARA").snap_to_grid, Some(false));
    assert_eq!(run(&d, 0, "Z").snap_to_grid, None);
}

#[test]
fn a_character_style_carries_it() {
    let d = open(
        r"{\stylesheet{\s0 Normal;}{\*\cs10 \additive \cgrid0 NoSnap;}}\pard\plain A{\cs10\cgrid0 STY}B\par",
    );
    let s = d.styles.styles.iter().find(|s| s.name == "NoSnap").expect("style NoSnap");
    assert_eq!(s.chr.snap_to_grid, Some(false));
    let r = run(&d, 0, "STY");
    let resolved = r.snap_to_grid.or(s.chr.snap_to_grid);
    assert_eq!(resolved, Some(false));
    assert_eq!(run(&d, 0, "B").snap_to_grid, None);
}

#[test]
fn rtf_round_trip_keeps_it() {
    let out = export(&open(ROW1));
    assert!(out.contains("\\cgrid0"), "\\cgrid0 missing: {out}");
    assert!(!out.contains("\\cgrid1"), "\\cgrid1 written: {out}");
    let again = import(out.as_bytes()).unwrap();
    assert_eq!(run(&again, 0, "OFF").snap_to_grid, Some(false));
    assert_eq!(run(&again, 0, "ON1").snap_to_grid, None);
}

#[test]
fn a_plain_document_writes_no_cgrid0() {
    let out = export(&open(r"\pard\cgrid plain text\par"));
    assert!(!out.contains("\\cgrid0"), "{out}");
}
