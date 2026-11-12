//! Tracked formatting changes ([MS-DOC] §2.6.1 sprmCPropRMark90 / sprmCPropRMark, §2.6.2
//! sprmPPropRMark90 / sprmPPropRMark): a run or a paragraph whose formatting was changed with
//! Track Changes on opens with a `Format` revision (author from SttbfRMark, date from the
//! DTTM), the way the DOCX reader keeps `w:rPrChange` and `w:pPrChange`. The mark stores no
//! previous formatting, so the change's `old` is the formatting as it is now.

use wordcraft_doc::RevisionKind;

use super::{RichExtra, RichSpec, dttm, grpprl, read, rich_doc_with};

const C_PROP_RMARK_90: u16 = 0xCA57;
const C_PROP_RMARK: u16 = 0xCA89;
const P_PROP_RMARK_90: u16 = 0xC63F;
const P_PROP_RMARK: u16 = 0xC66F;
const BOLD: u16 = 0x0835; // sprmCFBold
const P_JC: u16 = 0x2461; // sprmPJc

/// "plain " 0..6, "bold" 6..10, " tail" 10..15, the paragraph mark at 15, "next" 16..20.
const TEXT: &str = "plain bold tail\rnext\r";
const BOLD_AT: u32 = 6;
const TAIL_AT: u32 = 10;
const MARK_AT: u32 = 15;
const NEXT_AT: u32 = 16;

/// A PropRMarkOperand: cb (7), fPropRMark, ibstshort, dttm.
fn prop_rmark(on: bool, ibst: u16, at: [u8; 4]) -> Vec<u8> {
    let mut v = vec![7u8, u8::from(on)];
    v.extend_from_slice(&ibst.to_le_bytes());
    v.extend_from_slice(&at);
    v
}

fn d1() -> [u8; 4] {
    dttm(2015, 5, 25, 13, 47)
}

fn d2() -> [u8; 4] {
    dttm(2016, 4, 20, 14, 23)
}

fn open(chpx_runs: Vec<(u32, Vec<u8>)>, papx_entries: Vec<(u32, u16, Vec<u8>)>, extra: RichExtra) -> wordcraft_doc::Document {
    let spec = RichSpec { text: TEXT, styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())], fonts: vec!["Times New Roman"], chpx_runs, papx_entries };
    let extra = RichExtra { rmark_authors: vec!["Alice", "Bob"], ..extra };
    read(&rich_doc_with(&spec, &extra)).expect("opens")
}

fn plain_paras() -> Vec<(u32, u16, Vec<u8>)> {
    vec![(0, 0, Vec::new()), (NEXT_AT, 0, Vec::new())]
}

fn para(doc: &wordcraft_doc::Document, i: usize) -> &wordcraft_doc::Paragraph {
    doc.body.get(i).and_then(|b| b.as_para()).expect("paragraph")
}

fn rev(doc: &wordcraft_doc::Document, i: u32) -> Option<(RevisionKind, &str, &str)> {
    doc.revisions.get(i as usize).map(|r| (r.kind, r.author.as_str(), r.date.as_str()))
}

#[test]
fn a_run_formatting_change_keeps_its_author_and_date() {
    for op in [C_PROP_RMARK_90, C_PROP_RMARK] {
        let changed = grpprl(&[(BOLD, &[0x01]), (op, &prop_rmark(true, 1, d1()))]);
        let doc = open(vec![(0, Vec::new()), (BOLD_AT, changed), (TAIL_AT, Vec::new())], plain_paras(), RichExtra::default());
        let p = para(&doc, 0);
        assert_eq!(p.text, "plain bold tail");
        let c = p.props_of_char(6);
        assert_eq!(c.bold, Some(true), "sprm {op:#06X}: the new formatting shows");
        let ch = c.fmt_change.as_ref().unwrap_or_else(|| panic!("sprm {op:#06X}: 'bold' carries a formatting change: {:?}", p.runs));
        assert_eq!(rev(&doc, ch.rev), Some((RevisionKind::Format, "Bob", "2015-05-25T13:47:00Z")), "sprm {op:#06X}");
        assert_eq!(ch.old.bold, Some(true), "the mark stores no previous formatting: Reject changes nothing");
        assert!(ch.old.fmt_change.is_none() && ch.old.ins.is_none() && ch.old.del.is_none());
        assert!(p.props_of_char(0).fmt_change.is_none(), "'plain ' is untouched");
        assert!(p.props_of_char(10).fmt_change.is_none(), "' tail' is untouched");
        assert!(p.mark.fmt_change.is_none() && p.props.fmt_change.is_none());
        assert_eq!(doc.revisions.len(), 1);
    }
}

#[test]
fn a_cleared_property_mark_is_no_change() {
    let off = grpprl(&[(BOLD, &[0x01]), (C_PROP_RMARK_90, &prop_rmark(false, 1, d1()))]);
    let doc = open(vec![(0, Vec::new()), (BOLD_AT, off), (TAIL_AT, Vec::new())], plain_paras(), RichExtra::default());
    let p = para(&doc, 0);
    assert_eq!(p.props_of_char(6).bold, Some(true));
    assert!(p.props_of_char(6).fmt_change.is_none());
    assert!(doc.revisions.is_empty(), "no revision for a cleared mark: {:?}", doc.revisions);
}

#[test]
fn a_piece_prc_overrides_the_run_property_mark() {
    let on = grpprl(&[(C_PROP_RMARK_90, &prop_rmark(true, 1, d1()))]);
    let prcs = vec![grpprl(&[(C_PROP_RMARK_90, &prop_rmark(false, 0, [0; 4]))])];
    let doc = open(vec![(0, on)], plain_paras(), RichExtra { prcs, prm: 0x0001, ..Default::default() });
    let p = para(&doc, 0);
    assert!(p.props_of_char(0).fmt_change.is_none(), "the Prc applies after the CHPX");
    assert!(doc.revisions.is_empty(), "{:?}", doc.revisions);
}

#[test]
fn an_inserted_and_reformatted_run_keeps_both_changes() {
    let both = grpprl(&[(0x0801, &[0x01]), (0x4804, &[0x00, 0x00]), (0x6805, &d2()), (BOLD, &[0x01]), (C_PROP_RMARK_90, &prop_rmark(true, 1, d1()))]);
    let doc = open(vec![(0, Vec::new()), (BOLD_AT, both), (TAIL_AT, Vec::new())], plain_paras(), RichExtra::default());
    let c = para(&doc, 0).props_of_char(6);
    let ins = c.ins.expect("inserted");
    let ch = c.fmt_change.as_ref().expect("and reformatted");
    assert_eq!(rev(&doc, ins), Some((RevisionKind::Insert, "Alice", "2016-04-20T14:23:00Z")));
    assert_eq!(rev(&doc, ch.rev), Some((RevisionKind::Format, "Bob", "2015-05-25T13:47:00Z")));
    assert_eq!(doc.revisions.len(), 2);
}

#[test]
fn a_paragraph_mark_formatting_change_is_kept_on_the_mark() {
    let changed = grpprl(&[(BOLD, &[0x01]), (C_PROP_RMARK_90, &prop_rmark(true, 0, d2()))]);
    let doc = open(vec![(0, Vec::new()), (MARK_AT, changed), (NEXT_AT, Vec::new())], plain_paras(), RichExtra::default());
    let p = para(&doc, 0);
    let ch = p.mark.fmt_change.as_ref().unwrap_or_else(|| panic!("the mark carries the change: {:?}", p.mark));
    assert_eq!(rev(&doc, ch.rev), Some((RevisionKind::Format, "Alice", "2016-04-20T14:23:00Z")));
    assert!(p.props_of_char(10).fmt_change.is_none(), "the text before the mark is untouched");
    assert!(para(&doc, 1).mark.fmt_change.is_none());
}

#[test]
fn a_paragraph_formatting_change_keeps_its_author_and_date() {
    for op in [P_PROP_RMARK_90, P_PROP_RMARK] {
        let changed = grpprl(&[(P_JC, &[0x01]), (op, &prop_rmark(true, 0, d2()))]);
        let doc = open(vec![(0, Vec::new())], vec![(0, 0, changed), (NEXT_AT, 0, Vec::new())], RichExtra::default());
        let p = para(&doc, 0);
        assert_eq!(p.props.align, Some(wordcraft_doc::props::Align::Center), "sprm {op:#06X}: the new formatting shows");
        let ch = p.props.fmt_change.as_ref().unwrap_or_else(|| panic!("sprm {op:#06X}: the paragraph carries a formatting change"));
        assert_eq!(rev(&doc, ch.rev), Some((RevisionKind::Format, "Alice", "2016-04-20T14:23:00Z")), "sprm {op:#06X}");
        assert_eq!(ch.old.align, p.props.align, "the mark stores no previous formatting: Reject changes nothing");
        assert!(ch.old.fmt_change.is_none());
        assert!(p.props_of_char(0).fmt_change.is_none() && p.mark.fmt_change.is_none(), "a paragraph change, not a run change");
        let next = para(&doc, 1);
        assert_eq!(next.text, "next");
        assert!(next.props.fmt_change.is_none(), "the next paragraph is untouched");
        assert_eq!(doc.revisions.len(), 1);
    }
}

#[test]
fn a_cleared_paragraph_property_mark_is_no_change() {
    let off = grpprl(&[(P_JC, &[0x01]), (P_PROP_RMARK_90, &prop_rmark(false, 0, d2()))]);
    let doc = open(vec![(0, Vec::new())], vec![(0, 0, off), (NEXT_AT, 0, Vec::new())], RichExtra::default());
    let p = para(&doc, 0);
    assert_eq!(p.props.align, Some(wordcraft_doc::props::Align::Center));
    assert!(p.props.fmt_change.is_none());
    assert!(doc.revisions.is_empty(), "{:?}", doc.revisions);
}

#[test]
fn changes_by_one_author_at_one_time_share_a_revision() {
    let run = grpprl(&[(C_PROP_RMARK_90, &prop_rmark(true, 1, d1()))]);
    let par = grpprl(&[(P_PROP_RMARK_90, &prop_rmark(true, 1, d1()))]);
    let doc = open(vec![(0, run.clone()), (BOLD_AT, Vec::new()), (TAIL_AT, run), (MARK_AT, Vec::new())], vec![(0, 0, par), (NEXT_AT, 0, Vec::new())], RichExtra::default());
    let p = para(&doc, 0);
    let a = p.props_of_char(0).fmt_change.as_ref().map(|c| c.rev);
    let b = p.props_of_char(10).fmt_change.as_ref().map(|c| c.rev);
    let c = p.props.fmt_change.as_ref().map(|c| c.rev);
    assert!(a.is_some(), "'plain ' carries the change");
    assert!(p.props_of_char(6).fmt_change.is_none(), "'bold' in between does not");
    assert_eq!((a, a), (b, c), "one revision for all three");
    assert_eq!(doc.revisions.len(), 1, "{:?}", doc.revisions);
    assert_eq!(rev(&doc, a.unwrap_or(u32::MAX)), Some((RevisionKind::Format, "Bob", "2015-05-25T13:47:00Z")));
}
