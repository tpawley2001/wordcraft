//! A formula field (`= …`, ECMA-376 §17.16.5.1) can name a bookmark as an operand, as in Microsoft
//! Word: `= Total * 2` reads the number the bookmark `Total` marks, in a table cell or in an
//! ordinary paragraph. Word keeps the stored result until the field is updated (measured: opening
//! or exporting the file does not recompute it), so these tests update the fields.

use serde_json::{Value, json};
use wordcraft_doc::{CharProps, InlineObject, Path, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (bookmark name, the text it marks): one paragraph each, at the start of the body.
const BOOKMARKS: [(&str, &str); 4] = [("Total", "12"), ("Money", "$1,234.50"), ("Neg", "(5)"), ("Net_Total", "7.5")];

/// (field code, the result after Update Field): 12 * 2 = 24; 12 + 5 = 17; 1234.5 + 1 = 1235.5;
/// -5 + 10 = 5; 7.5 * 2 = 15; 12 - 15 = -3; 12 > 10 gives 1.
fn rows() -> Vec<(&'static str, &'static str)> {
    vec![
        ("= Total * 2", "24"),
        ("= total*2", "24"),
        ("=SUM(Total, 5)", "17"),
        (r#"= Money + 1 \# "0.00""#, "1235.50"),
        ("= Neg + 10", "5"),
        ("= Net_Total * 2", "15"),
        ("= Total - Net_Total * 2", "-3"),
        ("=IF(Total > 10, 1, 0)", "1"),
        ("= Missing * 2", "!Undefined Bookmark, MISSING"),
        ("= 2 + 3", "5"),
    ]
}

/// Wrap the whole of paragraph `path` in the bookmark `name`.
fn mark(doc: &mut wordcraft_doc::Document, path: &Path, name: &str) {
    let p = doc.para_mut(StoryRef::Body, path).unwrap();
    p.insert_object(p.len(), InlineObject::BookmarkEnd { name: name.into() }, &CharProps::default()).unwrap();
    p.insert_object(0, InlineObject::BookmarkStart { name: name.into() }, &CharProps::default()).unwrap();
}

/// The bookmark paragraphs, then one paragraph `Fnn` per row ending in its field with the stale
/// result "OLD" (`locked` for the rows listed).
fn session(locked_rows: &[usize]) -> Session {
    let rows = rows();
    let mut text: Vec<String> = BOOKMARKS.iter().map(|b| b.1.to_string()).collect();
    text.extend((0..rows.len()).map(|i| format!("F{:02}", i + 1)));
    let mut doc = wordcraft_doc::Document::from_text(&text.join("\n"));
    for (i, path) in doc.para_paths(StoryRef::Body).into_iter().enumerate() {
        if let Some((name, _)) = BOOKMARKS.get(i) {
            mark(&mut doc, &path, name);
            continue;
        }
        let row = i - BOOKMARKS.len();
        let p = doc.para_mut(StoryRef::Body, &path).unwrap();
        let field = InlineObject::Field {
            instr: rows[row].0.to_string(),
            result: "OLD".into(),
            locked: locked_rows.contains(&row),
            dirty: false,
            code: false,
            form: String::new(),
code_paras: None,
        };
        p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    }
    Session::new(doc)
}

/// Every body field's (code, stored result), in document order.
fn results(s: &Session) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o {
                out.push((instr.clone(), result.clone()));
            }
        }
    }
    out
}

#[test]
fn the_bookmarks_mark_their_numbers() {
    let s = session(&[]);
    for (name, text) in BOOKMARKS {
        assert_eq!(crate::cmd::references::bookmark_text(&s.doc, name).as_deref(), Some(text), "{name}");
    }
    assert_eq!(results(&s).len(), rows().len());
}

#[test]
fn update_field_works_out_formulas_that_name_bookmarks() {
    let mut s = session(&[]);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let want: Vec<(String, String)> = rows().into_iter().map(|(code, result)| (code.to_string(), result.to_string())).collect();
    assert_eq!(results(&s), want);
}

#[test]
fn a_locked_formula_keeps_its_stored_result() {
    let mut s = session(&[0]);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let got = results(&s);
    assert_eq!(got[0], ("= Total * 2".to_string(), "OLD".to_string()));
    assert_eq!(got[1], ("= total*2".to_string(), "24".to_string()));
}

#[test]
fn a_formula_outside_a_table_is_worked_out_from_the_document() {
    let s = session(&[]);
    let eval = |instr: &str| crate::cmd::table_formula::evaluate_text_field(&s.doc, None, instr);
    assert_eq!(eval("= Total * 2").ok().as_deref(), Some("24"));
    assert_eq!(eval("=SUM(Total, Neg, 5)").ok().as_deref(), Some("12"), "12 - 5 + 5");
    assert_eq!(eval(r#"= Money + 1 \# "0.00""#).ok().as_deref(), Some("1235.50"));
    let missing = eval("= Missing * 2").expect_err("no bookmark is called Missing");
    assert_eq!(format!("!{missing}"), "!Undefined Bookmark, MISSING");
    assert!(eval("=SUM(ABOVE)").is_err(), "there are no cells above a formula outside a table");
}

#[test]
fn a_table_formula_adds_a_bookmark_to_its_cells() {
    // The matrix's column B holds 1, 4 and 7 above the formula's cell; the bookmark holds 12.
    let mut s = super::formula_table();
    let first = Path::top(0);
    assert!(s.doc.para(StoryRef::Body, &first).is_some_and(|p| p.plain_text().is_empty()), "the paragraph before the table is empty");
    s.doc.para_mut(StoryRef::Body, &first).unwrap().insert_text(0, "12", &CharProps::default()).unwrap();
    mark(&mut s.doc, &first, "Total");
    let r = run(&mut s, "table.formula", json!({"formula": "=SUM(ABOVE) + Total"}));
    assert_eq!(r["result"], "24");
    assert_eq!(super::formula_result(&s, 4, 1).as_deref(), Some("24"));
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(super::formula_result(&s, 4, 1).as_deref(), Some("24"), "updating the field reads the bookmark again");
}
