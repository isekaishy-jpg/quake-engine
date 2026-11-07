//! Skeletons and clips from glTF (proposal 0140). What comes in is turned to our axes and scale
//! here, once, so that nothing after this knows glTF's.

use std::collections::{HashMap, HashSet};

use engine_core::math::Vec3;
use engine_map::{
    Channel, ChannelPath, Clip, Interpolation, Joint, JointPose, NO_MOVE, Rows, Skeleton,
    SkeletonPlace, moved,
};
use gltf::animation::util::ReadOutputs;
use gltf::buffer::Data;

use crate::error::CompileError;
use crate::mesh::Mesh;

/// A skeleton, and how a file's numbers find its joints.
#[derive(Debug, Default)]
pub(crate) struct ImportedSkeleton {
    /// The skeleton.
    pub(crate) skeleton: Skeleton,
    /// Our joint for each of a skin's, by the skin and its place in that.
    by_skin: HashMap<(usize, usize), u16>,
}

/// A point or direction turned to our axes, and scaled.
#[expect(clippy::cast_possible_truncation, reason = "joints store f32")]
fn translation_of(value: [f32; 3], scale: f64) -> [f32; 3] {
    let scale = scale as f32;
    [value[0] * scale, -value[2] * scale, value[1] * scale]
}

/// A turn in our axes. Turning the axes is a proper rotation, so it's a change of the vector
/// part of the quaternion, the same way a point changes.
const fn rotation_of(value: [f32; 4]) -> [f32; 4] {
    [value[0], -value[2], value[1], value[3]]
}

/// A stretch in our axes, where Y and Z have changed places.
const fn scale_of(value: [f32; 3]) -> [f32; 3] {
    [value[0], value[2], value[1]]
}

/// A matrix by columns in glTF's axes, as rows in ours.
fn rows_of(columns: [[f32; 4]; 4], scale: f64) -> Rows {
    // Our axes in glTF's: x is x, y is -z, and z is y.
    let axis = [(0, 1.0), (2, -1.0), (1, 1.0)];
    let mut rows = NO_MOVE;
    for (row, (from_row, row_sign)) in axis.iter().enumerate() {
        for (column, (from_column, column_sign)) in axis.iter().enumerate() {
            rows[row][column] = columns[*from_column][*from_row] * row_sign * column_sign;
        }
    }
    let moved = [columns[3][0], columns[3][1], columns[3][2]];
    let moved = translation_of(moved, scale);
    for (row, part) in rows.iter_mut().zip(moved) {
        row[3] = part;
    }
    rows
}

/// A node's pose, in our axes.
fn pose_of(node: &gltf::Node<'_>, scale: f64) -> JointPose {
    let (translation, rotation, stretch) = node.transform().decomposed();
    JointPose {
        translation: translation_of(translation, scale),
        rotation: rotation_of(rotation),
        scale: scale_of(stretch),
    }
}

/// Every node that has to be a joint: those in skins, and everything above them. What's above a
/// joint moves it too, and being joints means it needs nothing special, even if it's animated.
fn joint_nodes(document: &gltf::Document) -> HashSet<usize> {
    let mut parents = HashMap::new();
    for node in document.nodes() {
        for child in node.children() {
            parents.insert(child.index(), node.index());
        }
    }
    let mut joints = HashSet::new();
    for skin in document.skins() {
        for joint in skin.joints() {
            let mut at = Some(joint.index());
            while let Some(node) = at {
                joints.insert(node);
                at = parents.get(&node).copied();
            }
        }
    }
    joints
}

/// Adds `node` and what's under it to `order`, each after what it hangs from.
fn walk(node: &gltf::Node<'_>, wanted: &HashSet<usize>, order: &mut Vec<usize>) {
    if wanted.contains(&node.index()) {
        order.push(node.index());
    }
    for child in node.children() {
        walk(&child, wanted, order);
    }
}

/// The clips of `document`, for what's in `by_node`.
fn read_clips(
    document: &gltf::Document,
    buffers: &[Data],
    by_node: &HashMap<usize, u16>,
    scale: f64,
) -> Vec<Clip> {
    let mut clips = Vec::new();
    for (number, animation) in document.animations().enumerate() {
        let mut clip = Clip {
            name: animation
                .name()
                .map_or_else(|| format!("clip{number}"), str::to_owned),
            ..Clip::default()
        };
        for channel in animation.channels() {
            let joint = by_node.get(&channel.target().node().index()).copied();
            let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(|data| &data[..]));
            let (Some(inputs), Some(outputs)) = (reader.read_inputs(), reader.read_outputs())
            else {
                continue;
            };
            let times: Vec<f32> = inputs.collect();
            let (path, values): (ChannelPath, Vec<f32>) = match outputs {
                ReadOutputs::MorphTargetWeights(values) => {
                    (ChannelPath::Weights, values.into_f32().collect())
                }
                // Moving what isn't a joint moves nothing of ours.
                _ if joint.is_none() => continue,
                ReadOutputs::Translations(values) => (
                    ChannelPath::Translation,
                    values
                        .flat_map(|value| translation_of(value, scale))
                        .collect(),
                ),
                ReadOutputs::Rotations(values) => (
                    ChannelPath::Rotation,
                    values.into_f32().flat_map(rotation_of).collect(),
                ),
                ReadOutputs::Scales(values) => {
                    (ChannelPath::Scale, values.flat_map(scale_of).collect())
                }
            };
            clip.length = clip.length.max(times.last().copied().unwrap_or(0.0));
            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::CubicSpline => Interpolation::Cubic,
            };
            clip.channels.push(Channel {
                joint: joint.unwrap_or(u16::MAX),
                path,
                interpolation,
                times,
                values,
            });
        }
        if !clip.channels.is_empty() {
            clips.push(clip);
        }
    }
    clips
}

/// The skeleton of `document`, if it has skins.
///
/// # Errors
///
/// Returns an error if it has more joints than a corner can name.
pub(crate) fn read_skeleton(
    document: &gltf::Document,
    buffers: &[Data],
    scale: f64,
) -> Result<Option<ImportedSkeleton>, CompileError> {
    let wanted = joint_nodes(document);
    let morphs = document
        .meshes()
        .flat_map(|mesh| mesh.primitives().collect::<Vec<_>>())
        .any(|primitive| primitive.morph_targets().len() > 0);
    if wanted.is_empty() && !morphs {
        return Ok(None);
    }
    let mut order = Vec::new();
    let scenes = document.default_scene().or(document.scenes().next());
    for node in scenes.iter().flat_map(gltf::Scene::nodes) {
        walk(&node, &wanted, &mut order);
    }
    if order.len() > usize::from(u8::MAX) + 1 {
        return Err(CompileError::Bad(format!(
            "a model has {} joints, and a mesh can have 256",
            order.len()
        )));
    }
    let by_node: HashMap<usize, u16> = order
        .iter()
        .enumerate()
        .map(|(at, node)| (*node, u16::try_from(at).unwrap_or(u16::MAX)))
        .collect();
    let nodes: Vec<gltf::Node<'_>> = document.nodes().collect();
    let mut parents = HashMap::new();
    for node in &nodes {
        for child in node.children() {
            parents.insert(child.index(), node.index());
        }
    }
    let mut joints: Vec<Joint> = order
        .iter()
        .map(|at| Joint {
            name: nodes[*at].name().unwrap_or_default().to_owned(),
            parent: parents
                .get(at)
                .and_then(|parent| by_node.get(parent))
                .copied(),
            rest: pose_of(&nodes[*at], scale),
            inverse_bind: NO_MOVE,
        })
        .collect();
    let mut by_skin = HashMap::new();
    for skin in document.skins() {
        let reader = skin.reader(|buffer| buffers.get(buffer.index()).map(|data| &data[..]));
        let mut binds = reader.read_inverse_bind_matrices();
        for (place, joint) in skin.joints().enumerate() {
            let Some(&ours) = by_node.get(&joint.index()) else {
                continue;
            };
            by_skin.insert((skin.index(), place), ours);
            // With none given, a joint's inverse bind matrix is the identity.
            if let Some(bind) = binds.as_mut().and_then(Iterator::next) {
                joints[usize::from(ours)].inverse_bind = rows_of(bind, scale);
            }
        }
    }
    let clips = read_clips(document, buffers, &by_node, scale);
    let mut places = Vec::new();
    for node in &nodes {
        let Some(name) = node.name().and_then(|name| {
            name.to_ascii_lowercase()
                .strip_prefix("attach_")
                .map(str::to_owned)
        }) else {
            continue;
        };
        let parent = parents
            .get(&node.index())
            .and_then(|parent| by_node.get(parent));
        if let Some(joint) = parent {
            places.push(SkeletonPlace {
                name,
                joint: Some(*joint),
                offset: pose_of(node, scale).rows(),
            });
        }
    }
    Ok(Some(ImportedSkeleton {
        skeleton: Skeleton {
            joints,
            clips,
            places,
            reach: 0.0,
            ..Skeleton::default()
        },
        by_skin,
    }))
}

impl ImportedSkeleton {
    /// The four strongest of what moves a corner, with their shares as bytes that sum to 255.
    /// `moves` is each joint of `skin` and how much it counts.
    pub(crate) fn weights_of(&self, skin: usize, moves: &[(usize, f32)]) -> ([u8; 4], [u8; 4]) {
        let mut strongest: Vec<(u8, f32)> = moves
            .iter()
            .filter(|(_, weight)| *weight > 0.0)
            .filter_map(|(joint, weight)| {
                let ours = self.by_skin.get(&(skin, *joint))?;
                Some((u8::try_from(*ours).ok()?, *weight))
            })
            .collect();
        strongest.sort_by(|a, b| b.1.total_cmp(&a.1));
        strongest.truncate(4);
        let total: f32 = strongest.iter().map(|(_, weight)| weight).sum();
        let (mut joints, mut weights) = ([0; 4], [0; 4]);
        if total <= 0.0 {
            return (joints, weights);
        }
        let mut given = 0_u32;
        for (at, (joint, weight)) in strongest.iter().enumerate() {
            joints[at] = *joint;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a share of 255"
            )]
            let share = (weight / total * 255.0).round() as u32;
            weights[at] = u8::try_from(share.min(255)).unwrap_or(u8::MAX);
            given += share;
        }
        // What rounding lost goes to the strongest, as weights have to come to 255.
        weights[0] = u8::try_from((i64::from(weights[0]) + 255 - i64::from(given)).clamp(0, 255))
            .unwrap_or(u8::MAX);
        (joints, weights)
    }
}

/// The strings in the list called `targetNames` in `extras`, which is JSON. This is the only
/// thing read from it, so it's read by hand.
fn listed_names(extras: &str) -> Vec<String> {
    let Some((_, after)) = extras.split_once("\"targetNames\"") else {
        return Vec::new();
    };
    let Some((_, after)) = after.split_once('[') else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut characters = after.chars();
    let mut name: Option<String> = None;
    while let Some(character) = characters.next() {
        match (character, &mut name) {
            ('\\', Some(name)) => name.extend(characters.next()),
            ('"', Some(_)) => names.extend(name.take()),
            ('"', None) => name = Some(String::new()),
            (']', None) => break,
            (character, Some(name)) => name.push(character),
            _ => {}
        }
    }
    names
}

/// The names of `mesh`'s morph targets, which aren't in glTF proper. Exporters put them in the
/// mesh's extras, as `targetNames`, and with none they're numbered.
pub(crate) fn morph_names(mesh: &gltf::Mesh<'_>, count: usize) -> Vec<String> {
    let listed = mesh
        .extras()
        .as_ref()
        .map_or_else(Vec::new, |raw| listed_names(raw.get()));
    (0..count)
        .map(|at| {
            let name = listed.get(at);
            name.map_or_else(|| format!("target{at}"), |name| name.replace(' ', "_"))
        })
        .collect()
}

/// How far any corner of `mesh` gets from a joint that carries it, at rest.
#[must_use]
pub(crate) fn reach_of(mesh: &Mesh, skeleton: &Skeleton) -> f32 {
    let placed = skeleton.placed(&skeleton.pose(None, 0.0));
    let joints: Vec<Vec3> = placed.iter().map(|rows| moved(rows, Vec3::ZERO)).collect();
    let mut reach = 0.0_f32;
    for part in &mesh.parts {
        for (corner, (carried, weights)) in part
            .points
            .iter()
            .zip(part.joints.iter().zip(&part.weights))
        {
            #[expect(clippy::cast_possible_truncation, reason = "joints store f32")]
            let corner = Vec3::new(corner[0] as f32, corner[1] as f32, corner[2] as f32);
            for (joint, weight) in carried.iter().zip(weights) {
                if *weight > 0
                    && let Some(place) = joints.get(usize::from(*joint))
                {
                    reach = reach.max((corner - *place).length());
                }
            }
        }
    }
    reach
}
