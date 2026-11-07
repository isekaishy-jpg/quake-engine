//! Making coarser levels of a mesh (proposal 0140). This is for tools: a finished game only
//! reads what it made, so it doesn't carry `meshopt`.

use meshopt::{SimplifyOptions, VertexDataAdapter, simplify};

use crate::mesh::{Mesh, MeshPart};

/// The share of the finest level's triangles each coarser level aims for.
const LEVEL_SHARES: [f64; 3] = [0.5, 0.25, 0.125];

/// How far each level can stray, as a share of the mesh's size. Further away, more is hidden.
const LEVEL_ERRORS: [f32; 3] = [0.01, 0.03, 0.08];

/// A part with fewer triangles than this is left as it is.
const FEWEST_TRIANGLES: usize = 16;

/// A level has to lose at least this share of the last one's triangles to be worth having.
const LEAST_SAVING: f64 = 0.15;

/// `part`'s triangles aiming for `target`, within `error`.
fn simplified(part: &MeshPart, triangles: &[[u32; 3]], target: usize, error: f32) -> Vec<[u32; 3]> {
    let flat: Vec<u32> = triangles.iter().flatten().copied().collect();
    let mut bytes = Vec::with_capacity(part.points.len() * 12);
    for point in &part.points {
        for axis in point {
            #[expect(clippy::cast_possible_truncation, reason = "positions to simplify by")]
            bytes.extend_from_slice(&(*axis as f32).to_ne_bytes());
        }
    }
    let Ok(adapter) = VertexDataAdapter::new(&bytes, 12, 0) else {
        return triangles.to_vec();
    };
    // Corners split at a seam in the UVs look like an open edge. Locking those keeps the
    // seam shut, so that nothing tears.
    let kept = simplify(
        &flat,
        &adapter,
        target * 3,
        error,
        SimplifyOptions::LockBorder,
        None,
    );
    kept.as_chunks::<3>().0.to_vec()
}

/// Gives `mesh` coarser levels, for the parts that have none and are worth it. Says how many
/// levels the mesh has past its finest.
pub fn add_levels(mesh: &mut Mesh) -> usize {
    for part in mesh.parts.iter_mut().filter(|part| part.levels.is_empty()) {
        if part.triangles.len() < FEWEST_TRIANGLES {
            continue;
        }
        // Not by way of normals: splitting corners by which way they face would leave almost
        // nothing joined to simplify, and each level works out its own anyway.
        let finest = part.triangles.len();
        let mut last = part.triangles.clone();
        for (share, error) in LEVEL_SHARES.iter().zip(LEVEL_ERRORS) {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                clippy::cast_sign_loss,
                reason = "a count of triangles"
            )]
            let target = (finest as f64 * share) as usize;
            let made = simplified(part, &last, target, error);
            #[expect(clippy::cast_precision_loss, reason = "a count of triangles")]
            let saving = 1.0 - made.len() as f64 / last.len() as f64;
            if made.is_empty() || saving < LEAST_SAVING {
                break;
            }
            last.clone_from(&made);
            part.levels.push(made);
        }
    }
    mesh.parts
        .iter()
        .map(|part| part.levels.len())
        .max()
        .unwrap_or(0)
}
