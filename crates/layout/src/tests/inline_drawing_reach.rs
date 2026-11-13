//! Drawings in line with text, as a Word 97-2003 file's `SHAPE` fields give them: each one sits
//! on the baseline where the text position is, and the line keeps room for an outline and a
//! shadow around it (the object's effect extent). Built from model objects only.
use super::*;
use wordcraft_doc::para::Float;
use wordcraft_doc::props::LineSpacing;

fn image(w: f32, h: f32, effect: [f32; 4]) -> InlineObject {
    InlineObject::Image {
        media: "m".into(),
        w,
        h,
        alt: String::new(),
        float: Float { effect, ..Default::default() },
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    }
}

/// Single spacing, no space before or after, for every paragraph.
fn single(d: &mut Document) {
    for i in 0..d.body.len() {
        if let Ok(p) = d.para_mut(StoryRef::Body, &Path::top(i)) {
            p.props =
                ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
        }
    }
}

fn drawn_images(d: &Document, l: &DocLayout) -> Vec<Rect> {
    let mut v: Vec<Rect> = display::page_display(d, &l.pages[0], &Default::default())
        .into_iter()
        .filter_map(|it| if let display::Draw::Image { rect, .. } = it { Some(rect) } else { None })
        .collect();
    v.sort_by(|a, b| a.x.total_cmp(&b.x));
    v
}

/// The body paragraphs' (top of the placed lines, their first line) on the first page.
fn para_lines(l: &DocLayout) -> Vec<(f32, para::Line)> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|i| match i.clone() {
            Placed::Lines { story: StoryRef::Body, para, l0, y, .. } => para.lines.get(l0).map(|line| (y, line.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn two_inline_pictures_follow_each_other_on_the_baseline() {
    crate::test_fonts();
    let mut d = Document::from_text(" ");
    single(&mut d);
    // Picture, space, picture (the second inserted first, so the space's offset holds).
    d.insert_object(&Pos::body(0, 1), image(224.75, 198.0, [0.0; 4]), &Default::default()).unwrap();
    d.insert_object(&Pos::body(0, 0), image(228.0, 172.2, [0.0; 4]), &Default::default()).unwrap();
    let l = lay(&d);
    let got = drawn_images(&d, &l);
    assert_eq!(got.len(), 2, "{got:?}");
    let (a, b) = (got[0], got[1]);
    assert!((a.w, b.w) == (228.0, 224.75), "{got:?}");
    assert!(b.x >= a.x + 228.0 - 0.01, "the second starts after the first: {got:?}");
    assert!((a.y + a.h - (b.y + b.h)).abs() < 0.01, "both bottoms on the baseline: {got:?}");
    let lines = para_lines(&l);
    assert!(lines[0].1.height >= 198.0 - 0.01, "{:?}", lines[0].1);
}

#[test]
fn an_inline_picture_with_outline_and_shadow_makes_its_line_taller() {
    crate::test_fonts();
    let mut d = Document::from_text("\nNext");
    single(&mut d);
    d.insert_object(&Pos::body(0, 0), image(209.2, 156.85, [0.75, 7.5, 7.5, 0.75]), &Default::default()).unwrap();
    let l = lay(&d);
    let lines = para_lines(&l);
    assert!(lines.len() >= 2, "{lines:?}");
    let (top, line) = (lines[0].0 + lines[0].1.top, &lines[0].1);
    let next = lines[1].0 + lines[1].1.top;
    assert!((next - top - 165.1).abs() <= 0.3, "next paragraph {} below this one", next - top);
    let got = drawn_images(&d, &l);
    assert_eq!(got.len(), 1, "{got:?}");
    let Some(Placed::Lines { x, .. }) = l.pages[0].items.iter().find(|i| matches!(i, Placed::Lines { .. })) else { panic!() };
    let start = x + line.left;
    assert!((got[0].x - start - 0.75).abs() < 0.05, "picture at {:?}, line starts at {start}", got[0]);
    assert!((got[0].y - top - 7.5).abs() < 0.05, "picture at {:?}, line top {top}", got[0]);
}
