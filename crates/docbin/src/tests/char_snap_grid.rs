//! Character "Snap to grid" (sprmCFUsePgsuSettings 0x0868, [MS-DOC] §2.6.1; OOXML `w:rPr/w:snapToGrid`,
//! ECMA-376 Part 1 §17.3.2.34): a one-byte ToggleOperand read into `CharProps::snap_to_grid`.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

const C_F_USE_PGSU: u16 = 0x0868;

/// "Hello" in one run with `run` as its CHPX, in a paragraph of style Body (istd 1) whose CHPX is `style_chpx`.
fn doc(run: Vec<u8>, style_chpx: Vec<u8>) -> Document {
    let spec = RichSpec {
        text: "Hello\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body", Vec::new(), style_chpx)],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, run)],
        papx_entries: vec![(5, 1, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn run_props(d: &Document) -> CharProps {
    d.body.iter().find_map(|b| b.as_para()).expect("paragraph").props_of_char(0).clone()
}

#[test]
fn snap_to_grid_off_and_on_are_read() {
    assert_eq!(run_props(&doc(grpprl(&[(C_F_USE_PGSU, &[0])]), Vec::new())).snap_to_grid, Some(false), "0x0868 = 00: off");
    assert_eq!(run_props(&doc(grpprl(&[(C_F_USE_PGSU, &[1])]), Vec::new())).snap_to_grid, Some(true), "0x0868 = 01: on");
    assert_ne!(run_props(&doc(Vec::new(), Vec::new())).snap_to_grid, Some(false), "no sprm: not off");
}

#[test]
fn a_style_turns_snap_to_grid_off_and_0x81_flips_it_back_on() {
    let style = grpprl(&[(C_F_USE_PGSU, &[0])]);
    let d = doc(Vec::new(), style.clone());
    let st = d.styles.styles.iter().find(|s| s.name == "Body").expect("style").chr.clone();
    assert_eq!(st.snap_to_grid, Some(false), "style Body carries Snap to grid off");
    let flipped = doc(grpprl(&[(C_F_USE_PGSU, &[0x81])]), style);
    assert_eq!(run_props(&flipped).snap_to_grid, Some(true), "0x81 over a style with it off: on");
}

#[test]
fn a_cut_off_sprm_is_ignored() {
    // The opcode with no operand byte left in the grpprl.
    let d = doc(C_F_USE_PGSU.to_le_bytes().to_vec(), Vec::new());
    assert_ne!(run_props(&d).snap_to_grid, Some(false), "a cut-off 0x0868 is not read as off");
}
