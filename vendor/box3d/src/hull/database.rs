//! World hull database: content-keyed sharing with reference counts.
//!
//! Port of `b3AddHullToDatabase` / `b3RemoveHullFromDatabase` from
//! `physics_world.c`. C uses a verstable map keyed by hull pointer with
//! content hash/equality; Rust keeps canonical `Arc<HullData>` entries and
//! deduplicates with [`compare_hull_data`].
//!
//! SPDX-FileCopyrightText: 2025 Erin Catto
//! SPDX-License-Identifier: MIT

use super::{clone_hull, compare_hull_data, destroy_hull, hash_hull_data, HullData};
use std::collections::HashMap;
use std::sync::Arc;

/// Shared hull store for a world. (b3HullMap + world->hullDatabase)
#[derive(Debug, Default)]
pub struct HullDatabase {
    /// Canonical strong refs — one per unique hull content, by content hash.
    /// Looking through a plain list for each hull added made a map with tens
    /// of thousands of solids take seconds to load.
    entries: HashMap<u64, Vec<Arc<HullData>>>,
    /// How many there are.
    count: usize,
}

impl HullDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a shared copy matching `src` by content, if any.
    fn find(&self, src: &HullData) -> Option<Arc<HullData>> {
        let h = hash_hull_data(src);
        for entry in self.entries.get(&h)? {
            if compare_hull_data(entry, src) {
                return Some(Arc::clone(entry));
            }
        }
        None
    }

    /// Clone `src` into the database on miss; share on hit.
    /// (b3AddHullToDatabase)
    pub fn add(&mut self, src: &HullData) -> Arc<HullData> {
        if let Some(existing) = self.find(src) {
            return existing;
        }

        let owned = clone_hull(src).expect("valid hull");
        let rc = Arc::new(owned);
        self.insert(&rc);
        rc
    }

    /// Keeps a canonical reference to `hull`.
    fn insert(&mut self, hull: &Arc<HullData>) {
        let same_hash = self.entries.entry(hash_hull_data(hull)).or_default();
        same_hash.push(Arc::clone(hull));
        self.count += 1;
    }

    /// Take ownership of `owned` on miss; destroy the duplicate on hit.
    /// (b3AddOwnedHullToDatabase)
    pub fn add_owned(&mut self, owned: HullData) -> Arc<HullData> {
        if let Some(existing) = self.find(&owned) {
            destroy_hull(owned);
            return existing;
        }

        let rc = Arc::new(owned);
        self.insert(&rc);
        rc
    }

    /// Drop the database's strong ref once the last shape releases its clone.
    /// Call while the shape still holds its `Arc` (strong_count >= 2 if DB also holds).
    /// (b3RemoveHullFromDatabase)
    pub fn release(&mut self, hull: &Arc<HullData>) {
        // Shape + DB = 2. After this release, only the shape's Arc remains (or none).
        if Arc::strong_count(hull) == 2 {
            let hash = hash_hull_data(hull);
            if let Some(same_hash) = self.entries.get_mut(&hash) {
                let before = same_hash.len();
                same_hash.retain(|e| !Arc::ptr_eq(e, hull));
                self.count -= before - same_hash.len();
                if same_hash.is_empty() {
                    self.entries.remove(&hash);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }
}
