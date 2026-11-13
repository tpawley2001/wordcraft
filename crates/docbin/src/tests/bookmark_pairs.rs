//! Bookmarks ([MS-DOC] §2.8.10 PlcfBkf, §2.8.12 PlcfBkl, §2.9.71 FBKF): PlcfBkl holds only CPs
//! (n + 1 of them for n bookmarks, no data after them), and a bookmark's end is the PlcfBkl CP
//! its FBKF names (`ibkl`), not the one at its own index: PlcfBkl is sorted by CP, so nested
//! bookmarks end in another order than they start. Every bookmark of the file is read.

use super::{StructSpec, plc, read, struct_doc, structured_spec, sttbf_bkmk};
use std::collections::BTreeMap;
use wordcraft_doc::para::InlineObject;

/// PlcfBkf: the start CPs and the text end, then one FBKF (`ibkl`, `bkc` = 0) per bookmark.
fn bkf(starts: &[u32], ccp: u32, ibkl: &[u16]) -> Vec<u8> {
    let mut cps = starts.to_vec();
    cps.push(ccp);
    let mut data = Vec::new();
    for i in ibkl {
        data.extend_from_slice(&i.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
    }
    plc(&cps, &data)
}

fn open(text: &'static str, names: &[&str], plcf_bkf: Vec<u8>, plcf_bkl: Vec<u8>) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let spec = StructSpec { text, extra_pairs: vec![(21, sttbf_bkmk(names)), (22, plcf_bkf), (23, plcf_bkl)], ..structured_spec(text, ccp) };
    read(&struct_doc(&spec)).expect("opens")
}

/// The text between each bookmark's start and end in the first paragraph (object characters
/// left out). A bookmark with no end is not listed.
fn ranges(doc: &wordcraft_doc::Document) -> BTreeMap<String, String> {
    let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let mut objects = p.objects.iter();
    let mut open: BTreeMap<String, String> = BTreeMap::new();
    let mut done = BTreeMap::new();
    for c in p.text.chars() {
        if c != '\u{FFFC}' {
            for text in open.values_mut() {
                text.push(c);
            }
            continue;
        }
        match objects.next() {
            Some(InlineObject::BookmarkStart { name }) => {
                open.insert(name.clone(), String::new());
            }
            Some(InlineObject::BookmarkEnd { name }) => {
                if let Some(text) = open.remove(name) {
                    done.insert(name.clone(), text);
                }
            }
            _ => {}
        }
    }
    done
}

fn expect(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(name, text)| (name.to_string(), text.to_string())).collect()
}

const TEXT: &str = "One two three four\r";
const CCP: u32 = 19;

#[test]
fn every_bookmark_is_read_when_the_ends_hold_only_cps() {
    let doc = open(TEXT, &["a", "b", "c"], bkf(&[0, 4, 8], CCP, &[0, 1, 2]), plc(&[3, 7, 13, CCP], &[]));
    assert_eq!(ranges(&doc), expect(&[("a", "One"), ("b", "two"), ("c", "three")]));
}

#[test]
fn a_nested_bookmark_and_the_one_around_it_take_the_ends_their_starts_name() {
    // The ends are sorted by CP: the inner bookmark's (7) comes before the outer one's (13).
    let doc = open(TEXT, &["outer", "inner"], bkf(&[0, 4], CCP, &[1, 0]), plc(&[7, 13, CCP], &[]));
    assert_eq!(ranges(&doc), expect(&[("outer", "One two three"), ("inner", "two")]));
    let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
    assert_eq!(p.text, "\u{FFFC}One \u{FFFC}two\u{FFFC} three\u{FFFC} four");
}

#[test]
fn nine_bookmarks_are_nine_bookmarks() {
    let names = ["b0", "b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8"];
    let starts: Vec<u32> = (0..9).collect();
    let mut ends: Vec<u32> = (1..10).collect();
    ends.push(10);
    let ibkl: Vec<u16> = (0..9).collect();
    let doc = open("abcdefghi\r", &names, bkf(&starts, 10, &ibkl), plc(&ends, &[]));
    let want: Vec<(&str, String)> = names.iter().zip("abcdefghi".chars()).map(|(n, c)| (*n, c.to_string())).collect();
    let want: BTreeMap<String, String> = want.into_iter().map(|(n, t)| (n.to_string(), t)).collect();
    assert_eq!(ranges(&doc), want);
}

#[test]
fn a_bookmark_naming_an_end_that_is_not_there_is_skipped_alone() {
    let doc = open(TEXT, &["a", "b", "c"], bkf(&[0, 4, 8], CCP, &[0, 9, 2]), plc(&[3, 7, 13, CCP], &[]));
    assert_eq!(ranges(&doc), expect(&[("a", "One"), ("c", "three")]));
}

#[test]
fn a_cut_off_end_table_keeps_the_bookmarks_it_covers() {
    let doc = open(TEXT, &["a", "b", "c"], bkf(&[0, 4, 8], CCP, &[0, 1, 2]), plc(&[3, 7], &[]));
    let got = ranges(&doc);
    assert_eq!(got.get("a").map(String::as_str), Some("One"), "{got:?}");
    assert!(!got.contains_key("c"), "{got:?}");
}

#[test]
fn spare_bytes_after_the_end_cps_are_ignored() {
    let doc = open(TEXT, &["a", "b"], bkf(&[0, 4], CCP, &[0, 1]), plc(&[3, 7, CCP], &[0, 0, 0, 0]));
    assert_eq!(ranges(&doc), expect(&[("a", "One"), ("b", "two")]));
}

#[test]
fn past_the_cap_each_kept_bookmark_ends_where_its_own_fbkf_says() {
    // More bookmarks than the reader keeps: the FBKFs still start after all the CPs, not after
    // the kept ones. Bookmark i covers character i alone (its end, PlcfBkl[i], is i + 1); every
    // character is different, so a wrong end shows.
    let max = crate::notes::MAX;
    let n = max + 2;
    let text: String = (0..n).filter_map(|i| char::from_u32(0x4E00 + i as u32)).chain(['\r']).collect();
    let text: &'static str = Box::leak(text.into_boxed_str());
    let ccp = text.chars().count() as u32;
    let starts: Vec<u32> = (0..n as u32).map(|i| i % ccp).collect();
    let ibkl: Vec<u16> = (0..n as u16).collect();
    let ends: Vec<u32> = (1..=n as u32 + 1).collect();
    let names: Vec<String> = (0..max).map(|i| format!("b{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let started = std::time::Instant::now();
    let doc = open(text, &names, bkf(&starts, ccp, &ibkl), plc(&ends, &[]));
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "{:?}", started.elapsed());
    let got = ranges(&doc);
    assert_eq!(got.len(), max);
    for i in [0, 9, max - 1] {
        let want = char::from_u32(0x4E00 + i as u32).map(String::from);
        assert_eq!(got.get(&format!("b{i}")), want.as_ref(), "bookmark {i}");
    }
}
