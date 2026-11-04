//! Leaf lookups and visible sets.

use engine_core::math::Vec3;
use engine_map::{Leaf, Node, NodeChild, Plane, Tree};

/// A tree split at x = 0: leaf 1 in front and leaf 2 behind.
fn tree() -> Tree {
    Tree {
        planes: vec![Plane {
            normal: Vec3::new(1.0, 0.0, 0.0),
            distance: 0.0,
        }],
        nodes: vec![Node {
            plane: 0,
            children: [NodeChild::Leaf(1), NodeChild::Leaf(2)],
        }],
        leaves: vec![Leaf::default(); 3],
        ..Tree::default()
    }
}

#[test]
fn points_find_their_side() {
    let tree = tree();
    assert_eq!(tree.leaf_at(Vec3::new(5.0, 0.0, 0.0)), 1);
    assert_eq!(tree.leaf_at(Vec3::new(-5.0, 0.0, 0.0)), 2);
    assert_eq!(tree.leaf_at(Vec3::ZERO), 1, "on the plane counts as front");
}

#[test]
fn an_empty_tree_is_all_outside() {
    assert_eq!(Tree::default().leaf_at(Vec3::ZERO), 0);
}

/// A tree with `count` visible leaves and a set for leaf 1.
fn with_set(count: u32, set: &[u8]) -> Tree {
    let total = usize::try_from(count).expect("small") + 1;
    let mut leaves = vec![Leaf::default(); total];
    leaves[1].visibility = Some(0);
    Tree {
        leaves,
        visible_leaf_count: count,
        visibility: set.to_vec(),
        ..Tree::default()
    }
}

/// The leaves visible from leaf 1.
fn visible(tree: &Tree) -> Vec<usize> {
    let mut flags = vec![false; tree.leaves.len()];
    tree.mark_visible(1, &mut flags);
    flags
        .iter()
        .enumerate()
        .filter(|(_, flag)| **flag)
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn bit_n_is_leaf_n_plus_one() {
    assert_eq!(visible(&with_set(8, &[0b0000_0101])), [1, 3]);
}

/// `gl_model.c`, `Mod_DecompressVis`: a zero byte and a count skip whole bytes.
#[test]
fn a_zero_run_skips_bytes() {
    assert_eq!(visible(&with_set(32, &[1, 0, 2, 1])), [1, 25]);
}

#[test]
fn bits_past_the_leaf_count_are_ignored() {
    assert_eq!(visible(&with_set(2, &[0b1111_1111])), [1, 2]);
}

#[test]
fn everything_is_visible_without_a_set() {
    let tree = tree();
    let mut flags = vec![false; 3];
    tree.mark_visible(0, &mut flags);
    assert_eq!(flags, [true; 3]);
    let mut flags = vec![false; 3];
    tree.mark_visible(2, &mut flags);
    assert_eq!(flags, [true; 3]);
}
