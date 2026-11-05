//! Host-scoped checked identities. Native handles never serve as identifiers.

use crate::error::{Error, IdentityKind};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_HOST: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SlotKey {
    pub(crate) index: u32,
    pub(crate) generation: u64,
}

/// Recipient incarnation changes whenever its retired storage is reused.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecipientGeneration(u64);

/// Host-scoped recipient token; every operation checks its incarnation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecipientId {
    pub(crate) host: HostId,
    pub(crate) key: SlotKey,
}
impl RecipientId {
    /// The issuing host.
    pub fn host(self) -> HostId {
        self.host
    }
    /// The exact recipient incarnation.
    pub fn generation(self) -> RecipientGeneration {
        RecipientGeneration(self.key.generation)
    }
}

/// Host-scoped subscription token with checked slot reuse.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubscriptionId {
    pub(crate) host: HostId,
    pub(crate) key: SlotKey,
}

/// Host-scoped one-shot timer token with checked slot reuse.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimerId {
    pub(crate) host: HostId,
    pub(crate) key: SlotKey,
}

/// Monotonic host-scoped identity of an owned post receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PostId {
    pub(crate) host: HostId,
    pub(crate) sequence: u64,
}

pub(crate) struct Arena<T> {
    entries: Vec<Slot<T>>,
    kind: IdentityKind,
    free_head: Option<u32>,
    #[cfg(test)]
    reservation_slot_reads: usize,
}
impl<T> Arena<T> {
    pub(crate) fn new(kind: IdentityKind) -> Self {
        Self {
            entries: Vec::new(),
            kind,
            free_head: None,
            #[cfg(test)]
            reservation_slot_reads: 0,
        }
    }
    pub(crate) fn reserve(&mut self) -> Result<SlotKey, Error> {
        // The candidate remains available until insert commits it. Companion
        // reservations may fail without needing a separate rollback operation.
        if let Some(index) = self.free_head {
            #[cfg(test)]
            {
                // Count the existing slot actually read below, not reserve calls.
                // Fresh growth writes a new slot and may allocate/copy backing;
                // neither is an existing-slot selection read.
                self.reservation_slot_reads += 1;
            }
            return Ok(SlotKey {
                index,
                generation: self.entries[index as usize].incarnation.unwrap(),
            });
        }
        let index =
            u32::try_from(self.entries.len()).map_err(|_| Error::IdentityExhausted(self.kind))?;
        self.entries
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        self.entries.push(Slot {
            incarnation: Some(1),
            value: None,
            next_free: self.free_head,
        });
        self.free_head = Some(index);
        Ok(SlotKey {
            index,
            generation: 1,
        })
    }
    pub(crate) fn insert(&mut self, key: SlotKey, value: T) {
        assert_eq!(
            self.free_head,
            Some(key.index),
            "insert commits the current candidate"
        );
        let slot = &mut self.entries[key.index as usize];
        assert_eq!(slot.incarnation, Some(key.generation));
        assert!(slot.value.is_none());
        self.free_head = slot.next_free.take();
        slot.value = Some(value);
    }
    pub(crate) fn get(&self, key: SlotKey) -> Option<&T> {
        self.entries
            .get(key.index as usize)
            .filter(|slot| slot.incarnation == Some(key.generation))?
            .value
            .as_ref()
    }
    pub(crate) fn get_mut(&mut self, key: SlotKey) -> Option<&mut T> {
        self.entries
            .get_mut(key.index as usize)
            .filter(|slot| slot.incarnation == Some(key.generation))?
            .value
            .as_mut()
    }
    pub(crate) fn remove(&mut self, key: SlotKey) -> Option<T> {
        let slot = self
            .entries
            .get_mut(key.index as usize)
            .filter(|slot| slot.incarnation == Some(key.generation))?;
        let value = slot.value.take()?;
        slot.incarnation = slot
            .incarnation
            .and_then(|generation| generation.checked_add(1));
        if slot.incarnation.is_some() {
            slot.next_free = self.free_head;
            self.free_head = Some(key.index);
        }
        Some(value)
    }
    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SlotKey, &T)> {
        self.entries.iter().enumerate().filter_map(|(index, slot)| {
            Some((
                SlotKey {
                    index: index as u32,
                    generation: slot.incarnation?,
                },
                slot.value.as_ref()?,
            ))
        })
    }
}

/// Process-local host identity. Values never wrap or silently repeat.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostId(u64);

impl HostId {
    pub(crate) fn allocate() -> Result<Self, Error> {
        // Only uniqueness is shared; no host state is published by this counter.
        NEXT_HOST
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(Self)
            .map_err(|_| Error::IdentityExhausted(IdentityKind::Host))
    }
}

/// Stable window slot identity, including host and slot incarnation.
/// It is distinct from the native-window generation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowId {
    host: HostId,
    slot: u32,
    incarnation: u64,
}

impl WindowId {
    /// The host that issued this identity.
    pub fn host(self) -> HostId {
        self.host
    }
}

/// A native window generation; independent of slot reuse and renderer state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowGeneration(u64);

impl WindowGeneration {
    pub(crate) const INITIAL: Self = Self(1);
}

/// A command target binds stable identity to its expected native generation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowTarget {
    /// The stable, host-scoped window identity.
    pub id: WindowId,
    /// The exact native generation to which the command applies.
    pub generation: WindowGeneration,
}

struct Slot<T> {
    incarnation: Option<u64>,
    value: Option<T>,
    next_free: Option<u32>,
}

pub(crate) struct WindowSlots<T> {
    host: HostId,
    slots: Vec<WindowSlot<T>>,
}

struct WindowSlot<T> {
    incarnation: Option<u64>,
    state: WindowSlotState<T>,
}

enum WindowSlotState<T> {
    Vacant,
    Reserved,
    Occupied(T),
}

impl<T> WindowSlot<T> {
    fn value(&self) -> Option<&T> {
        match &self.state {
            WindowSlotState::Occupied(value) => Some(value),
            _ => None,
        }
    }

    fn value_mut(&mut self) -> Option<&mut T> {
        match &mut self.state {
            WindowSlotState::Occupied(value) => Some(value),
            _ => None,
        }
    }

    fn retire(&mut self) -> WindowSlotState<T> {
        self.incarnation = self.incarnation.and_then(|value| value.checked_add(1));
        std::mem::replace(&mut self.state, WindowSlotState::Vacant)
    }
}

impl<T> WindowSlots<T> {
    pub(crate) fn new(host: HostId) -> Self {
        Self {
            host,
            slots: Vec::new(),
        }
    }

    // Reserve before native construction: acquired resources always have room
    // for their retirement record. A failed initialization stays reserved until
    // its native destruction is acknowledged.
    pub(crate) fn reserve(&mut self) -> Result<WindowId, Error> {
        for (slot, entry) in self.slots.iter_mut().enumerate() {
            if matches!(entry.state, WindowSlotState::Vacant)
                && let Some(incarnation) = entry.incarnation
            {
                entry.state = WindowSlotState::Reserved;
                return Ok(WindowId {
                    host: self.host,
                    slot: slot as u32,
                    incarnation,
                });
            }
        }
        let slot = u32::try_from(self.slots.len())
            .map_err(|_| Error::IdentityExhausted(IdentityKind::Window))?;
        self.slots
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        self.slots.push(WindowSlot {
            incarnation: Some(1),
            state: WindowSlotState::Reserved,
        });
        Ok(WindowId {
            host: self.host,
            slot,
            incarnation: 1,
        })
    }

    pub(crate) fn insert(&mut self, id: WindowId, value: T) {
        assert_eq!(id.host, self.host);
        let entry = &mut self.slots[id.slot as usize];
        assert_eq!(entry.incarnation, Some(id.incarnation));
        assert!(matches!(entry.state, WindowSlotState::Reserved));
        entry.state = WindowSlotState::Occupied(value);
    }

    // Releases only this exact reservation, never an occupied or reused slot.
    pub(crate) fn cancel_reservation(&mut self, id: WindowId) -> bool {
        if id.host != self.host {
            return false;
        }
        let Some(entry) = self.slots.get_mut(id.slot as usize) else {
            return false;
        };
        if entry.incarnation != Some(id.incarnation)
            || !matches!(entry.state, WindowSlotState::Reserved)
        {
            return false;
        }
        entry.retire();
        true
    }

    pub(crate) fn get(&self, id: WindowId) -> Result<&T, Error> {
        if id.host != self.host {
            return Err(Error::ForeignHost);
        }
        let entry = self
            .slots
            .get(id.slot as usize)
            .ok_or(Error::StaleIdentity)?;
        if entry.incarnation != Some(id.incarnation) {
            return Err(Error::StaleIdentity);
        }
        entry.value().ok_or(Error::StaleIdentity)
    }

    pub(crate) fn remove_where(&mut self, mut predicate: impl FnMut(&T) -> bool) -> Option<T> {
        for slot in &mut self.slots {
            if slot.value().is_some_and(&mut predicate)
                && let WindowSlotState::Occupied(value) = slot.retire()
            {
                return Some(value);
            }
        }
        None
    }

    pub(crate) fn get_mut(&mut self, id: WindowId) -> Result<&mut T, Error> {
        self.get(id)?;
        Ok(self.slots[id.slot as usize].value_mut().unwrap())
    }

    pub(crate) fn remove(&mut self, id: WindowId) -> Result<T, Error> {
        self.get(id)?;
        let entry = &mut self.slots[id.slot as usize];
        let WindowSlotState::Occupied(value) = entry.retire() else {
            unreachable!("checked occupied window slot")
        };
        Ok(value)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (WindowId, &T)> {
        self.slots.iter().enumerate().filter_map(|(slot, entry)| {
            Some((
                WindowId {
                    host: self.host,
                    slot: slot as u32,
                    incarnation: entry.incarnation?,
                },
                entry.value()?,
            ))
        })
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (WindowId, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(slot, entry)| {
                Some((
                    WindowId {
                        host: self.host,
                        slot: slot as u32,
                        incarnation: entry.incarnation?,
                    },
                    entry.value_mut()?,
                ))
            })
    }

    #[cfg(test)]
    pub(crate) fn retire_all(&mut self) {
        while self.remove_where(|_| true).is_some() {}
    }
}

#[cfg(test)]
#[path = "../tests/unit/identity.rs"]
mod tests;
