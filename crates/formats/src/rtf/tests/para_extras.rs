//! Paragraph words `\contextualspace`, `\mirrorindents`, `\sbautoN`, `\saautoN`, `\hyphparN` and
//! `\nosnaplinegrid` (RTF 1.9.1): read into the paragraph's own properties, reset by `\pard`, and
//! written back, the RTF form of DOCX `w:contextualSpacing`, `w:mirrorIndents`,
//! `w:spacing/@w:beforeAutospacing`/`@w:afterAutospacing`, `w:suppressAutoHyphens` and `w:snapToGrid`.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::ParaProps;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<ParaProps> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.clone()).collect()
}

/// (contextual, mirror, before auto, after auto, suppress hyphens, snap to grid).
type Row = (Option<bool>, Option<bool>, Option<bool>, Option<bool>, Option<bool>, Option<bool>);

fn row(p: &ParaProps) -> Row {
    (p.contextual_spacing, p.mirror_indents, p.space_before_auto, p.space_after_auto, p.suppress_hyphens, p.snap_to_grid)
}

const ALL: &str = r"\pard\contextualspace\mirrorindents\sb100\sbauto1\sa100\saauto1\hyphpar0\nosnaplinegrid ";
const ALL_ROW: Row = (Some(true), Some(true), Some(true), Some(true), Some(true), Some(false));
const UNSET: Row = (None, None, None, None, None, None);

#[test]
fn each_word_is_read() {
    let d = open(&format!(
        r"{ALL}all\par\pard\contextualspace a\par\pard\mirrorindents b\par\pard\sbauto1 c\par\pard\saauto1 d\par\pard\hyphpar0 e\par\pard\nosnaplinegrid f\par\pard unset\par"
    ));
    let p = paras(&d);
    assert_eq!(row(&p[0]), ALL_ROW);
    assert_eq!(row(&p[1]), (Some(true), None, None, None, None, None));
    assert_eq!(row(&p[2]), (None, Some(true), None, None, None, None));
    assert_eq!(row(&p[3]), (None, None, Some(true), None, None, None));
    assert_eq!(row(&p[4]), (None, None, None, Some(true), None, None));
    assert_eq!(row(&p[5]), (None, None, None, None, Some(true), None));
    assert_eq!(row(&p[6]), (None, None, None, None, None, Some(false)));
    assert_eq!(row(&p[7]), UNSET);
}

#[test]
fn the_state_lasts_across_par_until_pard() {
    let d = open(&format!(r"{ALL}one\par two\par\pard three\par"));
    let p = paras(&d);
    assert_eq!(row(&p[0]), ALL_ROW);
    assert_eq!(row(&p[1]), ALL_ROW, "\\par keeps paragraph state");
    assert_eq!(row(&p[2]), UNSET, "\\pard resets it");
}

#[test]
fn zero_and_one_parameters() {
    let d = open(r"\pard\sbauto0\saauto0 off\par\pard\hyphpar on\par\pard\hyphpar1 one\par");
    let p = paras(&d);
    assert_eq!((p[0].space_before_auto, p[0].space_after_auto), (Some(false), Some(false)));
    assert_eq!(p[1].suppress_hyphens, Some(false), "\\hyphpar turns hyphenation on");
    assert_eq!(p[2].suppress_hyphens, Some(false));
}

#[test]
fn auto_spacing_keeps_the_point_values() {
    let d = open(r"\pard\sb100\sbauto1\sa200\saauto1 x\par");
    let plain = open(r"\pard\sb100\sa200 x\par");
    let (a, b) = (&paras(&d)[0], &paras(&plain)[0]);
    assert_eq!((a.space_before, a.space_after), (b.space_before, b.space_after), "\\sbauto/\\saauto leave \\sb/\\sa alone");
}

#[test]
fn every_setting_round_trips() {
    let d = open(&format!(r"{ALL}all\par\pard\sbauto0\saauto0\hyphpar1 off\par\pard unset\par"));
    let out = export(&d);
    let p = paras(&import(out.as_bytes()).unwrap());
    assert_eq!(row(&p[0]), ALL_ROW, "{out}");
    assert_eq!(row(&p[1]), (None, None, Some(false), Some(false), Some(false), None), "{out}");
    assert_eq!(row(&p[2]), UNSET, "{out}");
}

#[test]
fn plain_paragraphs_write_none_of_the_words() {
    let out = export(&open(r"\pard plain\par"));
    for w in [r"\contextualspace", r"\mirrorindents", r"\sbauto", r"\saauto", r"\hyphpar", r"\nosnaplinegrid"] {
        assert!(!out.contains(w), "{w} in:\n{out}");
    }
}
