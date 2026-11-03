//! Word 2007 WordArt shadows beyond a single offset copy (ECMA-376 Part 4 §19.1.2.18 `v:shadow`
//! `type`): `double`, `emboss` and `perspective` keep a shadow whose first copy is the `offset`
//! one, in `color` at `opacity`, crisp. Fixtures are hand-written; the attribute values are the
//! ones Word 2007 writes for its shadow styles.

use std::io::Write;

use wordcraft_doc::effects::Shadow;
use wordcraft_doc::{Document, InlineObject, Rgb};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A document whose body is one paragraph holding `shape` in a `w:pict`.
fn read_shape(shape: &str) -> Document {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body><w:p><w:r><w:pict>{shape}</w:pict></w:r></w:p></w:body></w:document>"#
    );
    wordcraft_docx::read(&zip(&[("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())])).unwrap()
}

/// The first shadow copy of a WordArt shape with shape attributes `attrs` and children `children`.
fn shadow_of(attrs: &str, children: &str) -> Option<Shadow> {
    let shape = format!(
        r##"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t136" style="position:absolute;margin-left:28pt;margin-top:28pt;width:242.25pt;height:41.25pt" {attrs}>{children}<v:textpath style="font-family:&quot;Arial&quot;;v-text-kern:t" trim="t" fitpath="t" string="Your Text Here"/></v:shape>"##
    );
    let d = read_shape(&shape);
    let p = d.body.iter().find_map(|b| b.as_para()).expect("paragraph");
    let Some(InlineObject::Shape { story: Some(id), .. }) = p.objects.first() else { panic!("no WordArt shape: {:?}", p.objects) };
    let run = &d.parts[id].blocks[0].as_para().unwrap().runs[0];
    run.props.text_effects.as_deref().and_then(|fx| fx.shadow)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// `double`: the first copy is the `offset` one in `color` (Word 2007's "Shadow Style 4").
#[test]
fn double_keeps_its_offset_copy() {
    let s = shadow_of(
        "",
        r##"<v:shadow on="t" type="double" color="#868686" opacity=".5" color2="shadow add(102)" offset="-3pt,-3pt" offset2="-6pt,-6pt"/>"##,
    )
    .expect("double shadow");
    assert_eq!(s.color, Rgb(0x86, 0x86, 0x86));
    assert!(close(s.transparency, 50.0), "{s:?}");
    assert!(close(s.blur, 0.0), "{s:?}");
    assert!(close(s.distance, 4.242_641), "{s:?}");
    assert!(close(s.angle, 225.0), "{s:?}");
}

/// `emboss`: the dark copy at `offset`; its colour "lineOrFill darken(153)" is the letters' line
/// colour (their fill colour when not stroked) scaled by 153/255.
#[test]
fn emboss_keeps_its_dark_copy() {
    let children = r#"<v:shadow on="t" type="emboss" color="lineOrFill darken(153)" color2="shadow add(102)" offset="1pt,1pt"/>"#;
    // Not stroked, default white fill: 255 * 153 / 255 = 153.
    let s = shadow_of(r#"stroked="f""#, children).expect("emboss shadow");
    assert_eq!(s.color, Rgb(153, 153, 153));
    assert!(close(s.transparency, 0.0), "{s:?}");
    assert!(close(s.distance, std::f32::consts::SQRT_2), "{s:?}");
    assert!(close(s.angle, 45.0), "{s:?}");
    // Stroked in #FF0000: line colour wins, (153, 0, 0).
    let s = shadow_of(r##"strokecolor="#ff0000""##, children).expect("emboss shadow, stroked");
    assert_eq!(s.color, Rgb(153, 0, 0));
}

/// `perspective`: the first copy keeps its offset, colour and opacity (Word 2007's "Shadow Style 7").
#[test]
fn perspective_keeps_its_copy() {
    let s = shadow_of(
        "",
        r##"<v:shadow on="t" type="perspective" color="#868686" opacity=".5" origin="-.5,.5" offset="-6pt,-6pt" matrix=",-56756f,,.5"/>"##,
    )
    .expect("perspective shadow");
    assert_eq!(s.color, Rgb(0x86, 0x86, 0x86));
    assert!(close(s.transparency, 50.0), "{s:?}");
    assert!(close(s.blur, 0.0), "{s:?}");
    assert!(close(s.distance, 8.485_281), "{s:?}");
    assert!(close(s.angle, 225.0), "{s:?}");
}

/// Unknown types and shadows that are off still give none; hostile values stay finite.
#[test]
fn unknown_and_hostile() {
    assert_eq!(shadow_of("", r##"<v:shadow on="t" type="sideways" color="#868686" offset="6pt,6pt"/>"##), None);
    assert_eq!(shadow_of("", r##"<v:shadow on="f" type="double" color="#868686" offset="6pt,6pt"/>"##), None);
    for ty in ["double", "emboss", "perspective"] {
        let children = format!(
            r#"<v:shadow on="t" type="{ty}" opacity="NaN" color2="zz" offset="1e39pt,NaN" offset2="NaN,1e40pt" origin="NaN,9" matrix="zzf,1e40,NaN,,,"/>"#
        );
        if let Some(s) = shadow_of("", &children) {
            assert!(s.distance.is_finite() && s.angle.is_finite() && s.transparency.is_finite(), "{ty}: {s:?}");
            assert!((0.0..=100.0).contains(&s.transparency), "{ty}: {s:?}");
        }
    }
}
