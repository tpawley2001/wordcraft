//! Unequal column widths, RTF 1.9.1 section formatting `\colnoN`, `\colwN`, `\colsrN`: the RTF form of
//! DOCX `w:cols w:equalWidth="0"` with its `w:col w:w w:space` children.

use super::*;
use wordcraft_doc::{Block, SectionProps};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn last(rtf: &str) -> SectionProps {
    doc(rtf).last_section.clone()
}

fn first_section(d: &Document) -> SectionProps {
    d.body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref().cloned(),
            _ => None,
        })
        .expect("first section")
}

#[test]
fn two_unequal_columns_are_read() {
    let s = last(r"{\rtf1\ansi\sectd\cols2\colsx720\colno1\colsr360\colw3000\colno2\colw5000 a\par}");
    assert_eq!((s.columns.count, s.columns.space), (2, 36.0));
    assert_eq!(s.columns.widths, vec![(150.0, 18.0), (250.0, 0.0)]);
}

#[test]
fn three_columns_in_any_word_order() {
    let s = last(r"{\rtf1\ansi\sectd\colno2\colw2000\colsr200\cols3\colno1\colw1000\colsr100\colno3\colw3000 a\par}");
    assert_eq!(s.columns.count, 3);
    assert_eq!(s.columns.widths, vec![(50.0, 5.0), (100.0, 10.0), (150.0, 0.0)]);
}

#[test]
fn a_column_without_a_width_leaves_them_equal() {
    for rtf in [
        r"{\rtf1\ansi\sectd\cols2\colno1\colw3000 a\par}",
        r"{\rtf1\ansi\sectd\cols2\colno1\colw3000\colno2\colw-100 a\par}",
        r"{\rtf1\ansi\sectd\cols2\colno1\colw3000\colno0\colw5000 a\par}",
        r"{\rtf1\ansi\sectd\cols2\colno1\colw3000\colno46\colw5000 a\par}",
        r"{\rtf1\ansi\sectd\colno1\colw3000\colno2\colw5000 a\par}",
    ] {
        let s = last(rtf);
        assert!(s.columns.widths.is_empty(), "{rtf}: {:?}", s.columns.widths);
    }
    assert_eq!(last(r"{\rtf1\ansi\sectd\colno1\colw3000\colno2\colw5000 a\par}").columns.count, 1);
}

#[test]
fn columns_past_the_count_are_ignored_and_values_clamp() {
    let s = last(r"{\rtf1\ansi\sectd\cols2\colno1\colw40000\colsr-5\colno2\colw5000\colno3\colw9000 a\par}");
    assert_eq!(s.columns.count, 2);
    assert_eq!(s.columns.widths, vec![(1584.0, 0.0), (250.0, 0.0)]);
}

#[test]
fn sectd_clears_them() {
    let d = doc(r"{\rtf1\ansi\sectd\cols2\colno1\colw3000\colno2\colw5000 a\par\sect\sectd\cols2 b\par}");
    assert_eq!(first_section(&d).columns.widths, vec![(150.0, 0.0), (250.0, 0.0)]);
    assert!(d.last_section.columns.widths.is_empty());
    assert_eq!(d.last_section.columns.count, 2);
}

#[test]
fn equal_columns_write_no_column_words() {
    let out = export(&doc(r"{\rtf1\ansi\sectd\cols2\colsx720 a\par}"));
    assert!(out.contains("\\cols2"), "{out}");
    for w in ["\\colno", "\\colw", "\\colsr"] {
        assert!(!out.contains(w), "{w} in {out}");
    }
}

#[test]
fn unequal_columns_survive_a_round_trip() {
    let src = doc(
        r"{\rtf1\ansi\sectd\cols3\colsx360\colno1\colsr100\colw1000\colno2\colsr300\colw2000\colno3\colw3000 a\par\sect\sectd\cols2\colno1\colsr360\colw3000\colno2\colw5000 b\par}",
    );
    let out = export(&src);
    assert!(out.contains("\\colno1\\colsr100\\colw1000\\colno2\\colsr300\\colw2000\\colno3\\colw3000"), "{out}");
    let back = doc(&out);
    assert_eq!(first_section(&back).columns.widths, vec![(50.0, 5.0), (100.0, 15.0), (150.0, 0.0)]);
    assert_eq!(back.last_section.columns.widths, vec![(150.0, 18.0), (250.0, 0.0)]);
    assert_eq!((first_section(&back).columns.count, back.last_section.columns.count), (3, 2));
}
