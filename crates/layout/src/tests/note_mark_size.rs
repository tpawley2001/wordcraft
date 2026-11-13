//! A note reference mark formatted at the size of the text around it (a 10 pt mark in 10 pt text
//! in a 12 pt paragraph style, as Word 97-2003 files store it) leaves its line as tall as the
//! others: the superscript counts at its own 10 pt, not at the style's 12 pt.
use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{LineSpacing, ParaProps, VertAlign};
use wordcraft_doc::{PartKind, Pos, para_block};

/// The heights of the body lines of a 12 pt-style paragraph of 10 pt text with a footnote
/// reference in its middle, the reference formatted with `mark`.
fn line_heights(mark: CharProps) -> Vec<f32> {
    crate::test_fonts();
    let mut d = Document::new();
    if let Some(n) = Arc::make_mut(&mut d.styles).get_mut("Normal") {
        n.chr.size = Some(12.0);
    }
    let text = "Words of a report typed at ten points in a style of twelve points, long enough to fill three lines of the page, \
                and still a few more words here and there before the footnote reference somewhere in the middle line of the \
                paragraph, and then some more words after it, so that the last line has text of its own to finish the paragraph.";
    let mut p = Paragraph::with_text(text, CharProps { size: Some(10.0), ..Default::default() });
    p.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    d.body = vec![para_block(p)];
    let id = d.add_part(PartKind::Footnote, vec![para_block(Paragraph::with_text("Note.", Default::default()))]);
    let at = text.find("somewhere").unwrap();
    d.insert_object(&Pos::body(0, at), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &mark).unwrap();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let mut heights = Vec::new();
    for it in l.pages.iter().flat_map(|p| p.items.iter()) {
        if let Placed::Lines { story: StoryRef::Body, para, l0, l1, .. } = it {
            heights.extend(para.lines[*l0..*l1].iter().map(|line| line.height));
        }
    }
    heights
}

#[test]
fn a_mark_at_the_text_size_keeps_its_line_height() {
    let styled = CharProps { style: Some("FootnoteReference".into()), size: Some(10.0), ..Default::default() };
    let h = line_heights(styled);
    assert_eq!(h.len(), 3, "three lines: {h:?}");
    assert!(h.iter().all(|x| (x - h[0]).abs() < 0.01), "all lines equally tall: {h:?}");

    // A mark at the style's 12 pt makes its line taller (what the lost formatting gave).
    let style_size = CharProps { vert_align: Some(VertAlign::Superscript), ..Default::default() };
    let h12 = line_heights(style_size);
    assert_eq!(h12.len(), 3, "{h12:?}");
    assert!(h12[1] > h12[0] + 1.0 && h12[1] > h12[2] + 1.0, "the 12 pt mark's middle line is taller: {h12:?}");
}
