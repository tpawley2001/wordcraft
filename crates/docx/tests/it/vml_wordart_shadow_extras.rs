//! Word 2007 WordArt shadows (ECMA-376 Part 4 §19.1.2.18 `v:shadow`): the second copy of a
//! `double` or `emboss` shadow, a `perspective` shadow's transform, and their round trip.
//! Fixtures are hand-written; the attribute values are the ones Word 2007 writes.

use std::io::Write;

use wordcraft_doc::effects::{ShadowCopy, ShadowExtra, ShadowTransform};
use wordcraft_doc::wordart::TextEffects;
use wordcraft_doc::{Document, InlineObject, Rgb};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn read_shadow(children: &str) -> Document {
    let shape = format!(
        r##"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t136" style="position:absolute;margin-left:28pt;margin-top:28pt;width:242.25pt;height:41.25pt">{children}<v:textpath style="font-family:&quot;Arial&quot;;v-text-kern:t" trim="t" fitpath="t" string="Your Text Here"/></v:shape>"##
    );
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body><w:p><w:r><w:pict>{shape}</w:pict></w:r></w:p></w:body></w:document>"#
    );
    wordcraft_docx::read(&zip(&[("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())])).unwrap()
}

/// The WordArt run's text effects.
fn effects(d: &Document) -> TextEffects {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("paragraph");
    let Some(InlineObject::Shape { story: Some(id), .. }) = p.objects.first() else { panic!("no WordArt shape: {:?}", p.objects) };
    d.parts[id].blocks[0].as_para().unwrap().runs[0].props.text_effects.as_deref().cloned().unwrap_or_default()
}

fn extra(children: &str) -> Option<ShadowExtra> {
    effects(&read_shadow(children)).shadow_extra
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

const DOUBLE: &str =
    r##"<v:shadow on="t" type="double" color="#868686" opacity=".5" color2="shadow add(102)" offset="-3pt,-3pt" offset2="-6pt,-6pt"/>"##;
const PERSPECTIVE: &str =
    r##"<v:shadow on="t" type="perspective" color="#868686" opacity=".5" origin="-.5,.5" offset="-6pt,-6pt" matrix=",-56756f,,.5"/>"##;

/// `double`: the second copy at `offset2`, in the first copy's colour plus 102 ("shadow add(102)").
#[test]
fn double_second_copy() {
    let e = extra(DOUBLE).expect("extra");
    let c = e.second.expect("second copy");
    assert!(close(c.dx, -6.0) && close(c.dy, -6.0), "{c:?}");
    assert_eq!(c.color, Rgb(0xEC, 0xEC, 0xEC));
    assert!(close(c.transparency, 50.0), "{c:?}");
    assert_eq!(e.transform, None);
    // A plain `color2` is taken as it is; `offset2` defaults to -2pt,-2pt.
    let e = extra(r##"<v:shadow on="t" type="double" color="#868686" color2="#123456"/>"##).unwrap();
    let c = e.second.unwrap();
    assert_eq!(c.color, Rgb(0x12, 0x34, 0x56));
    assert!(close(c.dx, -2.0) && close(c.dy, -2.0), "{c:?}");
}

/// `emboss`: the light copy at the negated offset, the dark copy's colour plus 102.
#[test]
fn emboss_light_copy() {
    let e = extra(r#"<v:shadow on="t" type="emboss" color="lineOrFill darken(153)" color2="shadow add(102)" offset="1pt,1pt"/>"#).unwrap();
    let c = e.second.expect("light copy");
    assert!(close(c.dx, -1.0) && close(c.dy, -1.0), "{c:?}");
    // Line colour black (the default outline): dark copy black, light copy 102 grey.
    assert_eq!(c.color, Rgb(102, 102, 102));
}

/// `perspective`: the 2x2 part of `matrix` (65536ths with `f`, empty items the identity's)
/// about `origin`.
#[test]
fn perspective_transform() {
    let t = extra(PERSPECTIVE).and_then(|e| e.transform).expect("transform");
    let [sxx, sxy, syx, syy] = t.matrix;
    assert!(close(sxx, 1.0) && close(sxy, -0.866) && close(syx, 0.0) && close(syy, 0.5), "{t:?}");
    assert!(close(t.origin[0], -0.5) && close(t.origin[1], 0.5), "{t:?}");
    // Single shadows have no extras.
    assert_eq!(extra(r##"<v:shadow on="t" color="#868686" offset="2pt,2pt"/>"##), None);
}

/// Hostile numbers stay finite and in range.
#[test]
fn hostile_extras_stay_finite() {
    for ty in ["double", "emboss", "perspective"] {
        let children = format!(
            r#"<v:shadow on="t" type="{ty}" opacity="NaN" color2="zz" offset="1e39pt,NaN" offset2="NaN,1e40pt" origin="NaN,9" matrix="zzf,1e40,NaN,1e40f,,"/>"#
        );
        let Some(e) = extra(&children) else { continue };
        if let Some(c) = e.second {
            assert!(c.dx.is_finite() && c.dy.is_finite() && c.dx.abs() <= 200.0 && c.dy.abs() <= 200.0, "{ty}: {c:?}");
            assert!((0.0..=100.0).contains(&c.transparency), "{ty}: {c:?}");
        }
        if let Some(t) = e.transform {
            assert!(t.matrix.iter().all(|v| v.is_finite() && v.abs() <= 10.0), "{ty}: {t:?}");
            assert!(t.origin.iter().all(|v| v.is_finite() && v.abs() <= 0.5), "{ty}: {t:?}");
        }
    }
    // Sanitizing clamps whatever the model is handed.
    let e = ShadowExtra {
        second: Some(ShadowCopy { dx: f32::NAN, dy: 1e30, color: Rgb::BLACK, transparency: f32::INFINITY }),
        transform: Some(ShadowTransform { matrix: [f32::NAN, 1e30, -1e30, 0.5], origin: [f32::NAN, 9.0] }),
    }
    .sanitized()
    .unwrap();
    let c = e.second.unwrap();
    assert!(c.dx == 0.0 && c.dy == 200.0 && c.transparency == 0.0, "{c:?}");
    let t = e.transform.unwrap();
    assert_eq!(t.matrix, [1.0, 10.0, -10.0, 0.5]);
    assert_eq!(t.origin, [0.0, 0.5]);
}

/// Saved and read back, the shadow keeps its type, second copy and transform (written as VML,
/// which DrawingML's `w14:shadow` can't hold).
#[test]
fn round_trip() {
    for children in
        [DOUBLE, PERSPECTIVE, r#"<v:shadow on="t" type="emboss" color="lineOrFill darken(153)" color2="shadow add(102)" offset="1pt,1pt"/>"#]
    {
        let d = read_shadow(children);
        let before = effects(&d);
        assert!(before.shadow_extra.is_some(), "{children}");
        let back = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
        let after = effects(&back);
        let (s0, s1) = (before.shadow.unwrap(), after.shadow.expect("shadow kept"));
        assert_eq!(s0.color, s1.color, "{children}");
        assert!(close(s0.distance, s1.distance) && close(s0.angle, s1.angle) && close(s0.transparency, s1.transparency), "{s0:?} {s1:?}");
        let (e0, e1) = (before.shadow_extra.unwrap(), after.shadow_extra.expect("extra kept"));
        match (e0.second, e1.second) {
            (Some(a), Some(b)) => {
                assert_eq!(a.color, b.color);
                assert!(close(a.dx, b.dx) && close(a.dy, b.dy) && close(a.transparency, b.transparency), "{a:?} {b:?}");
            }
            (None, None) => {}
            other => panic!("second copy changed: {other:?}"),
        }
        match (e0.transform, e1.transform) {
            (Some(a), Some(b)) => {
                assert!(a.matrix.iter().zip(b.matrix).all(|(x, y)| (x - y).abs() < 1e-3), "{a:?} {b:?}");
                assert!(a.origin.iter().zip(b.origin).all(|(x, y)| (x - y).abs() < 1e-3), "{a:?} {b:?}");
            }
            (None, None) => {}
            other => panic!("transform changed: {other:?}"),
        }
        // The letters' fill and outline survive too.
        assert_eq!(before.fill, after.fill, "{children}");
        assert_eq!(before.outline, after.outline, "{children}");
    }
}
