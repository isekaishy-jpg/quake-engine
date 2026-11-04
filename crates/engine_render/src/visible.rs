//! Working out which faces to draw.

use engine_core::math::Vec3;
use engine_map::{Contents, Map};

use crate::world::WorldMesh;

/// How far above and below the view to look for a second leaf, so a view at a
/// water surface sees both sides of it. From `R_MarkLeaves` in `gl_rsurf.c`.
const SURFACE_REACH: f32 = 16.0;

/// The visible triangles, grouped by batch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisibleSet {
    /// The two leaves this was built for.
    leaves: Option<[u32; 2]>,
    /// All indices, batch after batch.
    pub indices: Vec<u32>,
    /// Each batch's first index and count.
    pub ranges: Vec<(u32, u32)>,
}

impl VisibleSet {
    /// The leaves that decide what's visible from `origin`.
    fn view_leaves(map: &Map, origin: Vec3) -> [u32; 2] {
        let tree = &map.tree;
        let leaf = tree.leaf_at(origin);
        let contents = tree
            .leaf(leaf)
            .map_or(Contents::Solid, |leaf| leaf.contents);
        let offset = if contents == Contents::Empty {
            -SURFACE_REACH
        } else {
            SURFACE_REACH
        };
        let other = tree.leaf_at(origin + Vec3::new(0.0, 0.0, offset));
        let reachable = tree
            .leaf(other)
            .is_some_and(|other| other.contents != Contents::Solid);
        [leaf, if reachable { other } else { leaf }]
    }

    /// Rebuilds the set if the view has moved to another leaf. Returns
    /// whether it changed.
    pub fn update(&mut self, map: &Map, mesh: &WorldMesh, origin: Vec3) -> bool {
        let leaves = Self::view_leaves(map, origin);
        if self.leaves == Some(leaves) {
            return false;
        }
        self.leaves = Some(leaves);
        let tree = &map.tree;
        let mut visible = vec![false; tree.leaves.len()];
        for leaf in leaves {
            tree.mark_visible(leaf, &mut visible);
        }
        let mut drawn = vec![false; map.faces.len()];
        let mut per_batch: Vec<Vec<u32>> = vec![Vec::new(); mesh.batches.len()];
        let mut add = |face: u32| {
            let at = face as usize;
            if let Some(flag) = drawn.get_mut(at)
                && !*flag
            {
                *flag = true;
                if let Some(batch) = mesh.face_batches[at] {
                    per_batch[batch as usize].extend_from_slice(&mesh.face_indices[at]);
                }
            }
        };
        for (leaf, _) in visible.iter().enumerate().filter(|(_, flag)| **flag) {
            let leaf = &tree.leaves[leaf];
            let first = leaf.first_face as usize;
            let count = leaf.face_count as usize;
            for &face in tree
                .leaf_faces
                .get(first..first + count)
                .unwrap_or_default()
            {
                add(face);
            }
        }
        self.indices.clear();
        self.ranges.clear();
        for batch in per_batch {
            let first = u32::try_from(self.indices.len()).unwrap_or(u32::MAX);
            let count = u32::try_from(batch.len()).unwrap_or(u32::MAX);
            self.ranges.push((first, count));
            self.indices.extend(batch);
        }
        true
    }
}
