//! Gutter position Top (`w:gutterAtTop`, ECMA-376 Part 1 §17.15.1.49). Word then adds each
//! section's gutter to its top margin, on every page, and leaves the left and right margins
//! plain: mirror margins still swap them, but no page has an inside gutter, and `w:rtlGutter`
//! has nothing to move. Headers and footers keep their distance from the page edge and the plain
//! left margin. The document is laid out with its sections changed that way.

use std::sync::Arc;

use wordcraft_doc::{Block, Document, SectionProps};

/// `doc` with every section's gutter moved into its top margin, or `None` when the gutter is
/// not at the top or no section has one.
pub(crate) fn plan(doc: &Document) -> Option<Document> {
    if !doc.settings.gutter_at_top || !doc.sections().iter().any(|(_, s)| has_gutter(s)) {
        return None;
    }
    let mut out = doc.clone();
    for b in &mut out.body {
        if b.as_para().is_some_and(|p| p.section.as_deref().is_some_and(has_gutter))
            && let Block::Para(p) = Arc::make_mut(b)
            && let Some(s) = p.section.as_deref_mut()
        {
            to_top(s);
        }
    }
    to_top(&mut out.last_section);
    Some(out)
}

fn has_gutter(s: &SectionProps) -> bool {
    s.gutter.is_finite() && s.gutter > 0.0
}

fn to_top(s: &mut SectionProps) {
    if has_gutter(s) {
        s.margin_top += s.gutter;
        s.gutter = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(gutter: f32, at_top: bool) -> Document {
        let mut d = Document::default();
        d.last_section.gutter = gutter;
        d.last_section.margin_top = 72.0;
        d.settings.gutter_at_top = at_top;
        d
    }

    #[test]
    fn the_gutter_moves_to_the_top_margin() {
        let out = plan(&doc(36.0, true)).unwrap();
        assert_eq!((out.last_section.margin_top, out.last_section.gutter), (108.0, 0.0));
    }

    #[test]
    fn nothing_changes_without_gutter_at_top_or_a_gutter() {
        assert!(plan(&doc(36.0, false)).is_none());
        assert!(plan(&doc(0.0, true)).is_none());
        assert!(plan(&doc(f32::NAN, true)).is_none());
    }
}
