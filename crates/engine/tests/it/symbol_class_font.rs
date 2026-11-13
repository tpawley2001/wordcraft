//! A font no machine has, which the RTF font table marks as a symbol font (`\fcharset2` with a
//! `\panose`, or a PANOSE whose family kind is 5), is laid out as Symbol: Symbol's character for
//! a code Symbol has, its empty box for one it lacks, and Symbol's taller line. Converted
//! documents set check boxes and bullets in such icon fonts. Every expected value comes from the
//! same document with the font entry renamed to `Symbol`. The run keeps its own font name.

use std::sync::Arc;

use wordcraft_doc::Document;
use wordcraft_engine::io;
use wordcraft_layout::{LayoutCache, LayoutOptions, ParaLayout, Placed, layout};

/// A made-up family no machine has.
const MADEUP: &str = "Zzyx Madeup";

/// An RTF document: Times New Roman (f0) and `entry` as font f1 (its words and name, no `;`),
/// with the paragraph `body` at `half_points` size.
fn rtf(entry: &str, body: &str, half_points: u32) -> String {
    format!(
        "{{\\rtf1\\ansi\\ansicpg1252\\deff0{{\\fonttbl{{\\f0\\froman\\fcharset0 Times New Roman;}}{{\\f1{entry};}}}}\n\\pard\\plain\\sa0\\sb0\\f0\\fs{half_points} {body}\\par}}"
    )
}

fn open(rtf: &str) -> Document {
    io::open_bytes("probe.rtf", rtf.as_bytes()).unwrap()
}

fn first_para(d: &Document) -> Arc<ParaLayout> {
    let l = layout(d, &mut LayoutCache::new(), &LayoutOptions::default());
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

fn line_height(d: &Document) -> f32 {
    first_para(d).lines[0].height
}

fn line_width(pl: &ParaLayout) -> f32 {
    let line = &pl.lines[0];
    pl.clusters[line.c0..line.c1].iter().map(|c| c.adv).sum()
}

/// The font-table entry `words` with the made-up name, and with `Symbol` (the reference).
fn entries(words: &str) -> (String, String) {
    (format!("{words} {MADEUP}"), format!("{words} Symbol"))
}

/// U+F091 (Symbol has no glyph at 0x91) as RTF writes a private-use character: `\uN` with an
/// 8-bit fallback. The reader decodes a bare `\'91` through the document's code page whatever the
/// font's charset, for Symbol as for any font, so that form is compared with Symbol only.
fn pua(c: char) -> String {
    format!("\\uc1\\u{}?", u32::from(c) as i32 - 0x10000)
}

/// Row 2 of the measurements: the symbol charset with a pictorial PANOSE.
const ICONIC: &str = "\\fnil\\fcharset2{\\*\\panose 05010101010101010101}";

#[test]
fn t1_a_check_box_line_is_as_tall_as_symbols() {
    assert!(wordcraft_fonts::word::resolve(MADEUP, false, false).substituted, "the fixture's font must be missing");
    let (madeup, symbol) = entries(ICONIC);
    let body = &format!("{{\\f1{}}}\\tab No", pua('\u{F091}'));
    let got = line_height(&open(&rtf(&madeup, body, 22)));
    let want = line_height(&open(&rtf(&symbol, body, 22)));
    let plain = line_height(&open(&rtf(&madeup, "\\tab No", 22)));
    assert!((got - want).abs() < 0.001, "{got} vs Symbol's {want}");
    assert!(got > plain + 0.1, "{got} vs the text alone {plain}");
}

#[test]
fn t2_other_symbol_class_entries_too() {
    let body = "{\\f1\\'91}\\tab No";
    for words in [
        "\\fnil\\fcharset0{\\*\\panose 05010101010101010101}",
        "\\fnil\\fcharset2{\\*\\panose 05000000000000000000}",
        "\\fnil\\fcharset2{\\*\\panose 050b0604020202030204}",
        "\\froman\\fcharset2{\\*\\panose 02020603050405020304}",
    ] {
        let (madeup, symbol) = entries(words);
        let got = line_height(&open(&rtf(&madeup, body, 22)));
        let want = line_height(&open(&rtf(&symbol, body, 22)));
        let plain = line_height(&open(&rtf(&madeup, "\\tab No", 22)));
        assert!((got - want).abs() < 0.001, "{words}: {got} vs Symbol's {want}");
        assert!(got > plain + 0.1, "{words}: {got} vs the text alone {plain}");
    }
}

#[test]
fn t3_characters_are_symbols_and_a_missing_code_is_symbols_box() {
    let (madeup, symbol) = entries(ICONIC);
    let codes = ['\u{F091}', '\u{F0BA}', '\u{F021}', '\u{F061}'].map(pua);
    for code in ["\\'91", "\\'ba", "\\'21", "\\'61"].iter().map(|c| c.to_string()).chain(codes) {
        let body = format!("A{{\\f1{code}}}B");
        let got = first_para(&open(&rtf(&madeup, &body, 40)));
        let want = first_para(&open(&rtf(&symbol, &body, 40)));
        assert!((line_width(&got) - line_width(&want)).abs() < 0.001, "{code}: {} vs Symbol's {}", line_width(&got), line_width(&want));
        let k = got.clusters.iter().position(|c| c.start == 1).unwrap();
        let kw = want.clusters.iter().position(|c| c.start == 1).unwrap();
        assert!((got.clusters[k].adv - want.clusters[kw].adv).abs() < 0.001, "{code}");
        if code == pua('\u{F091}') {
            // Symbol has no glyph at 0x91: its empty box, one blank glyph under it.
            let c = &got.clusters[k];
            assert!((c.adv - 20.0 * 1224.0 / 2048.0).abs() < 0.01, "{}", c.adv);
            let face = got.styles[c.style as usize].face;
            let glyphs = &got.glyphs[c.g0 as usize..c.g1 as usize];
            assert_eq!(glyphs.len(), 1);
            assert_eq!(glyphs[0].gid, face.glyph_for(' '), "no glyph from another face");
        }
    }
}

/// The layout of `body` with f1 as `words` + the made-up name, against the same with f1 a plain
/// font entry nothing marks (the layout before this rule existed).
fn unchanged(words: &str, name: &str) {
    for (body, size) in [("{\\f1\\'91}\\tab No", 22), ("A{\\f1\\'61}B", 40), ("A{\\f1\\'91}B", 40)] {
        let got = first_para(&open(&rtf(&format!("{words} {name}"), body, size)));
        let plain = first_para(&open(&rtf(&format!("\\fnil {name}"), body, size)));
        assert!(open(&rtf(&format!("\\fnil {name}"), body, size)).settings.symbol_class_fonts.is_empty());
        assert_eq!(got.lines[0].height, plain.lines[0].height, "{words} {name}: {body}");
        assert_eq!(line_width(&got), line_width(&plain), "{words} {name}: {body}");
    }
}

#[test]
fn t4_t5_without_a_panose_nothing_changes() {
    unchanged("\\fnil\\fcharset0", MADEUP);
    unchanged("\\fnil\\fcharset2", MADEUP);
    unchanged("\\ftech\\fcharset2", MADEUP);
}

#[test]
fn t6_t7_known_and_shipped_symbol_fonts_keep_their_own_path() {
    unchanged("\\fnil\\fcharset2{\\*\\panose 05000000000000000000}", "Wingdings");
    unchanged("\\fnil\\fcharset2{\\*\\panose 05050102010205020202}", "MT Extra");
}

#[test]
fn t8_the_run_keeps_its_font_name_and_text_on_save() {
    let (madeup, _) = entries(ICONIC);
    let d = open(&rtf(&madeup, &format!("{{\\f1{}}}\\tab No", pua('\u{F091}')), 22));
    let check = |d: &Document, how: &str| {
        let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
        assert!(p.text.starts_with('\u{F091}'), "{how}: {:?}", p.text);
        let font = p.runs.first().and_then(|r| r.props.font.as_deref());
        assert_eq!(font, Some(MADEUP), "{how}");
    };
    check(&d, "read");
    for name in ["back.rtf", "back.docx"] {
        let bytes = io::save_bytes(name, &d).unwrap();
        check(&io::open_bytes(name, &bytes).unwrap(), name);
    }
}
