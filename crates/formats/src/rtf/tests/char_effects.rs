//! Character effects `\caps`, `\scaps`, `\outl`, `\shad`, `\embo`, `\impr`, `\striked1`,
//! `\charscalexN` and `\kerningN`, RTF 1.9.1: read into the run's own properties and written back,
//! the RTF form of DOCX `w:caps`, `w:smallCaps`, `w:outline`, `w:shadow`, `w:emboss`, `w:imprint`,
//! `w:dstrike`, `w:w` and `w:kern`.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// The properties of the run holding `needle` in paragraph 0.
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
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

/// (caps, small caps, outline, shadow, emboss, engrave, double strike) as plain bools.
fn effects(c: &CharProps) -> [bool; 7] {
    [c.caps, c.small_caps, c.outline, c.shadow, c.emboss, c.engrave, c.double_strike].map(|v| v == Some(true))
}

#[test]
fn each_effect_is_read() {
    let d = open(r"\pard{\caps A}{\scaps B}{\outl C}{\shad D}{\embo E}{\impr F}{\striked1 G}plain\par");
    for (i, t) in ["A", "B", "C", "D", "E", "F", "G"].iter().enumerate() {
        let mut want = [false; 7];
        want[i] = true;
        assert_eq!(effects(&run(&d, t)), want, "{t}");
    }
    assert_eq!(effects(&run(&d, "plain")), [false; 7]);
}

#[test]
fn double_strike_is_not_single_strike() {
    let d = open(r"\pard{\striked1 dbl}{\strike one}\par");
    assert_eq!((run(&d, "dbl").double_strike, run(&d, "dbl").strike), (Some(true), None));
    assert_eq!(run(&d, "one").double_strike, None);
}

#[test]
fn zero_and_plain_turn_effects_off() {
    let d = open(r"\pard\caps\scaps\shad on {\caps0\scaps0\shad0 off}{\plain reset}\par");
    assert_eq!(effects(&run(&d, "on")), [true, true, false, true, false, false, false]);
    assert_eq!(effects(&run(&d, "off")), [false; 7]);
    assert_eq!(effects(&run(&d, "reset")), [false; 7]);
}

#[test]
fn scale_and_kerning_are_read() {
    let d = open(r"\pard{\charscalex80 narrow}{\charscalex150 wide}{\charscalex100 normal}{\kerning28 kerned}{\kerning0 unkerned}\par");
    assert_eq!(run(&d, "narrow").scale, Some(80.0));
    assert_eq!(run(&d, "wide").scale, Some(150.0));
    assert_eq!(run(&d, "normal").scale, None);
    assert_eq!(run(&d, "kerned").kern, Some(14.0));
    assert_eq!(run(&d, "unkerned").kern, None);
}

#[test]
fn group_end_restores_the_outer_effects() {
    let d = open(r"\pard\scaps outer {\scaps0\caps inner} after\par");
    assert_eq!(effects(&run(&d, "inner")), [true, false, false, false, false, false, false]);
    assert_eq!(effects(&run(&d, "after"))[..2], [false, true]);
}

#[test]
fn effects_survive_export() {
    let d =
        open(r"\pard{\caps A}{\scaps B}{\outl C}{\shad D}{\embo E}{\impr F}{\striked1 G}{\charscalex80 H}{\kerning28 I}{\expndtw-3\caps J}plain\par");
    let back = import(export(&d).as_bytes()).unwrap();
    for t in ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "plain"] {
        let (a, b) = (run(&d, t), run(&back, t));
        assert_eq!(effects(&b), effects(&a), "{t}");
        assert_eq!((b.scale, b.kern, b.spacing), (a.scale, a.kern, a.spacing), "{t}");
    }
    assert_eq!(effects(&run(&back, "J")), [true, false, false, false, false, false, false]);
    assert_eq!(run(&back, "J").spacing, Some(-0.15));
}

#[test]
fn a_plain_run_writes_none_of_the_words() {
    let out = export(&open(r"\pard plain text\par"));
    // Whole control words only: `\outl` is not `\outlinelevel`, `\shad` is not `\shading`.
    let has = |w: &str| out.match_indices(w).any(|(i, _)| !out[i + w.len()..].starts_with(|c: char| c.is_ascii_alphabetic()));
    for w in [r"\caps", r"\scaps", r"\outl", r"\shad", r"\embo", r"\impr", r"\striked", r"\charscalex", r"\kerning"] {
        assert!(!has(w), "{w} in {out}");
    }
}
