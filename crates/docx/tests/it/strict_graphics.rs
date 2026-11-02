//! Charts and SmartArt diagrams in a Strict Open XML file (ISO/IEC 29500 Strict, `http://purl.oclc.org/ooxml/...`)
//! keep their graphic frame on save: `a:graphicData` gets the Transitional `uri` and keeps its `c:chart` or
//! `dgm:relIds` child, whose relationship ids resolve to parts in the package (#3548). Hand-written fixtures.

use std::collections::BTreeMap;
use std::io::{Read, Write};

const S_W: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
const S_R: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";
const S_WP: &str = "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing";
const S_A: &str = "http://purl.oclc.org/ooxml/drawingml/main";
const S_C: &str = "http://purl.oclc.org/ooxml/drawingml/chart";
const S_DGM: &str = "http://purl.oclc.org/ooxml/drawingml/diagram";
const T_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const T_CHART_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const T_DIAGRAM_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";

fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, s) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(s.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        out.insert(f.name().to_string(), b);
    }
    out
}

fn text(files: &BTreeMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8_lossy(files.get(name).unwrap_or_else(|| panic!("no {name} in {:?}", files.keys()))).into_owned()
}

fn rels(list: &[(&str, String, &str)]) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (id, ty, target) in list {
        s.push_str(&format!(r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"/>"#));
    }
    s.push_str("</Relationships>");
    s
}

/// The value of attribute `name` in the first start tag that begins with `tag` (e.g. `<c:chart`).
fn attr_of(xml: &str, tag: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("{tag} ")).or_else(|| xml.find(&format!("{tag}>")))?;
    let end = start + xml[start..].find('>')?;
    let el = &xml[start..end];
    let key = format!(" {name}=\"");
    let at = el.find(&key)? + key.len();
    Some(el[at..at + el[at..].find('"')?].to_string())
}

/// (type, target) of relationship `id` in `word/_rels/document.xml.rels`.
fn rel(files: &BTreeMap<String, Vec<u8>>, id: &str) -> (String, String) {
    let r = text(files, "word/_rels/document.xml.rels");
    let at = r.find(&format!("Id=\"{id}\"")).unwrap_or_else(|| panic!("no relationship {id} in {r}"));
    let start = r[..at].rfind('<').unwrap();
    let el = &r[start..start + r[start..].find('>').unwrap()];
    let get = |k: &str| {
        let key = format!("{k}=\"");
        let a = el.find(&key).unwrap() + key.len();
        el[a..a + el[a..].find('"').unwrap()].to_string()
    };
    (get("Type"), get("Target"))
}

fn assert_resolves(files: &BTreeMap<String, Vec<u8>>, id: &str, kind: &str) {
    let (ty, target) = rel(files, id);
    assert_eq!(ty, format!("{T_REL}/{kind}"), "relationship {id}");
    let part = format!("word/{}", target.trim_start_matches('/').trim_start_matches("word/"));
    assert!(files.contains_key(&part), "{id} -> {part} is not in the package: {:?}", files.keys());
}

fn assert_no_strict_uri(files: &BTreeMap<String, Vec<u8>>) {
    for (name, bytes) in files {
        let s = String::from_utf8_lossy(bytes);
        assert!(!s.contains("purl.oclc.org/ooxml"), "{name} still has a Strict URI:\n{s}");
    }
}

fn ct(extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>{extra}</Types>"#
    )
}

/// A Strict package whose only paragraph holds one inline graphic frame with `graphic_data`.
fn package(graphic_data: &str, extra_ct: &str, extra_rels: &[(&str, String, &str)], parts: Vec<(&str, String)>) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{S_W}" xmlns:r="{S_R}" xmlns:wp="{S_WP}" xmlns:a="{S_A}" conformance="strict"><w:body><w:p><w:r><w:t>Before</w:t></w:r><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3657600" cy="2286000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="Graphic 1"/><wp:cNvGraphicFramePr/><a:graphic>{graphic_data}</a:graphic></wp:inline></w:drawing></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{S_W}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#
    );
    let mut doc_rels = vec![("rId1", format!("{S_R}/styles"), "styles.xml")];
    doc_rels.extend(extra_rels.iter().cloned());
    let mut entries = vec![
        ("[Content_Types].xml", ct(extra_ct)),
        ("_rels/.rels", rels(&[("rId1", format!("{S_R}/officeDocument"), "word/document.xml")])),
        ("word/document.xml", doc),
        ("word/_rels/document.xml.rels", rels(&doc_rels)),
        ("word/styles.xml", styles),
    ];
    entries.extend(parts);
    zip(&entries)
}

fn chart_package() -> Vec<u8> {
    let gd = format!(r#"<a:graphicData uri="{S_C}"><c:chart xmlns:c="{S_C}" r:id="rId9"/></a:graphicData>"#);
    let chart = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="{S_C}" xmlns:a="{S_A}" xmlns:r="{S_R}"><c:chart><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>4</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:crossAx val="1"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#
    );
    package(
        &gd,
        r#"<Override PartName="/word/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>"#,
        &[("rId9", format!("{S_R}/chart"), "charts/chart1.xml")],
        vec![("word/charts/chart1.xml", chart)],
    )
}

fn diagram_package() -> Vec<u8> {
    let gd = format!(
        r#"<a:graphicData uri="{S_DGM}"><dgm:relIds xmlns:dgm="{S_DGM}" r:dm="rId11" r:lo="rId12" r:qs="rId13" r:cs="rId14"/></a:graphicData>"#
    );
    let head = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
    let data = format!(
        r#"{head}<dgm:dataModel xmlns:dgm="{S_DGM}" xmlns:a="{S_A}"><dgm:ptLst><dgm:pt modelId="0" type="doc"/><dgm:pt modelId="1"><dgm:t><a:bodyPr/><a:p><a:r><a:t>Step</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst><dgm:cxnLst><dgm:cxn modelId="2" srcId="0" destId="1" srcOrd="0" destOrd="0"/></dgm:cxnLst></dgm:dataModel>"#
    );
    let layout = format!(
        r#"{head}<dgm:layoutDef xmlns:dgm="{S_DGM}" uniqueId="urn:microsoft.com/office/officeart/2005/8/layout/default"><dgm:layoutNode name="diagram"/></dgm:layoutDef>"#
    );
    let style = format!(r#"{head}<dgm:styleDef xmlns:dgm="{S_DGM}" uniqueId="urn:microsoft.com/office/officeart/2005/8/quickstyle/simple1"/>"#);
    let colors = format!(r#"{head}<dgm:colorsDef xmlns:dgm="{S_DGM}" uniqueId="urn:microsoft.com/office/officeart/2005/8/colors/accent1_2"/>"#);
    package(
        &gd,
        r#"<Override PartName="/word/diagrams/data1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml"/><Override PartName="/word/diagrams/layout1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml"/><Override PartName="/word/diagrams/quickStyle1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml"/><Override PartName="/word/diagrams/colors1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramColors+xml"/>"#,
        &[
            ("rId11", format!("{S_R}/diagramData"), "diagrams/data1.xml"),
            ("rId12", format!("{S_R}/diagramLayout"), "diagrams/layout1.xml"),
            ("rId13", format!("{S_R}/diagramQuickStyle"), "diagrams/quickStyle1.xml"),
            ("rId14", format!("{S_R}/diagramColors"), "diagrams/colors1.xml"),
        ],
        vec![
            ("word/diagrams/data1.xml", data),
            ("word/diagrams/layout1.xml", layout),
            ("word/diagrams/quickStyle1.xml", style),
            ("word/diagrams/colors1.xml", colors),
        ],
    )
}

fn save(bytes: &[u8]) -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let doc = wordcraft_docx::read(bytes).expect("read");
    let out = wordcraft_docx::write(&doc).expect("write");
    let files = unzip(&out);
    (out, files)
}

fn assert_chart_kept(files: &BTreeMap<String, Vec<u8>>) {
    let doc = text(files, "word/document.xml");
    assert_eq!(attr_of(&doc, "<a:graphicData", "uri").as_deref(), Some(T_CHART_URI), "graphicData uri in:\n{doc}");
    let id = attr_of(&doc, "<c:chart", "r:id").unwrap_or_else(|| panic!("no c:chart r:id in the graphic frame:\n{doc}"));
    assert_resolves(files, &id, "chart");
}

#[test]
fn a_strict_chart_keeps_its_graphic_frame_and_chart_part() {
    let (_, files) = save(&chart_package());
    assert_chart_kept(&files);
    assert_no_strict_uri(&files);
}

#[test]
fn a_strict_chart_survives_a_second_save() {
    let (once, _) = save(&chart_package());
    let (_, files) = save(&once);
    assert_chart_kept(&files);
    assert_no_strict_uri(&files);
}

#[test]
fn a_strict_smartart_diagram_keeps_its_relationship_ids() {
    let (_, files) = save(&diagram_package());
    let doc = text(&files, "word/document.xml");
    assert_eq!(attr_of(&doc, "<a:graphicData", "uri").as_deref(), Some(T_DIAGRAM_URI), "graphicData uri in:\n{doc}");
    for (attr, kind) in [("r:dm", "diagramData"), ("r:lo", "diagramLayout"), ("r:qs", "diagramQuickStyle"), ("r:cs", "diagramColors")] {
        let id = attr_of(&doc, "<dgm:relIds", attr).unwrap_or_else(|| panic!("no dgm:relIds {attr} in:\n{doc}"));
        assert_resolves(&files, &id, kind);
    }
    assert_no_strict_uri(&files);
}
