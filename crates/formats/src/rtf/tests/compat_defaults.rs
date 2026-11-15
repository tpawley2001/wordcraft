//! Four Word compatibility options are ON in an RTF file unless a document formatting word turns
//! them off, as Word reads them (Microsoft 365 Word, 2026-11-14): `\spltpgpar` (split page break
//! and paragraph mark), `\ftnlytwnine` (Word 6.0/95/97 footnote layout), `\expshrtn` (don't expand
//! character spaces on a line ending in Shift+Return) and `\nocxsptable` (space between
//! paragraphs of the same style in a table). Word 2007 and later write all four.

use super::*;
use wordcraft_doc::Settings;

const WORDS: [&str; 4] = ["spltpgpar", "ftnlytwnine", "expshrtn", "nocxsptable"];

fn field(s: &Settings, word: &str) -> bool {
    match word {
        "spltpgpar" => s.split_pg_break_and_para_mark,
        "ftnlytwnine" => s.footnote_layout_like_ww8,
        "expshrtn" => s.do_not_expand_shift_return,
        "nocxsptable" => s.allow_space_of_same_style_in_table,
        _ => false,
    }
}

fn set_field(s: &mut Settings, word: &str, on: bool) {
    match word {
        "spltpgpar" => s.split_pg_break_and_para_mark = on,
        "ftnlytwnine" => s.footnote_layout_like_ww8 = on,
        "expshrtn" => s.do_not_expand_shift_return = on,
        "nocxsptable" => s.allow_space_of_same_style_in_table = on,
        _ => {}
    }
}

fn doc_with(words: &str) -> String {
    format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{words}\pard\plain\f0\fs24 Some text.\par}}")
}

fn has_word(out: &str, word: &str) -> bool {
    let pat = format!("\\{word}");
    out.match_indices(&pat).any(|(i, _)| !out[i + pat.len()..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '0'))
}

#[test]
fn without_the_words_all_four_are_on() {
    let d = import(doc_with("").as_bytes()).unwrap();
    for w in WORDS {
        assert!(field(&d.settings, w), "{w} should be on when the RTF doesn't say");
    }
    let d = import(doc_with(r"\nouicompat").as_bytes()).unwrap();
    for w in WORDS {
        assert!(field(&d.settings, w), "\\nouicompat alone: {w}");
    }
}

#[test]
fn each_word_turns_only_its_option_off() {
    for w in WORDS {
        for form in [format!("\\{w}"), format!("\\{w}1")] {
            let d = import(doc_with(&form).as_bytes()).unwrap();
            for other in WORDS {
                assert_eq!(field(&d.settings, other), other != w, "{form}: {other}");
            }
        }
        let d = import(doc_with(&format!("\\{w}0")).as_bytes()).unwrap();
        assert!(field(&d.settings, w), "\\{w}0 leaves it on");
    }
}

#[test]
fn a_word_2010_header_turns_all_four_off() {
    let header = r"\noxlattoyen\expshrtn\noultrlspc\dntblnsbdb\nospaceforul\splytwnine\ftnlytwnine\htmautsp\nolnhtadjtbl\useltbaln\alntblind\lytcalctblwd\lyttblrtgr\lnbrkrule\nobrkwrptbl\snaptogridincell\allowfieldendsel\wrppunct\asianbrkrule\newtblstyruls\nogrowautofit\usenormstyforlist\noindnmbrts\felnbrelev\nocxsptable\indrlsweleven\noafcnsttbl\afelev\utinl\hwelev\spltpgpar\notcvasp\notbrkcnstfrctbl\notvatxbx\krnprsnet\cachedcolbal \nouicompat \fet0{\*\wgrffmtfilter 2450}\nofeaturethrottle1";
    let d = import(doc_with(header).as_bytes()).unwrap();
    for w in WORDS {
        assert!(!field(&d.settings, w), "{w}");
    }
}

#[test]
fn export_writes_the_word_when_the_option_is_off() {
    for w in WORDS {
        let mut d = import(doc_with("").as_bytes()).unwrap();
        let on = export(&d);
        assert!(!has_word(&on, w), "{w} is on: no word: {on}");
        set_field(&mut d.settings, w, false);
        let off = export(&d);
        assert!(has_word(&off, w), "{w} is off: the word: {off}");
        let back = import(off.as_bytes()).unwrap();
        assert!(!field(&back.settings, w), "{w} stays off after a round trip");
    }
}
