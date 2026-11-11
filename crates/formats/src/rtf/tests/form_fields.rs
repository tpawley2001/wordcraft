//! Legacy form fields (RTF 1.9.1, "Form Fields": `{\*\formfield …}` in a FORMCHECKBOX, FORMDROPDOWN
//! or FORMTEXT field instruction) come in as the field's `w:ffData` (ECMA-376 Part 1 §17.16.17)
//! and are written back the way Word writes them.

use super::*;
use wordcraft_doc::legacy_form::LegacyForm;
use wordcraft_doc::para::InlineObject;

const CHECK: &[u8] = br"{\rtf1\ansi Box {\field{\*\fldinst {FORMCHECKBOX {\*\formfield{\fftype1\ffres25\fftypetxt0\ffhps20\ffdefres1{\*\ffname Check1}}}}}{\fldrslt }} end\par}";
const CHECK_EXACT: &[u8] = br"{\rtf1\ansi {\field{\*\fldinst {FORMCHECKBOX {\*\formfield{\fftype1\ffres1\fftypetxt0\ffsize1\ffhps24\ffdefres0{\*\ffname Check2}}}}}{\fldrslt }}\par}";
const DROP: &[u8] = br"{\rtf1\ansi {\field{\*\fldinst {FORMDROPDOWN {\*\formfield{\fftype2\ffres1\fftypetxt0\ffhaslistbox{\*\ffname Dropdown1}{\*\ffl Red}{\*\ffl Green}{\*\ffl Blue}}}}}{\fldrslt }}\par}";
const DROP_DEFAULT: &[u8] = br"{\rtf1\ansi {\field{\*\fldinst {FORMDROPDOWN {\*\formfield{\fftype2\ffres25\ffdefres2\fftypetxt0\ffhaslistbox{\*\ffname Dropdown2}{\*\ffl Red}{\*\ffl Green}{\*\ffl Blue}}}}}{\fldrslt }}\par}";
const TEXT: &[u8] = br"{\rtf1\ansi Value {\field{\*\fldinst {FORMTEXT {\*\formfield{\fftype0\ffownhelp\ffownstat\fftypetxt1\ffmaxlen5{\*\ffname Text1}{\*\ffdeftext 12}{\*\ffformat 0.00}{\*\ffhelptext Help me}{\*\ffstattext Status}}}}}{\fldrslt {12}}}\par}";
const TEXT_PROT: &[u8] =
    br"{\rtf1\ansi {\field{\*\fldinst {FORMTEXT {\*\formfield{\fftype0\ffprot\fftypetxt0{\*\ffname Locked}}}}}{\fldrslt {x}}}\par}";

/// Every field in the body: (instr, result, form).
fn fields(d: &wordcraft_doc::Document) -> Vec<(String, String, String)> {
    d.body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| &p.objects)
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, form, .. } => Some((instr.clone(), result.clone(), form.clone())),
            _ => None,
        })
        .collect()
}

fn only_form(rtf: &[u8]) -> (String, String, String) {
    let d = import(rtf).unwrap();
    let f = fields(&d);
    assert_eq!(f.len(), 1, "one field: {f:?}");
    f.into_iter().next().unwrap()
}

fn parsed(form: &str) -> LegacyForm {
    LegacyForm::parse(form).unwrap_or_else(|| panic!("no legacy form in {form:?}"))
}

#[test]
fn check_box_default_with_no_own_result_is_checked_and_auto_sized() {
    let (instr, _, form) = only_form(CHECK);
    assert_eq!(instr.trim(), "FORMCHECKBOX");
    assert_eq!(parsed(&form), LegacyForm::CheckBox { name: "Check1".into(), checked: true, size: None });
    assert!(form.contains("<w:sizeAuto/>"), "{form}");
    assert!(!form.contains("<w:checked"), "\\ffres25 means no own result: {form}");
}

#[test]
fn check_box_exact_size_and_own_result() {
    let (_, _, form) = only_form(CHECK_EXACT);
    assert_eq!(parsed(&form), LegacyForm::CheckBox { name: "Check2".into(), checked: true, size: Some(12.0) });
    assert!(form.contains(r#"<w:size w:val="24"/>"#), "{form}");
    assert!(form.contains(r#"<w:default w:val="0"/>"#), "{form}");
}

#[test]
fn drop_down_entries_and_selection() {
    let (instr, _, form) = only_form(DROP);
    assert_eq!(instr.trim(), "FORMDROPDOWN");
    assert_eq!(
        parsed(&form),
        LegacyForm::DropDown { name: "Dropdown1".into(), entries: vec!["Red".into(), "Green".into(), "Blue".into()], selected: 1 }
    );
    let (_, _, form) = only_form(DROP_DEFAULT);
    assert_eq!(
        parsed(&form),
        LegacyForm::DropDown { name: "Dropdown2".into(), entries: vec!["Red".into(), "Green".into(), "Blue".into()], selected: 2 }
    );
    assert!(!form.contains("<w:result"), "\\ffres25 means none: {form}");
}

#[test]
fn text_field_type_default_length_format_help() {
    let d = import(TEXT).unwrap();
    let (instr, result, form) = fields(&d).into_iter().next().unwrap();
    assert_eq!(instr.trim(), "FORMTEXT");
    assert_eq!(result, "12");
    assert_eq!(parsed(&form), LegacyForm::Text { name: "Text1".into(), max_len: Some(5) });
    for needle in [
        r#"<w:type w:val="number"/>"#,
        r#"<w:default w:val="12"/>"#,
        r#"<w:maxLength w:val="5"/>"#,
        r#"<w:format w:val="0.00"/>"#,
        r#"<w:helpText w:type="text" w:val="Help me"/>"#,
        r#"<w:statusText w:type="text" w:val="Status"/>"#,
    ] {
        assert!(form.contains(needle), "{needle} missing in {form}");
    }
    let text: String = d.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
    assert!(!text.contains("Help me") && !text.contains("Status") && !text.contains("Text1"), "{text:?}");
}

#[test]
fn protected_field_is_not_enabled() {
    let (_, _, form) = only_form(TEXT_PROT);
    assert!(form.contains(r#"<w:enabled w:val="0"/>"#), "{form}");
}

#[test]
fn form_fields_round_trip() {
    for rtf in [CHECK, CHECK_EXACT, DROP, DROP_DEFAULT, TEXT, TEXT_PROT] {
        let (instr, _, form) = only_form(rtf);
        let out = export(&import(rtf).unwrap());
        assert!(out.contains(r"\*\formfield"), "{out}");
        let (instr2, _, form2) = only_form(out.as_bytes());
        assert_eq!(instr2.trim(), instr.trim());
        assert_eq!(parsed(&form2), parsed(&form), "{out}");
        assert_eq!(form2, form, "same w:ffData after a round trip: {out}");
    }
}

#[test]
fn docx_shaped_form_exports_as_word_writes_it() {
    let mut d = import(br"{\rtf1\ansi {\field{\*\fldinst FORMCHECKBOX }{\fldrslt }}\par}").unwrap();
    for b in d.body.iter_mut() {
        if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b) {
            for o in p.objects.iter_mut() {
                if let InlineObject::Field { form, .. } = o {
                    *form = r#"<w:ffData><w:name w:val="C"/><w:enabled/><w:calcOnExit w:val="0"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/><w:checked/></w:checkBox></w:ffData>"#.into();
                }
            }
        }
    }
    let out = export(&d);
    for needle in [r"\fftype1", r"{\*\ffname C}", r"\ffres1", r"\ffdefres0"] {
        assert!(out.contains(needle), "{needle} missing in {out}");
    }
    let (_, _, form) = only_form(out.as_bytes());
    assert_eq!(parsed(&form), LegacyForm::CheckBox { name: "C".into(), checked: true, size: None });
}

#[test]
fn ordinary_field_has_no_form() {
    let (instr, _, form) = only_form(br"{\rtf1\ansi {\field{\*\fldinst PAGE}{\fldrslt 1}}\par}");
    assert_eq!(instr.trim(), "PAGE");
    assert!(form.is_empty());
    assert!(!export(&import(br"{\rtf1\ansi {\field{\*\fldinst PAGE}{\fldrslt 1}}\par}").unwrap()).contains("formfield"));
}
