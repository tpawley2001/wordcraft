//! The Formula dialog's Paste bookmark list (Table Layout › Data › Formula): the names it offers
//! (`table_formula::paste_bookmark_names`) and that a name it offers works as a formula operand.

use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};
use wordcraft_engine::cmd::table_formula::{evaluate_text_field, paste_bookmark_names};

/// A body of one paragraph per (bookmark name, text), each wrapped in its bookmark.
fn marked(marks: &[(&str, &str)]) -> Document {
    let text: Vec<&str> = marks.iter().map(|m| m.1).collect();
    let mut doc = Document::from_text(&text.join("\n"));
    let paths = doc.para_paths(StoryRef::Body);
    assert!(paths.len() >= marks.len(), "one paragraph per bookmark");
    for (path, (name, _)) in paths.into_iter().zip(marks) {
        let p = doc.para_mut(StoryRef::Body, &path).unwrap();
        p.insert_object(p.len(), InlineObject::BookmarkEnd { name: (*name).into() }, &CharProps::default()).unwrap();
        p.insert_object(0, InlineObject::BookmarkStart { name: (*name).into() }, &CharProps::default()).unwrap();
    }
    doc
}

#[test]
fn a_document_without_bookmarks_offers_none() {
    assert_eq!(paste_bookmark_names(&Document::from_text("12\nabc")), Vec::<String>::new());
}

#[test]
fn names_are_sorted_ignoring_case_and_keep_their_own_case() {
    let doc = marked(&[("Zeta", "1"), ("alpha", "2"), ("Net_Total", "3")]);
    assert_eq!(paste_bookmark_names(&doc), ["alpha", "Net_Total", "Zeta"]);
}

#[test]
fn hidden_bookmarks_are_left_out() {
    let doc = marked(&[("_Toc123", "Heading"), ("Total", "12"), ("_Ref456", "Figure 1"), ("_GoBack", "x")]);
    assert_eq!(paste_bookmark_names(&doc), ["Total"]);
}

#[test]
fn a_name_that_differs_only_in_case_is_listed_once_as_first_written() {
    let doc = marked(&[("Total", "12"), ("TOTAL", "99"), ("total", "7")]);
    assert_eq!(paste_bookmark_names(&doc), ["Total"]);
}

#[test]
fn names_shaped_like_cells_or_functions_are_still_listed() {
    let doc = marked(&[("A1", "4"), ("SUM", "5"), ("Rate", "6")]);
    assert_eq!(paste_bookmark_names(&doc), ["A1", "Rate", "SUM"]);
}

#[test]
fn an_offered_name_works_as_a_formula_operand() {
    let doc = marked(&[("Zeta", "4"), ("alpha", "abc"), ("Net_Total", "12")]);
    let names = paste_bookmark_names(&doc);
    assert_eq!(names.len(), 3, "{names:?}");
    // The pasted name, then what the user types after it. Text that is not a number counts as 0.
    let results: Vec<String> = names.iter().map(|n| evaluate_text_field(&doc, None, &format!("={n}*2")).unwrap()).collect();
    assert_eq!(results, ["0", "24", "8"]);
}
