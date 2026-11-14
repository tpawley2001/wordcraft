use super::{Rect, Rgb};

/// How an area is painted: one colour or a gradient (page points), opacities 0..1.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Rgb, f32),
    /// A linear gradient from `p0` to `p1`; stops are (offset 0..1, colour, opacity).
    Linear {
        p0: (f32, f32),
        p1: (f32, f32),
        stops: Vec<(f32, Rgb, f32)>,
    },
    /// A circular gradient out from `c` to radius `r`; stops as [`Paint::Linear`]'s.
    Radial {
        c: (f32, f32),
        r: f32,
        stops: Vec<(f32, Rgb, f32)>,
    },
}

impl Paint {
    /// Opaque `c`.
    pub fn solid(c: Rgb) -> Paint {
        Paint::Solid(c, 1.0)
    }
    /// This paint with every opacity times `a` (soft edges' bands).
    pub fn faded(&self, a: f32) -> Paint {
        let fade = |s: &Vec<(f32, Rgb, f32)>| s.iter().map(|(o, c, x)| (*o, *c, x * a)).collect();
        match self {
            Paint::Solid(c, x) => Paint::Solid(*c, x * a),
            Paint::Linear { p0, p1, stops } => Paint::Linear { p0: *p0, p1: *p1, stops: fade(stops) },
            Paint::Radial { c, r, stops } => Paint::Radial { c: *c, r: *r, stops: fade(stops) },
        }
    }
    /// Inset soft-edge paints. Solid bands composite to the fill's target opacity.
    pub fn soft_bands(&self, radius: f32) -> Vec<(f32, Paint)> {
        match self {
            Paint::Solid(color, alpha) => {
                let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
                wordcraft_doc::effects::bands(-radius, 0.0, alpha).into_iter().map(|(grow, band)| (grow, Paint::Solid(*color, band))).collect()
            }
            _ => wordcraft_doc::effects::bands(-radius, 0.0, 1.0).into_iter().map(|(grow, alpha)| (grow, self.faded(alpha))).collect(),
        }
    }
    /// The colour and opacity this paint gives the point (`x`, `y`): stops blended, padded past
    /// the ends.
    pub fn at(&self, x: f32, y: f32) -> (Rgb, f32) {
        let (stops, t) = match self {
            Paint::Solid(c, a) => return (*c, *a),
            Paint::Linear { p0, p1, stops } => {
                let d = (p1.0 - p0.0, p1.1 - p0.1);
                (stops, ((x - p0.0) * d.0 + (y - p0.1) * d.1) / (d.0 * d.0 + d.1 * d.1).max(1e-6))
            }
            Paint::Radial { c, r, stops } => (stops, (x - c.0).hypot(y - c.1) / r.max(1e-6)),
        };
        let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return (Rgb::BLACK, 0.0) };
        let t = if t.is_finite() { t } else { 0.0 };
        let Some(i) = stops.iter().position(|s| s.0 >= t) else { return (last.1, last.2) };
        let Some(a) = i.checked_sub(1).and_then(|j| stops.get(j)) else { return (first.1, first.2) };
        let b = &stops[i];
        let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 0.0 };
        let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * f).round().clamp(0.0, 255.0) as u8;
        (Rgb(mix(a.1.0, b.1.0), mix(a.1.1, b.1.1), mix(a.1.2, b.1.2)), a.2 + (b.2 - a.2) * f)
    }
}

/// How shape fill `fill` paints the shape's frame `frame` (#1500). A linear gradient runs across
/// the frame at its angle (`scaled`: the angle of the frame squashed to a square); a path
/// gradient runs from the centre of its focus rectangle out to the farthest corner, drawn
/// circular whatever its outline (rectangular and shape-following ones are approximated).
/// `tileRect` and `flip` are kept but not drawn. `None` for no fill.
pub fn paint_for(fill: &wordcraft_doc::fill::Fill, frame: Rect) -> Option<Paint> {
    use wordcraft_doc::fill::{Fill, GradientKind};
    let g = match fill.sanitized()? {
        Fill::Solid { color, transparency } => return Some(Paint::Solid(color, 1.0 - transparency / 100.0)),
        Fill::Gradient(g) => g,
    };
    let stops = g.stops.iter().map(|s| (s.pos / 100.0, s.color, 1.0 - s.transparency / 100.0)).collect();
    let (cx, cy) = (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0);
    Some(match g.kind {
        GradientKind::Linear { angle, scaled } => {
            let (s, c) = angle.to_radians().sin_cos();
            // Scaled: the isolines of the square's gradient stretched with the frame.
            let (dx, dy) = if scaled && frame.w > 0.0 && frame.h > 0.0 { (c / frame.w, s / frame.h) } else { (c, s) };
            let len = dx.hypot(dy);
            let (dx, dy) = if len > 0.0 && len.is_finite() { (dx / len, dy / len) } else { (1.0, 0.0) };
            let half = (frame.w * dx.abs() + frame.h * dy.abs()) / 2.0;
            Paint::Linear { p0: (cx - dx * half, cy - dy * half), p1: (cx + dx * half, cy + dy * half), stops }
        }
        GradientKind::Path { focus: [l, t, r, b], .. } => {
            let x0 = frame.x + frame.w * l / 100.0;
            let x1 = frame.right() - frame.w * r / 100.0;
            let y0 = frame.y + frame.h * t / 100.0;
            let y1 = frame.bottom() - frame.h * b / 100.0;
            let c = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let corners = [(frame.x, frame.y), (frame.right(), frame.y), (frame.x, frame.bottom()), (frame.right(), frame.bottom())];
            let r = corners.iter().map(|(x, y)| (x - c.0).hypot(y - c.1)).fold(0.0f32, f32::max);
            Paint::Radial { c, r: r.max(0.01), stops }
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stroke {
    Solid,
    Dotted,
    Dashed,
    Double,
    /// Three thin lines side by side (a `triple` border).
    Triple,
    Wave,
    /// Word's border patterns (paragraph, cell and table borders): dots one width on, one off;
    /// dashes three on, one off; dash and dot.
    BorderDotted,
    BorderDashed,
    BorderDotDash,
    /// Underline styles as Word draws them: dash patterns of fixed lengths in points (see
    /// [`Stroke::underline_pattern`]), whatever the text size and line width.
    UnderlineDotted,
    UnderlineDash,
    UnderlineDotDash,
    UnderlineDotDotDash,
    /// A double underline: two lines filling the band the line's width covers, each `line`
    /// thousandths of that band thick (see [`Stroke::underline_double`]).
    UnderlineDouble {
        line: u16,
    },
    /// A wavy underline: a zig-zag whose centre line spans the band the line's width covers,
    /// drawn with a thin pen (see [`underline_waves`]).
    UnderlineWave,
    /// Two wavy underlines, one at the top and one at the bottom of the band.
    UnderlineDoubleWave,
}

impl Stroke {
    /// Dash and gap lengths, alternately, in line widths; `None` for the strokes that aren't
    /// dashed.
    pub fn dashes(self) -> Option<&'static [f32]> {
        match self {
            Stroke::Dotted => Some(&[1.0, 2.0]),
            Stroke::Dashed => Some(&[4.0, 2.0]),
            Stroke::BorderDotted => Some(&[1.0, 1.0]),
            Stroke::BorderDashed => Some(&[3.0, 1.0]),
            Stroke::BorderDotDash => Some(&[3.0, 1.0, 1.0, 1.0]),
            Stroke::Solid
            | Stroke::Double
            | Stroke::Triple
            | Stroke::Wave
            | Stroke::UnderlineDotted
            | Stroke::UnderlineDash
            | Stroke::UnderlineDotDash
            | Stroke::UnderlineDotDotDash
            | Stroke::UnderlineDouble { .. }
            | Stroke::UnderlineWave
            | Stroke::UnderlineDoubleWave => None,
        }
    }

    /// An underline's dash and gap lengths, alternately, in points; `None` for the strokes that
    /// aren't dashed underlines. Measured 2026-11-14 in Microsoft 365 Word's PDF export (16.0,
    /// Times New Roman and Arial, 8-36 pt): the lengths are the same at every size and in every
    /// font, and the pattern starts at the underline's left end (the last dash is cut off at
    /// its right end).
    pub fn underline_pattern(self) -> Option<&'static [f32]> {
        match self {
            Stroke::UnderlineDotted => Some(&[1.44, 1.44]),
            Stroke::UnderlineDash => Some(&[3.96, 1.92]),
            Stroke::UnderlineDotDash => Some(&[3.0, 0.96, 0.96, 0.96]),
            Stroke::UnderlineDotDotDash => Some(&[2.16, 0.72, 0.72, 0.72, 0.72, 0.72]),
            _ => None,
        }
    }

    /// A double underline drawn along a line `width` thick: each of its two lines as (offset of
    /// its centre from the line's centre, across the line; thickness). `None` for other strokes.
    pub fn underline_double(self, width: f32) -> Option<[(f32, f32); 2]> {
        let Stroke::UnderlineDouble { line } = self else { return None };
        if !width.is_finite() || width <= 0.0 {
            return None;
        }
        let t = width * f32::from(line.min(500)) / 1000.0;
        let off = (width - t) / 2.0;
        Some([(-off, t), (off, t)])
    }
}

/// Half a wavy underline's period along the line, in points (Word: 1.20 pt at every size).
pub const UNDERLINE_WAVE_HALF_PERIOD: f32 = 1.2;
/// The pen a wavy underline is drawn with, in points (Word: 0.24 pt at every size).
pub const UNDERLINE_WAVE_PEN: f32 = 0.24;
/// The room between the two waves of a double wavy underline, in points.
pub const UNDERLINE_DOUBLE_WAVE_GAP: f32 = 0.3;
/// At most this many points in one wave (a hostile extent can't make it endless).
const MAX_WAVE_POINTS: usize = 20_000;

/// The zig-zags of a wavy (`UnderlineWave`) or double wavy (`UnderlineDoubleWave`) underline drawn
/// from `(x0, y0)` to `(x1, y1)` with `width`, the height of the band the wave's centre line spans:
/// one polyline per wave, to be stroked with [`UNDERLINE_WAVE_PEN`]. The centre line starts at
/// the top of its band and turns every [`UNDERLINE_WAVE_HALF_PERIOD`]; the last half-wave is cut
/// off at the end. A double wave puts one wave at the top and one at the bottom of the band, each
/// `(width - UNDERLINE_DOUBLE_WAVE_GAP) / 2` tall. Empty for other strokes and unusable values.
pub fn underline_waves(stroke: Stroke, (x0, y0, x1, y1): (f32, f32, f32, f32), width: f32) -> Vec<Vec<(f32, f32)>> {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = dx.hypot(dy);
    if ![x0, y0, x1, y1, width, len].iter().all(|v| v.is_finite()) || len < 1e-3 || width < 0.0 {
        return Vec::new();
    }
    // Along the line, and across it (downwards for a line drawn left to right).
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    // Each wave: (offset of its band's top from the line's centre, its height).
    let bands: Vec<(f32, f32)> = match stroke {
        Stroke::UnderlineWave => vec![(-width / 2.0, width)],
        Stroke::UnderlineDoubleWave => {
            let h = ((width - UNDERLINE_DOUBLE_WAVE_GAP) / 2.0).max(0.0);
            vec![(-width / 2.0, h), (width / 2.0 - h, h)]
        }
        _ => return Vec::new(),
    };
    let point = |t: f32, off: f32| (x0 + ux * t + nx * off, y0 + uy * t + ny * off);
    bands
        .into_iter()
        .map(|(top, h)| {
            let mut pts = vec![point(0.0, top)];
            let mut t = 0.0f32;
            let mut down = true;
            while t < len && pts.len() < MAX_WAVE_POINTS {
                let nt = t + UNDERLINE_WAVE_HALF_PERIOD;
                if nt <= t {
                    break;
                }
                // The far end of this half-wave, or where the line ends partway through it.
                let (end, k) = if nt > len { (len, (len - t) / UNDERLINE_WAVE_HALF_PERIOD) } else { (nt, 1.0) };
                let off = if down { top + h * k } else { top + h * (1.0 - k) };
                pts.push(point(end, off));
                t = nt;
                down = !down;
            }
            pts
        })
        .collect()
}
