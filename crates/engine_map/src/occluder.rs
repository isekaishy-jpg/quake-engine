//! The shade moving things cast, from their sidecars (proposal 0078).

/// A ball of a moving thing's shade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OccluderSphere {
    /// Its middle, in the model's own space.
    pub center: [f32; 3],
    /// How big it is.
    pub radius: f32,
}

/// The shade a brush model casts wherever it goes. The first sphere is the
/// whole of it, and the others are its parts, for up close.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelOccluder {
    /// The model.
    pub model: u32,
    /// Its spheres.
    pub spheres: Vec<OccluderSphere>,
}
