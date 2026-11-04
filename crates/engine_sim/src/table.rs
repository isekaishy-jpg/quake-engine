//! The slot table that holds entities (proposal 0026).

/// How long a slot stays empty before reuse, from `ED_Alloc`.
const REUSE_DELAY: f64 = 0.5;

/// Early in a level, slots are reused at once, from `ED_Alloc`.
const STARTUP_TIME: f64 = 2.0;

/// An entity, which stops matching once it's removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId {
    /// The slot.
    index: u32,
    /// How many times the slot has been used.
    generation: u32,
}

impl EntityId {
    /// Packs this id into a number that survives a trip through a script,
    /// whose numbers are exact up to 2^53.
    #[must_use]
    pub fn to_bits(self) -> u64 {
        (u64::from(self.generation) << 24) | u64::from(self.index)
    }

    /// Unpacks an id.
    #[must_use]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            index: u32::try_from(bits & 0x00FF_FFFF).unwrap_or(0),
            generation: u32::try_from(bits >> 24).unwrap_or(0),
        }
    }
}

/// One place in the table.
#[derive(Clone, Debug)]
struct Slot<T> {
    /// How many times it's been used.
    generation: u32,
    /// What's in it.
    value: Option<T>,
    /// When it was last emptied.
    freed_at: f64,
}

/// A table that keeps slot order, and reuses the lowest free slot once it's
/// been free long enough, as the original's entity list does.
#[derive(Clone, Debug)]
pub struct SlotTable<T> {
    /// The slots.
    slots: Vec<Slot<T>>,
}

impl<T> Default for SlotTable<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> SlotTable<T> {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `value` at game time `time`.
    pub fn insert(&mut self, value: T, time: f64) -> EntityId {
        let free = self.slots.iter_mut().enumerate().find(|(_, slot)| {
            slot.value.is_none()
                && (slot.freed_at < STARTUP_TIME || time - slot.freed_at > REUSE_DELAY)
        });
        if let Some((index, slot)) = free {
            slot.generation = slot.generation.wrapping_add(1);
            slot.value = Some(value);
            return EntityId {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                generation: slot.generation,
            };
        }
        let index = u32::try_from(self.slots.len()).unwrap_or(u32::MAX);
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
            freed_at: 0.0,
        });
        EntityId {
            index,
            generation: 0,
        }
    }

    /// The slot `id` points at, if it's still that entity.
    fn slot(&self, id: EntityId) -> Option<&Slot<T>> {
        self.slots
            .get(usize::try_from(id.index).ok()?)
            .filter(|slot| slot.generation == id.generation)
    }

    /// Removes `id` at game time `time`.
    pub fn remove(&mut self, id: EntityId, time: f64) -> Option<T> {
        let slot = self.slots.get_mut(usize::try_from(id.index).ok()?)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take();
        if value.is_some() {
            slot.freed_at = time;
        }
        value
    }

    /// The value of `id`.
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&T> {
        self.slot(id)?.value.as_ref()
    }

    /// The value of `id`.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut T> {
        let slot = self.slots.get_mut(usize::try_from(id.index).ok()?)?;
        if slot.generation == id.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Every id, in slot order.
    #[must_use]
    pub fn ids(&self) -> Vec<EntityId> {
        self.iter().map(|(id, _)| id).collect()
    }

    /// Every entry, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let value = slot.value.as_ref()?;
            let id = EntityId {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                generation: slot.generation,
            };
            Some((id, value))
        })
    }
}
