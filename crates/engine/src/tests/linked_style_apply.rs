//! A linked style (paragraph + character style joined by `w:link`, ECMA-376 §17.7.4.6) applied to
//! part of one paragraph formats only the selected text with its character half, as in Word; with
//! the Styles pane's Disable Linked Styles on it is always a paragraph style.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-linkedapply-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Quote / QuoteChar are a linked pair; Plain is a paragraph style with no link; Lonely links to a
/// character style that doesn't link back.
const STYLES: &str = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:customStyle="1" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:link w:val="QuoteChar"/><w:rPr><w:i/></w:rPr></w:style>
<w:style w:type="character" w:customStyle="1" w:styleId="QuoteChar"><w:name w:val="Quote Char"/><w:link w:val="Quote"/><w:rPr><w:i/></w:rPr></w:style>
<w:style w:type="paragraph" w:customStyle="1" w:styleId="Plain"><w:name w:val="Plain"/><w:basedOn w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:customStyle="1" w:styleId="Lonely"><w:name w:val="Lonely"/><w:link w:val="Other"/></w:style>
<w:style w:type="character" w:customStyle="1" w:styleId="Other"><w:name w:val="Other"/></w:style>
</w:styles>"#;

/// A session on two body paragraphs, "Alpha beta gamma" and "Delta epsilon", both Normal.
fn open(name: &str) -> Session {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/_rels/document.xml.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Alpha beta gamma</w:t></w:r></w:p><w:p><w:r><w:t>Delta epsilon</w:t></w:r></w:p></w:body></w:document></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/styles.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml">
    <pkg:xmlData>{STYLES}</pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let dir = temp_dir(name);
    let file = dir.join("in.xml");
    std::fs::write(&file, xml).unwrap();
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "file.open", json!({"path": p(&file)}));
    s
}

/// Each body paragraph's style (None = default) and its (text, character style) runs, merged
/// where neighbours share a character style.
fn paras(s: &Session) -> Vec<(Option<String>, Vec<(String, Option<String>)>)> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .map(|p| {
            let mut runs: Vec<(String, Option<String>)> = Vec::new();
            for (r, c) in p.run_ranges() {
                let text = p.text.get(r).unwrap_or_default().to_string();
                match runs.last_mut() {
                    Some(last) if last.1 == c.style => last.0.push_str(&text),
                    _ => runs.push((text, c.style.clone())),
                }
            }
            let style = p.props.style.clone().filter(|x| x != "Normal");
            (style, runs)
        })
        .collect()
}

fn plain(text: &str) -> Vec<(String, Option<String>)> {
    vec![(text.to_string(), None)]
}

#[test]
fn a_linked_style_on_part_of_a_paragraph_formats_only_the_selection() {
    let mut s = open("part");
    run(&mut s, "select.text", json!({"text": "beta"}));
    run(&mut s, "para.style", json!({"style": "Quote"}));
    assert_eq!(
        paras(&s),
        [(None, vec![("Alpha ".into(), None), ("beta".into(), Some("QuoteChar".into())), (" gamma".into(), None)]), (None, plain("Delta epsilon")),]
    );
    // One undo step puts it back.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(paras(&s), [(None, plain("Alpha beta gamma")), (None, plain("Delta epsilon"))]);
}

#[test]
fn a_linked_style_at_the_caret_or_on_a_whole_paragraph_is_a_paragraph_style() {
    // Caret: no selection.
    let mut s = open("caret");
    run(&mut s, "select.text", json!({"text": "beta"}));
    s.sel = crate::Selection::caret(s.sel.focus.clone());
    run(&mut s, "para.style", json!({"style": "Quote"}));
    assert_eq!(paras(&s), [(Some("Quote".into()), plain("Alpha beta gamma")), (None, plain("Delta epsilon"))]);

    // All of one paragraph's text, without its mark.
    let mut s = open("whole");
    run(&mut s, "select.text", json!({"text": "Delta epsilon"}));
    run(&mut s, "para.style", json!({"style": "Quote"}));
    assert_eq!(paras(&s), [(None, plain("Alpha beta gamma")), (Some("Quote".into()), plain("Delta epsilon"))]);
}

#[test]
fn a_linked_style_across_paragraphs_is_a_paragraph_style() {
    let mut s = open("across");
    run(&mut s, "select.text", json!({"text": "gamma"}));
    let start = s.sel.anchor.clone();
    run(&mut s, "select.text", json!({"text": "Delta"}));
    s.sel.anchor = start;
    run(&mut s, "para.style", json!({"style": "Quote"}));
    assert_eq!(paras(&s), [(Some("Quote".into()), plain("Alpha beta gamma")), (Some("Quote".into()), plain("Delta epsilon"))]);
}

#[test]
fn unlinked_styles_still_apply_to_the_paragraph() {
    for style in ["Plain", "Lonely"] {
        let mut s = open(&format!("unlinked-{style}"));
        run(&mut s, "select.text", json!({"text": "beta"}));
        run(&mut s, "para.style", json!({"style": style}));
        assert_eq!(paras(&s), [(Some(style.into()), plain("Alpha beta gamma")), (None, plain("Delta epsilon"))], "{style}");
    }
}

#[test]
fn disable_linked_styles_makes_them_paragraph_styles() {
    let mut s = open("disabled");
    assert_eq!(run(&mut s, "styles.disableLinked", json!({})), json!({"value": true}), "toggles on");
    run(&mut s, "select.text", json!({"text": "beta"}));
    run(&mut s, "para.style", json!({"style": "Quote"}));
    assert_eq!(paras(&s), [(Some("Quote".into()), plain("Alpha beta gamma")), (None, plain("Delta epsilon"))]);
    assert_eq!(run(&mut s, "styles.disableLinked", json!({"value": false})), json!({"value": false}));
    assert_eq!(run(&mut s, "styles.disableLinked", json!({"value": false})), json!({"value": false}), "set, not toggled");
}
