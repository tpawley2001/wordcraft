use super::*;

#[test]
fn shape_story_retains_fields_tabs_and_insets() {
    let source = br"{\rtf1 Before {\shp{\*\shpinst\shpleft0\shptop0\shpright4000\shpbottom600\shpwr3{\sp{\sn dxTextLeft}{\sv 12700}}{\sp{\sn fFilled}{\sv 0}}{\sp{\sn fLine}{\sv 0}}{\shptxt BOXTEXT\tab {\field{\*\fldinst page}{\fldrslt 1}}\par}}{\shprslt duplicate}} after.\par}";
    let flow = parse(source).unwrap();
    let FBlock::Para(p) = &flow.blocks[0] else { panic!("paragraph") };
    let shapes: Vec<_> = p.inlines.iter().filter(|i| matches!(i, Inline::Shape { .. })).collect();
    assert_eq!(shapes.len(), 1);
    let Inline::Shape { blocks, look, .. } = shapes[0] else { panic!("shape") };
    assert!(look.fill.is_none() && look.line.is_none());
    assert_eq!(look.inset, Some([1.0, 3.6, 7.2, 3.6]));
    let FBlock::Para(box_p) = &blocks[0] else { panic!("box paragraph") };
    assert!(
        box_p.inlines.iter().any(|i| matches!(i, Inline::Field { instr, result, .. } if instr.trim().eq_ignore_ascii_case("page") && result == "1"))
    );
    assert!(box_p.inlines.iter().any(|i| matches!(i, Inline::Text(t, _) if t.contains('\t'))));
    let d = import(source).unwrap();
    assert!(String::from_utf8(crate::txt::export(&d)).unwrap().contains("BOXTEXT\t1"));
}

#[test]
fn truncated_shape_story_is_retained_without_leaking_into_body() {
    let d = import(br"{\rtf1 Before {\shp{\*\shpinst{\shptxt Kept").unwrap();
    let text = String::from_utf8(crate::txt::export(&d)).unwrap();
    assert!(text.contains("Kept"));
    assert!(!d.body[0].as_para().unwrap().text.contains("Kept"));
}

#[test]
fn shape_page_field_round_trips_in_header() {
    let d = import(br"{\rtf1{\header{\shp{\*\shpinst\shpleft3024\shpright10512\shpbottom240\shpbxpage{\shptxt BOXTEXT {\field{\*\fldinst page}{\fldrslt 1}}\par}}}}Body.\par}").unwrap();
    let back = import(export(&d).as_bytes()).unwrap();
    assert!(
        back.parts
            .values()
            .filter(|p| p.kind == wordcraft_doc::PartKind::TextBox)
            .flat_map(|p| &p.blocks)
            .filter_map(|b| b.as_para())
            .flat_map(|p| &p.objects)
            .any(|o| matches!(o, wordcraft_doc::para::InlineObject::Field { instr, .. } if instr.trim().eq_ignore_ascii_case("page")))
    );
    assert_eq!(back.body[0].as_para().unwrap().text, "Body.");
}

#[test]
fn adjacent_and_empty_box_fields_stay_separate() {
    let flow = parse(br"{\rtf1{\shp{\*\shpinst{\shptxt {\field{\*\fldinst page}{\fldrslt 1}}{\field{\*\fldinst page}{\fldrslt 1}}{\field{\*\fldinst page}{\fldrslt }}\par}}}}").unwrap();
    let FBlock::Para(p) = &flow.blocks[0] else { panic!("paragraph") };
    let Inline::Shape { blocks, .. } = &p.inlines[0] else { panic!("box") };
    let FBlock::Para(p) = &blocks[0] else { panic!("box paragraph") };
    let results: Vec<_> = p
        .inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Field { result, .. } => Some(result.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(results, ["1", "1", ""]);
}
