//! Underline styles (`\ul`, `\ulw`, `\uldb`, `\ulth`, `\uld`, `\ulthd`, `\uldash`, `\ulldash`,
//! `\ulthdash`, `\ulthldash`, `\uldashd`, `\ulthdashd`, `\uldashdd`, `\ulthdashdd`, `\ulwave`,
//! `\ulhwave`, `\ululdbwave`) and underline colour (`\ulcN`), RTF 1.9.1: read into the run's
//! underline style and colour and written back, the RTF form of DOCX `w:u/@w:val` and `@w:color`.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{CharProps, Rgb, Underline};

const COLORS: &str = r"{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{COLORS}{body}}}").as_bytes()).unwrap()
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

fn no_underline(c: &CharProps) -> bool {
    matches!(c.underline, None | Some(Underline::None))
}

/// (control word, style), one run per row; the run text is the row index as letters.
const TABLE: [(&str, Underline); 18] = [
    ("ul", Underline::Single),
    ("ulw", Underline::Words),
    ("uldb", Underline::Double),
    ("ulth", Underline::Thick),
    ("uld", Underline::Dotted),
    ("ulthd", Underline::Dotted),
    ("uldash", Underline::Dash),
    ("ulldash", Underline::Dash),
    ("ulthdash", Underline::Dash),
    ("ulthldash", Underline::Dash),
    ("uldashd", Underline::DotDash),
    ("ulthdashd", Underline::DotDash),
    ("uldashdd", Underline::DotDotDash),
    ("ulthdashdd", Underline::DotDotDash),
    ("ulwave", Underline::Wave),
    ("ulhwave", Underline::Wave),
    ("ululdbwave", Underline::DoubleWave),
    // A plain single underline again, so the table ends on the common case.
    ("ul", Underline::Single),
];

fn tag(i: usize) -> String {
    format!("w{}x", (b'a' + i as u8) as char)
}

fn table_doc() -> Document {
    let mut body = String::from(r"\pard ");
    for (i, (word, _)) in TABLE.iter().enumerate() {
        body.push_str(&format!(r"{{\{word} {} }}", tag(i)));
    }
    body.push_str(r"plain\par");
    open(&body)
}

#[test]
fn each_underline_word_reads_as_its_style() {
    let d = table_doc();
    for (i, (word, want)) in TABLE.iter().enumerate() {
        assert_eq!(run(&d, &tag(i)).underline, Some(*want), "\\{word}");
    }
    assert!(no_underline(&run(&d, "plain")), "{:?}", run(&d, "plain").underline);
}

#[test]
fn ulnone_and_zero_parameters_turn_the_underline_off() {
    let d = open(r"\pard\uldb dbl {\ulnone none} {\uldb0 zero} {\ulwave0 wzero} {\ul0 uzero}\par");
    assert_eq!(run(&d, "dbl").underline, Some(Underline::Double));
    for t in ["none", "zero", "wzero", "uzero"] {
        assert!(no_underline(&run(&d, t)), "{t}: {:?}", run(&d, t).underline);
    }
}

#[test]
fn a_later_underline_word_replaces_the_style() {
    let d = open(r"\pard{\uldb\ulwave wavy}{\ulwave\uld dotted}\par");
    assert_eq!(run(&d, "wavy").underline, Some(Underline::Wave));
    assert_eq!(run(&d, "dotted").underline, Some(Underline::Dotted));
}

#[test]
fn group_end_and_plain_restore_and_reset_the_style() {
    let d = open(r"\pard\uldb outer {\ulwave inner} after {\plain reset}\par");
    assert_eq!(run(&d, "outer").underline, Some(Underline::Double));
    assert_eq!(run(&d, "inner").underline, Some(Underline::Wave));
    assert_eq!(run(&d, "after").underline, Some(Underline::Double));
    assert!(no_underline(&run(&d, "reset")));
    assert_eq!(run(&d, "reset").underline_color, None);
}

#[test]
fn ulc_reads_the_colour_table_entry() {
    let d = open(r"\pard{\ul\ulc1 red}{\uldb\ulc2 blue}{\ul\ulc0 auto}{\ul\ulc9 past}{\ul plain}{\ulc1 nounderline}\par");
    assert_eq!(run(&d, "red").underline_color, Some(Rgb(255, 0, 0)));
    assert_eq!(run(&d, "blue").underline_color, Some(Rgb(0, 0, 255)));
    assert_eq!(run(&d, "blue").underline, Some(Underline::Double));
    assert_eq!(run(&d, "auto").underline_color, None, "\\ulc0 is the auto colour");
    assert_eq!(run(&d, "past").underline_color, None, "an index past the table");
    assert_eq!(run(&d, "plain").underline_color, None);
    assert_eq!(run(&d, "nounderline").underline_color, Some(Rgb(255, 0, 0)), "the colour stays without an underline");
    assert!(no_underline(&run(&d, "nounderline")));
}

#[test]
fn styles_and_colour_survive_export() {
    let d = table_doc();
    let out = export(&d);
    let back = import(out.as_bytes()).unwrap();
    for (i, (word, want)) in TABLE.iter().enumerate() {
        assert_eq!(run(&back, &tag(i)).underline, Some(*want), "\\{word} after export:\n{out}");
    }
    assert!(no_underline(&run(&back, "plain")));

    let d = open(r"\pard{\uldb\ulc1 red} {\ulwave\ulc2 blue} plain\par");
    let out = export(&d);
    let back = import(out.as_bytes()).unwrap();
    assert_eq!((run(&back, "red").underline, run(&back, "red").underline_color), (Some(Underline::Double), Some(Rgb(255, 0, 0))), "{out}");
    assert_eq!((run(&back, "blue").underline, run(&back, "blue").underline_color), (Some(Underline::Wave), Some(Rgb(0, 0, 255))), "{out}");
    assert!(no_underline(&run(&back, "plain")));
}

#[test]
fn export_writes_word_s_control_words() {
    let out = export(&open(r"\pard{\uldb a}{\ulw b}{\ululdbwave c}{\uldashdd d}\par"));
    for w in [r"\uldb", r"\ulw", r"\ululdbwave", r"\uldashdd"] {
        assert!(out.contains(w), "{w} in:\n{out}");
    }
    let plain = export(&open(r"\pard no underline here\par"));
    assert!(!plain.contains(r"\ul"), "no underline words for plain text:\n{plain}");
}
