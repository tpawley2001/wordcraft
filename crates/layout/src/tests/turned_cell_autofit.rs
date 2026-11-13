//! Turned (vertical) header cells in a table whose columns fit their content: a turned cell asks
//! its column for the room its lines take across it (their heights plus the cell's left and right
//! margins), never for the width of its words laid flat. Measured in Microsoft 365 Word on
//! synthetic DOCX files (compatibility mode 11): a table 186 pt wide, grid 150 / 18 / 18 pt,
//! 1.5 pt left and right cell margins, a header row at least 108 pt high whose cells in columns 2
//! and 3 hold "Credentialing Interface" in 9 pt Times New Roman, body cells "E" under them.
//! Widths that depend on the font are worked out from the test font's own line height and word
//! widths; the 18 pt columns are exact.

use super::*;
use wordcraft_doc::props::HeightRule;

const SIZE: f32 = 9.0;
const SIDE: f32 = 1.5;
const HEADER: &str = "Credentialing Interface";

fn chr() -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(SIZE), ..Default::default() }
}

fn para(text: &str) -> wordcraft_doc::Paragraph {
    let mut p = wordcraft_doc::Paragraph::with_text(text, chr());
    p.mark = chr();
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
    p
}

fn blocks(paras: &[&str]) -> wordcraft_doc::Blocks {
    paras.iter().map(|t| wordcraft_doc::para_block(para(t))).collect()
}

struct Probe {
    turn: TextDirection,
    fixed: bool,
    /// Body text of column 2.
    body2: &'static str,
    /// The header paragraphs of columns 2 and 3.
    header: &'static [&'static str],
}

const T1: Probe = Probe { turn: TextDirection::Up, fixed: false, body2: "E", header: &[HEADER] };

fn probe_doc(p: &Probe) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = 11;
    let mut t = Table::new(2, 3, 186.0);
    t.props.style = None;
    t.props.fixed = p.fixed;
    t.props.width = Some(186.0);
    t.props.cell_margins = Some([0.0, SIDE, 0.0, SIDE]);
    t.grid = vec![150.0, 18.0, 18.0];
    t.rows[0].props.height = Some(108.0);
    t.rows[0].props.height_rule = HeightRule::AtLeast;
    let texts = [["Requirement", "", ""], ["The record shows the provider was checked before the visit was booked.", p.body2, "E"]];
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.props.width = t.grid.get(ci).copied();
            cell.blocks = if ri == 0 && ci > 0 { blocks(p.header) } else { blocks(&[texts[ri][ci]]) };
            if ri == 0 && ci > 0 {
                cell.props.text_direction = p.turn;
            }
        }
    }
    d.body = vec![wordcraft_doc::para_block(para("before")), Arc::new(Block::Table(t)), wordcraft_doc::para_block(para("after"))];
    d
}

/// The widths of the table's three columns, from its body row's cells.
fn columns(p: &Probe) -> [f32; 3] {
    crate::test_fonts();
    let l = lay(&probe_doc(p));
    [0, 1, 2].map(|c| cell_rect(&l, 0, 1, c).expect("body cell").w)
}

/// The test font's width of `word` laid flat, and the height of one line, at 9 pt.
fn word_and_line(word: &str) -> (f32, f32) {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = vec![wordcraft_doc::para_block(para(word)), wordcraft_doc::para_block(para(word))];
    let l = lay(&d);
    let lines: Vec<_> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| if let Placed::Lines { para, x, y, .. } = it { Some((para.clone(), *x, *y)) } else { None })
        .collect();
    let (pl, x, y0) = &lines[0];
    let end = pl.x_of(0, word.len()).unwrap() + x;
    let start = pl.x_of(0, 0).unwrap() + x;
    (end - start, lines[1].2 - y0)
}

fn close(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= 0.5, "{what}: {got} pt, expected {want} pt");
}

#[test]
fn turned_bt_lr_header_keeps_its_narrow_columns() {
    let cols = columns(&T1);
    close(cols[1], 18.0, "column 2");
    close(cols[2], 18.0, "column 3");
    close(cols[0], 150.0, "the text column keeps the rest");
}

#[test]
fn turned_tb_rl_header_keeps_its_narrow_columns() {
    let cols = columns(&Probe { turn: TextDirection::Down, ..T1 });
    close(cols[1], 18.0, "column 2");
    close(cols[2], 18.0, "column 3");
    close(cols[0], 150.0, "the text column keeps the rest");
}

#[test]
fn an_unturned_header_still_widens_its_columns() {
    // Laid flat, the header's longest word sets each column's narrowest width (52 pt in Word).
    let (word, _) = word_and_line("Credentialing");
    let want = word + 2.0 * SIDE;
    assert!(want > 30.0, "the word is wider than the 18 pt grid column: {word}");
    let cols = columns(&Probe { turn: TextDirection::Horizontal, ..T1 });
    close(cols[1], want, "column 2");
    close(cols[2], want, "column 3");
}

#[test]
fn a_fixed_table_with_turned_cells_is_unchanged() {
    let cols = columns(&Probe { fixed: true, ..T1 });
    assert_eq!(cols, [150.0, 18.0, 18.0]);
}

#[test]
fn a_wide_body_word_widens_only_its_own_column() {
    // An unturned body cell still counts: "Wideword" makes column 2 as wide as it (40.7 pt in
    // Word); column 3 keeps 18 pt.
    let (word, _) = word_and_line("Wideword");
    let cols = columns(&Probe { body2: "Wideword", ..T1 });
    close(cols[1], word + 2.0 * SIDE, "column 2");
    close(cols[2], 18.0, "column 3");
}

#[test]
fn two_turned_paragraphs_take_two_line_heights() {
    // Two paragraphs: two turned lines side by side (28 pt in Word).
    let (_, line) = word_and_line("Credentialing");
    let want = 2.0 * line + 2.0 * SIDE;
    assert!(want > 18.0, "two lines don't fit the 18 pt grid column: {line}");
    let cols = columns(&Probe { header: &["Credentialing", "Interface"], ..T1 });
    close(cols[1], want, "column 2");
    close(cols[2], want, "column 3");
}
