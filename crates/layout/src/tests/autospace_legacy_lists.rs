//! Automatic (HTML) paragraph spacing between list items in a Word 2007-mode document
//! (compatibility mode 12, no `w:compatibilityMode`): Word drops the automatic space between
//! items of one list (one `w:num`, at any level) but keeps it, 14 pt, between items of two
//! different lists (Microsoft 365 Word's PDFs of two real Word 2007 documents). Mode 15 still
//! drops it between any two items (`automatic_spacing_drops_between_list_items`).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::props::{NumRef, ParaProps};
use wordcraft_doc::{Paragraph, Pos, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn auto() -> ParaProps {
    ParaProps { space_before: Some(5.0), space_after: Some(5.0), space_before_auto: Some(true), space_after_auto: Some(true), ..Default::default() }
}

fn plain() -> ParaProps {
    ParaProps { space_before: Some(0.0), space_after: Some(0.0), ..Default::default() }
}

/// `items` as (list 1 or 2, or 0 for none; level; automatic spacing), between two plain
/// paragraphs, in compatibility mode `mode`: the extra gap above paragraphs 1.. (beyond the line
/// pitch), rounded to 0.1 pt.
fn gaps(mode: u32, items: &[(u32, u8, bool)]) -> Vec<f32> {
    let build = |items: &[(u32, u8, bool)]| {
        let mut d = Document::new();
        d.settings.compat_mode = mode;
        let numbering = std::sync::Arc::make_mut(&mut d.numbering);
        let a = numbering.add_list(wordcraft_doc::ListKind::Bullet);
        let b = numbering.add_list(wordcraft_doc::ListKind::Bullet);
        let mut paras = vec![plain()];
        for &(list, level, is_auto) in items {
            let mut p = if is_auto { auto() } else { plain() };
            if list > 0 {
                p.numbering = Some(NumRef { num: if list == 1 { a } else { b }, level });
            }
            paras.push(p);
        }
        paras.push(plain());
        d.body = paras
            .into_iter()
            .enumerate()
            .map(|(i, props)| {
                let mut para = Paragraph::with_text(&format!("P{i}"), Default::default());
                para.props = props;
                para_block(para)
            })
            .collect();
        let l = lay(&d);
        (0..d.body.len()).map(|i| l.caret(&Pos::body(i, 0)).unwrap().top).collect::<Vec<f32>>()
    };
    let base = build(&vec![(0, 0, false); items.len()]);
    let line = base[1] - base[0];
    let tops = build(items);
    (1..tops.len()).map(|i| ((tops[i] - tops[i - 1] - line) * 10.0).round() / 10.0).collect()
}

#[test]
fn legacy_mode_keeps_automatic_spacing_between_two_lists() {
    let cases: [(&str, Vec<(u32, u8, bool)>, Vec<f32>); 4] = [
        ("one list, levels", vec![(1, 0, true), (1, 1, true), (1, 0, true)], vec![14.0, 0.0, 0.0, 14.0]),
        ("one list", vec![(1, 0, true), (1, 0, true), (1, 0, true)], vec![14.0, 0.0, 0.0, 14.0]),
        ("two lists", vec![(1, 0, true), (2, 0, true), (2, 0, true)], vec![14.0, 14.0, 0.0, 14.0]),
        ("into another list's level 1 and back", vec![(1, 0, true), (2, 1, true), (2, 2, true), (1, 0, true)], vec![14.0, 14.0, 0.0, 14.0, 14.0]),
    ];
    for (what, items, want) in cases {
        assert_eq!(gaps(wordcraft_doc::LEGACY_COMPAT_MODE, &items), want, "{what} (mode 12)");
    }
}

#[test]
fn current_mode_still_drops_automatic_spacing_between_two_lists() {
    assert_eq!(gaps(15, &[(1, 0, true), (2, 0, true), (2, 0, true)]), vec![14.0, 0.0, 0.0, 14.0]);
    assert_eq!(gaps(14, &[(1, 0, true), (2, 1, true), (1, 0, true)]), vec![14.0, 0.0, 0.0, 14.0]);
}
