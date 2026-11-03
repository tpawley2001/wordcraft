//! Word 2007 WordArt (a VML `v:shape` with a `v:textpath`) keeps its look: the VML fill and stroke
//! defaults (white letters, 0.75pt black outline), its `v:fill`/`v:stroke` overrides and a plain
//! offset `v:shadow` become the run's text effects. Fixtures are hand-written from ECMA-376 Part 4
//! (VML: `v:shape` §19.1.2.19, `v:fill` §19.1.2.5, `v:stroke` §19.1.2.21, `v:shadow` §19.1.2.18);
//! expected values were measured from Word's PDF of a WordArt sample.

use std::io::Write;

use wordcraft_doc::effects::Shadow;
use wordcraft_doc::wordart::{TextEffects, TextFill, TextOutline};
use wordcraft_doc::{Document, InlineObject, Rgb, TextColor};

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

/// The WordArt shape's run: (colour, text effects).
fn art(attrs: &str, children: &str) -> (Option<TextColor>, TextEffects) {
    let shape = format!(
        r##"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t136" style="position:absolute;margin-left:28pt;margin-top:28pt;width:242.25pt;height:41.25pt" {attrs}>{children}<v:textpath style="font-family:&quot;Arial&quot;;v-text-kern:t" trim="t" fitpath="t" string="Your Text Here"/></v:shape>"##
    );
    let d = read_shape(&shape);
    let p = d.body.iter().find_map(|b| b.as_para()).expect("paragraph");
    let Some(InlineObject::Shape { story: Some(id), .. }) = p.objects.first() else { panic!("no WordArt shape: {:?}", p.objects) };
    let run = &d.parts[id].blocks[0].as_para().unwrap().runs[0];
    (run.props.color, run.props.text_effects.as_deref().cloned().unwrap_or_default())
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn shadow_of(children: &str) -> Option<Shadow> {
    art("", children).1.shadow
}

/// No fill or stroke attributes: VML's defaults, white letters with a 0.75pt black outline.
#[test]
fn defaults_are_white_letters_with_a_black_outline() {
    let (color, fx) = art("", "");
    assert_eq!(color, Some(TextColor::Rgb(Rgb::WHITE)));
    assert_eq!(fx.fill, Some(TextFill::Solid { color: Rgb::WHITE, transparency: 0.0 }));
    assert_eq!(fx.outline, Some(TextOutline { color: Some(Rgb::BLACK), width: 0.75, transparency: 0.0 }));
    assert_eq!(fx.shadow, None);
}

/// `fillcolor`, `strokecolor`, `strokeweight` and the `v:fill`/`v:stroke` children override the defaults;
/// `stroked="f"` and `filled="f"` turn outline and fill off.
#[test]
fn fill_and_stroke_overrides() {
    let (color, fx) = art(r##"fillcolor="#336699" strokecolor="red" strokeweight="1.5pt""##, "");
    assert_eq!(color, Some(TextColor::Rgb(Rgb(0x33, 0x66, 0x99))));
    assert_eq!(fx.fill, Some(TextFill::Solid { color: Rgb(0x33, 0x66, 0x99), transparency: 0.0 }));
    assert_eq!(fx.outline, Some(TextOutline { color: Some(Rgb(255, 0, 0)), width: 1.5, transparency: 0.0 }));

    let fx = art(r##"fillcolor="#336699""##, r#"<v:fill opacity=".5"/>"#).1;
    assert_eq!(fx.fill, Some(TextFill::Solid { color: Rgb(0x33, 0x66, 0x99), transparency: 50.0 }));
    let fx = art(r##"fillcolor="#336699""##, r#"<v:fill opacity="32768f"/>"#).1;
    assert_eq!(fx.fill, Some(TextFill::Solid { color: Rgb(0x33, 0x66, 0x99), transparency: 50.0 }));

    let fx = art(r#"stroked="f""#, "").1;
    assert_eq!(fx.outline, None);
    let fx = art("", r#"<v:stroke on="f"/>"#).1;
    assert_eq!(fx.outline, None);
    let fx = art("", r##"<v:stroke color="#00ff00" weight="2pt"/>"##).1;
    assert_eq!(fx.outline, Some(TextOutline { color: Some(Rgb(0, 255, 0)), width: 2.0, transparency: 0.0 }));

    let fx = art(r#"filled="f""#, "").1;
    assert_eq!(fx.fill, Some(TextFill::None));
    let fx = art("", r#"<v:fill on="f"/>"#).1;
    assert_eq!(fx.fill, Some(TextFill::None));
}

/// A plain offset shadow: crisp, its colour and opacity, cast by the offset (x right, y down).
/// Word draws `offset="-6pt,-6pt"` 6pt up and left of the letters, #868686 at 50%, no blur.
#[test]
fn offset_shadows() {
    let expect = |offset: &str, angle: f32| {
        let s = shadow_of(&format!(r##"<v:shadow on="t" color="#868686" opacity=".5" offset="{offset}"/>"##)).expect(offset);
        assert_eq!(s.color, Rgb(0x86, 0x86, 0x86), "{offset}");
        assert!(close(s.transparency, 50.0), "{offset}: {s:?}");
        assert!(close(s.blur, 0.0), "{offset}: {s:?}");
        assert!(close(s.distance, 8.485_281), "{offset}: {s:?}");
        assert!(close(s.angle, angle), "{offset}: {s:?}");
        assert!(!s.rot_with_shape);
    };
    expect("6pt,6pt", 45.0);
    expect("-6pt,6pt", 135.0);
    expect("-6pt,-6pt", 225.0);
    expect("6pt,-6pt", 315.0);

    // Defaults: gray, opaque, 2pt right and 2pt down.
    let s = shadow_of(r#"<v:shadow on="t"/>"#).expect("default shadow");
    assert_eq!(s.color, Rgb(0x80, 0x80, 0x80));
    assert!(close(s.transparency, 0.0) && close(s.distance, 2.828_427) && close(s.angle, 45.0), "{s:?}");
}

/// Shadows that are off give none.
#[test]
fn other_shadows_give_none() {
    assert_eq!(shadow_of(r##"<v:shadow on="f" color="#868686" offset="6pt,6pt"/>"##), None);
    assert_eq!(shadow_of(r##"<v:shadow color="#868686" offset="6pt,6pt"/>"##), None);
}

/// Hostile values never panic and stay in range.
#[test]
fn hostile_values_stay_in_range() {
    let fx = art(r##"fillcolor="#zz" strokeweight="1e40pt""##, r#"<v:fill opacity="NaN"/><v:shadow on="t" opacity="-7" offset="1e39pt,NaN"/>"#).1;
    if let Some(TextFill::Solid { transparency, .. }) = fx.fill {
        assert!((0.0..=100.0).contains(&transparency));
    }
    if let Some(o) = fx.outline {
        assert!(o.width.is_finite() && o.width <= wordcraft_doc::wordart::MAX_OUTLINE);
    }
    if let Some(s) = fx.shadow {
        assert!(s.distance.is_finite() && s.angle.is_finite() && (0.0..=100.0).contains(&s.transparency), "{s:?}");
    }
}
