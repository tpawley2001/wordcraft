//! Balance SBCS characters and DBCS characters (`w:balanceSingleByteDoubleByteWidth`, ECMA-376
//! Part 1 §17.15.3.1, `Settings::balance_sbcs_dbcs_width`): a space (U+0020) touching East Asian
//! text, and every space of a run of two or more in a paragraph with East Asian text, is half an em
//! wide (0.528 em when no East Asian font is named); other spaces keep their width.
//!
//! Measured in Microsoft 365 Word on synthetic .docx files (Calibri + SimSun 10 pt: such a space
//! is 5.04 pt, a Latin-Latin space 2.25 pt; 14 pt: 6.96; Microsoft YaHei 5.04; no East Asian font
//! 5.28; compatibility modes 12, 14 and 15 alike; `useFELayout` alone changes nothing).
//! Assertions compare against half the size, or against the same layout with the flag off, never
//! against font metrics.

use crate::*;

fn lay(doc: &Document) -> DocLayout {
    layout(doc, &mut LayoutCache::new(), &LayoutOptions::default())
}

fn first_para(l: &DocLayout) -> Arc<ParaLayout> {
    l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

/// One paragraph of `text` in Calibri `size` pt, East Asian font `ea`, the flag as given.
fn doc(text: &str, size: f32, ea: Option<&str>, balance: bool) -> Document {
    let mut d = Document::from_text(text);
    d.settings.balance_sbcs_dbcs_width = balance;
    let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    let n = p.text.len();
    let ea = ea.map(str::to_string);
    p.format(0, n, &move |c: &mut CharProps| {
        c.font = Some("Calibri".into());
        c.font_ea = ea.clone();
        c.size = Some(size);
    })
    .unwrap();
    d
}

/// The advances of the paragraph's space clusters, rounded to a hundredth.
fn space_advances(d: &Document) -> Vec<f32> {
    let text = d.body.first().and_then(|b| b.as_para()).unwrap().text.clone();
    let pl = first_para(&lay(d));
    pl.clusters.iter().filter(|c| text.get(c.start..c.end) == Some(" ")).map(|c| (c.adv * 100.0).round() / 100.0).collect()
}

/// `want[i]`: `Some(w)` = the i-th space is `w` wide with the flag on; `None` = it keeps the width it
/// has with the flag off.
fn check(text: &str, size: f32, ea: Option<&str>, want: &[Option<f32>]) {
    crate::test_fonts();
    let on = space_advances(&doc(text, size, ea, true));
    let off = space_advances(&doc(text, size, ea, false));
    assert_eq!(on.len(), want.len(), "{text:?}: spaces {on:?}");
    for (i, w) in want.iter().enumerate() {
        match w {
            Some(w) => assert_eq!(on[i], *w, "{text:?} at {size} pt: space {i} with the flag ({on:?}, off {off:?})"),
            None => assert_eq!(on[i], off[i], "{text:?}: space {i} keeps its width ({on:?}, off {off:?})"),
        }
    }
}

#[test]
fn balance_sbcs_a_space_between_east_asian_words_is_half_an_em() {
    check("中文 中文", 10.0, Some("SimSun"), &[Some(5.0)]);
    check("中文 中文", 14.0, Some("SimSun"), &[Some(7.0)]);
    check("中文 中文", 10.0, Some("Microsoft YaHei"), &[Some(5.0)]);
}

#[test]
fn balance_sbcs_a_space_touching_east_asian_text_on_either_side_widens() {
    check("abc 中文 abc", 10.0, Some("SimSun"), &[Some(5.0), Some(5.0)]);
    check("12 中文 34", 10.0, Some("SimSun"), &[Some(5.0), Some(5.0)]);
    check("a 中", 10.0, Some("SimSun"), &[Some(5.0)]);
    check("中 a", 10.0, Some("SimSun"), &[Some(5.0)]);
    check("中文:中文 (中文)", 10.0, Some("SimSun"), &[Some(5.0)]);
    check("中文： abc", 10.0, Some("SimSun"), &[Some(5.0)]);
}

#[test]
fn balance_sbcs_a_single_space_between_latin_letters_keeps_its_width() {
    check("中文 abc def 中文", 10.0, Some("SimSun"), &[Some(5.0), None, Some(5.0)]);
    check("abc def", 10.0, Some("SimSun"), &[None]);
    check("abc abc abc", 10.0, Some("SimSun"), &[None, None]);
}

#[test]
fn balance_sbcs_runs_of_spaces_in_east_asian_paragraphs_widen() {
    check("中文  中文  abc  abc", 10.0, Some("SimSun"), &[Some(5.0); 6]);
}

#[test]
fn balance_sbcs_no_east_asian_font_named_gives_0_528_em() {
    check("中文 中文", 10.0, None, &[Some(5.28)]);
}

#[test]
fn balance_sbcs_the_rule_crosses_runs_and_ignores_the_compatibility_mode() {
    crate::test_fonts();
    for mode in [11u32, 12, 14, 15] {
        let mut d = doc("中文 abc def 中文", 10.0, Some("SimSun"), true);
        d.settings.compat_mode = mode;
        let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
        // The first space (bytes 6..7) and the last (14..15) in runs of their own.
        p.format(6, 7, &|c: &mut CharProps| c.bold = Some(true)).unwrap();
        p.format(14, 15, &|c: &mut CharProps| c.bold = Some(true)).unwrap();
        let a = space_advances(&d);
        assert_eq!((a[0], a[2]), (5.0, 5.0), "mode {mode}: {a:?}");
    }
}

#[test]
fn balance_sbcs_off_changes_nothing() {
    crate::test_fonts();
    let d = doc("中文 中文 abc", 10.0, Some("SimSun"), false);
    let a = space_advances(&d);
    assert!(a.iter().all(|w| *w < 4.0), "the Latin font's spaces without the flag: {a:?}");
}
