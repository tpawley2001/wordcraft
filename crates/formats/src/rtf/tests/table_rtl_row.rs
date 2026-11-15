//! Right-to-left tables through RTF (RTF 1.9.1, "Table row definitions"): `\rtlrow` in a row's
//! `\trowd` definition makes the table right-to-left (`TableProps::bidi_visual`), `\ltrrow` or
//! nothing leaves it left-to-right, and export writes `\rtlrow` back.

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn tables(d: &Document) -> Vec<&wordcraft_doc::Table> {
    d.body.iter().filter_map(|b| b.as_table()).collect()
}

fn row(words: &str) -> String {
    format!(r"\trowd{words}\cellx2000\cellx4000 \pard\intbl A\cell B\cell\row ")
}

fn rtl_of(body: &str) -> Vec<bool> {
    tables(&open(body)).iter().map(|t| t.props.bidi_visual).collect()
}

#[test]
fn rtlrow_makes_the_table_right_to_left() {
    assert_eq!(rtl_of(&format!(r"{}{}\pard After\par", row(r"\rtlrow"), row(r"\rtlrow"))), vec![true]);
}

#[test]
fn ltrrow_or_nothing_is_left_to_right() {
    assert_eq!(rtl_of(&format!(r"{}\pard After\par", row(""))), vec![false], "no word");
    assert_eq!(rtl_of(&format!(r"{}\pard After\par", row(r"\ltrrow"))), vec![false], "\\ltrrow");
    assert_eq!(rtl_of(&format!(r"{}\pard After\par", row(r"\rtlrow\ltrrow"))), vec![false], "last word wins");
}

#[test]
fn trowd_resets_the_direction() {
    let body = format!(r"{}\pard Mid\par {}\pard After\par", row(r"\rtlrow"), row(""));
    assert_eq!(rtl_of(&body), vec![true, false]);
}

#[test]
fn export_writes_rtlrow_and_reads_it_back() {
    let d = open(&format!(r"{}{}\pard After\par", row(r"\rtlrow"), row(r"\rtlrow")));
    assert!(tables(&d)[0].props.bidi_visual, "precondition: read as right-to-left");
    let rtf = export(&d);
    let defs: Vec<&str> = rtf.split(r"\trowd").skip(1).map(|r| &r[..r.find(r"\cell").unwrap_or(r.len())]).collect();
    assert!(!defs.is_empty(), "no row definitions in {rtf}");
    for def in &defs {
        assert!(def.contains(r"\rtlrow"), "row definition without \\rtlrow: {def}");
    }
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(tables(&back).iter().map(|t| t.props.bidi_visual).collect::<Vec<_>>(), vec![true]);
}

#[test]
fn export_of_an_ordinary_table_has_no_rtlrow() {
    let d = open(&format!(r"{}\pard After\par", row("")));
    assert!(!export(&d).contains(r"\rtlrow"));
}

// Table preferred width (`\trftsWidthN` 1 = auto, 2 = fiftieths of a percent, 3 = twips;
// `\trwWidthN`) and floating-table position (`\tph*`/`\tposx*`, `\tpv*`/`\tposy*`,
// `\tdfrmtxt*`, `\tabsnoovrlp`), RTF 1.9.1 "Table row definitions" / "Positioned tables".

use wordcraft_doc::para::{Anchor, FloatAlign};
use wordcraft_doc::props::TableFloat;

fn first_props(words: &str) -> wordcraft_doc::props::TableProps {
    tables(&open(&format!(r"{}\pard After\par", row(words))))[0].props.clone()
}

#[test]
fn preferred_width_in_twips_and_percent() {
    let p = first_props(r"\trftsWidth3\trwWidth5000");
    assert_eq!((p.width, p.width_pct), (Some(250.0), None), "twips");
    let p = first_props(r"\trftsWidth2\trwWidth2500");
    assert_eq!(p.width_pct, Some(50.0), "fiftieths of a percent");
    let p = first_props(r"\trftsWidth1");
    assert_eq!((p.width, p.width_pct), (None, None), "auto");
}

#[test]
fn positioned_table_reads_anchor_offsets_and_distances() {
    let p = first_props(
        r"\tphpg\tposx1440\tpvpg\tposy2880\tdfrmtxtLeft180\tdfrmtxtRight180\tdfrmtxtTop0\tdfrmtxtBottom0",
    );
    let want = TableFloat {
        h_rel: Anchor::Page,
        v_rel: Anchor::Page,
        x: 72.0,
        y: 144.0,
        h_align: None,
        v_align: None,
        dist: [9.0, 0.0, 9.0, 0.0],
        overlap: true,
    };
    assert_eq!(p.float, Some(want));
}

#[test]
fn positioned_table_alignments_and_no_overlap() {
    let f = first_props(r"\tphmrg\tposxc\tpvpara\tposy0\tabsnoovrlp").float.expect("floats");
    assert_eq!((f.h_rel, f.h_align, f.v_rel, f.y, f.overlap), (Anchor::Margin, Some(FloatAlign::Center), Anchor::Paragraph, 0.0, false));
    let f = first_props(r"\tphcol\tposnegx-360\tpvmrg\tposyb").float.expect("floats");
    assert_eq!((f.h_rel, f.x, f.h_align, f.v_rel, f.v_align), (Anchor::Column, -18.0, None, Anchor::Margin, Some(FloatAlign::End)));
}

#[test]
fn a_table_without_position_words_does_not_float() {
    assert_eq!(first_props("").float, None);
}

#[test]
fn width_and_position_survive_export() {
    let words = r"\trftsWidth3\trwWidth5000\tphpg\tposx1440\tpvpg\tposy2880\tdfrmtxtLeft180\tdfrmtxtRight180\tabsnoovrlp";
    let d = open(&format!(r"{}\pard After\par", row(words)));
    let before = tables(&d)[0].props.clone();
    assert!(before.float.is_some() && before.width == Some(250.0), "precondition: {before:?}");
    let back = import(export(&d).as_bytes()).unwrap();
    let after = &tables(&back)[0].props;
    assert_eq!((after.width, after.width_pct, after.float), (before.width, before.width_pct, before.float));
    let d = open(&format!(r"{}\pard After\par", row(r"\trftsWidth2\trwWidth2500")));
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(tables(&back)[0].props.width_pct, Some(50.0));
}
