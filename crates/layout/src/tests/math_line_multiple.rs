//! With multiple line spacing (`w:lineRule="auto"`), a line whose tallest item is an inline
//! equation is as tall as at single spacing plus the spacing's share of the text's line height:
//! the equation's own height is not multiplied (measured in Microsoft 365 Word with a 3x3 matrix
//! at single, 1.15, 1.5 and double spacing: 38.04, + 2.16, + 7.20 and + 14.16 pt for a 13.80 pt
//! text line). Synthetic documents only.

use super::*;
use wordcraft_doc::props::LineSpacing;

const MULTIPLES: [f32; 4] = [1.0, 1.15, 1.5, 2.0];

/// Paragraphs TOP, "MID <equation> END" (or "MID END" without one) and NEXT at multiple `m`.
fn doc(equation: Option<&str>, m: f32) -> Document {
    crate::test_fonts();
    let mut d = Document::from_text("TOP\nMID  END\nNEXT");
    Arc::make_mut(&mut d.styles).default_chr.font = Some(WRAP_TEST_FONT.into());
    Arc::make_mut(&mut d.styles).default_chr.size = Some(12.0);
    for block in &mut d.body {
        if let Block::Para(para) = Arc::make_mut(block) {
            para.props.line_spacing = Some(LineSpacing::Multiple(m));
            para.props.space_before = Some(0.0);
            para.props.space_after = Some(0.0);
        }
    }
    if let Some(s) = equation {
        let math = wordcraft_doc::math::Math { nodes: wordcraft_doc::math::parse_linear(s), ..Default::default() };
        let p = d.para_mut(StoryRef::Body, &Path::top(1)).unwrap();
        p.insert_object(4, InlineObject::Equation { linear: s.into(), display: false, math }, &Default::default()).unwrap();
    }
    d
}

/// The heights of the lines of body paragraph `block`.
fn heights(d: &Document, block: u32) -> Vec<f32> {
    let l = lay(d);
    let mut out = Vec::new();
    for page in &l.pages {
        for it in &page.items {
            if let Placed::Lines { para, l0, l1, story: StoryRef::Body, path, .. } = it
                && path.0 == [block]
            {
                out.extend(para.lines[*l0..*l1].iter().map(|li| li.height));
            }
        }
    }
    out
}

fn mid_height(equation: Option<&str>, m: f32) -> f32 {
    let h = heights(&doc(equation, m), 1);
    assert_eq!(h.len(), 1, "MID is one line: {h:?}");
    h[0]
}

/// The text-only line takes the multiple of the text's line height.
#[test]
fn a_text_line_takes_the_multiple_of_its_height() {
    let text = mid_height(None, 1.0);
    for m in MULTIPLES {
        let h = mid_height(None, m);
        assert!((h - m * text).abs() < 0.05, "at {m}: {h} vs {}", m * text);
    }
}

/// A 3x3 matrix line grows by the spacing's share of the text line only.
#[test]
fn an_equation_line_grows_by_the_text_lines_share_only() {
    let text = mid_height(None, 1.0);
    let single = mid_height(Some("■(a&b&c@d&e&f@g&h&i)"), 1.0);
    assert!(single > text + 5.0, "the matrix is taller than the text: {single} vs {text}");
    for m in MULTIPLES {
        let h = mid_height(Some("■(a&b&c@d&e&f@g&h&i)"), m);
        let want = single + (m - 1.0) * text;
        assert!((h - want).abs() < 0.05, "at {m}: {h}, want {want} (single {single}, text {text})");
    }
}

/// An equation no taller than the text leaves the line as tall as text alone at any multiple.
#[test]
fn a_short_equation_line_takes_the_multiple_of_the_text() {
    let text = mid_height(None, 1.0);
    for m in MULTIPLES {
        let h = mid_height(Some("x"), m);
        assert!((h - m * text).abs() < 0.05, "at {m}: {h} vs {}", m * text);
    }
}
