//! Phonetic Guide (ruby) text stored as an `EQ` field.
//!
//! A `.doc` file has no ruby structure of its own: Word writes a Phonetic Guide as an `EQ` field
//! (ECMA-376 Part 1 §17.16.5.20) with no stored result, of exactly this shape:
//!
//! ```text
//! EQ \* jc2 \* "Font:MS Mincho" \* hps11 \o\ad(\s\up 10(<ruby text>),<base text>)
//! ```
//!
//! [`parse`] recognises that shape and nothing else, so the reader can turn it into the model's
//! ruby object (the same object `w:ruby`, §17.3.3.25, becomes). Every other `EQ` code stays a
//! field. The parser is flat (no recursion) and bounded.

use wordcraft_doc::para::RubyAlign;

/// Longest instruction looked at, in bytes.
const MAX_INSTR: usize = 2048;
/// Longest ruby or base text, in characters.
const MAX_TEXT: usize = 255;
/// Largest `hpsN` (half-points) and `\up N` (points): 1584 pt, Word's largest font size.
const MAX_HPS: u32 = 3168;
const MAX_RAISE: u32 = 1584;
/// Longest `Font:` name, in characters.
const MAX_FONT: usize = 64;

/// A Phonetic Guide read from an `EQ` instruction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Phonetic {
    pub ruby: String,
    pub base: String,
    pub align: RubyAlign,
    /// Ruby text size, points (`hpsN` is half-points).
    pub size: f32,
    /// From the base text's baseline to the ruby text's baseline, points (`\up N`).
    pub raise: f32,
    /// The ruby text's font (`\* "Font:<name>"`).
    pub font: Option<String>,
}

/// The Phonetic Guide in an `EQ` field instruction, or `None` when the instruction is anything
/// else (including any malformed or oversized form).
pub(crate) fn parse(instr: &str) -> Option<Phonetic> {
    if instr.len() > MAX_INSTR {
        return None;
    }
    let s = instr.trim_matches(' ');
    let s = strip_ci(s, "eq")?;
    // The field name ends here: `EQx` is another name.
    if !(s.is_empty() || s.starts_with([' ', '\\'])) {
        return None;
    }
    let mut align = RubyAlign::Center;
    let mut hps = None;
    let mut font = None;
    let mut s = s.trim_start_matches(' ');
    // General switches, then `\o`.
    loop {
        if let Some(rest) = s.strip_prefix("\\*") {
            let (arg, rest) = switch_arg(rest.trim_start_matches(' '))?;
            general_switch(arg, &mut align, &mut hps, &mut font)?;
            s = rest.trim_start_matches(' ');
        } else {
            s = strip_ci(s, "\\o")?;
            break;
        }
    }
    let hps = hps?;
    // An optional alignment switch for the overstrike (the `jcN` switch decides).
    s = s.trim_start_matches(' ');
    for sw in ["\\ad", "\\ac", "\\al", "\\ar"] {
        if let Some(rest) = strip_ci(s, sw) {
            s = rest;
            break;
        }
    }
    s = s.trim_start_matches(' ').strip_prefix('(')?;
    // First argument: `\s\up N(<ruby text>)`.
    s = strip_ci(s.trim_start_matches(' '), "\\s")?;
    s = strip_ci(s.trim_start_matches(' '), "\\up")?;
    let (raise, rest) = number(s.trim_start_matches(' '))?;
    if raise > MAX_RAISE {
        return None;
    }
    s = rest.trim_start_matches(' ').strip_prefix('(')?;
    let (ruby, rest) = text(s)?;
    // The list separator, then the base text up to the list's `)`.
    s = rest.trim_start_matches(' ');
    s = s.strip_prefix(',').or_else(|| s.strip_prefix(';'))?;
    let (base, rest) = text(s)?;
    if !rest.trim_start_matches(' ').is_empty() {
        return None;
    }
    if ruby.trim().is_empty() || base.trim().is_empty() {
        return None;
    }
    Some(Phonetic { ruby, base, align, size: hps as f32 / 2.0, raise: raise as f32, font })
}

/// One general switch argument (`\* <arg>`). `jcN` is the alignment, in the order of Word's
/// Phonetic Guide Alignment list. Only `jc2` (1-2-1 distribution) is confirmed against Word's
/// output; the other values follow the list order and are not measured.
fn general_switch(arg: &str, align: &mut RubyAlign, hps: &mut Option<u32>, font: &mut Option<String>) -> Option<()> {
    if let Some(name) = strip_ci(arg, "font:") {
        let name = name.trim();
        if name.chars().count() > MAX_FONT {
            return None;
        }
        *font = (!name.is_empty()).then(|| name.to_string());
    } else if let Some(n) = strip_ci(arg, "jc") {
        *align = match whole_number(n)? {
            0 => RubyAlign::Center,
            1 => RubyAlign::DistributeLetter,
            2 => RubyAlign::DistributeSpace,
            3 => RubyAlign::Left,
            4 => RubyAlign::Right,
            _ => return None,
        };
    } else {
        // Any other general switch: not a Phonetic Guide field.
        let n = whole_number(strip_ci(arg, "hps")?)?;
        if !(1..=MAX_HPS).contains(&n) {
            return None;
        }
        *hps = Some(n);
    }
    Some(())
}

/// A switch argument: a `"quoted string"` or a bare word (up to a space or a backslash).
fn switch_arg(s: &str) -> Option<(&str, &str)> {
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"')?;
        Some((rest.get(..end)?, rest.get(end + 1..)?))
    } else {
        let end = s.find([' ', '\\']).unwrap_or(s.len());
        let word = s.get(..end)?;
        if word.is_empty() {
            return None;
        }
        Some((word, s.get(end..)?))
    }
}

/// A non-negative decimal integer at the start of `s` and the rest. At most 6 digits.
fn number(s: &str) -> Option<(u32, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 || end > 6 {
        return None;
    }
    Some((s.get(..end)?.parse().ok()?, s.get(end..)?))
}

/// `s` is a whole non-negative decimal integer.
fn whole_number(s: &str) -> Option<u32> {
    match number(s)? {
        (n, "") => Some(n),
        _ => None,
    }
}

/// A list argument's text up to its unescaped `)`, and what follows the `)`. A backslash escapes
/// the next character; a backslash before a letter is a nested switch and is refused, as is an
/// unescaped `(`, `,` or `;` (a nested group or a further argument).
fn text(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut count = 0usize;
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        let c = match c {
            ')' => return Some((out, s.get(i + 1..)?)),
            '(' | ',' | ';' => return None,
            '\\' => match chars.next() {
                Some((_, e)) if !e.is_alphabetic() => e,
                _ => return None,
            },
            c => c,
        };
        count += 1;
        if count > MAX_TEXT {
            return None;
        }
        out.push(c);
    }
    None
}

/// `s` without the ASCII case-insensitive prefix `p`.
fn strip_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    let head = s.get(..p.len())?;
    if head.eq_ignore_ascii_case(p) { s.get(p.len()..) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_form() {
        let p = parse("EQ \\* jc2 \\* \"Font:MS Mincho\" \\* hps11 \\o\\ad(\\s\\up 10(RT),BASE)").expect("ruby");
        assert_eq!(
            p,
            Phonetic {
                ruby: "RT".into(),
                base: "BASE".into(),
                align: RubyAlign::DistributeSpace,
                size: 5.5,
                raise: 10.0,
                font: Some("MS Mincho".into()),
            }
        );
    }

    #[test]
    fn font_name_is_trimmed_and_capped() {
        let p = parse("EQ \\* \"font:  Mincho \" \\* hps10 \\o(\\s\\up 0(r),b)").expect("ruby");
        assert_eq!(p.font.as_deref(), Some("Mincho"));
        assert_eq!(p.raise, 0.0);
        assert_eq!(parse("EQ \\* \"Font: \" \\* hps10 \\o(\\s\\up 1(r),b)").map(|p| p.font), Some(None));
        let long = "f".repeat(65);
        assert_eq!(parse(&format!("EQ \\* \"Font:{long}\" \\* hps10 \\o(\\s\\up 1(r),b)")), None);
    }

    #[test]
    fn escapes_and_multibyte_text() {
        let p = parse("EQ \\* hps10 \\o(\\s\\up 1(\u{3068}\\;\\\\),\u{6771}\\,)").expect("ruby");
        assert_eq!(p.ruby, "\u{3068};\\");
        assert_eq!(p.base, "\u{6771},");
    }

    #[test]
    fn refused_shapes() {
        for s in [
            "",
            "EQ",
            "EQX \\* hps10 \\o(\\s\\up 1(r),b)",
            "EQ \\* hps10 \\o(\\s\\up 1(r),b",
            "EQ \\* hps10 \\o(\\s\\up 1(r)",
            "EQ \\* hps10 \\o(\\s\\up 1(r),b,c)",
            "EQ \\* hps10 \\o(\\s\\up 1(r(x)),b)",
            "EQ \\* hps10 \\o(\\s\\up(r),b)",
            "EQ \\* hps10 \\o(\\s\\up -1(r),b)",
            "EQ \\* hps10 \\o(\\s\\do 1(r),b)",
            "EQ \\* hps10 \\o(\\s\\up 1( ),b)",
            "EQ \\* hps1x \\o(\\s\\up 1(r),b)",
            "EQ \\* jc \\* hps10 \\o(\\s\\up 1(r),b)",
            "EQ \\* hps99999999999 \\o(\\s\\up 1(r),b)",
            "EQ \\* hps10 \\o(\\s\\up 1(r),b\\)",
            "EQ \\* hps10 \\o(\\s\\up 1(r),b\\",
            "EQ \\* \"Font:x \\* hps10 \\o(\\s\\up 1(r),b)",
            "EQ \\* hps10 \\x(\\s\\up 1(r),b)",
            "\u{00E9}\u{00E9}",
        ] {
            assert_eq!(parse(s), None, "{s}");
        }
    }

    #[test]
    fn caps() {
        assert!(parse("EQ \\* hps3168 \\o(\\s\\up 1584(r),b)").is_some());
        let base = "b".repeat(256);
        assert_eq!(parse(&format!("EQ \\* hps10 \\o(\\s\\up 1(r),{base})")), None);
        let pad = " ".repeat(MAX_INSTR);
        assert_eq!(parse(&format!("EQ \\* hps10 \\o(\\s\\up 1(r),b){pad}")), None);
    }
}
