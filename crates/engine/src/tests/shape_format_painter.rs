//! Format Painter and Copy/Paste Formatting (Ctrl+Shift+C / Ctrl+Shift+V) with a shape selected:
//! Word copies the shape's fill, outline and effects onto the next selected shape, leaving its
//! size, position and name as they were. Synthetic DOCX fixture: two anchored rectangles.

use std::io::Write;

use serde_json::{Value, json};
use wordcraft_doc::InlineObject;

use crate::Session;

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape""#;

fn rect(id: u32, name: &str, z: u32) -> String {
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{z}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="914400" cy="457200"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#,
        x = id * 1_500_000
    )
}

fn session() -> Session {
    let doc = format!(
        r#"<w:document {NS}><w:body><w:p>{}{}<w:r><w:t>Text</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        rect(1, "Shape A", 2),
        rect(2, "Shape B", 3)
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
    Session::new(wordcraft_docx::read(&zw.finish().unwrap().into_inner()).expect("read"))
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Select the drawing the Selection Pane lists under `name`.
fn pick(s: &mut Session, name: &str) {
    let pane = run(s, "arrange.selectionPane", json!({}));
    let index = pane.as_array().unwrap().iter().position(|e| e["name"] == json!(name)).unwrap_or_else(|| panic!("{name} in {pane}"));
    run(s, "arrange.selectObject", json!({"index": index}));
}

/// The selected shape.
fn shape(s: &Session) -> InlineObject {
    let (_, o) = crate::cmd::objects::selected(s).expect("a shape is selected");
    assert!(matches!(o, InlineObject::Shape { .. }), "{o:?}");
    o
}

/// Give Shape A a red fill, a 3 pt blue outline and a 5 pt glow.
fn format_a(s: &mut Session) {
    pick(s, "Shape A");
    run(s, "shape.fill", json!({"color": "FF0000"}));
    run(s, "shape.outline", json!({"color": "0000FF", "width": 3.0}));
    run(s, "shape.effects", json!({"glow": 5.0}));
}

#[test]
fn copy_and_paste_formatting_carry_fill_outline_and_effects() {
    let mut s = session();
    format_a(&mut s);
    let a = shape(&s);
    pick(&mut s, "Shape B");
    let before = shape(&s);
    pick(&mut s, "Shape A");
    run(&mut s, "edit.copyFormat", json!({}));
    pick(&mut s, "Shape B");
    run(&mut s, "edit.pasteFormat", json!({}));
    let b = shape(&s);
    let (
        InlineObject::Shape { fill: af, stroke: ast, stroke_width: aw, effects: ae, .. },
        InlineObject::Shape { fill: bf, stroke: bst, stroke_width: bw, effects: be, w, h, float, .. },
        InlineObject::Shape { w: w0, h: h0, float: float0, .. },
    ) = (&a, &b, &before)
    else {
        unreachable!()
    };
    assert!(af.is_some(), "Shape A has its fill");
    assert_eq!(bf, af, "fill");
    assert_eq!(bst, ast, "outline colour");
    assert_eq!(*bw, 3.0, "outline width");
    assert_eq!(be, ae, "effects");
    assert_eq!((w, h), (w0, h0), "size unchanged");
    assert_eq!((float.x, float.y, &float.name), (float0.x, float0.y, &float0.name), "position and name unchanged");

    run(&mut s, "edit.undo", json!({}));
    pick(&mut s, "Shape B");
    let undone = shape(&s);
    let (
        InlineObject::Shape { fill, stroke, stroke_width, effects, .. },
        InlineObject::Shape { fill: f0, stroke: s0, stroke_width: w0, effects: e0, .. },
    ) = (&undone, &before)
    else {
        unreachable!()
    };
    assert_eq!((fill, stroke, stroke_width, effects), (f0, s0, w0, e0), "one undo step restores Shape B");
}

#[test]
fn the_one_shot_painter_ends_after_painting_a_shape() {
    let mut s = session();
    format_a(&mut s);
    pick(&mut s, "Shape A");
    assert_eq!(run(&mut s, "edit.formatPainter", json!({}))["active"], json!(true));
    pick(&mut s, "Shape B");
    run(&mut s, "edit.pasteFormat", json!({}));
    let InlineObject::Shape { fill, .. } = shape(&s) else { unreachable!() };
    assert!(fill.is_some(), "Shape B was painted");
    assert_eq!(run(&mut s, "edit.formatPainter", json!({}))["active"], json!(true), "the painter had ended, so this starts a new one");
}

#[test]
fn shape_format_does_not_paint_text() {
    let mut s = session();
    format_a(&mut s);
    pick(&mut s, "Shape A");
    run(&mut s, "edit.copyFormat", json!({}));
    run(&mut s, "select.text", json!({"text": "Text"}));
    let rev = s.rev();
    assert!(s.run("edit.pasteFormat", &json!({})).is_err(), "select a shape to paste its formatting");
    assert_eq!(s.rev(), rev, "nothing changed");
}
