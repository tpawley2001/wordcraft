//! Text frames (`w:framePr`) in headers and footers, as Microsoft 365 Word lays them out
//! (measured from its PDF export of small DOCX files: Letter page, margins top 72 / left 90 /
//! right 90 pt, header and footer distance 36 pt; header = a `Title` paragraph, a framed
//! paragraph 216 x 72 pt at page x 144.05 or 90, 1.5 pt below its place in the flow, 9.35 pt
//! of horizontal space around it, then the paragraphs under test):
//!
//! * compatibility mode below 15 (11, 12, 14 and files without the setting): the header's or
//!   footer's own paragraphs are laid out as if the frame were not there (full width, never
//!   moved below it), whatever the frame's wrap; mode 15 wraps them around the frame;
//! * every mode: the frame adds no height to the header, so the body starts at the top margin
//!   or right below the header's in-flow paragraphs;
//! * every mode: body text wraps around the part of a header's frame that reaches below the
//!   body's top (`none`: drawn over it; `notBeside`: continues below it).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps, TextFrame};
use wordcraft_doc::{PartKind, Pos, para_block};

const LEFT: f32 = 90.0;
const TOP: f32 = 72.0;
const HEADER: f32 = 36.0;
/// One unbreakable word, wider than the 44.65 pt between the left margin and the frame.
const U26: &str = "__________________________";
const LONG: &str = "Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega.";

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn para(text: &str, frame: Option<TextFrame>) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, Default::default());
    p.props = ParaProps {
        space_before: Some(0.0),
        space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.0)),
        widow_control: Some(false),
        frame,
        ..Default::default()
    };
    para_block(p)
}

/// The probes' frame: 216 x 72 pt (exact), at page x `x`, 1.5 pt below its paragraph's place.
fn frame(x: f32, wrap: &str) -> TextFrame {
    TextFrame {
        w: Some(216.0),
        h: Some(72.0),
        h_exact: true,
        x,
        y: 1.5,
        h_anchor: "page".into(),
        v_anchor: "text".into(),
        wrap: wrap.into(),
        h_space: 9.35,
        ..Default::default()
    }
}

struct Fixture {
    layout: DocLayout,
    part: u32,
    /// Index of the first paragraph after the frame (or after `Title` when there is no frame).
    after: usize,
}

/// A document in compatibility mode `mode` whose header (or footer) is `Title`, the framed
/// paragraph when `fr` is given, then `after`; the body is `body`.
fn fixture(mode: u32, footer: bool, fr: Option<TextFrame>, after: &[&str], body: &[&str]) -> Fixture {
    crate::test_fonts();
    let mut d = Document::new();
    // The bundled face the probes were sized for: a missing Aptos (the default) takes Word's
    // narrower underscores, which fit beside the frame.
    Arc::make_mut(&mut d.styles).default_chr.font = Some("Source Sans 3".into());
    d.settings.compat_mode = mode;
    d.last_section.margin_top = TOP;
    d.last_section.margin_left = LEFT;
    d.last_section.margin_right = 90.0;
    d.last_section.header = HEADER;
    d.last_section.footer = 36.0;
    d.body = body.iter().map(|t| para(t, None)).collect();
    let mut blocks = vec![para("Title", None)];
    let framed = fr.is_some();
    if let Some(fr) = fr {
        blocks.push(para("Frame one two three four five six seven eight nine ten eleven twelve thirteen fourteen.", Some(fr)));
    }
    blocks.extend(after.iter().map(|t| para(t, None)));
    let kind = if footer { PartKind::Footer } else { PartKind::Header };
    let part = d.add_part(kind, blocks);
    if footer {
        d.last_section.footers.default = Some(part);
    } else {
        d.last_section.headers.default = Some(part);
    }
    Fixture { layout: lay(&d), part, after: 1 + usize::from(framed) }
}

impl Fixture {
    fn caret(&self, story: StoryRef, block: usize, off: usize) -> crate::hit::Caret {
        self.layout
            .caret(&Pos { story, path: wordcraft_doc::Path::top(block), off })
            .unwrap_or_else(|| panic!("no caret at {story:?} block {block} offset {off}"))
    }
    fn hf(&self, block: usize) -> crate::hit::Caret {
        self.caret(StoryRef::Part(self.part), block, 0)
    }
    /// How far a caret's top is below its line's top: the header's `Title` line starts at the
    /// header distance (every paragraph here has the same font and size).
    fn lead(&self) -> f32 {
        self.hf(0).top - HEADER
    }
    /// The frame's bottom edge on the page (the framed paragraph is block 1).
    fn frame_bottom(&self) -> f32 {
        self.hf(1).top - self.lead() + 72.0
    }
    /// (line top, x of the line's start) for each line of a paragraph of `len` characters.
    fn lines(&self, story: StoryRef, block: usize, len: usize) -> Vec<(f32, f32)> {
        let mut out: Vec<(f32, f32)> = Vec::new();
        for off in 0..len {
            let c = self.caret(story, block, off);
            let top = c.top - self.lead();
            match out.iter_mut().find(|(t, _)| (*t - top).abs() < 0.5) {
                Some((_, x)) => *x = x.min(c.x),
                None => out.push((top, c.x)),
            }
        }
        out
    }
    fn body_start(&self) -> (f32, f32) {
        let c = self.caret(StoryRef::Body, 0, 0);
        (c.x, c.top - self.lead())
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

/// Rows a, b, d, g, t, r, v, x and the mode 11/12/14 repeats: in a document below mode 15 the
/// paragraph after the frame stands exactly where it stands without the frame (Word: x 90,
/// right under `Title`), for every wrap value and frame height.
#[test]
fn below_mode_15_header_text_ignores_the_frame() {
    let frames: Vec<(&str, TextFrame)> = vec![
        ("auto", frame(144.05, "auto")),
        ("around", frame(144.05, "around")),
        ("notBeside", frame(144.05, "notBeside")),
        ("none", frame(144.05, "none")),
        ("at the left margin", frame(LEFT, "auto")),
        ("at least 72 pt", TextFrame { h_exact: false, ..frame(144.05, "auto") }),
        ("no height", TextFrame { h: None, h_exact: false, ..frame(144.05, "auto") }),
        ("20 pt tall", TextFrame { h: Some(20.0), ..frame(144.05, "auto") }),
    ];
    for mode in [11, 12, 14] {
        for text in [U26, "ab"] {
            let plain = fixture(mode, false, None, &[text], &["Body starts here."]);
            let want = plain.hf(plain.after);
            assert!(close(want.x, LEFT), "control: {text:?} starts at the left margin, got {}", want.x);
            for (name, fr) in &frames {
                let f = fixture(mode, false, Some(fr.clone()), &[text], &["Body starts here."]);
                let got = f.hf(f.after);
                assert!(
                    close(got.x, want.x) && close(got.top, want.top),
                    "mode {mode}, frame {name}: {text:?} after the frame is at ({}, {}), without the frame at ({}, {})",
                    got.x,
                    got.top,
                    want.x,
                    want.top
                );
            }
        }
    }
}

/// Rows c and m: a long paragraph after the frame keeps its full-width lines (drawn over the
/// frame) instead of being wrapped around it.
#[test]
fn below_mode_15_long_header_text_keeps_its_lines() {
    for mode in [11, 12, 14] {
        let plain = fixture(mode, false, None, &["ab", LONG], &["Body starts here."]);
        let f = fixture(mode, false, Some(frame(144.05, "auto")), &["ab", LONG], &["Body starts here."]);
        let want = plain.lines(StoryRef::Part(plain.part), plain.after + 1, LONG.len());
        let got = f.lines(StoryRef::Part(f.part), f.after + 1, LONG.len());
        assert!(want.len() >= 2, "control: the long paragraph takes two or more lines: {want:?}");
        assert_eq!(got.len(), want.len(), "mode {mode}: lines with the frame {got:?}, without {want:?}");
        for (g, w) in got.iter().zip(&want) {
            assert!(close(g.0, w.0) && close(g.1, w.1), "mode {mode}: line at {g:?}, without the frame at {w:?}");
        }
    }
}

/// Row s2: the same in a footer. Word's footer is two lines tall (`Title` and the underscores),
/// the frame is drawn over them; the footer used to be as tall as the frame and more.
#[test]
fn below_mode_15_footer_text_ignores_the_frame() {
    for mode in [11, 12, 14] {
        let plain = fixture(mode, true, None, &[U26], &["Body starts here."]);
        let f = fixture(mode, true, Some(frame(144.05, "auto")), &[U26], &["Body starts here."]);
        for (block, name) in [(0, "Title"), (plain.after, "the underscores")] {
            let (want, got) = (plain.hf(block), f.hf(block + usize::from(block > 0)));
            assert!(
                close(got.x, want.x) && close(got.top, want.top),
                "mode {mode}: {name} at ({}, {}), without the frame at ({}, {})",
                got.x,
                got.top,
                want.x,
                want.top
            );
        }
    }
}

/// Rows h, q and s3 (controls, as before): in mode 15 the header's own text keeps clear of the
/// frame: a word that fits nowhere beside it goes below it, and beside a frame at the left
/// margin the text starts at the frame's right edge plus its space (Word: x 315.4).
#[test]
fn mode_15_header_text_still_wraps_around_the_frame() {
    let f = fixture(15, false, Some(frame(144.05, "auto")), &[U26], &["Body starts here."]);
    let rule = f.hf(f.after);
    assert!(close(rule.x, LEFT), "the underscores start at the left margin: {}", rule.x);
    assert!(rule.top - f.lead() > f.frame_bottom() - 0.6, "below the frame: line top {}, frame bottom {}", rule.top - f.lead(), f.frame_bottom());

    let plain = fixture(15, false, None, &[U26], &["Body starts here."]);
    let f = fixture(15, false, Some(frame(LEFT, "auto")), &[U26], &["Body starts here."]);
    let (rule, want) = (f.hf(f.after), plain.hf(plain.after));
    assert!(close(rule.x, LEFT + 216.0 + 9.35), "beside a frame at the left margin: x {}", rule.x);
    assert!(close(rule.top, want.top), "on the line right under Title: {} vs {}", rule.top, want.top);

    let f = fixture(15, true, Some(frame(144.05, "auto")), &[U26], &["Body starts here."]);
    assert!(f.hf(f.after).top - f.hf(0).top > 72.0, "footer: the underscores are below the frame");
}

/// Rows v, r, o/w and p: the frame adds no height to the header. The body starts at the top
/// margin when the header's in-flow paragraphs end above it, and right below them otherwise;
/// a frame with wrap `none` or one that ends above the body does not move the body sideways.
#[test]
fn a_frame_adds_no_height_to_the_header() {
    // Row v: wrap none, below mode 15. Word: body at x 90, y 72, drawn over the frame.
    let f = fixture(12, false, Some(frame(144.05, "none")), &[U26], &["Body starts here."]);
    let (x, top) = f.body_start();
    assert!(close(x, LEFT) && close(top, TOP), "wrap none: body at ({x}, {top}), Word at (90, 72)");
    // Row r: a 20 pt frame ends above the top margin. Word: body at x 90, y 72.
    let f = fixture(12, false, Some(TextFrame { h: Some(20.0), ..frame(144.05, "auto") }), &[U26], &["Body starts here."]);
    let (x, top) = f.body_start();
    assert!(close(x, LEFT) && close(top, TOP), "short frame: body at ({x}, {top}), Word at (90, 72)");
    // Rows o and w: the frame is the header's last paragraph, in both modes. Word: y 72.
    for mode in [12, 15] {
        let f = fixture(mode, false, Some(frame(LEFT, "auto")), &[], &["Body starts here."]);
        let (_, top) = f.body_start();
        assert!(close(top, TOP), "mode {mode}, frame last: body starts at {top}, Word at 72");
    }
    // Row p: mode 15, a long paragraph wrapped around the frame ends below the top margin but
    // above the frame's bottom. Word starts the body right below that paragraph's last line.
    let f = fixture(15, false, Some(frame(144.05, "auto")), &[LONG], &["Body starts here."]);
    let lines = f.lines(StoryRef::Part(f.part), f.after, LONG.len());
    let pitch = f.hf(f.after).top - f.hf(0).top;
    let flow_bottom = lines.iter().map(|l| l.0).fold(0.0, f32::max) + pitch;
    let (_, top) = f.body_start();
    assert!(flow_bottom > TOP && flow_bottom < f.frame_bottom(), "fixture: the header text ends at {flow_bottom}");
    assert!(close(top, flow_bottom), "mode 15: body starts at {top}, the header's text ends at {flow_bottom}");
}

/// Row u (control, as before): header text that ends below the frame pushes the body down to
/// its own bottom, and the body is not beside the frame any more.
#[test]
fn header_text_longer_than_the_frame_starts_the_body_below_it() {
    let after = ["ab"; 6];
    let plain = fixture(12, false, None, &after, &["Body starts here."]);
    let f = fixture(12, false, Some(frame(144.05, "auto")), &after, &["Body starts here."]);
    let (want, got) = (plain.body_start(), f.body_start());
    assert!(want.1 > f.frame_bottom(), "fixture: the header's text ends below the frame ({} vs {})", want.1, f.frame_bottom());
    assert!(close(got.0, LEFT) && close(got.1, want.1), "body at {got:?}, without the frame at {want:?}");
}

/// Rows y and y15: body text wraps around a header's frame that reaches below the top margin.
/// With the frame at the left margin every body line whose top is above the frame's bottom
/// starts at the frame's right edge plus its space (Word: x 315.4, lines at y 72, 85.8, 99.6
/// and 113.4; frame bottom 123.3), the lines below it at the left margin.
#[test]
fn body_text_wraps_around_a_header_frame() {
    for mode in [12, 15] {
        let f = fixture(mode, false, Some(frame(LEFT, "auto")), &[], &[LONG, LONG]);
        let bottom = f.frame_bottom();
        let mut lines = f.lines(StoryRef::Body, 0, LONG.len());
        lines.extend(f.lines(StoryRef::Body, 1, LONG.len()));
        assert!(close(lines[0].0, TOP), "mode {mode}: the body starts at the top margin, got {}", lines[0].0);
        let beside = lines.iter().filter(|l| l.0 < bottom - 0.6).count();
        assert!(beside >= 2 && beside < lines.len(), "fixture: {beside} of {} lines are beside the frame", lines.len());
        for (top, x) in &lines {
            let want = if *top < bottom - 0.6 { LEFT + 216.0 + 9.35 } else { LEFT };
            assert!(close(*x, want), "mode {mode}: the body line at y {top} starts at x {x}, Word at {want} (frame bottom {bottom})");
        }
    }
}

/// Row x: below a `notBeside` frame the body continues under the frame (Word: x 90, y 123.3),
/// while the header's own text ignores it as for any other wrap.
#[test]
fn body_text_continues_below_a_not_beside_header_frame() {
    let f = fixture(12, false, Some(frame(144.05, "notBeside")), &[U26], &["Body starts here."]);
    let (x, top) = f.body_start();
    assert!(close(x, LEFT) && close(top, f.frame_bottom()), "body at ({x}, {top}), Word at (90, frame bottom {})", f.frame_bottom());
}
