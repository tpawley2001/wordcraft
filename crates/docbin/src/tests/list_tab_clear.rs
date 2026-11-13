//! A paragraph style that deletes its list level's number tab ([MS-DOC] sprmPChgTabsPapx
//! `rgdxaDel`): the deleted position reaches the resolved paragraph, so layout doesn't use the
//! level's tab stop there and the text goes to the next stop, as in Word (a 101-page report's
//! numbered paragraphs, level tab at 1080 tw deleted by their style: Word sets the text at the
//! next stop, 1440 tw).

use super::{RichSpec, grpprl, read, rich_doc, tabs_papx};
use crate::list::{RawLvl, level_of};
use wordcraft_doc::props::TabAlign;

#[test]
fn a_style_deleting_the_level_tab_clears_that_position() {
    // The level: number at a 720 tw first-line indent, list tab (jc 6) at 1080 tw.
    let papx = [0x0F, 0x84, 0, 0, 0x11, 0x84, 0xD0, 0x02, 0x15, 0xC6, 5, 0, 1, 0x38, 0x04, 6];
    let raw = RawLvl { start: 1, rgbxch_nums: [1, 0, 0, 0, 0, 0, 0, 0, 0], papx: papx.to_vec(), xst: vec![0, 0x2E], ..Default::default() };
    assert_eq!(level_of(&raw, 0, &[]).tab, Some(54.0));
    // ParaNum: left stops at 720 and 1440 tw, and 1080 tw deleted.
    let para_num = grpprl(&[(0xC60D, &tabs_papx(&[1080], &[(720, 0), (1440, 0)]))]);
    let spec = RichSpec {
        text: "Section\rPlain\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "ParaNum", para_num, Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 1, Vec::new()), (8, 0, grpprl(&[(0xC60D, &tabs_papx(&[1080], &[]))]))],
    };
    let doc = read(&rich_doc(&spec)).expect("opens");
    let rp = |i: usize| doc.styles.resolve_para(&doc.body.get(i).and_then(|b| b.as_para()).expect("para").props);
    let styled = rp(0);
    assert_eq!(styled.style, "ParaNum");
    assert_eq!(styled.tabs.iter().map(|t| (t.pos, t.align)).collect::<Vec<_>>(), [(36.0, TabAlign::Left), (72.0, TabAlign::Left)]);
    assert_eq!(styled.tab_clears, [54.0], "the style's deleted stop");
    assert_eq!(rp(1).tab_clears, [54.0], "the paragraph's own deleted stop");
}

/// A PChgTabs operand claiming 255 deleted stops in a short buffer reads what is there and
/// never panics.
#[test]
fn a_short_operand_with_itbd_del_max_255_is_safe() {
    for operand in [vec![3u8, 255, 0x38, 0x04], vec![1, 255], vec![255, 255, 0x38]] {
        for op in [0xC615u16, 0xC60D] {
            let spec = RichSpec {
                text: "x\r",
                styles: vec![(1, 0x0FFF, "Normal", grpprl(&[(op, &operand)]), Vec::new())],
                fonts: vec!["Calibri"],
                chpx_runs: vec![(0, Vec::new())],
                papx_entries: vec![(0, 0, grpprl(&[(op, &operand)]))],
            };
            if let Ok(doc) = read(&rich_doc(&spec)) {
                let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
                assert!(doc.styles.resolve_para(&p.props).tab_clears.len() <= 64);
            }
        }
    }
}
