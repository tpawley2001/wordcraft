//! CSS `display: none` on import: Word keeps the content as hidden text (character `hidden`, the
//! paragraph mark too when the whole block is hidden) instead of showing it or dropping it.
//! Expected values are what Microsoft 365 Word shows for an HTML altChunk with these cases.

use wordcraft_doc::{Block, Document, Paragraph};

/// One body paragraph (tables walked in order): its text without hidden characters, its hidden
/// characters, whether its mark is hidden, and whether it is a list paragraph.
#[derive(Debug, PartialEq)]
struct P {
    shown: String,
    hidden: String,
    mark_hidden: bool,
    list: bool,
}

fn para(p: &Paragraph) -> P {
    let (mut shown, mut hidden) = (String::new(), String::new());
    let runs: Vec<_> = p.run_ranges().collect();
    for (i, c) in p.text.char_indices() {
        let h = runs.iter().find(|(r, _)| r.contains(&i)).is_some_and(|(_, props)| props.hidden == Some(true));
        if h { hidden.push(c) } else { shown.push(c) }
    }
    P { shown, hidden, mark_hidden: p.mark.hidden == Some(true), list: p.props.numbering.is_some() }
}

fn walk(blocks: &[std::sync::Arc<Block>], out: &mut Vec<P>) {
    for b in blocks {
        match &**b {
            Block::Para(p) => out.push(para(p)),
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        walk(&cell.blocks, out);
                    }
                }
            }
        }
    }
}

fn paras(html: &str) -> Vec<P> {
    let doc: Document = crate::html::import(html);
    let mut out = Vec::new();
    walk(&doc.body, &mut out);
    out
}

/// The text a reader sees: paragraphs with shown text, in order.
fn visible(html: &str) -> Vec<String> {
    paras(html).into_iter().filter(|p| !p.shown.is_empty()).map(|p| p.shown).collect()
}

/// All hidden text, in order, one entry per paragraph that has some.
fn hidden(html: &str) -> Vec<String> {
    paras(html).into_iter().filter(|p| !p.hidden.is_empty()).map(|p| p.hidden).collect()
}

#[test]
fn a_hidden_block_is_kept_as_hidden_text_with_a_hidden_mark() {
    let ps = paras("<p>A</p><p style='display:none'>B</p><p>C</p>");
    let b = ps.iter().find(|p| p.hidden == "B").expect("B is kept, not dropped");
    assert_eq!((b.shown.as_str(), b.mark_hidden), ("", true));
    assert_eq!(visible("<p>A</p><p style='display:none'>B</p><p>C</p>"), ["A", "C"]);
    // Shown paragraphs keep a shown mark.
    assert!(ps.iter().filter(|p| p.shown == "A" || p.shown == "C").all(|p| !p.mark_hidden));
}

#[test]
fn display_none_from_a_class_rule_and_an_ancestor_hides_descendants() {
    let html = "<style>.h{display:none}</style><p>V</p><div class='h'><p>CLASS</p></div>\
                <div style='display:none'><p>OUTER</p><p style='display:none'>NESTED</p></div><p>LAST</p>";
    assert_eq!(visible(html), ["V", "LAST"]);
    assert_eq!(hidden(html), ["CLASS", "OUTER", "NESTED"]);
}

#[test]
fn a_hidden_span_hides_only_its_text() {
    let ps = paras("<p>A_<span style='display:none'>SECRET</span>_B</p>");
    assert_eq!(ps.len(), 1);
    assert_eq!((ps[0].shown.as_str(), ps[0].hidden.as_str(), ps[0].mark_hidden), ("A__B", "SECRET", false));
}

#[test]
fn a_shown_display_on_a_child_reveals_it_inside_a_hidden_parent() {
    // Word shows REVEALED although its div is display:none; the sibling stays hidden.
    let html = "<p>V</p><div style='display:none'><p style='display:block'>REVEALED</p><p>STAYS</p></div><p>L</p>";
    assert_eq!(visible(html), ["V", "REVEALED", "L"]);
    assert_eq!(hidden(html), ["STAYS"]);
    // Other display values never hide.
    assert_eq!(visible("<p style='display:inline'>I</p><p style='display: Block'>B</p>"), ["I", "B"]);
}

#[test]
fn hidden_cells_rows_and_list_items_keep_their_structure() {
    let html = "<table border=1><tr><td style='display:none'>CELL_H</td><td>CELL_S</td></tr>\
                <tr style='display:none'><td>ROW_A</td><td>ROW_B</td></tr></table>\
                <ol><li>ONE</li><li style='display:none'>TWO</li><li>THREE</li></ol>";
    let doc = crate::html::import(html);
    let t = doc.body.iter().find_map(|b| if let Block::Table(t) = &**b { Some(t) } else { None }).expect("table");
    assert_eq!(t.rows.iter().map(|r| r.cells.len()).collect::<Vec<_>>(), [2, 2], "cells and rows are kept");
    assert_eq!(visible(html), ["CELL_S", "ONE", "THREE"]);
    assert_eq!(hidden(html), ["CELL_H", "ROW_A", "ROW_B", "TWO"]);
    // The hidden item stays a list paragraph, so the next item is numbered 3 as in Word.
    let items: Vec<_> = paras(html).into_iter().filter(|p| p.list).map(|p| (p.shown + &p.hidden, p.mark_hidden)).collect();
    assert_eq!(items, [("ONE".to_string(), false), ("TWO".to_string(), true), ("THREE".to_string(), false)]);
}

#[test]
fn an_unclosed_hidden_div_hides_the_rest_of_the_page() {
    // `<div/>` is not self-closing in HTML: Word hides everything after it.
    let html = "<p>BEFORE</p><p style='display:none'>INLINE</p><div style='display:none'/><p>AFTER</p>";
    assert_eq!(visible(html), ["BEFORE"]);
    assert_eq!(hidden(html), ["INLINE", "AFTER"]);
}

#[test]
fn pages_without_display_none_import_as_before() {
    let html = "<p>A <span>b</span></p><div><p>C</p></div><table><tr><td>D</td></tr></table>";
    assert_eq!(visible(html), ["A b", "C", "D"]);
    assert!(hidden(html).is_empty());
}
