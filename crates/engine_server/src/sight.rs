//! What a seat can see, so that what it can't isn't sent (proposal 0088).

use engine_core::math::Vec3;
use engine_map::Tree;

/// Within this, everything's sent, so nothing a player could touch or walk
/// round a corner into is ever missing.
const ALWAYS_NEAR: f32 = 384.0;

/// What can be seen from a place.
#[derive(Clone, Debug)]
pub struct Sight<'a> {
    /// The map's tree.
    tree: &'a Tree,
    /// A flag for each leaf that can be seen.
    seen: Vec<bool>,
    /// Where it's seen from.
    eye: Vec3,
}

impl<'a> Sight<'a> {
    /// What can be seen from `eye` and `body`, or `None` if the map has no
    /// visible sets, and everything shows.
    #[must_use]
    pub fn from(tree: &'a Tree, eye: Vec3, body: Vec3) -> Option<Self> {
        if tree.visibility.is_empty() {
            return None;
        }
        let mut seen = vec![false; tree.leaves.len()];
        tree.mark_visible(tree.leaf_at(eye), &mut seen);
        tree.mark_visible(tree.leaf_at(body), &mut seen);
        Some(Self { tree, seen, eye })
    }

    /// Whether anything in the box from `low` to `high` can be seen. A point in
    /// solid has no leaf to go by, so it shows.
    #[must_use]
    pub fn shows(&self, low: Vec3, high: Vec3) -> bool {
        let middle = (low + high) * 0.5;
        if (middle - self.eye).length() < ALWAYS_NEAR {
            return true;
        }
        let corners = [
            middle,
            Vec3::new(low.x, low.y, low.z),
            Vec3::new(high.x, low.y, low.z),
            Vec3::new(low.x, high.y, low.z),
            Vec3::new(high.x, high.y, low.z),
            Vec3::new(low.x, low.y, high.z),
            Vec3::new(high.x, low.y, high.z),
            Vec3::new(low.x, high.y, high.z),
            Vec3::new(high.x, high.y, high.z),
        ];
        corners.iter().any(|corner| {
            let leaf = self.tree.leaf_at(*corner);
            leaf == 0 || self.seen.get(leaf as usize).copied().unwrap_or(true)
        })
    }
}
