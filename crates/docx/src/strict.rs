//! ISO/IEC 29500 Strict → ECMA-376 Transitional, for what a save carries from a Strict file.
//!
//! WordCraft reads Strict packages (`http://purl.oclc.org/ooxml/...`) and writes its own parts in
//! Transitional namespaces. Parts it doesn't model are copied verbatim; left in Strict namespaces,
//! with Strict relationship types, they make a mixed package Word only opens after repair. So the
//! reader names relationships by their Transitional type ([`transitional_rel_type`]) and the
//! writer rewrites the namespace declarations of a kept part ([`to_transitional`]). Transitional
//! input passes through both unchanged.

use std::borrow::Cow;

/// Strict relationship types: `STRICT_REL/<name>`.
const STRICT_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships/";
/// Transitional relationship types: `TRANSITIONAL_REL/<name>`.
const TRANSITIONAL_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

/// Relationship type names that differ between Strict and Transitional; every other name is the same.
pub const REL_NAMES: &[(&str, &str)] =
    &[("extendedProperties", "extended-properties"), ("customProperties", "custom-properties"), ("officeDocument", "officeDocument")];

/// Strict namespace → Transitional namespace (ISO/IEC 29500-1 Strict vs ECMA-376 Transitional).
pub const NAMESPACES: &[(&str, &str)] = &[
    ("http://purl.oclc.org/ooxml/wordprocessingml/main", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"),
    ("http://purl.oclc.org/ooxml/officeDocument/relationships", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
    ("http://purl.oclc.org/ooxml/officeDocument/math", "http://schemas.openxmlformats.org/officeDocument/2006/math"),
    ("http://purl.oclc.org/ooxml/officeDocument/extendedProperties", "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"),
    ("http://purl.oclc.org/ooxml/officeDocument/customProperties", "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"),
    ("http://purl.oclc.org/ooxml/officeDocument/docPropsVTypes", "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"),
    ("http://purl.oclc.org/ooxml/officeDocument/sharedTypes", "http://schemas.openxmlformats.org/officeDocument/2006/sharedTypes"),
    ("http://purl.oclc.org/ooxml/officeDocument/customXml", "http://schemas.openxmlformats.org/officeDocument/2006/customXml"),
    ("http://purl.oclc.org/ooxml/officeDocument/bibliography", "http://schemas.openxmlformats.org/officeDocument/2006/bibliography"),
    ("http://purl.oclc.org/ooxml/drawingml/main", "http://schemas.openxmlformats.org/drawingml/2006/main"),
    ("http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
    ("http://purl.oclc.org/ooxml/drawingml/picture", "http://schemas.openxmlformats.org/drawingml/2006/picture"),
    ("http://purl.oclc.org/ooxml/drawingml/chart", "http://schemas.openxmlformats.org/drawingml/2006/chart"),
    ("http://purl.oclc.org/ooxml/drawingml/chartDrawing", "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing"),
    ("http://purl.oclc.org/ooxml/drawingml/diagram", "http://schemas.openxmlformats.org/drawingml/2006/diagram"),
    ("http://purl.oclc.org/ooxml/drawingml/lockedCanvas", "http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"),
    ("http://purl.oclc.org/ooxml/drawingml/compatibility", "http://schemas.openxmlformats.org/drawingml/2006/compatibility"),
    ("http://purl.oclc.org/ooxml/drawingml/spreadsheetDrawing", "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"),
    ("http://purl.oclc.org/ooxml/spreadsheetml/main", "http://schemas.openxmlformats.org/spreadsheetml/2006/main"),
    ("http://purl.oclc.org/ooxml/presentationml/main", "http://schemas.openxmlformats.org/presentationml/2006/main"),
];

/// What every Strict URI starts with: bytes without it need no rewriting.
const STRICT_MARK: &str = "purl.oclc.org/ooxml/";

/// The Transitional spelling of relationship type `t`: a Strict type
/// (`http://purl.oclc.org/ooxml/officeDocument/relationships/<name>`) maps by [`REL_NAMES`];
/// any other type (Transitional, Microsoft's, OPC's) is returned as it is.
pub fn transitional_rel_type(t: &str) -> Cow<'_, str> {
    let Some(name) = t.strip_prefix(STRICT_REL) else { return Cow::Borrowed(t) };
    let name = REL_NAMES.iter().find(|(s, _)| *s == name).map_or(name, |(_, tr)| tr);
    Cow::Owned(format!("{TRANSITIONAL_REL}{name}"))
}

/// Transitional spelling of a namespace URI, including `a:graphicData/@uri`.
/// Unknown and already Transitional values are borrowed unchanged.
pub fn transitional_uri(uri: &str) -> Cow<'_, str> {
    Cow::Borrowed(transitional_namespace(uri).unwrap_or(uri))
}

fn transitional_namespace(uri: &str) -> Option<&'static str> {
    NAMESPACES.iter().find(|(s, _)| *s == uri).map(|(_, t)| *t)
}

/// Part `bytes` (XML kept verbatim from a file) with Strict namespace declarations and
/// namespace `uri` attributes and relationship `Type` values in Transitional spelling. A root's
/// `conformance="strict"` is dropped. Only namespace declarations (`xmlns`, `xmlns:p`), `uri` and `Type` attributes are touched,
/// never text content. Borrowed (unchanged) when the bytes hold no Strict URI, or aren't
/// readable as markup.
pub fn to_transitional(bytes: &[u8]) -> Cow<'_, [u8]> {
    let Ok(s) = std::str::from_utf8(bytes) else { return Cow::Borrowed(bytes) };
    if !s.contains(STRICT_MARK) {
        return Cow::Borrowed(bytes);
    }
    match rewrite(s) {
        Some(out) if out != s => Cow::Owned(out.into_bytes()),
        _ => Cow::Borrowed(bytes),
    }
}

/// [`to_transitional`] on text. `None` on markup it can't follow (an unterminated tag or quote).
fn rewrite(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut root_seen = false;
    while let Some(lt) = rest.find('<') {
        out.push_str(rest.get(..lt)?);
        rest = rest.get(lt..)?;
        // Markup that holds no attributes is copied through whole.
        for (open, close) in [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>"), ("<!", ">")] {
            if rest.starts_with(open) {
                let end = rest.find(close)?.checked_add(close.len())?;
                out.push_str(rest.get(..end)?);
                rest = rest.get(end..)?;
                break;
            }
        }
        if !rest.starts_with('<') || rest.starts_with("<!") || rest.starts_with("<?") {
            continue;
        }
        if rest.starts_with("</") {
            let end = rest.find('>')?.checked_add(1)?;
            out.push_str(rest.get(..end)?);
            rest = rest.get(end..)?;
            continue;
        }
        let (tag, after) = start_tag(rest, !root_seen)?;
        root_seen = true;
        out.push_str(&tag);
        rest = after;
    }
    out.push_str(rest);
    Some(out)
}

/// The start tag at the beginning of `s` (which begins with `<`), rewritten, and what follows it.
fn start_tag(s: &str, root: bool) -> Option<(String, &str)> {
    let name_end = s.get(1..)?.find(|c: char| c.is_whitespace() || c == '>' || c == '/').map(|i| i + 1).unwrap_or(s.len());
    let mut out = String::from(s.get(..name_end)?);
    let mut rest = s.get(name_end..)?;
    loop {
        let trimmed = rest.trim_start();
        let ws = rest.get(..rest.len() - trimmed.len())?;
        if trimmed.starts_with('>') || trimmed.starts_with("/>") {
            let end = if trimmed.starts_with('>') { 1 } else { 2 };
            out.push_str(ws);
            out.push_str(trimmed.get(..end)?);
            return Some((out, trimmed.get(end..)?));
        }
        // name = "value"
        let eq = trimmed.find('=')?;
        let name = trimmed.get(..eq)?.trim_end();
        let after_eq = trimmed.get(eq + 1..)?;
        let value_start = after_eq.trim_start();
        let quote = value_start.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let value_end = value_start.get(1..)?.find(quote)?;
        let value = value_start.get(1..1 + value_end)?;
        let consumed = trimmed.len() - value_start.len() + value_end + 2;
        let attr = trimmed.get(..consumed)?;
        rest = trimmed.get(consumed..)?;
        if root && name == "conformance" && value == "strict" {
            continue;
        }
        let mapped = if name == "xmlns" || name.starts_with("xmlns:") || name == "uri" {
            transitional_namespace(value).map(Cow::Borrowed)
        } else if name == "Type" {
            Some(transitional_rel_type(value)).filter(|t| t.as_ref() != value)
        } else {
            None
        };
        out.push_str(ws);
        match mapped {
            Some(m) => {
                out.push_str(attr.get(..attr.len() - value.len() - 1)?);
                out.push_str(&m);
                out.push(quote);
            }
            None => out.push_str(attr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphic_namespace_uris_are_rewritten_but_text_and_other_attributes_are_not() {
        for (strict, transitional) in NAMESPACES {
            assert_eq!(transitional_uri(strict), *transitional);
            assert!(matches!(transitional_uri(transitional), Cow::Borrowed(_)));
            let xml = format!(r#"<a:graphicData uri='{strict}' label="{strict}">{strict}</a:graphicData>"#);
            let expected = format!(r#"<a:graphicData uri='{transitional}' label="{strict}">{strict}</a:graphicData>"#);
            assert_eq!(to_transitional(xml.as_bytes()).as_ref(), expected.as_bytes());
        }
        let unknown = "urn:example:graphic";
        assert_eq!(transitional_uri(unknown), unknown);
    }

    #[test]
    fn relationship_types() {
        let s = "http://purl.oclc.org/ooxml/officeDocument/relationships/";
        let t = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
        assert_eq!(transitional_rel_type(&format!("{s}extendedProperties")), format!("{t}extended-properties"));
        assert_eq!(transitional_rel_type(&format!("{s}customProperties")), format!("{t}custom-properties"));
        assert_eq!(transitional_rel_type(&format!("{s}webSettings")), format!("{t}webSettings"));
        let ms = "http://schemas.microsoft.com/office/2007/relationships/stylesWithEffects";
        assert!(matches!(transitional_rel_type(ms), Cow::Borrowed(x) if x == ms));
    }

    #[test]
    fn declarations_are_rewritten_but_text_is_not() {
        let xml = r#"<?xml version="1.0"?><!-- http://purl.oclc.org/ooxml/x --><w:settings xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main" conformance="strict" w:a='x'><w:t>http://purl.oclc.org/ooxml/wordprocessingml/main</w:t><w:e conformance="strict"/></w:settings>"#;
        let out = String::from_utf8(to_transitional(xml.as_bytes()).into_owned()).unwrap();
        assert_eq!(
            out,
            r#"<?xml version="1.0"?><!-- http://purl.oclc.org/ooxml/x --><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" w:a='x'><w:t>http://purl.oclc.org/ooxml/wordprocessingml/main</w:t><w:e conformance="strict"/></w:settings>"#
        );
    }

    #[test]
    fn transitional_and_hostile_input_is_unchanged() {
        let t = br#"<w:x xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
        assert!(matches!(to_transitional(t), Cow::Borrowed(_)));
        for junk in [
            &b"<a xmlns=\"http://purl.oclc.org/ooxml/drawingml/main"[..],
            b"<a b=c purl.oclc.org/ooxml/",
            b"<!-- purl.oclc.org/ooxml/",
            b"\xff\xfe<",
            b"<\xc3\xa9 x=\"purl.oclc.org/ooxml/\"",
        ] {
            let _ = to_transitional(junk);
        }
    }
}
