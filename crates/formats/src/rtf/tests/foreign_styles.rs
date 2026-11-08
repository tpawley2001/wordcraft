//! Stylesheet entries in RTF files written by other programs (RTF 1.9.1, "Style sheet"): a style
//! has what its entry and `\sbasedon` chain set, and RTF's defaults for the rest, not WordCraft's
//! built-in heading formatting. Files WordCraft wrote, or with no `\*\generator`, keep today's
//! fallback (see `heading_and_code_runs_take_their_style_formatting_on_import`).

use super::*;
use wordcraft_doc::{Document, TextColor};

const MSFTEDIT: &str = r"{\*\generator Msftedit 5.41.15.1507;}";

fn open(generator: &str, stylesheet: &str, body: &str) -> Document {
    import(
        format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}{{\f1 Arial;}}}}{generator}{{\stylesheet{{ Normal;}}{stylesheet}}}{body}}}")
            .as_bytes(),
    )
    .unwrap()
}

/// Paragraph `i`'s resolved (space before, space after, keep with next, keep lines together).
fn spacing(d: &Document, i: usize) -> (f32, f32, bool, bool) {
    let p = d.body[i].as_para().unwrap();
    let r = d.styles.resolve_para(&p.props);
    (r.space_before, r.space_after, r.keep_next, r.keep_lines)
}

/// The resolved character formatting of paragraph `i`'s first run.
fn first_run(d: &Document, i: usize) -> wordcraft_doc::resolve::ResolvedChar {
    let p = d.body[i].as_para().unwrap();
    d.styles.resolve_char(p.props.style.as_deref(), &p.runs[0].props)
}

#[test]
fn a_bare_heading_entry_from_another_program_adds_no_spacing_or_keep() {
    // A WordPad file: Word sets these paragraphs one plain line apart.
    let d = open(MSFTEDIT, r"{\s2 heading 2;}", r"\pard\nowidctlpar\s2\b\f1\fs20 First point\par\pard\nowidctlpar\s2\b\f1\fs20 Second point\par");
    assert_eq!(spacing(&d, 0), (0.0, 0.0, false, false));
    assert_eq!(spacing(&d, 1), (0.0, 0.0, false, false));
}

#[test]
fn a_bare_heading_entry_from_another_program_has_rtf_default_character_formatting() {
    let d = open(MSFTEDIT, r"{\s2 heading 2;}", r"\pard\s2 Plain heading text\par");
    let c = first_run(&d, 0);
    assert_eq!(c.size, 12.0);
    assert_eq!(c.color, TextColor::Auto);
}

#[test]
fn a_heading_entry_from_another_program_gives_exactly_its_own_formatting() {
    let d = open(MSFTEDIT, r"{\s2\sb240\sa60\keepn\b\fs28 heading 2;}", r"\pard\s2 Heading\par");
    assert_eq!(spacing(&d, 0), (12.0, 3.0, true, false));
    let c = first_run(&d, 0);
    assert_eq!((c.size, c.bold), (14.0, true));
}

#[test]
fn wordcraft_files_keep_the_built_in_heading_fallback() {
    // Guard: WordCraft's own export, and files with no generator, are read as today.
    for generator in [r"{\*\generator WordCraft;}", ""] {
        let d = open(generator, r"{\s2 heading 2;}", r"\pard\s2 Heading\par");
        let (before, _, keep_next, _) = spacing(&d, 0);
        assert_eq!((before, keep_next), (8.0, true), "generator {generator:?}");
    }
}
