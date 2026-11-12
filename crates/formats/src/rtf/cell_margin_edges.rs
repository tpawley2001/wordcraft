//! A cell's own margins and Wrap text off beyond the acceptance tests: margins a document sets
//! on all four sides (as DOCX gives them) are written back, cells of nested tables keep theirs,
//! and the unit word before or after its value counts the same.

use super::*;
use wordcraft_doc::{Block, Document, Table};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}{body}}}").as_bytes()).unwrap()
}

fn first_table(d: &Document) -> &Table {
    d.body.iter().find_map(|b| b.as_table()).unwrap()
}

#[test]
fn four_sides_set_in_the_document_are_written_and_read_back() {
    let mut d = open(r"\trowd\cellx3000\cellx6000\pard\intbl a\cell b\cell\row\pard after\par");
    let i = d.body.iter().position(|b| b.as_table().is_some()).unwrap();
    let Block::Table(t) = std::sync::Arc::make_mut(&mut d.body[i]) else { unreachable!() };
    t.rows[0].cells[1].props.margins = Some([1.0, 2.5, 3.0, 4.0]);
    t.rows[0].cells[1].props.no_wrap = true;
    let rtf = export(&d);
    for word in [r"\clpadl20\clpadfl3", r"\clpadt50\clpadft3", r"\clpadb60\clpadfb3", r"\clpadr80\clpadfr3", r"\clNoWrap"] {
        assert!(rtf.contains(word), "{word} missing in {rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap();
    let p = &first_table(&back).rows[0].cells[1].props;
    assert_eq!((p.margins, p.partial_margins, p.no_wrap), (Some([1.0, 2.5, 3.0, 4.0]), None, true));
    let p = &first_table(&back).rows[0].cells[0].props;
    assert_eq!((p.margins, p.partial_margins, p.no_wrap), (None, None, false));
}

#[test]
fn the_value_after_its_unit_word_and_a_repeated_trowd_reset() {
    // `\trowd` drops a half-written definition's padding and Wrap text setting.
    let d = open(r"\trowd\clpadfl3\clpadl100\clNoWrap\trowd\clpadfb3\clpadb40\cellx3000\cellx6000\pard\intbl a\cell b\cell\row\pard after\par");
    let c = &first_table(&d).rows[0].cells[0].props;
    assert_eq!((c.margins, c.partial_margins, c.no_wrap), (None, Some([None, None, Some(2.0), None]), false));
}

#[test]
fn nested_table_cells_keep_their_own_margins_and_no_wrap() {
    let d = open(concat!(
        r"\trowd\cellx6000\pard\intbl outer\par",
        r"\pard\intbl\itap2 inner\nestcell{\*\nesttableprops\trowd\clpadt200\clpadft3\clNoWrap\cellx3000\nestrow}{\nonesttables\par}",
        r"\pard\intbl\cell\row\pard after\par"
    ));
    let outer = first_table(&d);
    let inner = outer.rows[0].cells[0].blocks.iter().find_map(|b| b.as_table()).unwrap();
    let p = &inner.rows[0].cells[0].props;
    assert_eq!((p.partial_margins, p.no_wrap), (Some([None, Some(10.0), None, None]), true));
    assert_eq!(outer.rows[0].cells[0].props.partial_margins, None);
    let back = import(export(&d).as_bytes()).unwrap();
    let outer = first_table(&back);
    let inner = outer.rows[0].cells[0].blocks.iter().find_map(|b| b.as_table()).unwrap();
    let p = &inner.rows[0].cells[0].props;
    assert_eq!((p.partial_margins, p.no_wrap), (Some([None, Some(10.0), None, None]), true));
}
