//! The latent-style table (RTF 1.9.1, "Style and Formatting Restrictions": `{\*\latentstyles …}`):
//! for the built-in styles a file does not define, whether each shows in the Styles gallery, its
//! priority and whether it is hidden until used. Word writes it into every RTF file; DOCX has the
//! same table as `w:latentStyles` (ECMA-376 Part 1 §17.7.4.5, §17.7.4.8), which the model keeps as
//! XML. Read into that XML, written back as Word writes it.

use super::*;
use std::collections::BTreeMap;
use wordcraft_doc::{Document, StoryRef};

type Attrs = BTreeMap<String, String>;

fn open(latent: &str) -> Document {
    let rtf = format!(
        r"{{\rtf1\ansi\ansicpg1252\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\stylesheet{{\s0 Normal;}}}}{latent}\pard\f0 Hello\par}}"
    );
    import(rtf.as_bytes()).unwrap()
}

fn unescape(v: &str) -> String {
    v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The attributes of every `<tag …>` start tag in `xml`, in document order.
fn elements(xml: &str, tag: &str) -> Vec<Attrs> {
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let end = after.find('>').unwrap_or(after.len());
        let (head, tail) = after.split_at(end);
        rest = tail;
        if !(head.is_empty() || head.starts_with([' ', '/'])) {
            continue;
        }
        let mut attrs = Attrs::new();
        let mut s = head;
        while let Some(eq) = s.find("=\"") {
            let key = s[..eq].trim().to_string();
            let value = &s[eq + 2..];
            let Some(close) = value.find('"') else { break };
            attrs.insert(key, unescape(&value[..close]));
            s = &value[close + 1..];
        }
        out.push(attrs);
    }
    out
}

fn attrs(pairs: &[(&str, &str)]) -> Attrs {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The table's own attributes and its exceptions, from the model's `w:latentStyles` XML.
fn table(d: &Document) -> (Attrs, Vec<Attrs>) {
    let xml = d.styles.latent_styles.as_deref().expect("latent styles kept");
    let roots = elements(xml, "w:latentStyles");
    assert_eq!(roots.len(), 1, "{xml}");
    assert!(xml.starts_with("<w:latentStyles") && (xml.ends_with("</w:latentStyles>") || xml.ends_with("/>")), "{xml}");
    (roots.into_iter().next().unwrap(), elements(xml, "w:lsdException"))
}

/// A document whose model holds `xml` as its latent-style table (as the DOCX reader leaves it).
fn with_table(xml: &str) -> Document {
    let mut d = open("");
    std::sync::Arc::make_mut(&mut d.styles).latent_styles = Some(xml.to_string());
    d
}

/// The exported file without its line breaks (Word and the exporter may break lines anywhere).
fn flat(d: &Document) -> String {
    export(d).replace(['\r', '\n'], "")
}

const WORD_2003: &str = r"{\*\latentstyles\lsdstimax156\lsdlockeddef0}";
const WORD_2007: &str = concat!(
    r"{\*\latentstyles\lsdstimax267\lsdlockeddef0\lsdsemihiddendef1\lsdunhideuseddef1\lsdqformatdef0\lsdprioritydef99",
    r"{\lsdlockedexcept \lsdsemihidden0 \lsdunhideused0 \lsdqformat1 \lsdpriority0 \lsdlocked0 Normal;",
    r"\lsdqformat1 \lsdpriority9 \lsdlocked0 heading 2;\lsdpriority39 \lsdlocked0 toc 1;}}"
);

#[test]
fn a_file_without_the_table_has_none() {
    let d = open("");
    assert_eq!(d.styles.latent_styles, None);
    assert!(!export(&d).contains("latentstyles"));
}

#[test]
fn the_word_2003_table_keeps_its_two_defaults() {
    let d = open(WORD_2003);
    let (root, exceptions) = table(&d);
    assert_eq!(root, attrs(&[("w:count", "156"), ("w:defLockedState", "0")]));
    assert!(exceptions.is_empty(), "{exceptions:?}");
    assert_eq!(d.plain_text(StoryRef::Body).trim_end(), "Hello");
}

#[test]
fn the_word_2007_table_keeps_its_defaults_and_exceptions() {
    let d = open(WORD_2007);
    let (root, exceptions) = table(&d);
    assert_eq!(
        root,
        attrs(&[
            ("w:count", "267"),
            ("w:defLockedState", "0"),
            ("w:defSemiHidden", "1"),
            ("w:defUnhideWhenUsed", "1"),
            ("w:defQFormat", "0"),
            ("w:defUIPriority", "99"),
        ])
    );
    assert_eq!(
        exceptions,
        [
            attrs(&[("w:name", "Normal"), ("w:semiHidden", "0"), ("w:unhideWhenUsed", "0"), ("w:qFormat", "1"), ("w:uiPriority", "0"), ("w:locked", "0")]),
            attrs(&[("w:name", "heading 2"), ("w:qFormat", "1"), ("w:uiPriority", "9"), ("w:locked", "0")]),
            attrs(&[("w:name", "toc 1"), ("w:uiPriority", "39"), ("w:locked", "0")]),
        ]
    );
    assert_eq!(d.plain_text(StoryRef::Body).trim_end(), "Hello");
}

#[test]
fn line_breaks_inside_the_table_are_not_content() {
    let broken = WORD_2007.replace(r"\lsdqformat1 \lsdpriority9", "\r\n\\lsdqformat1 \\lsdpriority9").replace("heading 2;", "heading 2;\r\n");
    assert_eq!(table(&open(&broken)), table(&open(WORD_2007)));
}

#[test]
fn odd_values_are_normalised_or_dropped() {
    let d = open(r"{\*\latentstyles\lsdstimax10\lsdlockeddef0{\lsdlockedexcept \lsdqformat5 \lsdpriority-3 \lsdlocked0 Odd One;\lsdpriority1 \lsdlocked0 ;}}");
    let (_, exceptions) = table(&d);
    assert_eq!(exceptions, [attrs(&[("w:name", "Odd One"), ("w:qFormat", "1"), ("w:locked", "0")])]);
}

#[test]
fn a_name_with_markup_characters_is_escaped_in_the_xml() {
    let d = open(r"{\*\latentstyles\lsdstimax10\lsdlockeddef0{\lsdlockedexcept \lsdpriority1 \lsdlocked0 R&D <x>;}}");
    let xml = d.styles.latent_styles.clone().unwrap();
    assert!(xml.contains("R&amp;D &lt;x&gt;"), "{xml}");
    assert_eq!(table(&d).1[0]["w:name"], "R&D <x>");
}

#[test]
fn the_exceptions_are_capped() {
    let many: String = (0..2100).map(|i| format!(r"\lsdpriority1 \lsdlocked0 S{i};")).collect();
    let d = open(&format!(r"{{\*\latentstyles\lsdstimax2100\lsdlockeddef0{{\lsdlockedexcept {many}}}}}"));
    let (_, exceptions) = table(&d);
    assert_eq!(exceptions.len(), 2000);
    assert_eq!(exceptions[0]["w:name"], "S0");
    assert_eq!(exceptions[1999]["w:name"], "S1999");
    assert_eq!(d.plain_text(StoryRef::Body).trim_end(), "Hello");
}

#[test]
fn the_word_tables_are_written_back_as_they_were() {
    for latent in [WORD_2003, WORD_2007] {
        let out = flat(&open(latent));
        assert!(out.contains(latent), "{latent}\nnot in\n{out}");
        assert_eq!(out.matches("latentstyles").count(), 1, "{out}");
        assert!(out.find(r"{\*\latentstyles").unwrap() > out.find(r"{\stylesheet").expect("stylesheet"), "{out}");
    }
}

#[test]
fn a_docx_table_is_written_as_word_writes_it() {
    let d = with_table(concat!(
        r#"<w:latentStyles w:defLockedState="0" w:defUIPriority="99" w:defSemiHidden="0" w:defUnhideWhenUsed="0" w:defQFormat="0" w:count="376">"#,
        r#"<w:lsdException w:name="Normal" w:uiPriority="0" w:qFormat="1"/>"#,
        r#"<w:lsdException w:name="heading 2" w:semiHidden="1" w:uiPriority="9" w:unhideWhenUsed="1" w:qFormat="1"/>"#,
        r#"<w:lsdException w:name="R&amp;D" w:uiPriority="5" w:locked="true"/>"#,
        r#"</w:latentStyles>"#
        ));
    let out = flat(&d);
    let want = concat!(
        r"{\*\latentstyles\lsdstimax376\lsdlockeddef0\lsdsemihiddendef0\lsdunhideuseddef0\lsdqformatdef0\lsdprioritydef99",
        r"{\lsdlockedexcept \lsdqformat1 \lsdpriority0 \lsdlocked0 Normal;",
        r"\lsdsemihidden1 \lsdunhideused1 \lsdqformat1 \lsdpriority9 \lsdlocked0 heading 2;",
        r"\lsdpriority5 \lsdlocked1 R&D;}}"
    );
    assert!(out.contains(want), "{want}\nnot in\n{out}");
    let back = import(export(&d).as_bytes()).unwrap();
    let (root, exceptions) = table(&back);
    assert_eq!(root["w:count"], "376");
    assert_eq!(exceptions.len(), 3);
    assert_eq!(exceptions[2], attrs(&[("w:name", "R&D"), ("w:uiPriority", "5"), ("w:locked", "1")]));
}

#[test]
fn a_name_outside_ascii_survives_a_save() {
    let d = with_table(r#"<w:latentStyles w:defLockedState="0" w:count="1"><w:lsdException w:name="Caf&#233; Style" w:uiPriority="7"/></w:latentStyles>"#);
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(table(&back).1, [attrs(&[("w:name", "Caf\u{e9} Style"), ("w:uiPriority", "7"), ("w:locked", "0")])]);
}

#[test]
fn xml_that_is_not_a_latent_style_table_writes_nothing() {
    for bad in ["", "garbage", "<w:styles/>", "<w:latentStyles w:count=\"3\"><w:lsdException"] {
        let d = with_table(bad);
        let out = export(&d);
        assert!(!out.contains("latentstyles") && !out.contains("lsdlockedexcept"), "{bad:?}: {out}");
        assert_eq!(import(out.as_bytes()).unwrap().plain_text(StoryRef::Body).trim_end(), "Hello");
    }
}
