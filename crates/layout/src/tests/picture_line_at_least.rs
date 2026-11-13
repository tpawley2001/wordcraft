//! A line that holds only inline pictures under "at least" line spacing is as tall as its
//! pictures or the minimum, whichever is more; the font's ascent, descent and height do not count,
//! and the pictures end at the line's bottom. Word's numbers (US Letter, 72 pt margins): paragraph
//! 1 one exactly 100 pt line, paragraph 2 the picture, paragraph 3 `after` in Times New Roman 12.
//! Synthetic shapes only; no external documents.
use super::*;
use wordcraft_doc::props::LineSpacing;

const TOL: f32 = 0.15;

fn arial(size: f32) -> CharProps {
    CharProps { font: Some("Arial".into()), size: Some(size), ..Default::default() }
}

/// Three paragraphs: an exactly `lead` pt line, then `text` and a `pic` pt tall picture at
/// `spacing` with `font` on the run and the mark, then (when `after`) a Times New Roman 12 line.
fn document(
    lead: f32,
    spacing: LineSpacing,
    font: &CharProps,
    mark: &CharProps,
    (text, pic): (&str, f32),
    after: bool,
    space_after: f32,
) -> Document {
    let mut d = Document::from_text(if after { "a\n\nafter" } else { "a\n" });
    d.last_section.page_w = 612.0;
    d.last_section.page_h = 792.0;
    d.last_section.margin_top = 72.0;
    d.last_section.margin_bottom = 72.0;
    d.last_section.margin_left = 72.0;
    d.last_section.margin_right = 72.0;
    let tnr = CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() };
    for (i, b) in d.body.iter_mut().enumerate() {
        let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b) else { panic!("a paragraph") };
        p.props.space_before = Some(0.0);
        p.props.space_after = Some(0.0);
        p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
        match i {
            0 => p.props.line_spacing = Some(LineSpacing::Exactly(lead)),
            1 => {
                p.props.line_spacing = Some(spacing);
                p.props.space_after = Some(space_after);
                p.mark = mark.clone();
                p.insert_text(0, text, font).unwrap();
                let at = p.len();
                p.insert_object(at, picture(100.0, pic, Default::default()), font).unwrap();
            }
            _ => {
                p.mark = tnr.clone();
                let text = p.text.clone();
                p.runs.clear();
                p.text.clear();
                p.insert_text(0, &text, &tnr).unwrap();
            }
        }
    }
    d
}

/// Paragraph 2's line: its height, the picture's top and bottom under the line's top, the
/// picture's font height and descent, and the page count.
struct Got {
    h: f32,
    top: f32,
    bottom: f32,
    font: f32,
    desc: f32,
    pages: usize,
    pic_page: usize,
}

fn measure(d: &Document) -> Got {
    let l = lay(d);
    for (n, page) in l.pages.iter().enumerate() {
        for item in &page.items {
            let Placed::Lines { para, l0, .. } = item else { continue };
            let Some(c) = para.clusters.iter().find(|c| matches!(c.kind, para::ClKind::Object(_))) else { continue };
            let Some(line) = para.lines.get(*l0) else { continue };
            let st = &para.styles[c.style as usize];
            return Got {
                h: line.height,
                top: line.baseline - c.obj_h - line.top,
                bottom: line.baseline + c.obj_d - line.top,
                font: st.ascent + st.descent,
                desc: st.descent,
                pages: l.pages.len(),
                pic_page: n,
            };
        }
    }
    panic!("no picture line")
}

fn row(spacing: LineSpacing, size: f32, text: &str, pic: f32) -> Got {
    measure(&document(100.0, spacing, &arial(size), &arial(size), (text, pic), true, 0.0))
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() < TOL, "{what}: {got}, Word {want}");
}

fn at_least(size: f32, min: f32, pic: f32, h: f32, top: f32, bottom: f32) {
    let g = row(LineSpacing::AtLeast(min), size, "", pic);
    near(g.h, h, "line height");
    near(g.top, top, "picture top");
    near(g.bottom, bottom, "picture bottom");
}

#[test]
fn row_1_automatic_400_pt_picture_is_400() {
    let g = row(LineSpacing::Multiple(1.0), 10.0, "", 400.0);
    near(g.h, 400.0, "line height");
}

#[test]
fn row_2_at_least_12_arial_10_400_pt_picture_is_400() {
    at_least(10.0, 12.0, 400.0, 400.0, 0.0, 400.0);
}

#[test]
fn row_3_at_least_12_arial_20_400_pt_picture_is_400() {
    at_least(20.0, 12.0, 400.0, 400.0, 0.0, 400.0);
}

#[test]
fn row_4_at_least_12_6_pt_picture_sits_on_the_bottom() {
    at_least(10.0, 12.0, 6.0, 12.0, 6.0, 12.0);
}

#[test]
fn row_5_at_least_30_20_pt_picture_sits_on_the_bottom() {
    at_least(10.0, 30.0, 20.0, 30.0, 10.0, 30.0);
}

#[test]
fn row_6_at_least_30_29_pt_picture_is_30() {
    at_least(10.0, 30.0, 29.0, 30.0, 1.0, 30.0);
}

#[test]
fn row_7_at_least_12_arial_20_20_pt_picture_is_20() {
    at_least(20.0, 12.0, 20.0, 20.0, 0.0, 20.0);
}

#[test]
fn row_8_at_least_12_arial_20_6_pt_picture_is_12() {
    at_least(20.0, 12.0, 6.0, 12.0, 6.0, 12.0);
}

#[test]
fn row_9_automatic_arial_20_6_pt_picture_is_the_fonts_height() {
    let g = row(LineSpacing::Multiple(1.0), 20.0, "", 6.0);
    near(g.h, g.font, "line height");
}

#[test]
fn row_10_at_least_12_text_and_picture_keep_the_texts_descent() {
    let g = row(LineSpacing::AtLeast(12.0), 10.0, "x", 400.0);
    near(g.h, 400.0 + g.desc, "line height");
}

#[test]
fn row_11_automatic_text_and_picture_keep_the_texts_descent() {
    let g = row(LineSpacing::Multiple(1.0), 10.0, "x", 400.0);
    near(g.h, 400.0 + g.desc, "line height");
}

#[test]
fn row_12_exactly_120_is_120() {
    let g = row(LineSpacing::Exactly(120.0), 10.0, "", 20.0);
    near(g.h, 120.0, "line height");
}

/// Paragraph 1 exactly `twips` tall, paragraph 2 last with a 400 pt picture.
fn fit(twips: f32, spacing: LineSpacing, mark: &CharProps, space_after: f32) -> Got {
    measure(&document(twips / 20.0, spacing, &arial(10.0), mark, ("", 400.0), false, space_after))
}

#[test]
fn f1_automatic_fits() {
    let tnr = CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() };
    let g = fit(4930.0, LineSpacing::Multiple(1.0), &tnr, 0.0);
    assert_eq!((g.pages, g.pic_page), (1, 0));
}

#[test]
fn f2_at_least_12_fits() {
    let g = fit(4930.0, LineSpacing::AtLeast(12.0), &arial(10.0), 0.0);
    assert_eq!((g.pages, g.pic_page), (1, 0));
}

#[test]
fn f3_at_least_12_with_space_after_fits() {
    let g = fit(4930.0, LineSpacing::AtLeast(12.0), &arial(10.0), 12.0);
    assert_eq!((g.pages, g.pic_page), (1, 0));
}

#[test]
fn f4_at_least_12_bold_mark_fits() {
    let bold = CharProps { bold: Some(true), ..arial(10.0) };
    let g = fit(4930.0, LineSpacing::AtLeast(12.0), &bold, 0.0);
    assert_eq!((g.pages, g.pic_page), (1, 0));
}

#[test]
fn f5_at_least_12_past_the_margin_moves_on() {
    let g = fit(4975.0, LineSpacing::AtLeast(12.0), &arial(10.0), 0.0);
    assert_eq!((g.pages, g.pic_page), (2, 1));
}

#[test]
fn f6_at_least_12_with_room_fits() {
    let g = fit(4900.0, LineSpacing::AtLeast(12.0), &arial(10.0), 0.0);
    assert_eq!((g.pages, g.pic_page), (1, 0));
}
