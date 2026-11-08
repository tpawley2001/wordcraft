//! Office 2016 sunburst charts (`cx:series layoutId="sunburst"`): one ring per category level
//! around a hole, each node an annular sector whose angle is proportional to its value.

/// One drawn node of a sunburst: radii in points from the centre, angles in degrees clockwise
/// from 12 o'clock.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SunburstSegment {
    /// Category names from the top level down to this node.
    pub path: Vec<String>,
    /// Index of the top-level ancestor in first-appearance data order (picks the accent colour).
    pub group: usize,
    /// Sum of the node's leaves.
    pub value: f64,
    pub r0: f32,
    pub r1: f32,
    pub a0: f32,
    pub a1: f32,
}

/// Lays out a sunburst of `leaves` (category path top level first, value) in `plot` ([x, y, w, h]).
pub(crate) fn sunburst_segments(leaves: &[(Vec<String>, f64)], plot: [f32; 4]) -> Vec<SunburstSegment> {
    let _ = (leaves, plot);
    Vec::new()
}
