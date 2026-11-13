//! A Word 97-2003 file with `Copts80.f2ptExtLeadingOnly` lays a line of any font but Courier New
//! out its size plus 2 pt tall. A superscript counts at its full font size, and the 2 pt are added
//! to that once. Measured in Microsoft 365 Word's PDF of such a file (11 pt Times New Roman text,
//! footnote marks in a 12 pt superscript character style): plain lines are 13.0 pt apart, a line
//! with a mark starts 13.8 pt below the plain line before it, and the next line with a mark is
//! 14.1 pt below that. A 12 pt line plus 2 pt gives those distances. We used to add the 2 pt to
//! the shrunken superscript and then scale it up with the size, which gave 3 pt.

use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{LineSpacing, ParaProps, VertAlign};
use wordcraft_doc::{PartKind, Pos, para_block};

fn tnr(size: f32) -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() }
}

/// The body line heights of a paragraph of 11 pt text. Its second line holds a footnote
/// reference formatted with `mark`.
fn line_heights(flag: bool, mark: CharProps) -> Vec<f32> {
    crate::test_fonts();
    let mut d = Document::new();
    d.settings.ext_leading_2pt_only = flag;
    d.settings.compat_mode = 11;
    let text = "Words of a report typed at eleven points, long enough to fill three lines of the page, \
                and still a few more words here and there before the footnote reference somewhere in the middle line of the \
                paragraph, and then some more words after it, so that the last line has text of its own to finish the paragraph.";
    let mut p = Paragraph::with_text(text, tnr(11.0));
    p.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    d.body = vec![para_block(p)];
    let id = d.add_part(PartKind::Footnote, vec![para_block(Paragraph::with_text("Note.", tnr(10.0)))]);
    let at = text.find("somewhere").unwrap();
    d.insert_object(&Pos::body(0, at), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &mark).unwrap();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let mut heights = Vec::new();
    for it in l.pages.iter().flat_map(|p| p.items.iter()) {
        if let Placed::Lines { story: StoryRef::Body, para, l0, l1, .. } = it {
            heights.extend(para.lines[*l0..*l1].iter().map(|line| line.height));
        }
    }
    assert!(heights.len() >= 3, "at least three lines: {heights:?}");
    heights
}

fn superscript(size: f32) -> CharProps {
    CharProps { vert_align: Some(VertAlign::Superscript), ..tnr(size) }
}

#[test]
fn a_superscript_line_is_its_full_size_plus_2pt() {
    let h = line_heights(true, superscript(12.0));
    let plain = h.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, x)| *x);
    assert!(plain.clone().all(|x| (x - 13.0).abs() < 0.05), "plain lines are 11 + 2 pt: {h:?}");
    assert!((h[1] - 14.0).abs() < 0.05, "the line with a 12 pt superscript is 12 + 2 pt: {h:?}");
}

#[test]
fn a_superscript_at_the_text_size_keeps_the_line() {
    let h = line_heights(true, superscript(11.0));
    assert!(h.iter().all(|x| (x - 13.0).abs() < 0.05), "all lines 11 + 2 pt: {h:?}");
}

#[test]
fn without_the_flag_a_superscript_still_counts_at_its_full_size() {
    let plain = line_heights(false, superscript(11.0));
    let big = line_heights(false, superscript(12.0));
    assert!((plain[1] - plain[0]).abs() < 0.05, "{plain:?}");
    // The font's own line at 12 pt instead of 11 pt.
    let k = big[0] / 11.0;
    assert!((big[1] - 12.0 * k).abs() < 0.05, "{big:?}");
}
