//! Without `\widowctrl`, every paragraph that doesn't set widow control gets it off, in every
//! story: table cells (nested too), headers and footers, notes, comments and text boxes.

use super::*;
use wordcraft_doc::{Block, Document, PartKind, StoryRef};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn widow(d: &Document, s: StoryRef) -> Vec<Option<bool>> {
    d.para_paths(s).into_iter().filter_map(|p| d.para(s, &p)).map(|p| p.props.widow_control).collect()
}

/// Every paragraph's widow control in `blocks`, tables descended.
fn all_widow(blocks: &[std::sync::Arc<Block>], out: &mut Vec<Option<bool>>) {
    for b in blocks {
        match &**b {
            Block::Para(p) => out.push(p.props.widow_control),
            Block::Table(t) => {
                for row in &t.rows {
                    for c in &row.cells {
                        all_widow(&c.blocks, out);
                    }
                }
            }
        }
    }
}

#[test]
fn table_cell_paragraphs_are_off() {
    let d = open(r"\trowd\cellx2000\cellx4000\pard\intbl cell\cell\pard\intbl cell\cell\row\pard after\par");
    let mut all = Vec::new();
    all_widow(&d.body, &mut all);
    assert_eq!(all, [Some(false), Some(false), Some(false)]);
}

#[test]
fn nested_table_cell_paragraphs_are_off() {
    let d = open(r"\trowd\cellx2000\pard\intbl outer\par\trowd\cellx1000\pard\intbl inner\cell\row\cell\row\pard after\par");
    let mut all = Vec::new();
    all_widow(&d.body, &mut all);
    assert!(all.len() >= 3, "{all:?}");
    assert!(all.iter().all(|w| *w == Some(false)), "{all:?}");
}

#[test]
fn header_and_footer_paragraphs_are_off() {
    let d = open(r"{\header head\par}{\footer foot\par}\pard body\par");
    let mut seen = 0;
    for (id, part) in &d.parts {
        if matches!(part.kind, PartKind::Header | PartKind::Footer) {
            assert_eq!(widow(&d, StoryRef::Part(*id)), [Some(false)]);
            seen += 1;
        }
    }
    assert_eq!(seen, 2);
    assert_eq!(widow(&d, StoryRef::Body), [Some(false)]);
}

#[test]
fn note_comment_and_text_box_paragraphs_are_off() {
    let d = open(
        r"text{\footnote note\par}{\*\atnid a1}{\*\atnauthor A}{\*\atnref 1}{\*\atrfstart 1}commented{\*\atrfend 1}{\*\atnid A}{\*\atnauthor A}\chatn{\*\annotation{\*\atnref 1}note}{\shp{\*\shpinst\shpleft0\shptop0\shpright2000\shpbottom1000{\shptxt box\par}}}\par",
    );
    let mut seen = 0;
    for (id, part) in &d.parts {
        if matches!(part.kind, PartKind::Footnote | PartKind::Endnote | PartKind::TextBox) {
            assert_eq!(widow(&d, StoryRef::Part(*id)), [Some(false)], "{:?}", part.kind);
            seen += 1;
        }
    }
    assert!(seen >= 2, "footnote and text box read: {seen}");
    assert!(!d.comments.is_empty(), "comment read");
    for c in d.comments.values() {
        assert_eq!(widow(&d, StoryRef::Part(c.part)), [Some(false)], "comment");
    }
}

#[test]
fn with_widowctrl_stories_stay_unset() {
    let d = open(r"\widowctrl{\header head\par}\pard body\par");
    assert_eq!(widow(&d, StoryRef::Body), [None]);
    for (id, part) in &d.parts {
        if part.kind == PartKind::Header {
            assert_eq!(widow(&d, StoryRef::Part(*id)), [None]);
        }
    }
}
