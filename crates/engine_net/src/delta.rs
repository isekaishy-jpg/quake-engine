//! Snapshots as what changed since one the client already has (proposal 0046).

use std::collections::HashMap;

use engine_core::entity::{EntityState, ModelInstance};
use engine_core::math::Vec3;

use crate::message::{
    Reader, Snapshot, TAG_SNAPSHOT_DELTA, encode_player, encode_snapshot, put_vec3, read_player,
};

/// An entity's model changed.
const MODEL: u8 = 1;
/// An entity's frame changed.
const FRAME: u8 = 2;
/// An entity's skin changed.
const SKIN: u8 = 4;
/// An entity's effects changed.
const EFFECTS: u8 = 8;
/// An entity's origin changed.
const ORIGIN: u8 = 16;
/// An entity's angles changed.
const ANGLES: u8 = 32;

/// The most changes of one kind a delta may hold.
const MAX_CHANGES: usize = 8192;

/// `count` as it's sent.
fn put_count(out: &mut Vec<u8>, count: usize) {
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
}

/// Appends what differs between two entities.
fn put_entity(out: &mut Vec<u8>, old: Option<&EntityState>, new: &EntityState) -> bool {
    let changed = |test: fn(&EntityState, &EntityState) -> bool, bit: u8| {
        if old.is_none_or(|old| test(old, new)) {
            bit
        } else {
            0
        }
    };
    let mask = changed(|a, b| a.model != b.model, MODEL)
        | changed(|a, b| a.frame != b.frame, FRAME)
        | changed(|a, b| a.skin != b.skin, SKIN)
        | changed(|a, b| a.effects != b.effects, EFFECTS)
        | changed(|a, b| a.origin != b.origin, ORIGIN)
        | changed(|a, b| a.angles != b.angles, ANGLES);
    if mask == 0 {
        return false;
    }
    out.extend_from_slice(&new.id.to_le_bytes());
    out.push(mask);
    if mask & MODEL != 0 {
        out.extend_from_slice(&new.model.to_le_bytes());
    }
    if mask & FRAME != 0 {
        out.extend_from_slice(&new.frame.to_le_bytes());
    }
    if mask & SKIN != 0 {
        out.push(new.skin);
    }
    if mask & EFFECTS != 0 {
        out.push(new.effects);
    }
    if mask & ORIGIN != 0 {
        put_vec3(out, new.origin);
    }
    if mask & ANGLES != 0 {
        put_vec3(out, new.angles);
    }
    true
}

/// Appends what changed in the entities, then what's gone.
fn put_entities(out: &mut Vec<u8>, base: &[EntityState], entities: &[EntityState]) {
    let old: HashMap<u64, &EntityState> = base.iter().map(|entity| (entity.id, entity)).collect();
    let mut changes = Vec::new();
    let mut count = 0;
    for entity in entities {
        count += usize::from(put_entity(
            &mut changes,
            old.get(&entity.id).copied(),
            entity,
        ));
    }
    put_count(out, count);
    out.extend_from_slice(&changes);
    let kept: HashMap<u64, ()> = entities.iter().map(|entity| (entity.id, ())).collect();
    let gone: Vec<u64> = base
        .iter()
        .map(|entity| entity.id)
        .filter(|id| !kept.contains_key(id))
        .collect();
    put_count(out, gone.len());
    for id in gone {
        out.extend_from_slice(&id.to_le_bytes());
    }
    // The order drawn in is the server's, which additions can change.
    put_count(out, entities.len());
}

/// Appends a brush model.
fn put_instance(out: &mut Vec<u8>, instance: &ModelInstance) {
    out.extend_from_slice(&instance.model.to_le_bytes());
    put_vec3(out, instance.origin);
    out.extend_from_slice(&[u8::from(instance.visible), u8::from(instance.solid)]);
    out.extend_from_slice(&instance.frame.to_le_bytes());
}

/// Reads a brush model.
fn read_instance(reader: &mut Reader<'_>) -> Option<ModelInstance> {
    Some(ModelInstance {
        model: reader.u32()?,
        origin: reader.vec3()?,
        visible: reader.flag()?,
        solid: reader.flag()?,
        frame: u16::from_le_bytes(reader.take()?),
    })
}

/// Appends what changed in the brush models. They rarely change in number, so
/// any change there sends them all.
fn put_instances(out: &mut Vec<u8>, base: &[ModelInstance], instances: &[ModelInstance]) {
    let same_list = base.len() == instances.len()
        && base.iter().zip(instances).all(|(a, b)| a.model == b.model);
    out.push(u8::from(same_list));
    if same_list {
        let changed: Vec<(usize, &ModelInstance)> = base
            .iter()
            .zip(instances)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(index, (_, b))| (index, b))
            .collect();
        put_count(out, changed.len());
        for (index, instance) in changed {
            put_count(out, index);
            put_instance(out, instance);
        }
    } else {
        put_count(out, instances.len());
        for instance in instances {
            put_instance(out, instance);
        }
    }
}

/// Appends the boxes, if they changed, by place in the list.
fn put_boxes(out: &mut Vec<u8>, base: &[(Vec3, Vec3)], boxes: &[(Vec3, Vec3)]) {
    put_count(out, boxes.len());
    let changed: Vec<(usize, &(Vec3, Vec3))> = boxes
        .iter()
        .enumerate()
        .filter(|(index, pair)| base.get(*index) != Some(*pair))
        .collect();
    put_count(out, changed.len());
    for (index, (low, high)) in changed {
        put_count(out, index);
        put_vec3(out, *low);
        put_vec3(out, *high);
    }
}

/// `snapshot` as bytes: what differs from `base`, or all of it without one.
#[must_use]
pub fn delta_from(base: Option<&Snapshot>, snapshot: &Snapshot) -> Vec<u8> {
    let mut out = Vec::new();
    let Some(base) = base else {
        encode_snapshot(snapshot, &mut out);
        return out;
    };
    out.push(TAG_SNAPSHOT_DELTA);
    out.extend_from_slice(&snapshot.tick.to_le_bytes());
    out.extend_from_slice(&base.tick.to_le_bytes());
    out.extend_from_slice(&snapshot.acknowledged.to_le_bytes());
    encode_player(&snapshot.player, &mut out);
    out.extend_from_slice(&snapshot.weapon_model.to_le_bytes());
    out.extend_from_slice(&snapshot.weapon_frame.to_le_bytes());
    out.extend_from_slice(&snapshot.player_entity.to_le_bytes());
    out.push(snapshot.player_effects);
    put_vec3(&mut out, snapshot.punch);
    out.extend_from_slice(&snapshot.time.to_le_bytes());
    put_count(&mut out, snapshot.stats.len());
    let same_length = base.stats.len() == snapshot.stats.len();
    let changed: Vec<(usize, i32)> = snapshot
        .stats
        .iter()
        .enumerate()
        .filter(|(index, stat)| !same_length || base.stats.get(*index) != Some(*stat))
        .map(|(index, stat)| (index, *stat))
        .collect();
    put_count(&mut out, changed.len());
    for (index, stat) in changed {
        out.push(u8::try_from(index).unwrap_or(u8::MAX));
        out.extend_from_slice(&stat.to_le_bytes());
    }
    put_instances(&mut out, &base.instances, &snapshot.instances);
    put_entities(&mut out, &base.entities, &snapshot.entities);
    put_boxes(&mut out, &base.boxes, &snapshot.boxes);
    out
}

/// The snapshot a delta is against, by tick, from the delta's bytes after its
/// tag.
#[must_use]
pub fn delta_baseline(bytes: &[u8]) -> Option<u32> {
    let mut reader = Reader { bytes };
    let _tick = reader.u32()?;
    reader.u32()
}

/// Reads one entity's changes onto what it was.
fn read_entity(reader: &mut Reader<'_>, old: &HashMap<u64, EntityState>) -> Option<EntityState> {
    let id = u64::from_le_bytes(reader.take()?);
    let mask = reader.byte()?;
    let mut entity = old.get(&id).copied().unwrap_or(EntityState {
        id,
        ..EntityState::default()
    });
    if mask & MODEL != 0 {
        entity.model = u16::from_le_bytes(reader.take()?);
    }
    if mask & FRAME != 0 {
        entity.frame = u16::from_le_bytes(reader.take()?);
    }
    if mask & SKIN != 0 {
        entity.skin = reader.byte()?;
    }
    if mask & EFFECTS != 0 {
        entity.effects = reader.byte()?;
    }
    if mask & ORIGIN != 0 {
        entity.origin = reader.vec3()?;
    }
    if mask & ANGLES != 0 {
        entity.angles = reader.vec3()?;
    }
    Some(entity)
}

/// Reads a count, which has to be within reason.
fn read_count(reader: &mut Reader<'_>) -> Option<usize> {
    usize::try_from(reader.u32()?)
        .ok()
        .filter(|count| *count <= MAX_CHANGES)
}

/// Reads the entities.
fn read_entities(reader: &mut Reader<'_>, base: &[EntityState]) -> Option<Vec<EntityState>> {
    let old: HashMap<u64, EntityState> = base.iter().map(|entity| (entity.id, *entity)).collect();
    let mut changed = HashMap::new();
    for _ in 0..read_count(reader)? {
        let entity = read_entity(reader, &old)?;
        changed.insert(entity.id, entity);
    }
    let mut gone = HashMap::new();
    for _ in 0..read_count(reader)? {
        gone.insert(u64::from_le_bytes(reader.take()?), ());
    }
    let expected = read_count(reader)?;
    let mut entities: Vec<EntityState> = base
        .iter()
        .filter(|entity| !gone.contains_key(&entity.id))
        .map(|entity| changed.remove(&entity.id).unwrap_or(*entity))
        .collect();
    entities.extend(changed.into_values());
    // Ids hold a slot in their low bits, which is the order the server keeps.
    entities.sort_by_key(|entity| entity.id & 0x00FF_FFFF);
    (entities.len() == expected).then_some(entities)
}

/// Reads the brush models.
fn read_instances(reader: &mut Reader<'_>, base: &[ModelInstance]) -> Option<Vec<ModelInstance>> {
    if reader.flag()? {
        let mut instances = base.to_vec();
        for _ in 0..read_count(reader)? {
            let index = read_count(reader)?;
            *instances.get_mut(index)? = read_instance(reader)?;
        }
        Some(instances)
    } else {
        let count = read_count(reader)?;
        let mut instances = Vec::with_capacity(count);
        for _ in 0..count {
            instances.push(read_instance(reader)?);
        }
        Some(instances)
    }
}

/// Reads the boxes.
fn read_boxes(reader: &mut Reader<'_>, base: &[(Vec3, Vec3)]) -> Option<Vec<(Vec3, Vec3)>> {
    let count = read_count(reader)?;
    let mut boxes = base.to_vec();
    boxes.resize(count, (Vec3::ZERO, Vec3::ZERO));
    for _ in 0..read_count(reader)? {
        let index = read_count(reader)?;
        *boxes.get_mut(index)? = (reader.vec3()?, reader.vec3()?);
    }
    Some(boxes)
}

/// Builds a snapshot from a delta's bytes, after its tag, and what it's against.
#[must_use]
pub fn apply_delta(base: &Snapshot, bytes: &[u8]) -> Option<Snapshot> {
    let mut reader = Reader { bytes };
    let tick = reader.u32()?;
    let baseline = reader.u32()?;
    if baseline != base.tick {
        return None;
    }
    let acknowledged = reader.u32()?;
    let player = read_player(&mut reader)?;
    let weapon_model = u16::from_le_bytes(reader.take()?);
    let weapon_frame = u16::from_le_bytes(reader.take()?);
    let player_entity = u64::from_le_bytes(reader.take()?);
    let player_effects = reader.byte()?;
    let punch = reader.vec3()?;
    let time = reader.f64()?;
    let count = read_count(&mut reader)?;
    let mut stats = base.stats.clone();
    stats.resize(count, 0);
    for _ in 0..read_count(&mut reader)? {
        let index = usize::from(reader.byte()?);
        *stats.get_mut(index)? = i32::from_le_bytes(reader.take()?);
    }
    let instances = read_instances(&mut reader, &base.instances)?;
    let entities = read_entities(&mut reader, &base.entities)?;
    let boxes = read_boxes(&mut reader, &base.boxes)?;
    reader.finished().then_some(Snapshot {
        tick,
        acknowledged,
        player,
        instances,
        entities,
        boxes,
        weapon_model,
        weapon_frame,
        player_entity,
        player_effects,
        punch,
        time,
        stats,
    })
}
