//! Draft and Outline views wrap each line at its section's margins, as on the printed page (Word's
//! default); File › Options › Advanced › "Show text wrapped within the document window" (off by
//! default) wraps them to the window instead. Print Layout and Web Layout ignore the option.
//! Sources: Microsoft Q&A https://learn.microsoft.com/en-us/answers/a/11213013 and
//! wordribbon.tips.net T010690 ("Text Doesn't Wrap at Margin in Draft View").

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::Session;
use wordcraft_doc::StoryRef;
use wordcraft_layout::Placed;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

const WORDS: &str = "the quick brown fox jumps over the lazy dog while seven grumpy wizards make toxic brew ";

/// Letter paper; section 1 has 1-inch margins (468 pt of text), section 2 half-inch margins
/// (540 pt), each holding the same long paragraph.
fn session() -> Session {
    let text = WORDS.repeat(20);
    let text = text.trim_end();
    let sect = |m: u32| {
        format!(r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="{m}" w:bottom="1440" w:left="{m}" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#)
    };
    let body = format!(
        r#"<w:p><w:pPr>{s1}</w:pPr><w:r><w:t>{text}</w:t></w:r></w:p><w:p><w:r><w:t>{text}</w:t></w:r></w:p>{s2}"#,
        s1 = sect(1440),
        s2 = sect(720)
    );
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let doc = wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads");
    assert_eq!(doc.sections().len(), 2, "fixture has two sections");
    let mut s = Session::new(doc);
    s.dirty = false;
    s
}

/// Lines of each body paragraph in the current view's layout, in paragraph order.
fn line_counts(s: &mut Session) -> Vec<usize> {
    let l = s.layout();
    let mut by_para: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    for page in &l.pages {
        for item in &page.items {
            if let Placed::Lines { story: StoryRef::Body, path, l0, l1, .. } = item {
                *by_para.entry(path.0.clone()).or_default() += l1 - l0;
            }
        }
    }
    by_para.into_values().collect()
}

fn counts_in(s: &mut Session, view: &str) -> Vec<usize> {
    run(s, view, json!({}));
    line_counts(s)
}

fn set_wrap(s: &mut Session, on: bool) {
    run(s, "file.options", json!({"wrapToWindow": on}));
}

#[test]
fn the_option_is_off_by_default_and_setting_it_is_not_an_edit() {
    let mut s = session();
    assert_eq!(run(&mut s, "file.options", json!({}))["wrapToWindow"], json!(false));
    assert_eq!(serde_json::to_value(&s.prefs).unwrap()["wrapToWindow"], json!(false));
    let undo = s.undo_labels();
    run(&mut s, "view.draft", json!({}));
    set_wrap(&mut s, true);
    assert_eq!(run(&mut s, "file.options", json!({}))["wrapToWindow"], json!(true));
    assert_eq!(serde_json::to_value(&s.prefs).unwrap()["wrapToWindow"], json!(true));
    assert_eq!(s.undo_labels(), undo, "no undo step");
    assert!(!s.dirty, "a preference doesn't make the document unsaved");
    set_wrap(&mut s, false);
    assert_eq!(run(&mut s, "file.options", json!({}))["wrapToWindow"], json!(false));
}

#[test]
fn older_preferences_without_the_key_load_with_it_off() {
    let mut v = serde_json::to_value(crate::prefs::Prefs::default()).unwrap();
    v.as_object_mut().unwrap().remove("wrapToWindow");
    let p: crate::prefs::Prefs = serde_json::from_value(v).unwrap();
    assert_eq!(serde_json::to_value(&p).unwrap()["wrapToWindow"], json!(false));
}

#[test]
fn draft_and_outline_break_lines_where_print_layout_does_in_each_section() {
    let mut s = session();
    let print = counts_in(&mut s, "view.printLayout");
    assert_eq!(print.len(), 2);
    assert!(print[0] > print[1], "468 pt of text needs more lines than 540 pt: {print:?}");
    assert_eq!(counts_in(&mut s, "view.draft"), print, "Draft wraps at each section's margins");
    assert_eq!(counts_in(&mut s, "view.outline"), print, "Outline wraps at each section's margins");
}

#[test]
fn with_the_option_off_the_window_width_does_not_move_draft_lines() {
    let mut s = session();
    let print = counts_in(&mut s, "view.printLayout");
    run(&mut s, "view.draft", json!({}));
    for w in [300.0, 1500.0] {
        s.view.web_width = w;
        assert_eq!(line_counts(&mut s), print, "window {w} pt");
    }
}

#[test]
fn wrapped_to_the_window_draft_and_outline_follow_the_window() {
    let mut s = session();
    let print = counts_in(&mut s, "view.printLayout");
    set_wrap(&mut s, true);
    for view in ["view.draft", "view.outline"] {
        run(&mut s, view, json!({}));
        s.view.web_width = 1500.0;
        let wide = line_counts(&mut s);
        assert_eq!(wide.len(), 2);
        assert_eq!(wide[0], wide[1], "{view}: both sections wrap to the same window, not their margins");
        assert!(wide[0] < print[1], "{view}: a 1500 pt window takes fewer lines than 540 pt of text: {wide:?} vs {print:?}");
        s.view.web_width = 300.0;
        let narrow = line_counts(&mut s);
        assert_eq!(narrow[0], narrow[1], "{view}");
        assert!(narrow[0] > print[0], "{view}: a 300 pt window takes more lines than 468 pt of text: {narrow:?} vs {print:?}");
    }
    set_wrap(&mut s, false);
    run(&mut s, "view.draft", json!({}));
    assert_eq!(line_counts(&mut s), print, "turning it off goes back to the margins");
}

#[test]
fn print_layout_and_web_layout_ignore_the_option() {
    let mut s = session();
    let print = counts_in(&mut s, "view.printLayout");
    run(&mut s, "view.webLayout", json!({}));
    s.view.web_width = 700.0;
    let web = line_counts(&mut s);
    set_wrap(&mut s, true);
    assert_eq!(counts_in(&mut s, "view.printLayout"), print);
    run(&mut s, "view.webLayout", json!({}));
    s.view.web_width = 700.0;
    assert_eq!(line_counts(&mut s), web);
}
