//! Synthetic fixtures for `header_extent.rs`. Structures are described in that task's spec
//! (case A: nested header table on a landscape letter page; case B: a first-page header whose
//! paragraph holds a top-and-bottom text box anchored 7.2pt below it and an inline 40 x 39pt
//! drawing canvas). Built in code; never from files Word produced.
use crate::*;
use wordcraft_doc::para::{CanvasStyle, ShapeKind};
use wordcraft_doc::props::{Align, Borders, LineSpacing, ParaProps, TableProps, TextColor};
use wordcraft_doc::table::{Cell, Row, Table};
use wordcraft_doc::{PartKind, Pos, para_block};

pub(super) fn lay_fixture(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// Twentieths of a point.
fn tw(v: f32) -> f32 {
    v / 20.0
}

/// EMUs.
fn emu(v: f32) -> f32 {
    v / 12_700.0
}

/// Single spacing, nothing after (`w:spacing w:after="0" w:line="240" w:lineRule="auto"`).
fn tight(align: Option<Align>) -> ParaProps {
    ParaProps { space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), align, ..Default::default() }
}

fn para(text: &str, props: ParaProps, chr: CharProps) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, chr);
    p.props = props;
    para_block(p)
}

fn cell(width: f32, blocks: Vec<Arc<Block>>) -> Cell {
    let mut c = Cell::empty();
    c.props.width = Some(width);
    c.blocks = blocks;
    c
}

fn table(grid: Vec<f32>, row: Row) -> Table {
    Table { props: TableProps { cell_margins: Some([0.0; 4]), ..Default::default() }, grid, rows: vec![row], controls: Default::default() }
}

fn row(cells: Vec<Cell>, height: Option<f32>) -> Row {
    let mut r = Row { cells, ..Default::default() };
    if let Some(h) = height {
        r.props.height = Some(h);
        r.props.height_rule = HeightRule::AtLeast;
    }
    r
}

/// A table of one row whose first cell holds a one-cell nested table (row at least
/// `height`, cell margins 39 twips) showing `text`, then an empty paragraph.
fn two_level(text: &str, chr: CharProps, align: Option<Align>, height: f32, inner: Cell) -> Table {
    let w1 = tw(13575.0);
    let mut c = cell(w1, vec![para(text, tight(align), chr)]);
    let m = tw(39.0);
    c.props.margins = Some([m; 4]);
    c.props.shading = inner.props.shading;
    c.props.borders = inner.props.borders;
    let nested = table(vec![w1], row(vec![c], Some(height)));
    let outer = cell(w1, vec![Arc::new(Block::Table(nested)), para("", tight(None), CharProps::default())]);
    let side = cell(tw(1647.0), vec![para("", tight(None), CharProps::default())]);
    table(vec![w1, tw(1647.0)], row(vec![outer, side], None))
}

/// docDefaults: Times New Roman (10pt, OOXML's default size), 10pt after, 1.15 lines.
fn defaults(d: &mut Document) {
    d.styles.default_chr = CharProps { font: Some("Times New Roman".into()), size: Some(10.0), ..Default::default() };
    d.styles.default_para = ParaProps { space_after: Some(10.0), line_spacing: Some(LineSpacing::Multiple(1.15)), ..Default::default() };
    d.styles.default_para_present = true;
}

/// Case A of the spec.
pub(super) fn nested_header_table_doc() -> Document {
    let mut d = Document::new();
    defaults(&mut d);
    d.settings.compat_mode = 12;
    let s = &mut d.last_section;
    (s.page_w, s.page_h, s.landscape) = (tw(15840.0), tw(12240.0), true);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (tw(144.0), tw(144.0), tw(144.0), tw(144.0));
    (s.header, s.footer) = (0.0, 0.0);
    let title = CharProps {
        font: Some("Arial".into()),
        size: Some(18.0),
        bold: Some(true),
        color: Some(TextColor::Rgb(Rgb(0xFF, 0xFF, 0xFF))),
        ..Default::default()
    };
    let mut banner = Cell::empty();
    banner.props.shading = Some(Rgb(0, 0, 0x66));
    let rule = Border { color: Some(Rgb(0, 0, 0)), ..Border::single(23.0 / 8.0) };
    banner.props.borders = Some(Borders { bottom: Some(rule), ..Default::default() });
    let header = two_level("Study Design, Objectives and Endpoints", title, Some(Align::Center), tw(462.0), banner);
    let id = d.add_part(PartKind::Header, vec![Arc::new(Block::Table(header))]);
    d.last_section.headers.default = Some(id);
    let bold = CharProps { font: Some("Arial".into()), size: Some(10.0), bold: Some(true), ..Default::default() };
    let body = two_level("Objectives", bold, None, tw(282.0), Cell::empty());
    d.body = vec![Arc::new(Block::Table(body)), para_block(Paragraph::new())];
    d
}

/// Case B of the spec.
pub(super) fn header_text_box_and_canvas_doc() -> Document {
    let mut d = Document::new();
    d.styles.default_chr = CharProps { font: Some("Times New Roman".into()), size: Some(10.0), ..Default::default() };
    d.styles.default_para = ParaProps { space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    d.styles.default_para_present = true;
    if let Some(normal) = d.styles.styles.iter_mut().find(|s| s.id == "Normal") {
        normal.chr = CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() };
    }
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (tw(11906.0), tw(16838.0));
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (tw(851.0), tw(964.0), tw(1985.0), tw(1701.0));
    (s.header, s.footer) = (tw(346.0), tw(851.0));
    s.title_page = true;
    let story = d.add_part(PartKind::TextBox, vec![para_block(Paragraph::with_text("Meeting of the steering committee", CharProps::default()))]);
    let id = d.add_part(PartKind::Header, vec![para_block(Paragraph::new())]);
    d.last_section.headers.first = Some(id);
    let float = Float {
        wrap: Wrap::TopAndBottom,
        h_rel: Anchor::Column,
        v_rel: Anchor::Paragraph,
        x: emu(401_320.0),
        y: emu(91_440.0),
        ..Default::default()
    };
    let text_box = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w: emu(5_735_955.0),
        h: emu(375_285.0),
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        float,
        story: Some(story),
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    let (w, h) = (emu(508_635.0), emu(494_665.0));
    let canvas = InlineObject::Group { w, h, float: Float::default(), ch_w: w, ch_h: h, children: Vec::new(), canvas: Some(CanvasStyle::default()) };
    let at = |off| Pos { story: StoryRef::Part(id), path: Path::top(0), off };
    let _ = d.insert_object(&at(0), text_box, &CharProps::default());
    let off = d.para(StoryRef::Part(id), &Path::top(0)).map_or(0, |p| p.len());
    let _ = d.insert_object(&at(off), canvas, &CharProps::default());
    let agenda = CharProps { font: Some("Arial".into()), size: Some(14.0), bold: Some(true), ..Default::default() };
    let props = ParaProps { align: Some(Align::Center), line_spacing: Some(LineSpacing::Multiple(1.15)), ..Default::default() };
    d.body = vec![para("Agenda", props, agenda)];
    d
}

/// Letter page, 1in margins, header distance 0.5in, header = one 10pt paragraph, body = one
/// 10pt paragraph with no spacing before.
pub(super) fn short_header_doc() -> Document {
    let mut d = Document::new();
    let ten = CharProps { size: Some(10.0), ..Default::default() };
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 72.0, 72.0, 72.0);
    s.header = 36.0;
    let id = d.add_part(PartKind::Header, vec![para("Running head", ParaProps::default(), ten.clone())]);
    d.last_section.headers.default = Some(id);
    d.body = vec![para("Body text", ParaProps { space_before: Some(0.0), ..Default::default() }, ten)];
    d
}

/// The topmost line showing text of `story` among `items` (empty paragraphs in table cells
/// beside it don't count): (baseline, line top).
fn first_line(items: &[Placed], story: impl Fn(&StoryRef) -> bool) -> Option<(f32, f32)> {
    items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: s, para, l0, y, .. } if story(s) && para.text_len > 0 => {
                let l = para.lines.get(*l0)?;
                Some((y + l.baseline - l.top, *y))
            }
            _ => None,
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Baseline (pt from the page top) of the first line of the header on page `page`.
pub(super) fn first_header_baseline(l: &DocLayout, page: usize) -> f32 {
    let p = &l.pages[page];
    let id = p.header_story.unwrap();
    first_line(&p.header, |s| *s == StoryRef::Part(id)).unwrap().0
}

/// Baseline (pt from the page top) of the first body line on page `page` (inside a table cell
/// when the body starts with a table).
pub(super) fn first_body_baseline(l: &DocLayout, page: usize) -> f32 {
    first_line(&l.pages[page].items, |s| *s == StoryRef::Body).unwrap().0
}

/// Top of the first body line box on page `page`.
pub(super) fn first_body_line_top(l: &DocLayout, page: usize) -> f32 {
    first_line(&l.pages[page].items, |s| *s == StoryRef::Body).unwrap().1
}
