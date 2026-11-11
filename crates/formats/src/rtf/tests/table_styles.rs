//! Table styles through RTF (RTF 1.9.1, "Style sheet", table styles): `{\*\tsN\tsrowd … Name;}`
//! entries become table styles with their borders and cell padding, a row's `\tsN` names the
//! table's style, `\tbllk*` words give its look, and export writes them back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::styles::StyleKind;

const NORMAL_TABLE: &str = r"{\*\ts11\tsrowd\trpaddl108\trpaddr108\trpaddfl3\trpaddfr3\snext11\ssemihidden Normal Table;}";
const GRID_ROW: &str =
    r"\trbrdrt\brdrs\brdrw10 \trbrdrl\brdrs\brdrw10 \trbrdrb\brdrs\brdrw10 \trbrdrr\brdrs\brdrw10 \trbrdrh\brdrs\brdrw10 \trbrdrv\brdrs\brdrw10 ";

fn open(stylesheet: &str, body: &str) -> Document {
    import(
        format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\stylesheet{{\s0 Normal;}}{NORMAL_TABLE}{stylesheet}}}{body}}}").as_bytes(),
    )
    .unwrap()
}

fn report_grid() -> String {
    format!(r"{{\*\ts15\tsrowd{GRID_ROW}\trpaddl108\trpaddr108\trpaddfl3\trpaddfr3\tsvertalt\sbasedon11\snext15\spriority59 Report Grid;}}")
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

const ONE_ROW: &str = r"\trowd\ts15\cellx2000 \pard\intbl A\cell\row \pard After\par";

#[test]
fn a_table_style_entry_becomes_a_table_style() {
    let d = open(&report_grid(), ONE_ROW);
    let s = d.styles.get("ReportGrid").expect("style ReportGrid");
    assert_eq!(s.name, "Report Grid");
    assert_eq!(s.kind, StyleKind::Table);
    assert_eq!(s.based_on.as_deref(), Some("TableNormal"));
    assert_eq!(s.priority, Some(59));
    let t = s.table.as_ref().expect("table parts");
    let b = t.borders.as_ref().expect("borders");
    for (side, e) in
        [("top", &b.top), ("left", &b.left), ("bottom", &b.bottom), ("right", &b.right), ("between", &b.between), ("inside_v", &b.inside_v)]
    {
        let e = e.as_ref().unwrap_or_else(|| panic!("{side} border"));
        assert!((e.width - 0.5).abs() < 1e-3, "{side} width {}", e.width);
    }
    assert_eq!(t.cell_margins, Some([0.0, 5.4, 0.0, 5.4]));
}

#[test]
fn a_row_naming_a_table_style_keeps_it() {
    let d = open(&report_grid(), ONE_ROW);
    assert_eq!(table(&d).props.style.as_deref(), Some("ReportGrid"));
}

#[test]
fn the_built_in_table_grid_keeps_its_id() {
    let d = open(&format!(r"{{\*\ts15\tsrowd{GRID_ROW}\sbasedon11\snext15\spriority59 Table Grid;}}"), ONE_ROW);
    assert_eq!(table(&d).props.style.as_deref(), Some("TableGrid"));
}

#[test]
fn a_row_naming_a_missing_table_style_has_none() {
    let d = open("", ONE_ROW);
    assert_eq!(table(&d).props.style, None);
}

#[test]
fn table_look_words_set_the_look() {
    let d =
        open(&report_grid(), r"\trowd\ts15\tbllkhdrrows\tbllklastrow\tbllkhdrcols\tbllknocolband\cellx2000 \pard\intbl A\cell\row \pard After\par");
    let l = &table(&d).props.look;
    assert!(l.header_row && l.total_row && l.first_column && l.banded_rows, "{l:?}");
    assert!(!l.last_column && !l.banded_columns, "{l:?}");
}

#[test]
fn export_writes_the_table_style_and_the_look() {
    let d = open(&report_grid(), r"\trowd\ts15\tbllkhdrrows\tbllknocolband\cellx2000 \pard\intbl A\cell\row \pard After\par");
    let rtf = export(&d);
    let entry = rtf.find(r"{\*\ts").expect("a \\ts stylesheet entry");
    assert!(rtf[..entry].contains(r"{\stylesheet"), "entry outside the stylesheet");
    let n: String = rtf[entry + 6..].chars().take_while(|c| c.is_ascii_digit()).collect();
    assert!(!n.is_empty());
    assert!(rtf[entry..].split(";}").next().unwrap().ends_with("Report Grid"), "entry name");
    let row = &rtf[rtf.find(r"\trowd").expect("a row")..];
    let row = &row[..row.find(r"\cell").unwrap()];
    assert!(row.contains(&format!(r"\ts{n}\")) || row.contains(&format!(r"\ts{n} ")), "row names \\ts{n}: {row}");
    assert!(row.contains(r"\tbllkhdrrows"), "{row}");

    let back = import(rtf.as_bytes()).unwrap();
    let s = back.styles.get("ReportGrid").expect("style back");
    assert_eq!(s.kind, StyleKind::Table);
    assert_eq!(s.table.as_ref().and_then(|t| t.cell_margins), Some([0.0, 5.4, 0.0, 5.4]));
    assert!(s.table.as_ref().and_then(|t| t.borders.as_ref()).and_then(|b| b.top.as_ref()).is_some());
    let t = table(&back);
    assert_eq!(t.props.style.as_deref(), Some("ReportGrid"));
    assert!(t.props.look.header_row && t.props.look.banded_rows && !t.props.look.banded_columns);
}

#[test]
fn export_of_an_unstyled_table_names_no_table_style() {
    let d = open("", r"\trowd\cellx2000 \pard\intbl A\cell\row \pard After\par");
    assert_eq!(table(&d).props.style, None);
    let rtf = export(&d);
    for row in rtf.split(r"\trowd").skip(1) {
        let row = &row[..row.find(r"\cell").unwrap_or(row.len())];
        assert!(!row.contains(r"\ts"), "{row}");
    }
}
