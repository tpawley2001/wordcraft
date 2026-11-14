//! Indent keys as in Word: Hanging Indent (Ctrl+T) and Reduce Hanging Indent (Ctrl+Shift+T) move the left indent
//! to the next or previous tab stop and keep the first line where it is; Increase / Decrease Indent (Ctrl+M /
//! Ctrl+Shift+M) step the paragraph's leftmost edge over the default tab stops. Measured in Microsoft 365 Word
//! 16.0.20527 for Windows (WordBasic HangingIndent, UnHang, Indent, UnIndent), 2026-11-14, default stops 36 pt.

use serde_json::json;
use wordcraft_doc::{Document, Pos};

use crate::Session;

/// One paragraph with left indent `l` and first-line indent `f` (points; negative = hanging), caret in it.
fn para(l: f32, f: f32) -> Session {
    let mut s = Session::new(Document::from_text("Para one text"));
    s.doc.settings.default_tab = 36.0;
    s.run("para.indents", &json!({"left": l, "firstLine": f})).unwrap();
    s
}

/// The paragraph's (left, first-line) indents.
fn ind(s: &Session) -> (f32, f32) {
    let p = s.doc.para_at(&Pos::body(0, 0)).unwrap();
    let r = s.doc.styles.resolve_para(&p.props);
    (r.indent_left, r.indent_first)
}

/// The indents after each of `n` runs of `command`, from (l, f).
fn presses(command: &str, (l, f): (f32, f32), n: usize) -> Vec<(f32, f32)> {
    let mut s = para(l, f);
    (0..n)
        .map(|_| {
            s.run(command, &json!({})).unwrap();
            ind(&s)
        })
        .collect()
}

fn check(command: &str, table: &[((f32, f32), &[(f32, f32)])]) {
    for (start, want) in table {
        assert_eq!(presses(command, *start, want.len()), want.to_vec(), "{command} from {start:?}");
    }
}

#[test]
fn hanging_indent_keeps_the_first_line_and_moves_the_left_indent_to_the_next_stop() {
    check(
        "para.hangingIndent",
        &[
            ((0.0, 0.0), &[(36.0, -36.0), (72.0, -72.0), (108.0, -108.0)]),
            ((36.0, 0.0), &[(72.0, -36.0), (108.0, -72.0), (144.0, -108.0)]),
            ((18.0, 0.0), &[(36.0, -18.0), (72.0, -54.0), (108.0, -90.0)]),
            ((0.0, 18.0), &[(36.0, -18.0), (72.0, -54.0), (108.0, -90.0)]),
            ((36.0, -36.0), &[(72.0, -72.0), (108.0, -108.0), (144.0, -144.0)]),
            ((50.0, -10.0), &[(72.0, -32.0), (108.0, -68.0), (144.0, -104.0)]),
            ((10.0, 25.0), &[(36.0, -1.0), (72.0, -37.0), (108.0, -73.0)]),
        ],
    );
}

#[test]
fn hanging_indent_uses_the_paragraphs_own_tab_stops_first() {
    let mut s = para(0.0, 0.0);
    s.run("para.tabs", &json!({"tabs": [{"pos": 72.0}, {"pos": 180.0}]})).unwrap();
    let mut seen = Vec::new();
    for _ in 0..4 {
        s.run("para.hangingIndent", &json!({})).unwrap();
        seen.push(ind(&s));
    }
    assert_eq!(seen, [(72.0, -72.0), (180.0, -180.0), (216.0, -216.0), (252.0, -252.0)]);
}

#[test]
fn reduce_hanging_indent_keeps_the_first_line_and_moves_the_left_indent_back_a_stop() {
    check(
        "para.removeHanging",
        &[
            ((0.0, 0.0), &[(0.0, 0.0), (0.0, 0.0)]),
            ((36.0, 0.0), &[(0.0, 36.0), (0.0, 36.0)]),
            ((18.0, 0.0), &[(0.0, 18.0), (0.0, 18.0)]),
            ((0.0, 18.0), &[(0.0, 18.0), (0.0, 18.0)]),
            ((36.0, -36.0), &[(0.0, 0.0), (0.0, 0.0)]),
            ((50.0, -10.0), &[(36.0, 4.0), (0.0, 40.0)]),
            ((10.0, 25.0), &[(0.0, 35.0), (0.0, 35.0)]),
        ],
    );
}

#[test]
fn ctrl_t_then_ctrl_shift_t_undo_each_other() {
    let mut s = para(0.0, 0.0);
    s.run("para.hangingIndent", &json!({})).unwrap();
    s.run("para.hangingIndent", &json!({})).unwrap();
    assert_eq!(ind(&s), (72.0, -72.0));
    s.run("para.removeHanging", &json!({})).unwrap();
    assert_eq!(ind(&s), (36.0, -36.0));
    s.run("para.removeHanging", &json!({})).unwrap();
    assert_eq!(ind(&s), (0.0, 0.0));
}

#[test]
fn increase_indent_steps_the_leftmost_edge_over_default_stops() {
    check(
        "para.indent",
        &[
            ((0.0, 0.0), &[(36.0, 0.0), (72.0, 0.0), (108.0, 0.0)]),
            ((36.0, 0.0), &[(72.0, 0.0), (108.0, 0.0), (144.0, 0.0)]),
            ((18.0, 0.0), &[(36.0, 0.0), (72.0, 0.0), (108.0, 0.0)]),
            ((0.0, 18.0), &[(36.0, 18.0), (72.0, 18.0), (108.0, 18.0)]),
            ((36.0, -36.0), &[(72.0, -36.0), (108.0, -36.0), (144.0, -36.0)]),
            ((50.0, -10.0), &[(82.0, -10.0), (118.0, -10.0), (154.0, -10.0)]),
            ((10.0, 25.0), &[(36.0, 25.0), (72.0, 25.0), (108.0, 25.0)]),
        ],
    );
}

#[test]
fn increase_indent_ignores_custom_tab_stops() {
    let mut s = para(0.0, 0.0);
    s.run("para.tabs", &json!({"tabs": [{"pos": 72.0}, {"pos": 180.0}]})).unwrap();
    let mut seen = Vec::new();
    for _ in 0..4 {
        s.run("para.indent", &json!({})).unwrap();
        seen.push(ind(&s));
    }
    assert_eq!(seen, [(36.0, 0.0), (72.0, 0.0), (108.0, 0.0), (144.0, 0.0)]);
}

#[test]
fn decrease_indent_steps_the_leftmost_edge_back_and_never_into_the_margin() {
    check(
        "para.outdent",
        &[
            ((0.0, 0.0), &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            ((36.0, 0.0), &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            ((18.0, 0.0), &[(0.0, 0.0)]),
            ((0.0, 18.0), &[(0.0, 18.0)]),
            ((36.0, -36.0), &[(36.0, -36.0), (36.0, -36.0)]),
            ((50.0, -10.0), &[(46.0, -10.0), (10.0, -10.0), (10.0, -10.0)]),
            ((10.0, 25.0), &[(0.0, 25.0)]),
        ],
    );
}
