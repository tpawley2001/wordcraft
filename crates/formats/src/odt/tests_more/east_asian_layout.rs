//! Word's Asian Layout and no-proofing on text: ODF 1.3 §20.364 `style:text-combine`
//! (two lines in one, with §20.365/§20.366 its brackets), §20.370 `style:text-rotation-angle` and
//! §20.371 `style:text-rotation-scale` (horizontal in vertical), and `fo:language="zxx"` (no
//! linguistic content: Do not check spelling or grammar), read and written back.

use crate::odt::tests::{NS_T, content_of, package};
use crate::odt::{export, import};
use wordcraft_doc::props::{CharProps, CombineBrackets, EastAsianLayout};
use wordcraft_doc::{Document, Paragraph, para_block};

/// One bold span per entry of `props` (bold keeps each its own run), `|` between them.
fn spans(props: &[&str]) -> Document {
    let styles: String = props
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(r#"<style:style style:name="T{i}" style:family="text"><style:text-properties fo:font-weight="bold" {p}/></style:style>"#)
        })
        .collect();
    let body: String = (0..props.len()).map(|i| format!(r#"<text:span text:style-name="T{i}">s{i}</text:span>|"#)).collect();
    let content = format!(
        r#"<office:document-content {NS_T}><office:automatic-styles>{styles}</office:automatic-styles><office:body><office:text><text:p>{body}</text:p></office:text></office:body></office:document-content>"#
    );
    import(&package(&content, None)).unwrap()
}

fn runs(d: &Document) -> Vec<CharProps> {
    let p = d.body[0].as_para().unwrap();
    p.run_ranges()
        .filter(|(r, _)| p.text.get(r.clone()).is_some_and(|t| t.trim_start_matches('|').starts_with('s')))
        .map(|(_, props)| props.clone())
        .collect()
}

fn eal(combine: Option<bool>, brackets: Option<CombineBrackets>, vert: Option<bool>, compress: Option<bool>) -> Option<EastAsianLayout> {
    Some(EastAsianLayout { combine, combine_brackets: brackets, vert, vert_compress: compress, ..Default::default() })
}

#[test]
fn two_lines_in_one_and_horizontal_in_vertical_are_read() {
    let d = spans(&[
        r#"style:text-combine="lines""#,
        r#"style:text-combine="lines" style:text-combine-start-char="(" style:text-combine-end-char=")""#,
        r#"style:text-combine="lines" style:text-combine-start-char="[" style:text-combine-end-char="]""#,
        r#"style:text-combine="lines" style:text-combine-start-char="&lt;" style:text-combine-end-char="&gt;""#,
        r#"style:text-combine="lines" style:text-combine-start-char="{" style:text-combine-end-char="}""#,
        r#"style:text-combine="lines" style:text-combine-start-char="【" style:text-combine-end-char="】""#,
        r#"style:text-rotation-angle="90""#,
        r#"style:text-rotation-angle="270" style:text-rotation-scale="line-height""#,
        r#"style:text-rotation-angle="90" style:text-rotation-scale="fixed""#,
        r#"style:text-combine="none""#,
        r#"style:text-combine="letters""#,
        r#"style:text-rotation-angle="0""#,
    ]);
    let got: Vec<Option<EastAsianLayout>> = runs(&d).into_iter().map(|r| r.east_asian_layout).collect();
    let want = vec![
        eal(Some(true), None, None, None),
        eal(Some(true), Some(CombineBrackets::Round), None, None),
        eal(Some(true), Some(CombineBrackets::Square), None, None),
        eal(Some(true), Some(CombineBrackets::Angle), None, None),
        eal(Some(true), Some(CombineBrackets::Curly), None, None),
        eal(Some(true), None, None, None),
        eal(None, None, Some(true), None),
        eal(None, None, Some(true), Some(true)),
        eal(None, None, Some(true), None),
        None,
        None,
        None,
    ];
    assert_eq!(got, want);
}

#[test]
fn no_linguistic_content_is_do_not_check_spelling() {
    let d = spans(&[r#"fo:language="zxx" fo:country="none""#, r#"fo:language="de" fo:country="DE""#, ""]);
    let r = runs(&d);
    assert_eq!((r[0].no_proof, r[0].lang.as_deref()), (Some(true), None));
    assert_eq!((r[1].no_proof, r[1].lang.as_deref()), (None, Some("de-DE")));
    assert_eq!(r[2].no_proof, None);
}

#[test]
fn asian_layout_and_no_proofing_round_trip_from_the_model() {
    let want = [
        CharProps { east_asian_layout: eal(Some(true), None, None, None), ..Default::default() },
        CharProps { east_asian_layout: eal(Some(true), Some(CombineBrackets::Round), None, None), ..Default::default() },
        CharProps { east_asian_layout: eal(Some(true), Some(CombineBrackets::Square), None, None), ..Default::default() },
        CharProps { east_asian_layout: eal(Some(true), Some(CombineBrackets::Angle), None, None), ..Default::default() },
        CharProps { east_asian_layout: eal(Some(true), Some(CombineBrackets::Curly), None, None), ..Default::default() },
        CharProps { east_asian_layout: eal(None, None, Some(true), None), ..Default::default() },
        CharProps { east_asian_layout: eal(None, None, Some(true), Some(true)), ..Default::default() },
        CharProps { no_proof: Some(true), ..Default::default() },
    ];
    let mut p = Paragraph::new();
    for (i, props) in want.iter().enumerate() {
        let s = format!("s{i}|");
        p.text.push_str(&s);
        p.runs.push(wordcraft_doc::para::Run { len: s.len() - 1, props: props.clone() });
        p.runs.push(wordcraft_doc::para::Run { len: 1, props: Default::default() });
    }
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let saved = export(&d).unwrap();
    let c = content_of(&saved);
    for attr in [
        r#"style:text-combine="lines""#,
        r#"style:text-combine-start-char="(""#,
        r#"style:text-combine-end-char="]""#,
        r#"style:text-combine-start-char="&lt;""#,
        r#"style:text-combine-end-char="}""#,
        r#"style:text-rotation-angle="90""#,
        r#"style:text-rotation-scale="line-height""#,
        r#"style:text-rotation-scale="fixed""#,
        r#"fo:language="zxx""#,
    ] {
        assert!(c.contains(attr), "{attr} in {c}");
    }
    let got = runs(&import(&saved).unwrap());
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!((&g.east_asian_layout, g.no_proof), (&w.east_asian_layout, w.no_proof), "run {i}");
    }
}

#[test]
fn plain_text_writes_none_of_them() {
    let mut p = Paragraph::new();
    p.text = "plain".into();
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let c = content_of(&export(&d).unwrap());
    for attr in ["style:text-combine", "style:text-rotation-angle", r#""zxx""#] {
        assert!(!c.contains(attr), "{attr} in {c}");
    }
}
