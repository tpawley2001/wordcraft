//! The document options read from the `DopBase` ([MS-DOC] §2.7.3) of a Word 97-2003 file: the
//! default tab stop, Different odd and even pages, mirror margins, the hyphenation options and
//! Track Changes.

use super::{FibSpec, Piece, cfb_file, fib_bytes, plcpcd, read};
use wordcraft_doc::{DEFAULT_HYPHENATION_ZONE, Settings};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;
/// Where the test DOP starts in the Table stream (past the Clx).
const DOP_AT: u32 = 0x100;
/// Sizes of the Dop95 and Dop97 versions ([MS-DOC] §2.7).
const DOP95: usize = 88;
const DOP97: usize = 500;

/// A one-paragraph document whose Table stream holds `dop` (if any) at `DOP_AT`.
fn doc_with_dop(dop: Option<&[u8]>) -> wordcraft_doc::Document {
    let text = "Hello\r";
    let mut word = vec![0u8; super::TEXT_FC as usize];
    word.extend(text.bytes());
    let plc = plcpcd(&[Piece { compressed: true, text: text.into() }], super::TEXT_FC);
    let mut clx = vec![0x02u8];
    clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    clx.extend_from_slice(&plc);
    let mut table = vec![0u8; super::CLX_AT as usize];
    table.extend_from_slice(&clx);
    let mut pairs = vec![(33, super::CLX_AT, clx.len() as u32)];
    if let Some(dop) = dop {
        table.resize(DOP_AT as usize, 0);
        table.extend_from_slice(dop);
        pairs.push((DOP_PAIR, DOP_AT, dop.len() as u32));
    }
    let fib = fib_bytes(&FibSpec { ccp_text: text.chars().count() as u32, pairs, ..Default::default() });
    word[..fib.len()].copy_from_slice(&fib);
    read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens")
}

/// The `DopBase` options the test sets.
#[derive(Default, Clone, Copy)]
struct Base {
    facing: bool,
    hyph_caps: bool,
    auto_hyph: bool,
    rev_marking: bool,
    mirror: bool,
    dxa_tab: u16,
    dxa_hot_z: u16,
    consec: u16,
}

/// A zeroed DOP of `len` bytes holding `b`'s fields where [MS-DOC] §2.7.3 puts them.
fn dop(len: usize, b: Base) -> Vec<u8> {
    let mut d = vec![0u8; len];
    let mut put = |at: usize, v: u8| {
        if let Some(x) = d.get_mut(at) {
            *x |= v;
        }
    };
    if b.facing {
        put(0, 0x01);
    }
    if b.hyph_caps {
        put(5, 0x08);
    }
    if b.auto_hyph {
        put(5, 0x10);
    }
    if b.rev_marking {
        put(5, 0x80);
    }
    if b.mirror {
        put(6, 0x20);
    }
    for (at, v) in [(10, b.dxa_tab), (14, b.dxa_hot_z), (16, b.consec)] {
        if len >= at + 2 {
            d[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    d
}

/// (default tab, hyphenation zone, consecutive limit) and the six flags
/// (even/odd, mirror, auto hyphenation, hyphenate caps, track changes).
fn opts(s: &Settings) -> (f32, f32, u32, [bool; 5]) {
    (
        s.default_tab,
        s.hyphenation_zone,
        s.consecutive_hyphen_limit,
        [s.even_odd_headers, s.mirror_margins, s.auto_hyphenation, s.hyphenate_caps, s.track_changes],
    )
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn no_dop_keeps_the_defaults() {
    let d = doc_with_dop(None);
    let def = Settings::default();
    assert_eq!(opts(&d.settings), opts(&def));
}

#[test]
fn a_zeroed_dop_keeps_tab_and_zone_but_clears_the_flags() {
    let d = doc_with_dop(Some(&dop(DOP97, Base::default())));
    let (tab, zone, limit, flags) = opts(&d.settings);
    assert!(close(tab, 36.0), "dxaTab 0 keeps the default: {tab}");
    assert!(close(zone, DEFAULT_HYPHENATION_ZONE), "dxaHotZ 0 keeps the default: {zone}");
    assert_eq!(limit, 0);
    assert_eq!(flags, [false; 5], "hyphenate_caps follows fHyphCapitals even when it clears the default");
}

#[test]
fn every_option_is_read() {
    let b = Base { facing: true, hyph_caps: true, auto_hyph: true, rev_marking: true, mirror: true, dxa_tab: 708, dxa_hot_z: 425, consec: 3 };
    let d = doc_with_dop(Some(&dop(DOP97, b)));
    let (tab, zone, limit, flags) = opts(&d.settings);
    assert!(close(tab, 35.4), "{tab}");
    assert!(close(zone, 21.25), "{zone}");
    assert_eq!(limit, 3);
    assert_eq!(flags, [true; 5]);
}

#[test]
fn each_flag_sets_only_its_own_option() {
    let one = |i: usize| {
        let mut b = Base { dxa_tab: 720, dxa_hot_z: 360, ..Base::default() };
        match i {
            0 => b.facing = true,
            1 => b.mirror = true,
            2 => b.auto_hyph = true,
            3 => b.hyph_caps = true,
            _ => b.rev_marking = true,
        }
        b
    };
    for i in 0..5 {
        let d = doc_with_dop(Some(&dop(DOP97, one(i))));
        let (tab, zone, _, flags) = opts(&d.settings);
        assert!(close(tab, 36.0) && close(zone, 18.0));
        let mut want = [false; 5];
        want[i] = true;
        assert_eq!(flags, want, "flag {i}");
    }
}

#[test]
fn a_dop95_is_read_too() {
    let d = doc_with_dop(Some(&dop(DOP95, Base { dxa_tab: 567, facing: true, ..Base::default() })));
    assert!(close(d.settings.default_tab, 28.35), "{}", d.settings.default_tab);
    assert!(d.settings.even_odd_headers);
}

#[test]
fn a_dop_too_short_for_dopbase_keeps_the_defaults() {
    let b = Base { facing: true, mirror: true, rev_marking: true, dxa_tab: 708, ..Base::default() };
    let d = doc_with_dop(Some(&dop(12, b)));
    assert_eq!(opts(&d.settings), opts(&Settings::default()));
}

#[test]
fn out_of_range_tab_and_zone_keep_the_defaults() {
    let d = doc_with_dop(Some(&dop(DOP97, Base { dxa_tab: 40000, dxa_hot_z: 0xFFFF, ..Base::default() })));
    assert!(close(d.settings.default_tab, 36.0), "{}", d.settings.default_tab);
    assert!(close(d.settings.hyphenation_zone, DEFAULT_HYPHENATION_ZONE), "{}", d.settings.hyphenation_zone);
}
