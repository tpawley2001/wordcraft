//! Underline styles drawn with Word's geometry. Measured 2026-11-14 in Microsoft 365 Word (16.0,
//! compatibility mode 15) PDF export of synthetic documents: one underlined run "Hxgabcdefgh" per
//! size (8, 11, 14, 24, 36 pt) in Times New Roman and Arial. Word puts every edge on a 0.12 pt
//! grid, so edges are compared to within 0.15 pt.
//! - Double, thick and wavy underlines fill a band worked out from the font's `post` underline
//!   position and thickness ([`crate::display::underline_band`]); the tables below are Word's.
//! - Dotted and dashed underlines use fixed dash lengths in points at every size and in every font.
//! - Waves turn every 1.20 pt and are drawn with a 0.24 pt pen at every size.
//!
//! The test fonts carry other `post` values than Times New Roman and Arial, so Word's tables are
//! checked against the band rule with Times New Roman's and Arial's values, and the laid-out
//! underlines against the same rule with the test fonts' values.
use crate::display::{
    Draw, Stroke, UNDERLINE_DOUBLE_WAVE_GAP, UNDERLINE_WAVE_HALF_PERIOD, UNDERLINE_WAVE_PEN, page_display, underline_band, underline_waves,
};
use crate::*;
use wordcraft_doc::para_block;
use wordcraft_doc::props::Underline;

const TOL: f32 = 0.15;

/// A font's `post` table: underline position (top of the line), thickness, units per em.
struct Post {
    name: &'static str,
    pos: f32,
    thick: f32,
    upem: f32,
}

const TNR: Post = Post { name: "Times New Roman", pos: -223.0, thick: 100.0, upem: 2048.0 };
const ARIAL: Post = Post { name: "Arial", pos: -217.0, thick: 150.0, upem: 2048.0 };

/// One size's measurements: the single band, the double's two lines, the thick band, the wave's
/// centre-line band and the double wave's two centre-line bands (points below the baseline).
struct Row {
    size: f32,
    single: (f32, f32),
    double: [(f32, f32); 2],
    thick: (f32, f32),
    wave: (f32, f32),
    wavy_double: [(f32, f32); 2],
}

const TNR_ROWS: [Row; 5] = [
    Row {
        size: 8.0,
        single: (0.84, 1.20),
        double: [(0.48, 0.72), (1.68, 1.92)],
        thick: (0.48, 1.68),
        wave: (0.48, 1.08),
        wavy_double: [(0.48, 0.60), (0.96, 1.08)],
    },
    Row {
        size: 11.0,
        single: (1.20, 1.68),
        double: [(0.60, 0.84), (1.80, 2.04)],
        thick: (0.60, 1.80),
        wave: (0.60, 1.20),
        wavy_double: [(0.60, 0.72), (1.08, 1.20)],
    },
    Row {
        size: 14.0,
        single: (1.56, 2.28),
        double: [(0.84, 1.20), (2.16, 2.52)],
        thick: (0.84, 2.16),
        wave: (0.84, 1.56),
        wavy_double: [(0.84, 1.08), (1.32, 1.56)],
    },
    Row {
        size: 24.0,
        single: (2.64, 3.84),
        double: [(1.32, 1.92), (3.24, 3.84)],
        thick: (1.32, 3.24),
        wave: (1.32, 2.40),
        wavy_double: [(1.32, 1.68), (2.04, 2.40)],
    },
    Row {
        size: 36.0,
        single: (3.96, 5.76),
        double: [(2.04, 3.00), (4.80, 5.76)],
        thick: (2.04, 4.80),
        wave: (2.04, 3.72),
        wavy_double: [(2.04, 2.76), (3.00, 3.72)],
    },
];

const ARIAL_ROWS: [Row; 5] = [
    Row {
        size: 8.0,
        single: (0.84, 1.44),
        double: [(0.12, 0.48), (1.44, 1.80)],
        thick: (0.12, 1.44),
        wave: (0.12, 0.84),
        wavy_double: [(0.12, 0.36), (0.60, 0.84)],
    },
    Row {
        size: 11.0,
        single: (1.20, 2.04),
        double: [(0.48, 0.96), (1.92, 2.40)],
        thick: (0.48, 1.92),
        wave: (0.48, 1.32),
        wavy_double: [(0.48, 0.72), (1.08, 1.32)],
    },
    Row {
        size: 14.0,
        single: (1.44, 2.52),
        double: [(0.72, 1.32), (2.28, 2.88)],
        thick: (0.72, 2.28),
        wave: (0.72, 1.68),
        wavy_double: [(0.72, 1.08), (1.32, 1.68)],
    },
    Row {
        size: 24.0,
        single: (2.52, 4.32),
        double: [(1.32, 2.28), (3.36, 4.32)],
        thick: (1.32, 3.36),
        wave: (1.32, 2.64),
        wavy_double: [(1.32, 1.80), (2.16, 2.64)],
    },
    Row {
        size: 36.0,
        single: (3.84, 6.48),
        double: [(1.92, 3.24), (5.16, 6.48)],
        thick: (1.92, 5.16),
        wave: (1.92, 4.08),
        wavy_double: [(1.92, 2.88), (3.12, 4.08)],
    },
];

fn near(what: &str, got: (f32, f32), want: (f32, f32)) {
    assert!((got.0 - want.0).abs() <= TOL && (got.1 - want.1).abs() <= TOL, "{what}: {:.3}..{:.3}, Word {:.2}..{:.2}", got.0, got.1, want.0, want.1);
}

/// The double, thick, wave and double-wave bands from the rule, as the display list draws them
/// (from a band top, line thickness, gap and wave height).
fn bands(top: f32, line: f32, gap: f32, wave: f32) -> ([(f32, f32); 2], (f32, f32), (f32, f32), [(f32, f32); 2]) {
    let h = (wave - UNDERLINE_DOUBLE_WAVE_GAP) / 2.0;
    (
        [(top, top + line), (top + line + gap, top + 2.0 * line + gap)],
        (top, top + line + gap),
        (top, top + wave),
        [(top, top + h), (top + wave - h, top + wave)],
    )
}

#[test]
fn double_thick_and_wavy_bands_match_word_for_times_new_roman_and_arial() {
    for (font, rows) in [(TNR, &TNR_ROWS), (ARIAL, &ARIAL_ROWS)] {
        for r in rows.iter() {
            let (top, thick) = (-font.pos / font.upem * r.size, font.thick / font.upem * r.size);
            let name = format!("{} {} pt", font.name, r.size);
            near(&format!("{name} single"), (top, top + thick), r.single);
            let b = underline_band(top, thick, r.size);
            let (double, thick_band, wave, wavy_double) = bands(b.top, b.line, b.gap, b.wave);
            near(&format!("{name} double upper"), double[0], r.double[0]);
            near(&format!("{name} double lower"), double[1], r.double[1]);
            near(&format!("{name} thick"), thick_band, r.thick);
            near(&format!("{name} wave"), wave, r.wave);
            near(&format!("{name} wavyDouble upper"), wavy_double[0], r.wavy_double[0]);
            near(&format!("{name} wavyDouble lower"), wavy_double[1], r.wavy_double[1]);
        }
    }
}

/// One paragraph "Hxgabcdefgh" in `font` at `size` pt with `underline`: the glyphs' baseline and
/// the long horizontal lines drawn (x0, x1, y, width, stroke).
fn lay(font: &str, size: f32, underline: Underline) -> (f32, Vec<(f32, f32, f32, f32, Stroke)>) {
    crate::test_fonts();
    let mut d = Document::new();
    let run = CharProps { font: Some(font.into()), size: Some(size), underline: Some(underline), ..CharProps::default() };
    d.body = vec![para_block(Paragraph::with_text("Hxgabcdefgh", run))];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = draws
        .iter()
        .find_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains("Hxg") => glyphs.first().map(|g| g.2),
            _ => None,
        })
        .expect("the text is drawn");
    let lines = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Line { x0, y0, x1, y1, width, stroke, .. } if (y0 - y1).abs() < 0.01 && (x1 - x0).abs() > 1.0 => {
                Some((*x0, *x1, *y0, *width, *stroke))
            }
            _ => None,
        })
        .collect();
    (base, lines)
}

/// The test fonts' single underline: (top below the baseline, thickness) as a fraction of the size.
const FONTS: [(&str, f32, f32); 2] = [("Source Serif 4", 0.05, 0.05), ("Inter", 348.0 / 2048.0, 140.0 / 2048.0)];

#[test]
fn laid_out_underlines_fill_the_band_from_the_font() {
    for (font, top, thick) in FONTS {
        for size in [8.0f32, 11.0, 24.0, 36.0] {
            let b = underline_band(top * size, thick * size, size);
            let (double, thick_band, wave, wavy_double) = bands(b.top, b.line, b.gap, b.wave);
            let name = format!("{font} {size} pt");
            let one = |u: Underline| {
                let (base, lines) = lay(font, size, u);
                assert_eq!(lines.len(), 1, "{name} {u:?}: one line, got {lines:?}");
                let (_, _, y, w, stroke) = lines[0];
                (y - base, w, stroke)
            };
            // A double underline: one line over the band, its two lines inside it.
            let (c, w, stroke) = one(Underline::Double);
            near(&format!("{name} double"), (c - w / 2.0, c + w / 2.0), (double[0].0, double[1].1));
            let [(o0, t0), (o1, t1)] = stroke.underline_double(w).expect("a double underline stroke");
            near(&format!("{name} double upper"), (c + o0 - t0 / 2.0, c + o0 + t0 / 2.0), double[0]);
            near(&format!("{name} double lower"), (c + o1 - t1 / 2.0, c + o1 + t1 / 2.0), double[1]);
            let (c, w, stroke) = one(Underline::Thick);
            assert_eq!(stroke, Stroke::Solid);
            near(&format!("{name} thick"), (c - w / 2.0, c + w / 2.0), thick_band);
            let (c, w, stroke) = one(Underline::Wave);
            assert_eq!(stroke, Stroke::UnderlineWave);
            near(&format!("{name} wave"), (c - w / 2.0, c + w / 2.0), wave);
            let (c, w, stroke) = one(Underline::DoubleWave);
            assert_eq!(stroke, Stroke::UnderlineDoubleWave);
            let waves = underline_waves(stroke, (0.0, c, 50.0, c), w);
            assert_eq!(waves.len(), 2, "{name}: two waves");
            for (k, wv) in waves.iter().enumerate() {
                let ys = wv.iter().map(|p| p.1);
                let (lo, hi) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
                near(&format!("{name} wavyDouble {k}"), (lo, hi), wavy_double[k]);
            }
            // Dotted and dashed underlines keep the single underline's line.
            let (c, w, _) = one(Underline::Single);
            for u in [Underline::Dotted, Underline::Dash, Underline::DotDash, Underline::DotDotDash] {
                let (cu, wu, _) = one(u);
                assert!((cu - c).abs() < 0.01 && (wu - w).abs() < 0.01, "{name} {u:?}: on the single underline's line");
            }
        }
    }
}

#[test]
fn dashed_underlines_use_fixed_lengths_in_points() {
    let want: [(Underline, Stroke, &[f32]); 4] = [
        (Underline::Dotted, Stroke::UnderlineDotted, &[1.44, 1.44]),
        (Underline::Dash, Stroke::UnderlineDash, &[3.96, 1.92]),
        (Underline::DotDash, Stroke::UnderlineDotDash, &[3.0, 0.96, 0.96, 0.96]),
        (Underline::DotDotDash, Stroke::UnderlineDotDotDash, &[2.16, 0.72, 0.72, 0.72, 0.72, 0.72]),
    ];
    for (u, stroke, pattern) in want {
        assert_eq!(stroke.underline_pattern(), Some(pattern), "{u:?}");
        assert_eq!(stroke.dashes(), None, "{u:?}: not scaled by the pen");
        // The same pattern at every size, in both fonts.
        for (font, ..) in FONTS {
            for size in [8.0f32, 36.0] {
                let (_, lines) = lay(font, size, u);
                assert_eq!(lines.iter().map(|l| l.4).collect::<Vec<_>>(), [stroke], "{font} {size} pt {u:?}");
            }
        }
    }
}

#[test]
fn waves_turn_every_1_20_pt_with_a_0_24_pt_pen() {
    assert_eq!(UNDERLINE_WAVE_HALF_PERIOD, 1.2);
    assert_eq!(UNDERLINE_WAVE_PEN, 0.24);
    for (font, ..) in FONTS {
        for size in [8.0f32, 36.0] {
            let (_, lines) = lay(font, size, Underline::Wave);
            let (x0, x1, y, w, stroke) = lines[0];
            let waves = underline_waves(stroke, (x0, y, x1, y), w);
            assert_eq!(waves.len(), 1);
            let wave = &waves[0];
            // The centre line starts at the band's top and turns at its bottom and top in turn.
            assert!((wave[0].1 - (y - w / 2.0)).abs() < 1e-3, "{font} {size}: starts at the top");
            for (k, pair) in wave.windows(2).enumerate().take(wave.len().saturating_sub(2)) {
                assert!((pair[1].0 - pair[0].0 - 1.2).abs() < 1e-3, "{font} {size}: half-wave {k} is {} long", pair[1].0 - pair[0].0);
                assert!((pair[1].1 - pair[0].1).abs() > w * 0.99, "{font} {size}: half-wave {k} spans the band");
            }
            let last = wave.last().map_or(0.0, |p| p.0);
            assert!((last - x1).abs() < 1e-3, "{font} {size}: cut off at the right end ({last} vs {x1})");
        }
    }
}

/// Section-break rules, dotted borders and the proofing squiggles keep their own strokes.
#[test]
fn other_dotted_and_wavy_lines_are_unchanged() {
    assert_eq!(Stroke::Dotted.dashes(), Some(&[1.0f32, 2.0][..]));
    assert_eq!(Stroke::Dashed.dashes(), Some(&[4.0f32, 2.0][..]));
    assert_eq!(Stroke::BorderDotted.dashes(), Some(&[1.0f32, 1.0][..]));
    for s in [Stroke::Dotted, Stroke::Dashed, Stroke::Wave, Stroke::Double, Stroke::BorderDotted] {
        assert_eq!(s.underline_pattern(), None, "{s:?}");
        assert_eq!(s.underline_double(1.0), None, "{s:?}");
        assert!(underline_waves(s, (0.0, 0.0, 20.0, 0.0), 1.0).is_empty(), "{s:?}");
    }
    crate::test_fonts();
    let hlines = |d: &Document, opts: &LayoutOptions, marks: bool| -> Vec<(f32, Stroke, Rgb)> {
        let l = layout(d, &mut LayoutCache::new(), opts);
        page_display(d, &l.pages[0], &crate::display::DisplayOptions { marks, ..Default::default() })
            .into_iter()
            .filter_map(|i| match i {
                Draw::Line { y0, y1, width, stroke, color, .. } if (y0 - y1).abs() < 0.01 => Some((width, stroke, color)),
                _ => None,
            })
            .collect()
    };
    // The section-break mark: a 0.5 pt dotted double rule.
    use wordcraft_doc::section::{SectionProps, SectionStart};
    let mut d = Document::from_text("One\nTwo");
    d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().section = Some(Box::new(SectionProps::default()));
    d.last_section.start = SectionStart::Continuous;
    let rule = hlines(&d, &LayoutOptions::default(), true);
    assert!(!rule.is_empty() && rule.iter().all(|l| l.0 == 0.5 && l.1 == Stroke::Dotted), "{rule:?}");
    // A dotted paragraph border.
    let mut d = Document::from_text("Bordered");
    let b = wordcraft_doc::props::Border { style: wordcraft_doc::props::BorderStyle::Dotted, width: 1.0, color: None, space: 1.0 };
    d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().props.borders = Some(wordcraft_doc::props::Borders { bottom: Some(b), ..Default::default() });
    let border = hlines(&d, &LayoutOptions::default(), false);
    assert!(!border.is_empty() && border.iter().all(|l| l.1 == Stroke::BorderDotted), "{border:?}");
    // The spelling squiggle: a red 0.8 pt wave.
    let mut d = Document::from_text("A beautifull day");
    d.format_range(&wordcraft_doc::Pos::body(0, 0), &wordcraft_doc::Pos::body(0, 16), &|c| c.lang = Some("en-US".into())).unwrap();
    let squiggles: Vec<_> = hlines(&d, &LayoutOptions { proofing: true, ..LayoutOptions::default() }, false)
        .into_iter()
        .filter(|l| l.2 == Rgb(0xE0, 0x24, 0x24))
        .collect();
    assert_eq!(squiggles, [(0.8, Stroke::Wave, Rgb(0xE0, 0x24, 0x24))]);
}
