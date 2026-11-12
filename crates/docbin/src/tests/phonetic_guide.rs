//! Phonetic Guide (ruby) text in a `.doc` file is an `EQ` field with no result:
//! `EQ \* jc2 \* "Font:MS Mincho" \* hps11 \o\ad(\s\up 10(<ruby>),<base>)`. Word shows the base
//! text with the ruby text above it; the reader turns exactly this shape into
//! `InlineObject::Ruby` (ECMA-376 Part 1 §17.3.3.25) and leaves every other `EQ` code a field.
//! `hpsN` is the ruby size in half-points, `\up N` the baseline-to-baseline raise in points and
//! `jcN` the alignment in the order of Word's Phonetic Guide dialog (measured for `jc2`).

use wordcraft_doc::para::{InlineObject, RubyAlign};

use super::{RichSpec, grpprl, para0, read, rich_doc};

const HPS: u16 = 0x4A43; // sprmCHps, half-points

/// `x <field> y` in 11 pt Times New Roman; the field has `code` and the stored `result`.
fn open_text(text: String) -> wordcraft_doc::Paragraph {
    let spec = RichSpec {
        text: text.leak(),
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, grpprl(&[(HPS, &[22, 0])]))],
        papx_entries: vec![(0, 0, Vec::new())],
    };
    para0(&read(&rich_doc(&spec)).expect("opens"))
}

fn open(code: &str, result: &str) -> wordcraft_doc::Paragraph {
    open_text(format!("x \u{13}{code}\u{14}{result}\u{15} y\r"))
}

struct Ruby {
    base: String,
    ruby: String,
    align: RubyAlign,
    size: f32,
    raise: f32,
    base_size: f32,
    font: Option<String>,
    font_ea: Option<String>,
}

fn ruby_of(p: &wordcraft_doc::Paragraph) -> Ruby {
    assert_eq!(p.objects.len(), 1, "one object: {:?}", p.objects);
    match p.objects.first() {
        Some(InlineObject::Ruby { base, ruby, align, size, raise, base_size, props, .. }) => Ruby {
            base: base.clone(),
            ruby: ruby.clone(),
            align: *align,
            size: *size,
            raise: *raise,
            base_size: *base_size,
            font: props.font.clone(),
            font_ea: props.font_ea.clone(),
        },
        other => panic!("expected a ruby object, got {other:?}"),
    }
}

/// The instruction and result of the paragraph's single field object.
fn field_of(p: &wordcraft_doc::Paragraph) -> (String, String) {
    assert_eq!(p.objects.len(), 1, "one object: {:?}", p.objects);
    match p.objects.first() {
        Some(InlineObject::Field { instr, result, .. }) => (instr.clone(), result.clone()),
        other => panic!("expected a field object, got {other:?}"),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

const WORD_CODE: &str = "EQ \\* jc2 \\* \"Font:MS Mincho\" \\* hps11 \\o\\ad(\\s\\up 10(RT),BASE)";

#[test]
fn words_phonetic_guide_field_becomes_ruby() {
    let p = open(WORD_CODE, "");
    assert_eq!(p.text, "x \u{FFFC} y");
    let r = ruby_of(&p);
    assert_eq!(r.ruby, "RT");
    assert_eq!(r.base, "BASE");
    assert_eq!(r.align, RubyAlign::DistributeSpace);
    assert!(close(r.size, 5.5), "hps11 is 5.5 pt, got {}", r.size);
    assert!(close(r.raise, 10.0), "\\up 10 is a 10 pt raise, got {}", r.raise);
    assert!(close(r.base_size, 11.0), "the run is 11 pt, got {}", r.base_size);
    // The ruby font differs from the run's (Times New Roman): the ruby keeps it.
    assert_eq!(r.font.as_deref(), Some("MS Mincho"));
    assert_eq!(r.font_ea.as_deref(), Some("MS Mincho"));
}

#[test]
fn base_text_is_the_paragraphs_plain_text() {
    let p = open(WORD_CODE, "");
    assert_eq!(p.plain_text(), "x BASE y");
}

#[test]
fn field_without_a_separator_becomes_ruby_too() {
    let p = open_text(format!("x \u{13}{WORD_CODE}\u{15} y\r"));
    assert_eq!(p.text, "x \u{FFFC} y");
    let r = ruby_of(&p);
    assert_eq!((r.ruby.as_str(), r.base.as_str()), ("RT", "BASE"));
}

#[test]
fn japanese_texts_are_kept() {
    let code = "EQ \\* jc2 \\* \"Font:MS Mincho\" \\* hps11 \\o\\ad(\\s\\up 10(\u{3068}\u{3046}\u{304D}\u{3087}\u{3046}),\u{6771}\u{4EAC})";
    let p = open(code, "");
    let r = ruby_of(&p);
    assert_eq!(r.ruby, "\u{3068}\u{3046}\u{304D}\u{3087}\u{3046}");
    assert_eq!(r.base, "\u{6771}\u{4EAC}");
    assert_eq!(p.plain_text(), "x \u{6771}\u{4EAC} y");
}

#[test]
fn spaces_case_escapes_and_switch_order_are_tolerated() {
    let p = open(" eq \\* hps20 \\* jc0 \\o \\ac ( \\s \\up 9(a\\,b\\)),x\\(y)", "");
    let r = ruby_of(&p);
    assert_eq!(r.ruby, "a,b)");
    assert_eq!(r.base, "x(y");
    assert_eq!(r.align, RubyAlign::Center);
    assert!(close(r.size, 10.0), "hps20 is 10 pt, got {}", r.size);
    assert!(close(r.raise, 9.0), "got {}", r.raise);
    // No Font: switch: the ruby text sets no font of its own.
    assert_eq!(r.font, None);
    assert_eq!(r.font_ea, None);
}

#[test]
fn jc_switch_gives_the_alignment() {
    for (code, align) in [
        ("EQ \\* jc0 \\* hps10 \\o\\ac(\\s\\up 8(r),b)", RubyAlign::Center),
        ("EQ \\* jc1 \\* hps10 \\o\\ad(\\s\\up 8(r),b)", RubyAlign::DistributeLetter),
        ("EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(r),b)", RubyAlign::DistributeSpace),
        ("EQ \\* jc3 \\* hps10 \\o\\al(\\s\\up 8(r);b)", RubyAlign::Left),
        ("EQ \\* jc4 \\* hps10 \\o\\ar(\\s\\up 8(r),b)", RubyAlign::Right),
        ("EQ \\* hps10 \\o\\ad(\\s\\up 8(r),b)", RubyAlign::Center),
    ] {
        let r = ruby_of(&open(code, ""));
        assert_eq!(r.align, align, "{code}");
        assert_eq!((r.ruby.as_str(), r.base.as_str()), ("r", "b"), "{code}");
        assert!(close(r.size, 5.0) && close(r.raise, 8.0), "{code}: size {} raise {}", r.size, r.raise);
    }
}

#[test]
fn other_eq_codes_stay_fields() {
    for code in [
        // No ruby size.
        "EQ \\* jc2 \\o\\ad(\\s\\up 8(r),b)",
        // Not a Phonetic Guide alignment.
        "EQ \\* jc7 \\* hps10 \\o\\ad(\\s\\up 8(r),b)",
        // An enclosure and a fraction.
        "EQ \\o\\ac(O,A)",
        "EQ \\f(1,2)",
        // A nested switch in the base text.
        "EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(r),\\f(1,2))",
        // Text after the list.
        "EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(r),b) x",
        // An empty ruby text and an empty base text.
        "EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(),b)",
        "EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(r),)",
        // Another general switch.
        "EQ \\* jc2 \\* hps10 \\* MERGEFORMAT \\o\\ad(\\s\\up 8(r),b)",
    ] {
        let p = open(code, "");
        let (instr, result) = field_of(&p);
        assert_eq!(instr, code, "the code is kept");
        assert_eq!(result, "", "{code}");
        assert_eq!(p.plain_text(), "x  y", "{code}");
    }
}

#[test]
fn stored_result_wins() {
    let p = open(WORD_CODE, "zz");
    let (instr, result) = field_of(&p);
    assert_eq!(instr, WORD_CODE);
    assert_eq!(result, "zz");
}

#[test]
fn oversized_input_stays_a_field() {
    // A ruby text of 256 characters (the cap is 255).
    let long = "r".repeat(256);
    let code = format!("EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8({long}),b)");
    let (instr, _) = field_of(&open(&code, ""));
    assert_eq!(instr, code);
    // 255 characters are read.
    let fits = "r".repeat(255);
    let r = ruby_of(&open(&format!("EQ \\* jc2 \\* hps10 \\o\\ad(\\s\\up 8({fits}),b)"), ""));
    assert_eq!(r.ruby.chars().count(), 255);
    // An instruction over 2048 bytes is not parsed.
    let pad = " ".repeat(2048);
    let code = format!("EQ{pad}\\* jc2 \\* hps10 \\o\\ad(\\s\\up 8(r),b)");
    let p = open(&code, "");
    assert!(matches!(p.objects.first(), Some(InlineObject::Field { .. })), "{:?}", p.objects.first());
    // Sizes and raises out of range.
    for code in ["EQ \\* hps0 \\o\\ad(\\s\\up 8(r),b)", "EQ \\* hps3169 \\o\\ad(\\s\\up 8(r),b)", "EQ \\* hps10 \\o\\ad(\\s\\up 1585(r),b)"] {
        let (instr, _) = field_of(&open(code, ""));
        assert_eq!(instr, code);
    }
}
