//! Tracked section formatting changes (`{\*\oldsprops …}`, RTF 1.9.1), the RTF form of `w:sectPrChange`.

use super::*;
use wordcraft_doc::props::PropChange;
use wordcraft_doc::{Block, Revision, RevisionKind, SectionProps};

/// Sets section `s` on body paragraph `i`.
fn set_section(d: &mut Document, i: usize, s: SectionProps) {
    if let Some(Block::Para(p)) = d.body.get_mut(i).map(std::sync::Arc::make_mut) {
        p.section = Some(Box::new(s));
    }
}

#[test]
fn section_formatting_changes_round_trip() {
    // Alice widened the first section's left margin from 72 to 90 pt; Bob turned the last
    // section from portrait to landscape.
    let mut d = Document::from_text("one\ntwo");
    let old1 = SectionProps { margin_left: 72.0, ..Default::default() };
    let s1 = SectionProps { margin_left: 90.0, fmt_change: PropChange::boxed(0, old1), ..Default::default() };
    set_section(&mut d, 0, s1);
    let old2 = SectionProps::default();
    let (pw, ph) = (old2.page_h, old2.page_w);
    d.last_section = SectionProps { page_w: pw, page_h: ph, landscape: true, fmt_change: PropChange::boxed(1, old2.clone()), ..Default::default() };
    d.revisions = vec![
        Revision::new(RevisionKind::Format, "Alice", "2024-03-15T10:30:00Z"),
        Revision::new(RevisionKind::Format, "Bob", "2025-01-02T08:05:00Z"),
    ];
    let rtf = export(&d);
    assert_eq!(rtf.matches("{\\*\\oldsprops").count(), 2, "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    let secs = back.sections();
    assert_eq!(secs.len(), 2, "{rtf}");
    let (_, a) = secs[0];
    assert!((a.margin_left - 90.0).abs() < 0.1, "{rtf}");
    let ch = a.fmt_change.as_deref().expect("first section's change read back");
    assert!((ch.old.margin_left - 72.0).abs() < 0.1, "{rtf}");
    assert_eq!(back.revisions.get(ch.rev as usize).map(|r| r.kind), Some(RevisionKind::Format));
    let (_, b) = secs[1];
    assert!(b.landscape && b.page_w > b.page_h, "{rtf}");
    let ch = b.fmt_change.as_deref().expect("last section's change read back");
    assert!(!ch.old.landscape, "{rtf}");
    assert!((ch.old.page_w - old2.page_w).abs() < 0.1 && (ch.old.page_h - old2.page_h).abs() < 0.1, "{rtf}");
    assert_eq!(back.revisions.get(ch.rev as usize).map(|r| r.kind), Some(RevisionKind::Format));
}

#[test]
fn oldsprops_words_set_the_old_section_not_the_current_one() {
    // Letter portrait with a 1800-twip (90 pt) left margin now; landscape with 1440 (72 pt) before.
    let rtf = br"{\rtf1\ansi{\*\revtbl {Unknown;}{Alice;}}\sectd\pgwsxn12240\pghsxn15840\marglsxn1800{\*\oldsprops \sectd\pgwsxn15840\pghsxn12240\marglsxn1440\lndscpsxn}\pard one\par}";
    let doc = import(rtf).unwrap();
    let secs = doc.sections();
    assert_eq!(secs.len(), 1);
    let (_, s) = secs[0];
    assert!((s.margin_left - 90.0).abs() < 0.1, "{s:?}");
    assert!(!s.landscape, "{s:?}");
    let ch = s.fmt_change.as_deref().expect("section change read");
    assert!((ch.old.margin_left - 72.0).abs() < 0.1, "{:?}", ch.old);
    assert!(ch.old.landscape, "{:?}", ch.old);
    assert_eq!(doc.revisions.get(ch.rev as usize).map(|r| r.kind), Some(RevisionKind::Format));
    // Its text is untouched.
    assert_eq!(doc_paras(&doc).first().map(|p| p.text.as_str()), Some("one"));
}

#[test]
fn an_unchanged_section_writes_no_oldsprops() {
    let mut d = Document::from_text("one\ntwo");
    set_section(&mut d, 0, SectionProps { margin_left: 90.0, ..Default::default() });
    let rtf = export(&d);
    assert!(!rtf.contains("oldsprops"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert!(back.sections().iter().all(|(_, s)| s.fmt_change.is_none()), "{rtf}");
}
