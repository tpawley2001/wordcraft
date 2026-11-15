//! `lcs` returns a longest common subsequence of two line lists at any size: long documents
//! whose differing middle part is too big for the full table are matched too (they used to keep
//! only their equal first and last lines).

use super::lcs;

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("line {i}")).collect()
}

/// The pairs are strictly increasing on both sides and each pairs two equal lines.
fn assert_valid(a: &[String], b: &[String], pairs: &[(usize, usize)]) {
    for w in pairs.windows(2) {
        assert!(w[0].0 < w[1].0 && w[0].1 < w[1].1, "pairs not increasing: {:?} then {:?}", w[0], w[1]);
    }
    for &(i, j) in pairs {
        assert_eq!(a[i], b[j], "pair ({i}, {j}) matches different lines");
    }
}

/// The LCS length by the textbook table, for small inputs.
fn reference_len(a: &[String], b: &[String]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    t[0][0]
}

#[test]
fn a_long_document_with_a_few_early_differences_matches_all_its_other_lines() {
    let a = lines(20_000);
    let mut b = a.clone();
    b[5] = "changed near the top".to_string();
    b[9_000] = "changed in the middle".to_string();
    b.remove(15_000);
    let pairs = lcs(&a, &b);
    assert_valid(&a, &b, &pairs);
    assert_eq!(pairs.len(), 19_997);
}

#[test]
fn a_long_document_with_every_third_line_changed_matches_the_unchanged_lines() {
    let a = lines(15_000);
    let b: Vec<String> = a.iter().enumerate().map(|(i, l)| if i % 3 == 1 { format!("other {i}") } else { l.clone() }).collect();
    let pairs = lcs(&a, &b);
    assert_valid(&a, &b, &pairs);
    assert_eq!(pairs.len(), 10_000);
}

#[test]
fn small_inputs_give_a_longest_common_subsequence() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move |k: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % k
    };
    let words = ["alpha", "beta", "gamma", "delta"];
    for case in 0..300 {
        let (la, lb) = (next(61), next(61));
        let a: Vec<String> = (0..la).map(|_| words[next(4) as usize].to_string()).collect();
        let b: Vec<String> = (0..lb).map(|_| words[next(4) as usize].to_string()).collect();
        let pairs = lcs(&a, &b);
        assert_valid(&a, &b, &pairs);
        assert_eq!(pairs.len(), reference_len(&a, &b), "case {case}: {a:?} / {b:?}");
    }
}
