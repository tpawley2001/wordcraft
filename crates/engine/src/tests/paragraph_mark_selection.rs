//! Paragraph-mark edits restore the complete document and selection in one undo step.
use serde_json::json;
use wordcraft_doc::{Document, Pos, StoryRef};

use crate::Session;

#[test]
fn paragraph_mark_edits_restore_the_document_in_one_step() {
    for in_cell in [false, true] {
        for tracked in [false, true] {
            for reverse in [false, true] {
                // Interior tail, whole paragraph, mark alone, and the final paragraph's tail.
                for (paragraph, start, end_paragraph, end) in [(0, 3, 1, 0), (0, 0, 1, 0), (0, 10, 1, 0), (1, 3, 1, 10)] {
                    for action in ["text.insert", "text.delete", "text.backspace"] {
                        let mut s = Session::new(Document::new());
                        if in_cell {
                            s.run("insert.table", &json!({"rows": 1, "cols": 2})).unwrap();
                        }
                        let first = s.sel.focus.clone();
                        s.run("text.insert", &json!({"text": "Uno corto.\rDue corto."})).unwrap();
                        s.doc.settings.track_changes = tracked;
                        let at = |paragraph, off| Pos { story: StoryRef::Body, path: first.path.with_last(paragraph), off };
                        let (a, b) = (at(paragraph, start), at(end_paragraph, end));
                        let (anchor, focus) = if reverse { (b, a) } else { (a, b) };
                        s.run("select.range", &json!({"anchor": anchor, "focus": focus})).unwrap();
                        let selection = s.sel.clone();
                        let before = serde_json::to_value(&s.doc).unwrap();
                        let depth = s.undo_depth();
                        s.run(action, &json!({"text": "x"})).unwrap();
                        assert_eq!(s.undo_depth(), depth + 1, "{action}, cell={in_cell}, tracked={tracked}");
                        if paragraph == 0 && start == 3 {
                            let p = s.doc.para_at(&at(0, 0)).unwrap();
                            assert!(p.mark.del.is_none(), "the paragraph mark stays untracked");
                            if tracked {
                                assert!(p.run_ranges().any(|(_, props)| props.del.is_some()));
                            } else {
                                assert_eq!(p.text, if action == "text.insert" { "Unox" } else { "Uno" });
                            }
                            assert_eq!(s.doc.para_at(&at(1, 0)).unwrap().text, "Due corto.");
                        }
                        s.run("edit.undo", &json!({})).unwrap();
                        assert_eq!(s.undo_depth(), depth);
                        assert_eq!(serde_json::to_value(&s.doc).unwrap(), before);
                        assert_eq!(s.sel, selection);
                    }
                }
            }
        }
    }
}
