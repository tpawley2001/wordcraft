//! A paragraph or style that turns off `w:pageBreakBefore`, `w:keepNext` or `w:keepLines`
//! (ECMA-376 Part 1 §17.3.1.23, §17.3.1.15, §17.3.1.14) over a base style that turns it on is
//! saved with `w:val="0"`, so the off survives a save and reopen.

use std::sync::Arc;

use wordcraft_doc::props::ParaProps;
use wordcraft_doc::resolve::ResolvedPara;
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

type Field = fn(&mut ParaProps) -> &mut Option<bool>;
type Resolved = fn(&ResolvedPara) -> bool;

const PROPS: [(&str, Field, Resolved); 3] = [
    ("pageBreakBefore", |p| &mut p.page_break_before, |r| r.page_break_before),
    ("keepNext", |p| &mut p.keep_next, |r| r.keep_next),
    ("keepLines", |p| &mut p.keep_lines, |r| r.keep_lines),
];

fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut xml).unwrap();
    xml
}

fn style(id: &str, based_on: Option<&str>, value: Option<bool>, field: Field) -> Style {
    let mut para = ParaProps::default();
    *field(&mut para) = value;
    Style { id: id.into(), name: id.into(), kind: StyleKind::Paragraph, based_on: based_on.map(Into::into), para, ..Default::default() }
}

#[test]
fn an_off_over_an_on_style_is_saved_and_read_back() {
    for (tag, field, resolved) in PROPS {
        let mut d = Document::new();
        let sheet = Arc::make_mut(&mut d.styles);
        sheet.upsert(style("BaseOn", Some("Normal"), Some(true), field));
        sheet.upsert(style("ChildOff", Some("BaseOn"), Some(false), field));
        let mut off = Paragraph::with_text("off", CharProps::default());
        off.props.style = Some("BaseOn".into());
        *field(&mut off.props) = Some(false);
        let mut child = Paragraph::with_text("child", CharProps::default());
        child.props.style = Some("ChildOff".into());
        d.body = vec![para_block(off), para_block(child)];

        let bytes = wordcraft_docx::write(&d).unwrap();
        let off_xml = format!("<w:{tag} w:val=\"0\"/>");
        assert!(part(&bytes, "word/document.xml").contains(&off_xml), "{tag}: paragraph");
        assert!(part(&bytes, "word/styles.xml").contains(&off_xml), "{tag}: style");

        let back = wordcraft_docx::read(&bytes).unwrap();
        let mut child_style = back.styles.get("ChildOff").unwrap().para.clone();
        assert_eq!(*field(&mut child_style), Some(false), "{tag}");
        let paras: Vec<&Paragraph> = back.body.iter().filter_map(|b| b.as_para()).collect();
        let mut first = paras[0].props.clone();
        assert_eq!(*field(&mut first), Some(false), "{tag}");
        assert!(!resolved(&back.styles.resolve_para(&paras[0].props)), "{tag}");
        assert!(!resolved(&back.styles.resolve_para(&paras[1].props)), "{tag}");
    }
}
