//! A non-breaking hyphen (U+2011) is drawn and measured as the hyphen-minus of the font the text
//! around it is set in, as the reference layout draws it: the same face, the same width and the
//! same line height as `-`. Only line breaking differs: the line never breaks at it. Many fonts
//! have no U+2011 glyph; taking one from a fallback face made each such line taller (a 14-page
//! form came out 15 pages long). Each document is built twice ("twins"): once with `-`, once with
//! U+2011 in the same place.

use crate::*;
use wordcraft_doc::para::NB_HYPHEN;

const FONTS: [&str; 5] = ["Times New Roman", "Arial", "Courier New", "Inter", "DejaVu Sans"];

fn face(font: &str, bold: bool) -> wordcraft_fonts::FaceRef {
    crate::test_fonts();
    wordcraft_fonts::word::resolve(font, bold, false).face
}

/// The test fonts whose face (regular and bold) has no U+2011 glyph; at least one must exist.
fn fonts_without_nb_hyphen() -> Vec<&'static str> {
    let out: Vec<&str> = FONTS.into_iter().filter(|f| !face(f, false).covers(NB_HYPHEN) && face(f, false).covers('-')).collect();
    assert!(!out.is_empty(), "the test fonts changed: none of {FONTS:?} lacks a U+2011 glyph any more; pick one that does");
    out
}

/// The test fonts whose face has its own U+2011 glyph (may be none).
fn fonts_with_nb_hyphen() -> Vec<&'static str> {
    FONTS.into_iter().filter(|f| face(f, false).covers(NB_HYPHEN)).collect()
}

fn para(text: &str, font: &str, size: f32, bold: bool) -> Arc<wordcraft_doc::Block> {
    let chr = wordcraft_doc::CharProps { font: Some(font.into()), size: Some(size), bold: Some(bold), ..Default::default() };
    let mut p = wordcraft_doc::Paragraph::with_text(text, chr);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    wordcraft_doc::para_block(p)
}

fn doc(paras: Vec<Arc<wordcraft_doc::Block>>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = paras;
    d
}

/// Every placed run of lines in the body: (paragraph layout, first line, end line, y).
fn placed(d: &Document) -> Vec<(Arc<ParaLayout>, usize, usize, f32)> {
    let mut c = LayoutCache::new();
    let l = layout(d, &mut c, &LayoutOptions::default());
    l.pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|it| if let Placed::Lines { para, l0, l1, y, .. } = it { Some((para.clone(), *l0, *l1, *y)) } else { None })
        .collect()
}

fn one_para(text: &str, font: &str, size: f32, bold: bool) -> Arc<ParaLayout> {
    let p = placed(&doc(vec![para(text, font, size, bold)]));
    assert_eq!(p.len(), 1, "{font}: {text}");
    p[0].0.clone()
}

fn twins(template: &str, font: &str, size: f32, bold: bool) -> (Arc<ParaLayout>, Arc<ParaLayout>, String, String) {
    let plain = template.replace('#', "-");
    let nb = template.replace('#', "\u{2011}");
    (one_para(&plain, font, size, bold), one_para(&nb, font, size, bold), plain, nb)
}

/// The x of the cluster starting at byte `at` on line 0.
fn x_at(p: &ParaLayout, at: usize) -> f32 {
    let line = &p.lines[0];
    let ci = (line.c0..line.c1).find(|&i| p.clusters[i].start == at).expect("a cluster starts there");
    line.xs[ci - line.c0]
}

/// The face of every drawn cluster, in order.
fn faces(p: &ParaLayout) -> Vec<u32> {
    p.clusters.iter().filter(|c| c.g1 > c.g0).map(|c| p.styles[c.style as usize].face.id()).collect()
}

fn all_fonts() -> Vec<&'static str> {
    let mut f = fonts_without_nb_hyphen();
    f.extend(fonts_with_nb_hyphen());
    f
}

const LINE: &str = "MMMM A#B MMMM";

#[test]
fn the_line_is_as_tall_as_with_a_hyphen() {
    for font in all_fonts() {
        let (plain, nb, ..) = twins(LINE, font, 12.0, false);
        assert_eq!(plain.lines.len(), 1, "{font}");
        assert_eq!(nb.lines.len(), 1, "{font}");
        let (a, b) = (&plain.lines[0], &nb.lines[0]);
        assert!((a.height - b.height).abs() < 0.01, "{font}: line height {} vs {}", a.height, b.height);
        assert!((a.text_asc - b.text_asc).abs() < 0.01, "{font}: ascent {} vs {}", a.text_asc, b.text_asc);
        assert!((a.text_desc - b.text_desc).abs() < 0.01, "{font}: descent {} vs {}", a.text_desc, b.text_desc);
        assert!((plain.height - nb.height).abs() < 0.01, "{font}: paragraph height {} vs {}", plain.height, nb.height);
    }
}

#[test]
fn the_hyphen_is_as_wide_as_a_hyphen_minus() {
    for font in all_fonts() {
        let (plain, nb, ptext, ntext) = twins(LINE, font, 12.0, false);
        let xp = x_at(&plain, ptext.rfind(' ').unwrap() + 1);
        let xn = x_at(&nb, ntext.rfind(' ').unwrap() + 1);
        assert!((xp - xn).abs() < 0.01, "{font}: the last MMMM starts at {xp} with '-', {xn} with U+2011");
    }
}

#[test]
fn no_fallback_face_is_used() {
    for font in all_fonts() {
        let (plain, nb, ..) = twins(LINE, font, 12.0, false);
        assert_eq!(faces(&plain), faces(&nb), "{font}: every glyph comes from the same face");
        assert!(faces(&nb).iter().all(|&f| f == face(font, false).id()), "{font}: only the document's face");
    }
}

#[test]
fn the_text_after_ten_such_paragraphs_does_not_move() {
    for font in all_fonts() {
        let end_y = |h: &str| {
            let mut paras: Vec<_> = (0..10).map(|_| para(&LINE.replace('#', h), font, 12.0, false)).collect();
            paras.push(para("END", font, 12.0, false));
            let p = placed(&doc(paras));
            p.last().expect("END is placed").3
        };
        let (a, b) = (end_y("-"), end_y("\u{2011}"));
        assert!((a - b).abs() < 0.01, "{font}: END at {a} with '-', {b} with U+2011");
    }
}

#[test]
fn a_bold_heading_keeps_its_bold_face() {
    for font in all_fonts() {
        let (plain, nb, ..) = twins("PART II # DATA", font, 16.0, true);
        let (a, b) = (&plain.lines[0], &nb.lines[0]);
        assert!((a.height - b.height).abs() < 0.01, "{font}: line height {} vs {}", a.height, b.height);
        let (wa, wb) = (a.xs[a.xs.len() - 1], b.xs[b.xs.len() - 1]);
        assert!((wa - wb).abs() < 0.01, "{font}: width {wa} vs {wb}");
        assert!(faces(&nb).iter().all(|&f| f == face(font, true).id()), "{font}: only the bold face");
    }
}

#[test]
fn a_fixed_pitch_font_keeps_its_pitch() {
    let font = "Courier New";
    let f = face(font, false);
    assert!(f.covers('-') && f.covers('A'), "the test fonts changed: {font} resolves to a face without A or -");
    let p = one_para("A\u{2011}B", font, 12.0, false);
    let m = one_para("M", font, 12.0, false);
    let pitch = m.lines[0].xs[1] - m.lines[0].xs[0];
    let line = &p.lines[0];
    let width = line.xs[line.xs.len() - 1] - line.xs[0];
    assert!((width - 3.0 * pitch).abs() < 0.01, "A\u{2011}B is {width} wide, pitch {pitch}");
}

#[test]
fn the_line_never_breaks_at_the_non_breaking_hyphen() {
    let font = fonts_without_nb_hyphen()[0];
    let text = "Deed 2016\u{2011}2022";
    let mut wrapped = false;
    for m in 100..=540 {
        let mut d = doc(vec![para(text, font, 12.0, false)]);
        d.last_section.margin_right = m as f32;
        let p = placed(&d);
        let lines: Vec<&str> = p.iter().flat_map(|(para, l0, l1, _)| para.lines[*l0..*l1].iter().map(|l| &text[l.start..l.stop])).collect();
        if lines.len() < 2 {
            continue;
        }
        assert_eq!(lines[0], "Deed ", "{font}: the only break is at the space: {lines:?}");
        assert_eq!(lines[1], "2016\u{2011}2022", "{font}: the range stays whole: {lines:?}");
        wrapped = true;
        break;
    }
    assert!(wrapped, "some width wraps the text");
}

// A run in a symbol-encoded font (Symbol, Wingdings) keeps its own code mapping: the hyphen
// substitution applies only outside symbol runs, so such a run is laid out as before.

#[test]
fn the_line_still_counts_the_non_breaking_hyphen_bytes() {
    let font = fonts_without_nb_hyphen()[0];
    let (_, nb, _, text) = twins(LINE, font, 12.0, false);
    let line = &nb.lines[0];
    assert_eq!((line.start, line.stop), (0, text.len()));
    assert_eq!(&text[line.start..line.stop], text);
    let at = text.find('\u{2011}').unwrap();
    let c = nb.clusters.iter().find(|c| c.start == at).expect("a cluster for the hyphen");
    assert_eq!(c.end, at + '\u{2011}'.len_utf8(), "the cluster covers the 3-byte U+2011");
}
