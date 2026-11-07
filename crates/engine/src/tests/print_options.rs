//! Word's printing options (File › Options): Print hidden text (Display), Print field codes
//! instead of their values and Print pages in reverse order (Advanced). They are per-user
//! options, off by default; what the screen shows (hidden text, Alt+F9 field codes) never
//! changes what prints. Per-job params `hiddenText`, `fieldCodes` and `reverse` override them.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn flat_opc(body: &str) -> String {
    format!(
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
    )
}

fn open(body: &str) -> Session {
    Session::new(wordcraft_docx::read_xml(flat_opc(body).as_bytes()).expect("flat OPC reads"))
}

/// One paragraph: "Shown " + hidden "Secret" + " end", then a QUOTE field whose result is "Ada".
fn sample() -> Session {
    open(
        r#"<w:p><w:r><w:t xml:space="preserve">Shown </w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:t>Secret</w:t></w:r><w:r><w:t xml:space="preserve"> end</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Name: </w:t></w:r><w:fldSimple w:instr=" QUOTE &quot;Ada&quot; "><w:r><w:t>Ada</w:t></w:r></w:fldSimple></w:p>"#,
    )
}

/// What a print job's layout shows: field text drawn in place of fields (results or codes), joined
/// with '|', and the number of clusters that take room on the line (hidden text takes none).
fn printed(s: &Session, v: Value) -> (String, usize) {
    let l = s.print_layout(&v);
    let mut fields = String::new();
    let mut visible = 0;
    for page in &l.pages {
        for it in &page.items {
            if let wordcraft_layout::Placed::Lines { para, l0, l1, .. } = it {
                for (_, shown, _) in &para.shown {
                    fields.push_str(shown);
                    fields.push('|');
                }
                for line in &para.lines[*l0..*l1] {
                    visible += para.clusters[line.c0..line.c1].iter().filter(|c| c.adv > 0.0).count();
                }
            }
        }
    }
    (fields, visible)
}

/// "Secret" is six letters.
const SECRET: usize = 6;

#[test]
fn hidden_text_does_not_print_by_default_even_when_shown_on_screen() {
    let mut s = sample();
    assert!(!s.prefs.print_hidden_text, "Word's default is off");
    let (_, base) = printed(&s, json!({}));
    let (_, with) = printed(&s, json!({"hiddenText": true}));
    assert_eq!(with, base + SECRET, "the job param prints the hidden word");
    run(&mut s, "file.options", json!({"marks": {"hidden": true}}));
    assert_eq!(printed(&s, json!({})).1, base, "showing hidden text on screen doesn't print it");
}

#[test]
fn print_hidden_text_option_prints_it_and_the_param_overrides() {
    let mut s = sample();
    let (_, base) = printed(&s, json!({}));
    let r = run(&mut s, "file.options", json!({"printHiddenText": true}));
    assert_eq!(r["printHiddenText"], json!(true), "file.options returns the setting");
    assert!(s.prefs.print_hidden_text);
    assert_eq!(printed(&s, json!({})).1, base + SECRET, "the option prints hidden text");
    assert_eq!(printed(&s, json!({"hiddenText": false})).1, base, "the job param wins");
    run(&mut s, "file.options", json!({"printHiddenText": false}));
    assert_eq!(printed(&s, json!({"hiddenText": true})).1, base + SECRET);
}

#[test]
fn field_codes_print_only_with_the_option() {
    let mut s = sample();
    assert!(!s.prefs.print_field_codes, "Word's default is off");
    let (t, _) = printed(&s, json!({}));
    assert!(t.contains("Ada") && !t.contains("QUOTE"), "results print by default: {t}");
    // Alt+F9 on screen does not change the print.
    run(&mut s, "view.fieldCodes", json!({"value": true}));
    assert!(s.view.field_codes, "view.fieldCodes shows codes on screen");
    let (t, _) = printed(&s, json!({}));
    assert!(!t.contains("QUOTE"), "on-screen field codes don't print: {t}");
    run(&mut s, "view.fieldCodes", json!({"value": false}));
    let r = run(&mut s, "file.options", json!({"printFieldCodes": true}));
    assert_eq!(r["printFieldCodes"], json!(true), "file.options returns the setting");
    let (t, _) = printed(&s, json!({}));
    assert!(t.contains("QUOTE"), "codes print with the option: {t}");
    let (t, _) = printed(&s, json!({"fieldCodes": false}));
    assert!(!t.contains("QUOTE") && t.contains("Ada"), "the job param wins: {t}");
}

fn three_pages() -> Session {
    open(
        r#"<w:p><w:r><w:t>One</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>Two</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>Three</w:t></w:r></w:p>"#,
    )
}

/// (params, expected page indexes) for a 3-page document.
fn reverse_table() -> Vec<(Value, Vec<usize>)> {
    vec![
        (json!({}), vec![0, 1, 2]),
        (json!({"reverse": false}), vec![0, 1, 2]),
        (json!({"reverse": true}), vec![2, 1, 0]),
        (json!({"reverse": true, "copies": 2}), vec![2, 1, 0, 2, 1, 0]),
        (json!({"reverse": true, "copies": 2, "collate": false}), vec![2, 2, 1, 1, 0, 0]),
        (json!({"reverse": true, "pages": "1,3"}), vec![2, 0]),
        (json!({"reverse": true, "parity": "odd"}), vec![2, 0]),
    ]
}

#[test]
fn reverse_order_plan() {
    for (v, want) in reverse_table() {
        let plan = crate::print::plan(&v, 3, (0, 0)).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert_eq!(plan.pages, want, "{v}");
        let distinct = if v.get("pages").is_some() || v.get("parity").is_some() { 2 } else { 3 };
        assert_eq!(plan.chosen, distinct, "chosen counts distinct pages: {v}");
    }
}

#[test]
fn print_pages_in_reverse_order_option_reaches_the_print_job() {
    let mut s = three_pages();
    assert_eq!(s.export_layout().pages.len(), 3, "fixture has three pages");
    assert!(!s.prefs.print_reverse, "Word's default is off");
    let (_, plan) = s.print_job_pdf(&json!({})).expect("prints");
    assert_eq!(plan.pages, vec![0, 1, 2]);
    let r = run(&mut s, "file.options", json!({"printReverse": true}));
    assert_eq!(r["printReverse"], json!(true), "file.options returns the setting");
    let (_, plan) = s.print_job_pdf(&json!({})).expect("prints");
    assert_eq!(plan.pages, vec![2, 1, 0], "the option reverses the job");
    let (_, plan) = s.print_job_pdf(&json!({"reverse": false})).expect("prints");
    assert_eq!(plan.pages, vec![0, 1, 2], "the job param wins");
}

#[test]
fn print_options_persist_with_the_prefs() {
    let mut s = sample();
    run(&mut s, "file.options", json!({"printHiddenText": true, "printFieldCodes": true, "printReverse": true}));
    let saved = serde_json::to_value(&s.prefs).unwrap();
    for key in ["printHiddenText", "printFieldCodes", "printReverse"] {
        assert_eq!(saved[key], json!(true), "{key} saved: {saved}");
    }
    let old: crate::prefs::Prefs = serde_json::from_value(json!({"printMarkup": true})).unwrap();
    assert!(!old.print_hidden_text && !old.print_field_codes && !old.print_reverse, "old prefs load with Word's defaults");
}
