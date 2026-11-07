//! Word's Selection Pane (Layout › Arrange › Selection Pane, Alt+F10): the body's drawings listed by
//! their `wp:docPr/@name` (ECMA-376 Part 1 §20.4.2.5), front-most first; select, show/hide and rename
//! from the list. Synthetic DOCX fixture: two anchored rectangles.

use std::io::Write;

use serde_json::{Value, json};

use crate::{Session, cmd};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape""#;

fn rect(id: u32, name: &str, z: u32, hidden: bool) -> String {
    let hidden = if hidden { r#" hidden="1""# } else { "" };
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{z}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="914400" cy="457200"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"{hidden}/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#,
        x = id * 1_000_000
    )
}

fn docx() -> Vec<u8> {
    let doc = format!(
        r#"<w:document {NS}><w:body><w:p>{}{}<w:r><w:t>Text</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        rect(1, "Logo Box", 2, false),
        rect(2, "Second Shape", 3, true)
    );
    let files = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string(),
        ),
        ("word/document.xml", doc),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn session() -> Session {
    Session::new(wordcraft_docx::read(&docx()).expect("read"))
}

fn pane(s: &mut Session) -> Vec<Value> {
    s.run("arrange.selectionPane", &json!({})).expect("selectionPane").as_array().cloned().unwrap_or_default()
}

fn names(s: &mut Session) -> Vec<String> {
    pane(s).iter().map(|e| e["name"].as_str().unwrap_or("").to_string()).collect()
}

fn hidden(s: &mut Session) -> Vec<bool> {
    pane(s).iter().map(|e| e["hidden"].as_bool().unwrap_or(false)).collect()
}

#[test]
fn the_pane_lists_the_file_names_front_first() {
    let mut s = session();
    // "Second Shape" has the higher relativeHeight, so it is in front and listed first.
    assert_eq!(names(&mut s), ["Second Shape", "Logo Box"]);
    assert_eq!(hidden(&mut s), [true, false]);
    let idx: Vec<u64> = pane(&mut s).iter().filter_map(|e| e["index"].as_u64()).collect();
    assert_eq!(idx, [0, 1], "index is the position in the list");
}

#[test]
fn names_are_kept_on_save() {
    let s = session();
    let bytes = crate::io::save_bytes("p.docx", &s.doc).expect("save");
    let mut back = Session::new(wordcraft_docx::read(&bytes).expect("reopen"));
    assert_eq!(names(&mut back), ["Second Shape", "Logo Box"]);
    assert_eq!(hidden(&mut back), [true, false]);
}

#[test]
fn select_object_selects_the_listed_object() {
    let mut s = session();
    s.run("arrange.selectObject", &json!({"index": 1})).expect("selectObject");
    let sel: Vec<bool> = pane(&mut s).iter().map(|e| e["selected"].as_bool().unwrap_or(false)).collect();
    assert_eq!(sel, [false, true]);
    s.run("arrange.selectObject", &json!({"index": 0})).expect("selectObject");
    let sel: Vec<bool> = pane(&mut s).iter().map(|e| e["selected"].as_bool().unwrap_or(false)).collect();
    assert_eq!(sel, [true, false]);
    assert!(s.run("arrange.selectObject", &json!({"index": 2})).is_err(), "out of range");
}

#[test]
fn visibility_is_undoable_and_show_hide_all_cover_every_object() {
    let mut s = session();
    s.run("arrange.setVisible", &json!({"index": 1, "visible": false})).expect("setVisible");
    assert_eq!(hidden(&mut s), [true, true]);
    s.run("edit.undo", &json!({})).expect("undo");
    assert_eq!(hidden(&mut s), [true, false]);
    s.run("arrange.showAll", &json!({})).expect("showAll");
    assert_eq!(hidden(&mut s), [false, false]);
    s.run("arrange.hideAll", &json!({})).expect("hideAll");
    assert_eq!(hidden(&mut s), [true, true]);
    s.run("edit.undo", &json!({})).expect("undo");
    assert_eq!(hidden(&mut s), [false, false], "Hide All is one undo step");
    assert!(s.run("arrange.setVisible", &json!({"index": 5, "visible": true})).is_err());
}

#[test]
fn rename_is_undoable_and_saved() {
    let mut s = session();
    s.run("arrange.rename", &json!({"index": 1, "name": "  Company logo "})).expect("rename");
    assert_eq!(names(&mut s), ["Second Shape", "Company logo"]);
    let bytes = crate::io::save_bytes("p.docx", &s.doc).expect("save");
    let mut back = Session::new(wordcraft_docx::read(&bytes).expect("reopen"));
    assert_eq!(names(&mut back), ["Second Shape", "Company logo"]);
    s.run("edit.undo", &json!({})).expect("undo");
    assert_eq!(names(&mut s), ["Second Shape", "Logo Box"]);
    assert!(s.run("arrange.rename", &json!({"index": 0, "name": "   "})).is_err(), "empty name");
}

#[test]
fn alt_f10_toggles_the_pane() {
    let k = cmd::registry().by_shortcut("Alt+F10").map(|c| c.id);
    if cfg!(target_os = "macos") {
        assert_ne!(k, Some("view.selectionPane"));
    } else {
        assert_eq!(k, Some("view.selectionPane"));
    }
    let mut s = session();
    s.run("view.selectionPane", &json!({})).expect("view.selectionPane");
    assert_eq!(names(&mut s), ["Second Shape", "Logo Box"], "opening the pane changes no object");
}
