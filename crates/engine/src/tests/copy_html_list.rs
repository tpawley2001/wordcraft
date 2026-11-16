//! Copy puts numbered and bulleted paragraphs on the clipboard as an HTML list, so Word and other
//! programs paste them as real lists with their numbers (from storytold/wordcraft#4485).

use super::*;

/// Copied numbered paragraphs reach other programs as an HTML list (Word turns it back into
/// numbering when it pastes; it pasted them unnumbered): with the levels' number formats, and
/// starting at the numbers the copied items have, the levels above a copy that starts inside a
/// sub-list included, as Word's own copy does.
#[test]
fn copying_numbered_paragraphs_puts_an_html_list_on_the_clipboard() {
    let mut s = s();
    run(&mut s, "document.setText", json!({"text": "One\nTwo\nSub A\nSub B\nThree"}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "para.numbering", json!({}));
    for k in [2, 3] {
        s.sel = crate::Selection::caret(Pos::body(k, 0));
        run(&mut s, "para.listLevel", json!({"level": 1}));
    }
    let copied = |s: &mut Session, from: usize| {
        s.sel = crate::Selection { anchor: Pos::body(from, 0), focus: Pos::body(4, 5) };
        let r = run(s, "edit.copy", json!({}));
        r["html"].as_str().unwrap_or_default().replace('\n', "")
    };
    let num = s.doc.para(StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap().props.numbering.unwrap().num;
    let sub = match s.doc.numbering.level(num, 1).map(|l| l.format) {
        Some(wordcraft_doc::section::NumFormat::LowerLetter) => " type=\"a\"",
        Some(wordcraft_doc::section::NumFormat::LowerRoman) => " type=\"i\"",
        _ => "",
    };
    let all = copied(&mut s, 0);
    assert!(all.contains(&format!("<ol><li>One</li><li>Two<ol{sub}><li>Sub A</li><li>Sub B</li></ol></li><li>Three</li></ol>")), "{all}");
    let from_two = copied(&mut s, 1);
    assert!(from_two.contains(&format!("<ol start=\"2\"><li>Two<ol{sub}><li>Sub A</li>")), "{from_two}");
    // From the second sub-item: under item 2, starting at its second number, then item 3.
    let from_sub_b = copied(&mut s, 3);
    assert!(from_sub_b.contains(&format!("<ol start=\"2\"><li><ol{sub} start=\"2\"><li>Sub B</li></ol></li><li>Three</li></ol>")), "{from_sub_b}");
}

/// A copy of numbered paragraphs whose list a plain paragraph interrupts: the items after it
/// are an HTML list of their own, and it starts at the number they show (the list goes on past
/// the plain paragraph), not at 1.
#[test]
fn copying_a_list_a_plain_paragraph_interrupts_keeps_the_later_items_numbers() {
    let mut s = s();
    run(&mut s, "document.setText", json!({"text": "One\nTwo\nNote\nThree\nFour"}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "para.numbering", json!({}));
    s.sel = crate::Selection::caret(Pos::body(2, 0));
    run(&mut s, "para.numbering", json!({"off": true}));
    for from in [0, 1] {
        s.sel = crate::Selection { anchor: Pos::body(from, 0), focus: Pos::body(4, 4) };
        let html = run(&mut s, "edit.copy", json!({}))["html"].as_str().unwrap_or_default().replace('\n', "");
        assert!(html.contains("<ol start=\"3\"><li>Three</li><li>Four</li></ol>"), "{html}");
    }
}

/// Numbered paragraphs copied from a table cell keep the numbers the cell shows for them.
#[test]
fn copying_numbered_paragraphs_in_a_table_cell_keeps_their_numbers() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
    let mut at = Vec::new();
    for (k, t) in ["One", "Two", "Three", "Four"].into_iter().enumerate() {
        if k > 0 {
            run(&mut s, "text.newParagraph", json!({}));
        }
        at.push(s.sel.focus.clone());
        run(&mut s, "text.insert", json!({"text": t}));
    }
    let end = s.sel.focus.clone();
    s.sel = crate::Selection { anchor: at[0].clone(), focus: end.clone() };
    run(&mut s, "para.numbering", json!({}));
    s.sel = crate::Selection { anchor: at[2].clone(), focus: end };
    let html = run(&mut s, "edit.copy", json!({}))["html"].as_str().unwrap_or_default().replace('\n', "");
    assert!(html.contains("<ol start=\"3\"><li>Three</li><li>Four</li></ol>"), "{html}");
}
