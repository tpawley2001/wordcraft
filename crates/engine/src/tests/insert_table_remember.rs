//! Insert Table's Remember dimensions for new tables: `insert.table` with `remember` keeps the
//! size and AutoFit behavior in the preferences, and `remember: false` forgets them.

use serde_json::json;
use wordcraft_doc::Document;

use crate::Session;
use crate::prefs::InsertTablePrefs;

#[test]
fn insert_table_remember_keeps_and_forgets_dimensions() {
    let mut s = Session::new(Document::new());
    s.run("insert.table", &json!({"rows": 4, "cols": 3, "autofit": "fixed", "colWidth": 72, "remember": true})).unwrap();
    assert_eq!(s.prefs.insert_table, InsertTablePrefs { rows: 4, cols: 3, autofit: "fixed".into(), col_width: 72.0, remember: true });
    // Without `remember` the remembered size stays.
    s.run("insert.table", &json!({"rows": 1, "cols": 1})).unwrap();
    assert_eq!(s.prefs.insert_table.cols, 3);
    s.run("insert.table", &json!({"rows": 1, "cols": 1, "remember": false})).unwrap();
    assert_eq!(s.prefs.insert_table, InsertTablePrefs::default());
}

/// Guard: a bad AutoFit behavior or column width is refused, and nothing is inserted or kept.
#[test]
fn insert_table_refuses_bad_autofit_params() {
    let mut s = Session::new(Document::new());
    let blocks = s.doc.body.len();
    assert!(s.run("insert.table", &json!({"autofit": "wide", "remember": true})).is_err());
    assert!(s.run("insert.table", &json!({"autofit": "fixed", "colWidth": -5})).is_err());
    assert!(s.run("insert.table", &json!({"autofit": "fixed", "colWidth": 1e9})).is_err());
    assert_eq!(s.doc.body.len(), blocks);
    assert_eq!(s.prefs.insert_table, InsertTablePrefs::default());
}

/// Damaged saved preferences fall back to Word's 2 x 5 without losing the others.
#[test]
fn insert_table_prefs_load_leniently() {
    let p: crate::Prefs = serde_json::from_str(r#"{"insertTable": 7, "checkSpelling": false}"#).unwrap();
    assert_eq!(p.insert_table, InsertTablePrefs::default());
    assert!(!p.check_spelling);
}
