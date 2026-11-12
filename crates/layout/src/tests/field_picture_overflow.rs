//! Word starts a new line at the end of a field whose result is an inline picture too wide for
//! its line: the paragraph gets one more (empty) line. A bare picture of the same width, or one
//! followed by an empty field, keeps its single line. Measured in Word's PDF output with a
//! 432 pt column; synthetic shapes only, no external documents.
use super::*;
use wordcraft_doc::para::InlineObject as Obj;

const COLUMN: f32 = 432.0;

fn start(instr: &str) -> Obj {
    Obj::FieldStart { instr: instr.into(), locked: false, dirty: false }
}

fn pic(w: f32) -> Obj {
    picture(w, 50.0, Default::default())
}

/// The first paragraph of `text` with `objects` put at its start, in the order given.
fn laid_out(text: &str, objects: Vec<Obj>) -> std::sync::Arc<para::ParaLayout> {
    crate::test_fonts();
    let mut d = Document::from_text(text);
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + COLUMN;
    for o in objects.into_iter().rev() {
        d.insert_object(&Pos::body(0, 0), o, &Default::default()).unwrap();
    }
    let l = lay(&d);
    let Some(para) = l.pages[0].items.iter().find_map(|i| if let Placed::Lines { para, .. } = i { Some(para.clone()) } else { None }) else {
        panic!("no lines")
    };
    para
}

/// Each line as one char per cluster: `O` inline object, `T` text, `_` space; markers left out.
fn lines(para: &para::ParaLayout) -> Vec<String> {
    para.lines
        .iter()
        .map(|line| {
            let Some(clusters) = para.clusters.get(line.c0..line.c1) else {
                panic!("line clusters {}..{} out of range ({} clusters)", line.c0, line.c1, para.clusters.len())
            };
            clusters
                .iter()
                .filter_map(|c| match c.kind {
                    para::ClKind::Object(_) => Some('O'),
                    para::ClKind::Text => Some('T'),
                    para::ClKind::Space => Some('_'),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

fn in_field(instr: &str, w: f32) -> Vec<Obj> {
    vec![start(instr), pic(w), Obj::FieldEnd]
}

#[test]
fn a_bare_picture_wider_than_the_line_keeps_one_line() {
    for w in [430.0, 433.0, 456.0] {
        let got = lines(&laid_out("", vec![pic(w)]));
        assert_eq!(got, ["O"], "{w} pt wide");
    }
}

#[test]
fn a_wide_picture_before_an_empty_field_keeps_one_line() {
    let got = lines(&laid_out("", vec![pic(456.0), start(" EMBED Excel.Sheet.8 "), Obj::FieldEnd]));
    assert_eq!(got, ["O"]);
}

#[test]
fn a_field_result_picture_that_fits_keeps_one_line() {
    let got = lines(&laid_out("", in_field(" EMBED Excel.Sheet.8 ", 430.0)));
    assert_eq!(got, ["O"]);
}

#[test]
fn a_field_result_picture_one_point_too_wide_is_followed_by_an_empty_line() {
    let got = lines(&laid_out("", in_field(" EMBED Excel.Sheet.8 ", 433.0)));
    assert_eq!(got, ["O", ""]);
}

#[test]
fn any_field_kind_with_a_too_wide_picture_result_gets_the_line() {
    for instr in [" EMBED Excel.Sheet.8 ", " QUOTE x ", " HYPERLINK \"http://example.com\" "] {
        let got = lines(&laid_out("", in_field(instr, 456.0)));
        assert_eq!(got, ["O", ""], "{instr}");
    }
}

#[test]
fn the_added_line_is_as_high_as_an_empty_line() {
    let empty = laid_out("", vec![]);
    let bare = laid_out("", vec![pic(456.0)]);
    let field = laid_out("", in_field(" EMBED Excel.Sheet.8 ", 456.0));
    assert_eq!(empty.lines.len(), 1);
    assert_eq!(bare.lines.len(), 1);
    assert_eq!(field.lines.len(), 2, "{:?}", lines(&field));
    let (Some(first), Some(second)) = (field.lines.first(), field.lines.get(1)) else { panic!("two lines") };
    assert!((first.height - bare.height).abs() < 0.01, "picture line {} vs bare {}", first.height, bare.height);
    assert!((second.height - empty.height).abs() < 0.01, "added line {} vs empty line {}", second.height, empty.height);
    assert!((field.height - (bare.height + empty.height)).abs() < 0.01, "paragraph {} vs {} + {}", field.height, bare.height, empty.height);
}

#[test]
fn text_after_the_field_shares_the_second_line() {
    let got = lines(&laid_out("abc", in_field(" EMBED Excel.Sheet.8 ", 456.0)));
    assert_eq!(got, ["O", "TTT"]);
}
