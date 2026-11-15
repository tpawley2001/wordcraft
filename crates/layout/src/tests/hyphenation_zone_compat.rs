//! From compatibility mode 15 Word ignores the hyphenation zone (`w:hyphenationZone`): measured in
//! Microsoft 365 Word's PDFs, a 1-inch zone and a 1-twip zone hyphenate a paragraph exactly where
//! the default zone does. Below mode 15 the zone still changes the line breaks.

use super::*;

const TEXT: &str = "The international organization established comprehensive administrative responsibilities \
concerning environmental sustainability, extraordinary communications infrastructure, representational \
characteristics and unquestionably incomprehensible documentation, notwithstanding counterintuitive \
internationalization, decentralization and microelectronics manufacturing.";

/// Where each line of the paragraph starts (byte offsets into `TEXT`), with automatic hyphenation and the given zone.
fn line_starts(compat: u32, zone: Option<f32>) -> Vec<usize> {
    crate::test_fonts();
    let mut d = Document::from_text(TEXT);
    d.settings.auto_hyphenation = true;
    Arc::make_mut(&mut d.styles).default_chr.lang = Some("en-US".into());
    d.settings.compat_mode = compat;
    if let Some(z) = zone {
        d.settings.hyphenation_zone = z;
    }
    // A 4-inch column, as measured.
    d.last_section.margin_right = d.last_section.page_w - d.last_section.margin_left - 288.0;
    let l = lay(&d);
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, l0, l1, .. } => {
                Some(para.lines[*l0..*l1].iter().map(|ln| para.clusters.get(ln.c0).map_or(usize::MAX, |c| c.start)).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn the_fixture_hyphenates_with_the_default_zone() {
    // Guard: some line starts inside a word (after a hyphen), or the zone tests prove nothing.
    let starts = line_starts(15, None);
    assert!(starts.len() > 3, "too few lines: {starts:?}");
    assert!(starts.iter().skip(1).any(|&c| c < TEXT.len() && !TEXT[..c].ends_with(' ')), "no hyphenated line: {starts:?}");
}

#[test]
fn a_wide_zone_hyphenates_as_the_default_in_mode_15() {
    assert_eq!(line_starts(15, Some(72.0)), line_starts(15, None));
}

#[test]
fn a_tiny_zone_hyphenates_as_the_default_in_mode_15() {
    assert_eq!(line_starts(15, Some(0.05)), line_starts(15, None));
}

#[test]
fn the_zone_still_counts_below_mode_15() {
    assert_ne!(line_starts(14, Some(72.0)), line_starts(14, None));
}
