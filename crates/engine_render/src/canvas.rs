//! Turning a canvas's quads into vertices (proposal 0039).

use engine_core::canvas::Quad;

/// Floats in one vertex: position, texture coordinates and colour.
pub(crate) const VERTEX_FLOATS: usize = 8;

/// Vertices in a quad.
const QUAD_VERTICES: usize = 6;

/// The most quads drawn in a frame. A full console on a large screen is the
/// most anything asks for.
pub(crate) const MAX_QUADS: usize = 32768;

/// A run of quads that share a picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuadBatch {
    /// The picture.
    pub picture: u32,
    /// The first vertex.
    pub first: u32,
    /// How many vertices.
    pub count: u32,
}

/// A run of UI triangles, clipped to a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiBatch {
    /// The picture.
    pub picture: u32,
    /// The first vertex.
    pub first: u32,
    /// How many vertices.
    pub count: u32,
    /// The left, top, right and bottom edges of what's shown, in pixels.
    pub clip: [u32; 4],
}

/// A UI's triangles for a frame, drawn over everything (proposal 0074).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiFrame {
    /// Vertices, as clip space position, texture coordinates and colour.
    pub vertices: Vec<f32>,
    /// What to draw.
    pub batches: Vec<UiBatch>,
}

/// The vertices of `quads` on a picture of `size`, and the runs to draw them
/// in. Quads past [`MAX_QUADS`] are dropped.
#[must_use]
pub fn canvas_vertices(quads: &[Quad], size: (u32, u32)) -> (Vec<f32>, Vec<QuadBatch>) {
    let quads = quads.get(..MAX_QUADS).unwrap_or(quads);
    let mut vertices = Vec::with_capacity(quads.len() * QUAD_VERTICES * VERTEX_FLOATS);
    let mut batches: Vec<QuadBatch> = Vec::new();
    #[expect(clippy::cast_precision_loss, reason = "sizes are far below 2^24")]
    let (width, height) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let per_quad = u32::try_from(QUAD_VERTICES).unwrap_or(0);
    for (index, quad) in quads.iter().enumerate() {
        let [left, top, right, bottom] = quad.place;
        let [u0, v0, u1, v1] = quad.source;
        let [red, green, blue, alpha] = quad.color;
        // Clip space has y up, as in the world shader.
        let corner = |x: f32, y: f32, u: f32, v: f32| {
            [
                x / width * 2.0 - 1.0,
                1.0 - y / height * 2.0,
                u,
                v,
                red,
                green,
                blue,
                alpha,
            ]
        };
        let corners = [
            corner(left, top, u0, v0),
            corner(right, top, u1, v0),
            corner(right, bottom, u1, v1),
            corner(left, top, u0, v0),
            corner(right, bottom, u1, v1),
            corner(left, bottom, u0, v1),
        ];
        vertices.extend(corners.into_iter().flatten());
        match batches.last_mut() {
            Some(batch) if batch.picture == quad.picture => batch.count += per_quad,
            _ => batches.push(QuadBatch {
                picture: quad.picture,
                first: u32::try_from(index).unwrap_or(0) * per_quad,
                count: per_quad,
            }),
        }
    }
    (vertices, batches)
}
