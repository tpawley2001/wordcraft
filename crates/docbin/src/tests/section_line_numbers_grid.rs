//! Section line numbers and document grid ([MS-DOC] §2.6.4): sprmSLnc 0x3013, sprmSNLnnMod 0x5015,
//! sprmSDxaLnn 0x9016, sprmSLnnMin 0x501B, sprmSClm 0x5032, sprmSDyaLinePitch 0x9031 and
//! sprmSDxtCharSpace 0x7030, read into the same model fields as DOCX `w:lnNumType` and `w:docGrid`.

use wordcraft_doc::section::{DocGrid, DocGridType, LineNumberRestart, LineNumbering, SectionProps};

use super::{StructSpec, grpprl, one_section, read, struct_doc};

const S_LNC: u16 = 0x3013;
const S_NLNN_MOD: u16 = 0x5015;
const S_DXA_LNN: u16 = 0x9016;
const S_LNN_MIN: u16 = 0x501B;
const S_CLM: u16 = 0x5032;
const S_DYA_LINE_PITCH: u16 = 0x9031;
const S_DXT_CHAR_SPACE: u16 = 0x7030;

fn u16b(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn i16b(v: i16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn i32b(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// The last section of a one-section document whose Sepx holds `prls`.
fn section(prls: &[(u16, Vec<u8>)]) -> SectionProps {
    let prls: Vec<(u16, &[u8])> = prls.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    let spec = one_section("Text\r", grpprl(&prls), vec![(0, 0, Vec::new())]);
    read(&struct_doc(&spec)).expect("opens").last_section
}

fn line_numbers(prls: &[(u16, Vec<u8>)]) -> Option<LineNumbering> {
    section(prls).line_numbers
}

fn grid(prls: &[(u16, Vec<u8>)]) -> Option<DocGrid> {
    section(prls).doc_grid
}

#[test]
fn no_sprms_leave_line_numbers_and_grid_off() {
    let s = section(&[]);
    assert_eq!((s.line_numbers, s.doc_grid), (None, None));
}

#[test]
fn line_numbers_are_read() {
    let l = line_numbers(&[(S_NLNN_MOD, u16b(5)), (S_DXA_LNN, i16b(360)), (S_LNC, vec![1]), (S_LNN_MIN, i16b(9))]);
    assert_eq!(l, Some(LineNumbering { count_by: 5, start: 10, distance: 18.0, restart: LineNumberRestart::Section }));
}

#[test]
fn count_by_alone_takes_the_defaults() {
    let l = line_numbers(&[(S_NLNN_MOD, u16b(1))]);
    assert_eq!(l, Some(LineNumbering { count_by: 1, start: 1, distance: 0.0, restart: LineNumberRestart::Page }));
}

#[test]
fn sprms_before_the_count_by_are_kept() {
    let l = line_numbers(&[(S_LNC, vec![2]), (S_LNN_MIN, i16b(99)), (S_DXA_LNN, i16b(720)), (S_NLNN_MOD, u16b(10))]);
    assert_eq!(l, Some(LineNumbering { count_by: 10, start: 100, distance: 36.0, restart: LineNumberRestart::Continuous }));
}

#[test]
fn zero_count_by_turns_line_numbers_off() {
    assert_eq!(line_numbers(&[(S_NLNN_MOD, u16b(0)), (S_LNC, vec![1]), (S_LNN_MIN, i16b(4))]), None);
    assert_eq!(line_numbers(&[(S_LNC, vec![1]), (S_DXA_LNN, i16b(360))]), None, "no count-by: off");
}

#[test]
fn restart_values() {
    let restart = |v: u8| line_numbers(&[(S_NLNN_MOD, u16b(1)), (S_LNC, vec![v])]).map(|l| l.restart);
    assert_eq!(restart(0), Some(LineNumberRestart::Page));
    assert_eq!(restart(1), Some(LineNumberRestart::Section));
    assert_eq!(restart(2), Some(LineNumberRestart::Continuous));
    assert_eq!(restart(7), Some(LineNumberRestart::Page), "unknown value: per page");
}

#[test]
fn hostile_line_number_values_are_clamped() {
    let l = line_numbers(&[(S_NLNN_MOD, u16b(60_000)), (S_LNN_MIN, i16b(-5)), (S_DXA_LNN, i16b(-200))]);
    assert_eq!(l, Some(LineNumbering { count_by: 100, start: 1, distance: 0.0, restart: LineNumberRestart::Page }));
}

#[test]
fn short_operands_are_ignored() {
    // A one-byte operand where two are needed ends the grpprl: nothing may panic.
    let s = section(&[(S_NLNN_MOD, u16b(3)), (S_DXA_LNN, vec![0x10])]);
    assert_eq!(s.line_numbers.map(|l| l.count_by), Some(3));
}

#[test]
fn line_grid_is_read() {
    let g = grid(&[(S_CLM, u16b(2)), (S_DYA_LINE_PITCH, i16b(312))]).expect("grid");
    assert_eq!((g.kind, g.char_space), (DocGridType::Lines, 0));
    assert!((g.line_pitch - 15.6).abs() < 0.01, "312 twips = 15.6 pt, got {}", g.line_pitch);
}

#[test]
fn grid_types() {
    let kind = |v: u16| grid(&[(S_CLM, u16b(v))]).map(|g| g.kind);
    assert_eq!(kind(0), Some(DocGridType::Default));
    assert_eq!(kind(1), Some(DocGridType::LinesAndChars));
    assert_eq!(kind(2), Some(DocGridType::Lines));
    assert_eq!(kind(3), Some(DocGridType::SnapToChars));
    assert_eq!(kind(9), Some(DocGridType::Default), "unknown value: no grid");
}

#[test]
fn character_grid_is_read() {
    let g = grid(&[(S_DXT_CHAR_SPACE, i32b(2048)), (S_CLM, u16b(1)), (S_DYA_LINE_PITCH, i16b(360))]).expect("grid");
    assert_eq!(g, DocGrid { kind: DocGridType::LinesAndChars, line_pitch: 18.0, char_space: 2048 });
    let g = grid(&[(S_DXT_CHAR_SPACE, i32b(-4096))]).expect("grid");
    assert_eq!(g, DocGrid { kind: DocGridType::Default, line_pitch: 18.0, char_space: -4096 });
}

#[test]
fn pitch_alone_makes_a_default_grid() {
    let g = grid(&[(S_DYA_LINE_PITCH, i16b(400))]).expect("grid");
    assert_eq!(g, DocGrid { kind: DocGridType::Default, line_pitch: 20.0, char_space: 0 });
}

#[test]
fn hostile_char_space_is_clamped() {
    let g = grid(&[(S_CLM, u16b(1)), (S_DXT_CHAR_SPACE, i32b(i32::MAX))]).expect("grid");
    assert_eq!(g.char_space, wordcraft_doc::section::MAX_CHAR_SPACE);
    let g = grid(&[(S_CLM, u16b(1)), (S_DXT_CHAR_SPACE, i32b(i32::MIN))]).expect("grid");
    assert_eq!(g.char_space, -wordcraft_doc::section::MAX_CHAR_SPACE);
}

#[test]
fn settings_do_not_leak_into_the_next_section() {
    let first = grpprl(&[(S_NLNN_MOD, &u16b(5)), (S_CLM, &u16b(2)), (S_DYA_LINE_PITCH, &i16b(312))]);
    let spec = StructSpec {
        text: "first\rsecond\r",
        header_stories: vec![],
        sections: vec![(6, first), (12, Vec::new())],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 0, Vec::new()), (6, 0, Vec::new())],
        plf_lst: Vec::new(),
        plf_lfo: Vec::new(),
        note_stories: vec![],
        endnote_stories: vec![],
        extra_pairs: Vec::new(),
        data_stream: Vec::new(),
    };
    let doc = read(&struct_doc(&spec)).expect("opens");
    let secs = doc.sections();
    assert_eq!(secs.len(), 2);
    assert_eq!(secs[0].1.line_numbers.as_ref().map(|l| l.count_by), Some(5));
    assert_eq!(secs[0].1.doc_grid.map(|g| g.kind), Some(DocGridType::Lines));
    assert_eq!((secs[1].1.line_numbers.as_ref(), secs[1].1.doc_grid), (None, None));
}
