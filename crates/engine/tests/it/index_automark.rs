//! References › Insert Index › AutoMark… (Word's concordance file): `references.autoMark`.
//! Each row of the concordance table gives the text to look for (column 1) and the index entry
//! to mark it with (column 2, `Main:Sub` for a subentry; empty = column 1's text). Every body
//! paragraph gets at most one XE field per row, right after the first whole-word, same-case match.

use serde_json::{Value, json};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Document, Path, StoryRef};
use wordcraft_engine::Session;

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session(text: &str) -> Session {
    Session::new(Document::from_text(text))
}

/// (character offset of the field's OBJ in the paragraph text, XE instruction) for every XE field.
fn xe_fields(s: &Session, i: usize) -> Vec<(usize, String)> {
    let p = s.doc.para(StoryRef::Body, &Path::top(i)).expect("paragraph");
    let offs: Vec<usize> = p.text.chars().enumerate().filter(|(_, c)| *c == OBJ).map(|(n, _)| n).collect();
    assert_eq!(offs.len(), p.objects.len(), "one OBJ char per object");
    offs.into_iter()
        .zip(&p.objects)
        .filter_map(|(o, obj)| match obj {
            InlineObject::Field { instr, .. } if instr.trim_start().starts_with("XE") => Some((o, instr.clone())),
            _ => None,
        })
        .collect()
}

const TEXT: &str = "The quick fox saw a fox.\nA dog and a Dog.\nfoxes run.\nThe fox and the dog.\n";

fn entries() -> Value {
    json!({"entries": [["fox", "Animals:Fox"], ["dog", ""]]})
}

/// One XE per row per paragraph, after the first whole-word same-case match; empty column 2
/// uses column 1's text; "Dog" (case) and "foxes" (not a whole word) are left alone.
#[test]
fn automark_marks_first_whole_word_match_in_each_paragraph() {
    let mut s = session(TEXT);
    let r = run(&mut s, "references.autoMark", entries());
    assert_eq!(r["marked"], json!(4), "{r}");
    // "The quick fox" = 13 chars: the field follows the first "fox" only.
    assert_eq!(xe_fields(&s, 0), vec![(13, "XE \"Animals:Fox\"".to_string())]);
    // "A dog" = 5 chars; "Dog" is not marked.
    assert_eq!(xe_fields(&s, 1), vec![(5, "XE \"dog\"".to_string())]);
    assert_eq!(xe_fields(&s, 2), vec![]);
    // "The fox" = 7, then OBJ at 7, so "dog" ends at 7 + 1 + " and the dog".len() = 20.
    assert_eq!(xe_fields(&s, 3), vec![(7, "XE \"Animals:Fox\"".to_string()), (20, "XE \"dog\"".to_string())]);
    // XE fields are hidden text, as Mark Entry makes them.
    let p = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
    let byte = p.text.char_indices().nth(13).unwrap().0;
    assert_eq!(p.props_of_char(byte).hidden, Some(true));
}

/// Running AutoMark again adds nothing: a paragraph that already holds a row's XE is skipped.
#[test]
fn automark_twice_does_not_duplicate() {
    let mut s = session(TEXT);
    run(&mut s, "references.autoMark", entries());
    let r = run(&mut s, "references.autoMark", entries());
    assert_eq!(r["marked"], json!(0), "{r}");
    assert_eq!(xe_fields(&s, 3).len(), 2);
}

/// The whole AutoMark is one undo step.
#[test]
fn automark_is_one_undo_step() {
    let mut s = session(TEXT);
    let before: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
    run(&mut s, "references.autoMark", entries());
    run(&mut s, "edit.undo", json!({}));
    let after: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
    assert_eq!(before, after);
}

/// `path`: the concordance is the first table of a document (here HTML); a header-less two-column
/// table, blank rows skipped, cell text trimmed, quotes dropped from the entry.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn automark_reads_the_first_table_of_a_concordance_file() {
    let dir = std::env::temp_dir().join(format!("wc-automark-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("concordance.html");
    std::fs::write(
        &file,
        "<html><body><p>Concordance</p><table>\
         <tr><td>fox</td><td> Animals:Fox </td></tr>\
         <tr><td></td><td></td></tr>\
         <tr><td>dog</td><td>\"Pets\"</td></tr>\
         </table></body></html>",
    )
    .unwrap();
    let mut s = session(TEXT);
    let r = run(&mut s, "references.autoMark", json!({"path": file.to_string_lossy()}));
    assert_eq!(r["marked"], json!(4), "{r}");
    assert_eq!(xe_fields(&s, 1), vec![(5, "XE \"Pets\"".to_string())]);
    assert_eq!(xe_fields(&s, 0), vec![(13, "XE \"Animals:Fox\"".to_string())]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A concordance file without a table, or no `path`/`entries` at all, is refused and changes nothing.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn automark_refuses_missing_concordance() {
    let mut s = session(TEXT);
    assert!(s.run("references.autoMark", &json!({})).is_err());
    let dir = std::env::temp_dir().join(format!("wc-automark-none-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("plain.html");
    std::fs::write(&file, "<html><body><p>fox</p></body></html>").unwrap();
    assert!(s.run("references.autoMark", &json!({"path": file.to_string_lossy()})).is_err());
    for i in 0..4 {
        assert!(xe_fields(&s, i).is_empty());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Generated index/TOC entry paragraphs are never marked (as Mark All).
#[test]
fn automark_skips_generated_entries() {
    let mut s = session("fox\nfox\n");
    s.doc.para_mut(StoryRef::Body, &Path::top(1)).unwrap().props.style = Some("ListEntry".into());
    let r = run(&mut s, "references.autoMark", json!({"entries": [["fox", ""]]}));
    assert_eq!(r["marked"], json!(1));
    assert!(xe_fields(&s, 1).is_empty());
}
