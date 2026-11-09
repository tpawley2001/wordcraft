//! File › Print › Manually Print on Both Sides: a job's `duplexPass` (`front` | `back`) prints the
//! pages on the fronts of the sheets (1st, 3rd, 5th… chosen page) or on their backs (2nd, 4th…).
//! File › Options › Advanced › Print on front / back of the sheet for duplex printing (per-user, off
//! by default; job params `frontReverse` / `backReverse` override) reverse that pass.

use serde_json::{Value, json};

use crate::Session;
use crate::print::plan;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn open(body: &str) -> Session {
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
    Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"))
}

/// Five pages, one word each.
fn five_pages() -> Session {
    let brk = r#"<w:r><w:br w:type="page"/></w:r>"#;
    let words = ["One", "Two", "Three", "Four", "Five"].map(|w| format!("<w:r><w:t>{w}</w:t></w:r>"));
    open(&format!("<w:p>{}</w:p>", words.join(brk)))
}

/// (params, 0-based pages printed) for a 5-page document.
fn pass_table() -> Vec<(Value, Vec<usize>)> {
    vec![
        (json!({"duplexPass": "front"}), vec![0, 2, 4]),
        (json!({"duplexPass": "back"}), vec![1, 3]),
        // Sheets follow the job order, not page-number parity: pages 2-5 put 2 and 4 on the fronts.
        (json!({"duplexPass": "front", "pages": "2-5"}), vec![1, 3]),
        (json!({"duplexPass": "back", "pages": "2-5"}), vec![2, 4]),
        (json!({"duplexPass": "front", "pages": "1,3,5"}), vec![0, 4]),
        (json!({"duplexPass": "back", "pages": "1,3,5"}), vec![2]),
        (json!({"duplexPass": "front", "frontReverse": true}), vec![4, 2, 0]),
        (json!({"duplexPass": "back", "backReverse": true}), vec![3, 1]),
        // Each option reverses only its own pass.
        (json!({"duplexPass": "front", "backReverse": true}), vec![0, 2, 4]),
        (json!({"duplexPass": "back", "frontReverse": true}), vec![1, 3]),
        // Print pages in reverse order is replaced by the front/back options in a duplex pass.
        (json!({"duplexPass": "front", "reverse": true}), vec![0, 2, 4]),
        (json!({"duplexPass": "back", "reverse": true, "backReverse": true}), vec![3, 1]),
    ]
}

#[test]
fn a_duplex_pass_prints_the_fronts_or_the_backs_of_the_sheets() {
    for (v, want) in pass_table() {
        let p = plan(&v, 5, (0, 0)).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert_eq!(p.pages, want, "{v}");
        assert_eq!(p.copies, 1, "{v}");
    }
}

#[test]
fn chosen_counts_the_pages_before_the_split_so_the_back_pass_can_be_skipped() {
    assert_eq!(plan(&json!({"duplexPass": "front"}), 5, (0, 0)).unwrap().chosen, 5);
    assert_eq!(plan(&json!({"duplexPass": "front", "pages": "2-5"}), 5, (0, 0)).unwrap().chosen, 4);
    assert_eq!(plan(&json!({"duplexPass": "front", "range": "current"}), 5, (2, 2)).unwrap().pages, vec![2]);
    assert_eq!(plan(&json!({"duplexPass": "front", "range": "current"}), 5, (2, 2)).unwrap().chosen, 1);
}

#[test]
fn bad_duplex_jobs_are_refused() {
    for v in [
        json!({"duplexPass": "both"}),
        json!({"duplexPass": 1}),
        json!({"duplexPass": "front", "parity": "odd"}),
        json!({"duplexPass": "back", "parity": "even"}),
        json!({"duplexPass": "front", "copies": 2}),
        json!({"duplexPass": "front", "pagesPerSheet": 2}),
        json!({"duplexPass": "front", "pagesPerSheet": "booklet"}),
        // One chosen page has no back.
        json!({"duplexPass": "back", "pages": "3"}),
    ] {
        assert!(plan(&v, 5, (0, 0)).is_err(), "{v} is refused");
    }
    assert!(plan(&json!({"duplexPass": "front", "parity": "all"}), 5, (0, 0)).is_ok(), "parity all is the default");
    assert!(plan(&json!({"duplexPass": "front", "copies": 1, "pagesPerSheet": 1}), 5, (0, 0)).is_ok());
}

#[test]
fn jobs_without_a_duplex_pass_are_unchanged() {
    assert_eq!(plan(&json!({}), 5, (0, 0)).unwrap().pages, vec![0, 1, 2, 3, 4]);
    assert_eq!(plan(&json!({"reverse": true, "frontReverse": true}), 5, (0, 0)).unwrap().pages, vec![4, 3, 2, 1, 0]);
    assert_eq!(plan(&json!({"backReverse": true}), 5, (0, 0)).unwrap().pages, vec![0, 1, 2, 3, 4]);
}

#[test]
fn the_front_and_back_options_reach_the_print_job_and_the_job_wins() {
    let mut s = five_pages();
    assert_eq!(s.export_layout().pages.len(), 5, "fixture has five pages");
    assert!(!s.prefs.print_duplex_front && !s.prefs.print_duplex_back, "Word's defaults are off");
    let pages = |s: &Session, v: Value| s.print_job_pdf(&v).unwrap_or_else(|e| panic!("{v}: {e}")).1.pages;
    assert_eq!(pages(&s, json!({"duplexPass": "front"})), vec![0, 2, 4]);
    let r = run(&mut s, "file.options", json!({"printDuplexFront": true}));
    assert_eq!(r["printDuplexFront"], json!(true), "file.options returns the setting");
    assert_eq!(r["printDuplexBack"], json!(false));
    assert_eq!(pages(&s, json!({"duplexPass": "front"})), vec![4, 2, 0], "the front option reverses the fronts");
    assert_eq!(pages(&s, json!({"duplexPass": "back"})), vec![1, 3], "and not the backs");
    assert_eq!(pages(&s, json!({"duplexPass": "front", "frontReverse": false})), vec![0, 2, 4], "the job param wins");
    run(&mut s, "file.options", json!({"printDuplexBack": true}));
    assert_eq!(pages(&s, json!({"duplexPass": "back"})), vec![3, 1]);
    assert_eq!(pages(&s, json!({})), vec![0, 1, 2, 3, 4], "an ordinary job ignores both options");
}

#[test]
fn the_duplex_options_persist_with_the_prefs() {
    let mut s = five_pages();
    run(&mut s, "file.options", json!({"printDuplexFront": true, "printDuplexBack": true}));
    let saved = serde_json::to_value(&s.prefs).unwrap();
    assert_eq!(saved["printDuplexFront"], json!(true), "{saved}");
    assert_eq!(saved["printDuplexBack"], json!(true), "{saved}");
    let mut old = saved.as_object().unwrap().clone();
    old.remove("printDuplexFront");
    old.remove("printDuplexBack");
    let old: crate::prefs::Prefs = serde_json::from_value(Value::Object(old)).expect("old prefs load");
    assert!(!old.print_duplex_front && !old.print_duplex_back, "old prefs load with Word's defaults");
}
