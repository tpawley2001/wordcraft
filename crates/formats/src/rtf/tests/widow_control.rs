//! Paragraph widow/orphan control (`\widctlpar`, `\nowidctlpar`, RTF 1.9.1): read into the
//! paragraph's own properties, reset by `\pard`, and written back. The document turns widow control
//! on (`\widowctrl`), so an unset paragraph stays unset.

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}\widowctrl{body}}}").as_bytes()).unwrap()
}

fn widow(d: &Document) -> Vec<Option<bool>> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.widow_control).collect()
}

#[test]
fn paragraph_widow_control_is_read() {
    let d = open(r"\pard\widctlpar on\par\pard\nowidctlpar off\par still off\par\pard unset\par");
    assert_eq!(widow(&d), [Some(true), Some(false), Some(false), None]);
}

#[test]
fn widow_control_round_trips() {
    let d = open(r"\pard\widctlpar on\par\pard\nowidctlpar off\par\pard unset\par");
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(widow(&back), [Some(true), Some(false), None]);
}
