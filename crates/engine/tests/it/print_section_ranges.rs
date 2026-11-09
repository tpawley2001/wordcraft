//! Word's section-qualified print ranges: `p2s1` is page number 2 of section 1 (the number shown
//! after section restarts), `s3` is a whole section, `p2s1-p1s2` runs across sections.

use wordcraft_engine::print::{parse_page_range, parse_page_range_in, plan, plan_in};

/// Front matter i–iii (section 1), chapter 1 pages 1–3 (section 2), chapter 2 pages 1–2
/// (section 3): physical pages 0..8.
fn book() -> Vec<(usize, u32)> {
    vec![(1, 1), (1, 2), (1, 3), (2, 1), (2, 2), (2, 3), (3, 1), (3, 2)]
}

#[test]
fn page_of_a_section_uses_its_displayed_number() {
    let ids = book();
    assert_eq!(parse_page_range_in("p2s2", &ids), Ok(vec![4]));
    assert_eq!(parse_page_range_in("p1s3", &ids), Ok(vec![6]));
    assert_eq!(parse_page_range_in("P3S1", &ids), Ok(vec![2]), "letters are case-insensitive");
    assert_eq!(parse_page_range_in(" p 2 s 2 ", &ids), Ok(vec![4]), "whitespace is ignored");
}

#[test]
fn whole_sections_and_runs_across_sections() {
    let ids = book();
    assert_eq!(parse_page_range_in("s2", &ids), Ok(vec![3, 4, 5]));
    assert_eq!(parse_page_range_in("s3, s1", &ids), Ok(vec![0, 1, 2, 6, 7]), "document order");
    assert_eq!(parse_page_range_in("p2s1-p2s2", &ids), Ok(vec![1, 2, 3, 4]));
    assert_eq!(parse_page_range_in("p3s2-p2s3", &ids), Ok(vec![5, 6, 7]));
}

#[test]
fn mixes_with_physical_positions() {
    let ids = book();
    // Plain numbers stay physical positions: "1" is the first sheet (page i), not page 1 of s2.
    assert_eq!(parse_page_range_in("1, p1s2", &ids), Ok(vec![0, 3]));
    assert_eq!(parse_page_range_in("7-, s1", &ids), Ok(vec![0, 1, 2, 6, 7]));
    assert_eq!(parse_page_range_in("p1s2, 4", &ids), Ok(vec![3]), "no repeats");
}

#[test]
fn bad_section_items_are_errors() {
    let ids = book();
    for bad in ["s4", "s0", "p4s2", "p0s1", "p2s3-p1s1", "p2s1-5", "s", "ps1", ""] {
        assert!(parse_page_range_in(bad, &ids).is_err(), "`{bad}` should be refused");
    }
}

#[test]
fn plain_parser_and_plan_keep_their_behaviour() {
    assert_eq!(parse_page_range("1-3, 5", 6), Ok(vec![0, 1, 2, 4]));
    assert_eq!(parse_page_range("s1", 3), Ok(vec![0, 1, 2]), "an unsectioned page list is one section");
    let v = serde_json::json!({"pages": "2-3"});
    assert_eq!(plan(&v, 4, (0, 0)).unwrap().pages, vec![1, 2]);
}

#[test]
fn plan_in_prints_section_pages_with_copies() {
    let ids = book();
    let v = serde_json::json!({"pages": "p2s2-p1s3", "copies": 2});
    let got = plan_in(&v, &ids, (0, 0)).unwrap();
    assert_eq!(got.pages, vec![4, 5, 6, 4, 5, 6]);
    assert_eq!(got.chosen, 3);
    let odd = serde_json::json!({"pages": "s2", "parity": "odd"});
    // Odd/even keep counting physical sheets (indexes 3,4,5 -> 1-based 4,5,6 -> odd = 5).
    assert_eq!(plan_in(&odd, &ids, (0, 0)).unwrap().pages, vec![4]);
}
