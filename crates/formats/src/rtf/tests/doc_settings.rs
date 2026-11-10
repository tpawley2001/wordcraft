//! Document formatting `\viewscaleN`, `\viewzkN`, the drawing grid (`\dghspaceN`, `\dgvspaceN`,
//! `\dghoriginN`, `\dgvoriginN`, `\dghshowN`, `\dgvshowN`, `\dgmargin`), Restrict Editing
//! (`\protlevelN` + `\enforceprotN`, and the older `\revprot`, `\annotprot`, `\formprot`, `\readprot`),
//! `\linkstyles`, `\rempersonalinfo`, `\remdttm` and `\expshrtn`, RTF 1.9.1: the RTF form of DOCX
//! `w:zoom`, the drawing grid settings, `w:documentProtection`, `w:linkStyles`,
//! `w:removePersonalInformation`, `w:removeDateAndTime` and `w:doNotExpandShiftReturn`.

use super::*;
use wordcraft_doc::{Settings, Zoom, ZoomFit};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn round_trip(d: &Document) -> Document {
    doc(&export(d))
}

/// (h, v, every h, every v, use margins, origin x, origin y).
fn grid(s: &Settings) -> (f32, f32, u8, u8, bool, f32, f32) {
    (s.grid_h, s.grid_v, s.grid_every_h, s.grid_every_v, s.grid_use_margins, s.grid_origin_x, s.grid_origin_y)
}

#[test]
fn no_words_keep_the_defaults() {
    let d = doc(r"{\rtf1\ansi\sectd a\par}");
    let def = Settings::default();
    assert_eq!(d.settings.zoom, None);
    assert_eq!(grid(&d.settings), grid(&def));
    assert_eq!(d.settings.protection, None);
    assert!(!d.settings.link_styles && !d.settings.remove_personal_info && !d.settings.remove_date_time);
    assert!(!d.settings.do_not_expand_shift_return);
}

#[test]
fn zoom_is_read() {
    let z = |rtf: &str| doc(rtf).settings.zoom;
    assert_eq!(z(r"{\rtf1\ansi\viewscale150\sectd a\par}"), Some(Zoom { percent: 150, fit: None }));
    assert_eq!(z(r"{\rtf1\ansi\viewscale63\viewzk1\sectd a\par}"), Some(Zoom { percent: 63, fit: Some(ZoomFit::OnePage) }));
    assert_eq!(z(r"{\rtf1\ansi\viewzk2\viewscale118\sectd a\par}"), Some(Zoom { percent: 118, fit: Some(ZoomFit::PageWidth) }));
    assert_eq!(z(r"{\rtf1\ansi\viewzk2\sectd a\par}"), Some(Zoom { percent: 100, fit: Some(ZoomFit::PageWidth) }));
    assert_eq!(z(r"{\rtf1\ansi\viewscale100\viewzk0\sectd a\par}"), None);
    for rtf in [r"{\rtf1\ansi\viewscale5\sectd a\par}", r"{\rtf1\ansi\viewscale501\sectd a\par}", r"{\rtf1\ansi\viewzk7\sectd a\par}"] {
        assert_eq!(z(rtf), None, "{rtf}");
    }
}

#[test]
fn drawing_grid_is_read() {
    let d = doc(r"{\rtf1\ansi\dghspace120\dgvspace240\dghshow2\dgvshow0\dgmargin\sectd a\par}");
    assert_eq!(grid(&d.settings), (6.0, 12.0, 2, 0, true, 0.0, 0.0));
    let d = doc(r"{\rtf1\ansi\dghspace180\dgvspace180\dghorigin1700\dgvorigin1440\dghshow1\dgvshow1\sectd a\par}");
    assert_eq!(grid(&d.settings), (9.0, 9.0, 1, 1, false, 85.0, 72.0));
    let d = doc(r"{\rtf1\ansi\dghspace0\dgvspace-20\dghshow200\sectd a\par}");
    assert_eq!(grid(&d.settings), grid(&Settings::default()), "out-of-range values are ignored");
}

#[test]
fn enforced_protection_levels_are_read() {
    for (n, level) in [(0, "trackedChanges"), (1, "comments"), (2, "forms"), (3, "readOnly")] {
        let d = doc(&format!(r"{{\rtf1\ansi\enforceprot1\protlevel{n}\sectd a\par}}"));
        assert_eq!(d.settings.protection.as_deref(), Some(level), "protlevel{n}");
        let d = doc(&format!(r"{{\rtf1\ansi\protlevel{n}\enforceprot1\sectd a\par}}"));
        assert_eq!(d.settings.protection.as_deref(), Some(level), "protlevel{n} before enforceprot");
    }
    for rtf in [r"{\rtf1\ansi\protlevel3\sectd a\par}", r"{\rtf1\ansi\protlevel2\enforceprot0\sectd a\par}", r"{\rtf1\ansi\allprot\sectd a\par}"] {
        assert_eq!(doc(rtf).settings.protection, None, "{rtf}");
    }
}

#[test]
fn older_protection_words_are_read_and_lose_to_protlevel() {
    for (w, level) in [("revprot", "trackedChanges"), ("annotprot", "comments"), ("formprot", "forms"), ("readprot", "readOnly")] {
        let d = doc(&format!(r"{{\rtf1\ansi\{w}\sectd a\par}}"));
        assert_eq!(d.settings.protection.as_deref(), Some(level), "{w}");
    }
    let d = doc(r"{\rtf1\ansi\formprot\enforceprot1\protlevel1\sectd a\par}");
    assert_eq!(d.settings.protection.as_deref(), Some("comments"));
}

#[test]
fn flags_are_read_and_zero_turns_them_off() {
    let d = doc(r"{\rtf1\ansi\linkstyles\rempersonalinfo\remdttm\expshrtn\sectd a\par}");
    let s = &d.settings;
    assert_eq!((s.link_styles, s.remove_personal_info, s.remove_date_time, s.do_not_expand_shift_return), (true, true, true, true));
    let d = doc(r"{\rtf1\ansi\linkstyles\linkstyles0\rempersonalinfo0\expshrtn0\sectd a\par}");
    let s = &d.settings;
    assert_eq!((s.link_styles, s.remove_personal_info, s.do_not_expand_shift_return), (false, false, false));
}

#[test]
fn settings_survive_export() {
    let mut d = doc(r"{\rtf1\ansi\sectd a\par}");
    d.settings.zoom = Some(Zoom { percent: 63, fit: Some(ZoomFit::OnePage) });
    d.settings.grid_h = 6.0;
    d.settings.grid_v = 12.0;
    d.settings.grid_every_h = 2;
    d.settings.grid_every_v = 0;
    d.settings.grid_use_margins = false;
    d.settings.grid_origin_x = 85.0;
    d.settings.grid_origin_y = 72.0;
    d.settings.protection = Some("forms".into());
    d.settings.link_styles = true;
    d.settings.remove_personal_info = true;
    d.settings.remove_date_time = true;
    d.settings.do_not_expand_shift_return = true;
    let r = round_trip(&d);
    assert_eq!(r.settings.zoom, d.settings.zoom);
    assert_eq!(grid(&r.settings), grid(&d.settings));
    assert_eq!(r.settings.protection.as_deref(), Some("forms"));
    let s = &r.settings;
    assert_eq!((s.link_styles, s.remove_personal_info, s.remove_date_time, s.do_not_expand_shift_return), (true, true, true, true));

    d.settings.zoom = Some(Zoom { percent: 140, fit: Some(ZoomFit::TextWidth) });
    d.settings.protection = Some("readOnly".into());
    let r = round_trip(&d);
    assert_eq!(r.settings.zoom, Some(Zoom { percent: 140, fit: None }), "text width has no RTF form");
    assert_eq!(r.settings.protection.as_deref(), Some("readOnly"));
}

#[test]
fn a_default_document_writes_none_of_the_words() {
    let out = export(&doc(r"{\rtf1\ansi\sectd a\par}"));
    for w in [
        r"\viewscale",
        r"\viewzk",
        r"\dghspace",
        r"\dgvspace",
        r"\dghshow",
        r"\dgvshow",
        r"\dgmargin",
        r"\dghorigin",
        r"\protlevel",
        r"\enforceprot",
        r"\linkstyles",
        r"\rempersonalinfo",
        r"\remdttm",
        r"\expshrtn",
    ] {
        assert!(!out.contains(w), "{w} in {out}");
    }
}

#[test]
fn a_protection_level_with_no_rtf_form_writes_nothing() {
    let mut d = doc(r"{\rtf1\ansi\sectd a\par}");
    d.settings.protection = Some("none".into());
    let out = export(&d);
    assert!(!out.contains(r"\protlevel") && !out.contains(r"\enforceprot"), "{out}");
}
