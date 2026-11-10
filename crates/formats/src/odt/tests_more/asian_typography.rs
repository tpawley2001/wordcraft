//! East Asian paragraph typography (Word's Paragraph › Asian Typography and Font alignment):
//! ODF 1.3 §20.305 `style:line-break`, §20.318 `style:punctuation-wrap`, §20.358
//! `style:text-autospace`, §20.324 `style:snap-to-layout-grid` and §20.386 `style:vertical-align`
//! of `style:paragraph-properties`, read into WordCraft's document and written back.

use crate::odt::tests::{NS_T, content_of, package};
use crate::odt::{export, import};
use wordcraft_doc::props::{ParaProps, TextAlignment};
use wordcraft_doc::{Document, Paragraph, para_block};

fn doc(para_props: &[&str]) -> Document {
    let styles: String = para_props
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                r#"<style:style style:name="P{i}" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties {p}/></style:style>"#
            )
        })
        .collect();
    let body: String = (0..para_props.len()).map(|i| format!(r#"<text:p text:style-name="P{i}">p{i}</text:p>"#)).collect();
    let content = format!(
        r#"<office:document-content {NS_T}><office:automatic-styles>{styles}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"#
    );
    import(&package(&content, None)).unwrap()
}

fn paras(d: &Document) -> Vec<&ParaProps> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| &p.props).collect()
}

/// (kinsoku, overflow_punct, auto_space_de, auto_space_dn, snap_to_grid, text_alignment)
type Asian = (Option<bool>, Option<bool>, Option<bool>, Option<bool>, Option<bool>, Option<TextAlignment>);

fn asian(p: &ParaProps) -> Asian {
    (p.kinsoku, p.overflow_punct, p.auto_space_de, p.auto_space_dn, p.snap_to_grid, p.text_alignment)
}

#[test]
fn asian_typography_attributes_are_read() {
    let d = doc(&[
        r#"style:line-break="strict""#,
        r#"style:line-break="normal""#,
        r#"style:punctuation-wrap="hanging""#,
        r#"style:punctuation-wrap="simple""#,
        r#"style:text-autospace="ideograph-alpha""#,
        r#"style:text-autospace="none""#,
        r#"style:snap-to-layout-grid="true""#,
        r#"style:snap-to-layout-grid="false""#,
        r#"style:vertical-align="top""#,
        r#"style:vertical-align="middle""#,
        r#"style:vertical-align="bottom""#,
        r#"style:vertical-align="baseline""#,
        r#"style:vertical-align="auto""#,
        r#"style:line-break=" STRICT ""#,
    ]);
    let got: Vec<Asian> = paras(&d).into_iter().map(asian).collect();
    let n = None;
    let want: Vec<Asian> = vec![
        (Some(true), n, n, n, n, None),
        (Some(false), n, n, n, n, None),
        (n, Some(true), n, n, n, None),
        (n, Some(false), n, n, n, None),
        (n, n, Some(true), Some(true), n, None),
        (n, n, Some(false), Some(false), n, None),
        (n, n, n, n, Some(true), None),
        (n, n, n, n, Some(false), None),
        (n, n, n, n, n, Some(TextAlignment::Top)),
        (n, n, n, n, n, Some(TextAlignment::Center)),
        (n, n, n, n, n, Some(TextAlignment::Bottom)),
        (n, n, n, n, n, Some(TextAlignment::Baseline)),
        (n, n, n, n, n, Some(TextAlignment::Auto)),
        (Some(true), n, n, n, n, None),
    ];
    assert_eq!(got, want);
}

#[test]
fn unstated_and_unknown_values_stay_unset() {
    let d = doc(&[
        "",
        r#"style:line-break="loose" style:punctuation-wrap="x" style:text-autospace="y" style:snap-to-layout-grid="maybe" style:vertical-align="sideways""#,
    ]);
    let none: Asian = (None, None, None, None, None, None);
    for (i, p) in paras(&d).into_iter().enumerate() {
        assert_eq!(asian(p), none, "paragraph {i}");
    }
    // Nothing stated in the model, nothing written.
    let mut plain = Document::new();
    let mut p = Paragraph::new();
    p.text = "plain".into();
    plain.body = vec![para_block(p)];
    let c = content_of(&export(&plain).unwrap());
    for attr in ["style:line-break", "style:punctuation-wrap", "style:text-autospace", "style:snap-to-layout-grid"] {
        assert!(!c.contains(attr), "{attr} in {c}");
    }
}

#[test]
fn a_parent_styles_values_reach_its_paragraphs() {
    let styles = r#"<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"><office:styles><style:style style:name="Asian" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties style:line-break="strict" style:punctuation-wrap="hanging" style:vertical-align="middle"/></style:style></office:styles></office:document-styles>"#;
    let content = format!(
        r#"<office:document-content {NS_T}><office:automatic-styles><style:style style:name="P1" style:family="paragraph" style:parent-style-name="Asian"><style:paragraph-properties style:punctuation-wrap="simple"/></style:style></office:automatic-styles><office:body><office:text><text:p text:style-name="P1">a</text:p></office:text></office:body></office:document-content>"#
    );
    let d = import(&package(&content, Some(styles))).unwrap();
    let p = paras(&d)[0];
    // The automatic style's own value wins; the rest comes from its parent.
    assert_eq!((p.kinsoku, p.overflow_punct, p.text_alignment), (Some(true), Some(false), Some(TextAlignment::Center)));
}

#[test]
fn asian_typography_round_trips_from_the_model() {
    let want = [
        ParaProps { kinsoku: Some(true), ..Default::default() },
        ParaProps { kinsoku: Some(false), ..Default::default() },
        ParaProps { overflow_punct: Some(true), ..Default::default() },
        ParaProps { overflow_punct: Some(false), ..Default::default() },
        ParaProps { auto_space_de: Some(true), auto_space_dn: Some(true), ..Default::default() },
        ParaProps { auto_space_de: Some(false), auto_space_dn: Some(false), ..Default::default() },
        ParaProps { snap_to_grid: Some(true), ..Default::default() },
        ParaProps { snap_to_grid: Some(false), ..Default::default() },
        ParaProps { text_alignment: Some(TextAlignment::Top), ..Default::default() },
        ParaProps { text_alignment: Some(TextAlignment::Center), ..Default::default() },
        ParaProps { text_alignment: Some(TextAlignment::Bottom), ..Default::default() },
        ParaProps { text_alignment: Some(TextAlignment::Baseline), ..Default::default() },
        ParaProps { text_alignment: Some(TextAlignment::Auto), ..Default::default() },
        ParaProps {
            kinsoku: Some(false),
            overflow_punct: Some(true),
            auto_space_de: Some(false),
            auto_space_dn: Some(false),
            snap_to_grid: Some(false),
            text_alignment: Some(TextAlignment::Bottom),
            ..Default::default()
        },
    ];
    let mut d = Document::new();
    d.body = want
        .iter()
        .enumerate()
        .map(|(i, props)| {
            let mut p = Paragraph::new();
            p.text = format!("p{i}");
            p.props = props.clone();
            para_block(p)
        })
        .collect();
    let saved = export(&d).unwrap();
    let c = content_of(&saved);
    for attr in [
        r#"style:line-break="strict""#,
        r#"style:line-break="normal""#,
        r#"style:punctuation-wrap="hanging""#,
        r#"style:punctuation-wrap="simple""#,
        r#"style:text-autospace="ideograph-alpha""#,
        r#"style:text-autospace="none""#,
        r#"style:snap-to-layout-grid="true""#,
        r#"style:snap-to-layout-grid="false""#,
        r#"style:vertical-align="middle""#,
        r#"style:vertical-align="baseline""#,
    ] {
        assert!(c.contains(attr), "{attr} in {c}");
    }
    let back = import(&saved).unwrap();
    let got: Vec<Asian> = paras(&back).into_iter().map(asian).collect();
    let want: Vec<Asian> = want.iter().map(asian).collect();
    assert_eq!(got, want);
}
