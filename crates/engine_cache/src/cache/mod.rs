//! Typed keyed membership and protected retained-payload retrieval.

use std::fmt;
use std::hash::{BuildHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::accounting::AllocationClass;
use crate::ownership::{Backing, View};
use crate::storage::{Entry, IdentityToken, Storage};

mod retention;
pub use retention::RetentionReport;

/// Cache-scoped logical membership, stable across payload eviction.
///
/// Removal and recreation create distinct identities, including recycled slots.
/// Identity retention does not retain a payload. Key type and cache namespace
/// prevent unrelated memberships from being substituted for one another.
pub struct Identity<K> {
    pub(crate) token: Arc<IdentityToken>,
    key_type: PhantomData<fn() -> K>,
}

impl<K> Clone for Identity<K> {
    fn clone(&self) -> Self {
        Self {
            token: Arc::clone(&self.token),
            key_type: PhantomData,
        }
    }
}
impl<K> PartialEq for Identity<K> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.token, &other.token)
    }
}
impl<K> Eq for Identity<K> {}
impl<K> Hash for Identity<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.token).hash(state);
    }
}
impl<K> fmt::Debug for Identity<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("token", &Arc::as_ptr(&self.token))
            .finish()
    }
}

/// Retrieval distinguishes usable backing from missing or failed membership.
/// No unavailable outcome creates consumer demand or starts production.
pub enum Lookup<'entry, R, E> {
    /// Nothing is known about this key.
    Absent,
    /// The key is known, with nothing stored.
    Vacant,
    /// Producing it failed, with this error.
    Failed(&'entry E),
    /// It's here.
    Ready(R),
}

/// Owned payload state returned by replacement or removal.
/// Dropping a ready state releases its owner using that backing's cleanup contract.
pub enum StoredPayload<'cleanup, T, E> {
    /// Nothing is stored.
    Vacant,
    /// Producing it failed, with this error.
    Failed(E),
    /// It's stored in this backing.
    Ready(Backing<'cleanup, T>),
}
impl<T, E> fmt::Debug for StoredPayload<'_, T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Vacant => "Vacant",
            Self::Failed(_) => "Failed(..)",
            Self::Ready(_) => "Ready(..)",
        })
    }
}

/// Why an installation rejected its original owned inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallErrorReason {
    /// Its size would take the policy past what it can count.
    PolicyOverflow,
    /// Its allocation class isn't one this cache takes.
    InvalidClass,
}

/// Rejected installation retaining its key, backing and declared policy cost.
pub struct InstallError<'cleanup, K, T> {
    key: K,
    backing: Backing<'cleanup, T>,
    policy_bytes: u64,
    reason: InstallErrorReason,
}

/// Installation result containing stable membership and the displaced owned state.
pub type InstallResult<'cleanup, K, T, E = ()> =
    Result<(Identity<K>, StoredPayload<'cleanup, T, E>), InstallError<'cleanup, K, T>>;

pub(crate) enum IdentityReplaceReason {
    Missing,
    InvalidClass,
    PolicyOverflow,
}

pub(crate) struct IdentityReplaceFailure<'cleanup, T, E> {
    pub(crate) payload: StoredPayload<'cleanup, T, E>,
    pub(crate) policy_bytes: u64,
    pub(crate) reason: IdentityReplaceReason,
}

impl<'cleanup, K, T> InstallError<'cleanup, K, T> {
    /// Why it wasn't installed.
    pub fn reason(&self) -> InstallErrorReason {
        self.reason
    }
    /// The key, the backing and the policy bytes that were offered, handed back.
    pub fn into_parts(self) -> (K, Backing<'cleanup, T>, u64) {
        (self.key, self.backing, self.policy_bytes)
    }
}
impl<K, T> fmt::Debug for InstallError<'_, K, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstallError")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Single-owner typed cache with synchronous operations and no table lock.
///
/// Keys use semantic `Hash` and `Eq`; collisions never substitute for equality.
/// Stored key hashing and equality must remain stable during membership, as for
/// a standard `HashMap`; changing them is a logical error, never memory unsafety.
/// Borrowed lookup protects backing through the cache borrow, while sharing or
/// taking retains independent backing that survives replacement and removal.
/// Mutations require exclusive access. Key callbacks and payload cleanup execute
/// on the caller. This facility creates membership, not production or demand.
/// Backing accounting and deferred cleanup can acquire their own bookkeeping
/// locks; table operations do not promise lock-free execution.
///
/// Each ready slot contributes its supplied policy bytes independently. Aliased
/// backing has one physical charge, but multiple slot owners conservatively pin
/// one another against collection. Slots retain peak membership capacity; bounded
/// maintenance counts empty slots as work rather than hiding an unbounded scan.
pub struct Cache<'cleanup, K, T, E = ()> {
    pub(super) storage: Storage<'cleanup, K, T, E>,
    pub(super) policy_bytes: u64,
}

impl<'cleanup, K, T, E> Cache<'cleanup, K, T, E> {
    /// An empty cache.
    pub fn new() -> Self {
        Self {
            storage: Storage::new(),
            policy_bytes: 0,
        }
    }
    /// How many entries it has.
    pub fn len(&self) -> usize {
        self.storage.slots.len() - self.storage.free.len()
    }
    /// Whether it has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Reports retained slot policy cost, independently of physical accounting.
    pub fn policy_bytes(&self) -> u64 {
        self.policy_bytes
    }
    /// Tests a handle without running key callbacks. Foreign and stale handles fail.
    pub fn contains_identity(&self, identity: &Identity<K>) -> bool {
        self.entry_identity(identity).is_some()
    }
    fn entry_identity(&self, identity: &Identity<K>) -> Option<&Entry<'cleanup, K, T, E>> {
        if !Arc::ptr_eq(&self.storage.namespace, &identity.token.namespace) {
            return None;
        }
        self.storage
            .slots
            .get(identity.token.slot)?
            .as_ref()
            .filter(|entry| entry.identity == *identity)
    }
    /// Borrows ready backing by handle; foreign or removed membership is absent.
    pub fn lookup_identity(&self, identity: &Identity<K>) -> Lookup<'_, View<'_, T>, E> {
        observe(self.entry_identity(identity))
    }

    /// Acquires ready backing by identity without invoking key callbacks.
    /// Foreign or removed membership is absent; the returned owner survives
    /// subsequent replacement, invalidation and table removal.
    pub fn share_identity(&self, identity: &Identity<K>) -> Lookup<'_, Backing<'cleanup, T>, E> {
        match self.entry_identity(identity).map(|entry| &entry.state) {
            None => Lookup::Absent,
            Some(StoredPayload::Vacant) => Lookup::Vacant,
            Some(StoredPayload::Failed(error)) => Lookup::Failed(error),
            Some(StoredPayload::Ready(backing)) => Lookup::Ready(backing.clone()),
        }
    }

    /// Publication commits against a protected identity after validating its
    /// authority. No key callbacks or displaced payload destructors run here.
    /// All fallible checks precede changes; failure returns the original input.
    pub(crate) fn replace_identity(
        &mut self,
        identity: &Identity<K>,
        payload: StoredPayload<'cleanup, T, E>,
        policy_bytes: u64,
    ) -> Result<StoredPayload<'cleanup, T, E>, IdentityReplaceFailure<'cleanup, T, E>> {
        let Some(entry) = self.entry_identity(identity) else {
            return Err(IdentityReplaceFailure {
                payload,
                policy_bytes,
                reason: IdentityReplaceReason::Missing,
            });
        };
        let ready = if let StoredPayload::Ready(backing) = &payload {
            if backing.allocation_class() != AllocationClass::Resident {
                return Err(IdentityReplaceFailure {
                    payload,
                    policy_bytes,
                    reason: IdentityReplaceReason::InvalidClass,
                });
            }
            true
        } else {
            false
        };
        let next_cost = if ready { policy_bytes } else { 0 };
        let Some(total) = (self.policy_bytes - entry.policy_bytes).checked_add(next_cost) else {
            return Err(IdentityReplaceFailure {
                payload,
                policy_bytes,
                reason: IdentityReplaceReason::PolicyOverflow,
            });
        };
        let entry = self.storage.slots[identity.token.slot]
            .as_mut()
            .expect("validated identity remains present under exclusive access");
        let previous = std::mem::replace(&mut entry.state, payload);
        entry.policy_bytes = next_cost;
        self.policy_bytes = total;
        Ok(previous)
    }
}
impl<K, T, E> Default for Cache<'_, K, T, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'cleanup, K: Eq + Hash, T, E> Cache<'cleanup, K, T, E> {
    /// Creates vacant membership only; an existing failure or payload is preserved.
    pub fn ensure(&mut self, key: K) -> Identity<K> {
        let hash = self.storage.keys.hasher().hash_one(&key);
        let slot = self.find_hashed(hash, &key).map(|(slot, _)| slot);
        self.ensure_hashed(key, hash, slot)
    }

    fn find_hashed(&self, hash: u64, key: &K) -> Option<(usize, Option<usize>)> {
        let mut slot = self.storage.keys.get(&hash).copied();
        let mut previous = None;
        while let Some(index) = slot {
            let entry = self.storage.slots[index]
                .as_ref()
                .expect("bucket owns a live slot");
            if entry.key == *key {
                return Some((index, previous));
            }
            previous = Some(index);
            slot = entry.next;
        }
        None
    }

    fn find(&self, key: &K) -> Option<usize> {
        let hash = self.storage.keys.hasher().hash_one(key);
        self.find_hashed(hash, key).map(|(slot, _)| slot)
    }

    fn ensure_hashed(&mut self, key: K, hash: u64, existing: Option<usize>) -> Identity<K> {
        if let Some(slot) = existing {
            // An incoming duplicate key may run arbitrary Drop; do it before mutation.
            drop(key);
            return self.storage.slots[slot]
                .as_ref()
                .expect("key owns a live slot")
                .identity
                .clone();
        }
        self.storage.keys.reserve(1);
        if self.storage.free.is_empty() {
            self.storage.slots.reserve(1);
        }
        let slot = self
            .storage
            .free
            .last()
            .copied()
            .unwrap_or(self.storage.slots.len());
        let identity = Identity {
            token: Arc::new(IdentityToken {
                namespace: Arc::clone(&self.storage.namespace),
                slot,
            }),
            key_type: PhantomData,
        };
        let next = self.storage.keys.get(&hash).copied();
        let entry = Some(Entry {
            key,
            next,
            identity: identity.clone(),
            state: StoredPayload::Vacant,
            policy_bytes: 0,
        });
        if slot == self.storage.slots.len() {
            self.storage.slots.push(entry);
        } else {
            self.storage.free.pop();
            self.storage.slots[slot] = entry;
        }
        // Capacity is reserved and the index contains only cached integer hashes.
        // No user callbacks occur while linking a new slot into the index.
        self.storage.keys.insert(hash, slot);
        identity
    }
    /// The identity of `key`, if it has one.
    ///
    /// # Panics
    ///
    /// Panics if a known key has lost its slot, which is a bug in this crate.
    pub fn identity(&self, key: &K) -> Option<Identity<K>> {
        let slot = self.find(key)?;
        Some(
            self.storage.slots[slot]
                .as_ref()
                .expect("key owns a live slot")
                .identity
                .clone(),
        )
    }
    /// Observes readiness without cloning backing or creating demand.
    pub fn lookup(&self, key: &K) -> Lookup<'_, View<'_, T>, E> {
        observe(
            self.find(key)
                .and_then(|slot| self.storage.slots[slot].as_ref()),
        )
    }
    /// Acquires an existing ready owner under a protected cache borrow.
    ///
    /// # Panics
    ///
    /// Panics if a known key has lost its slot, which is a bug in this crate.
    pub fn share(&self, key: &K) -> Lookup<'_, Backing<'cleanup, T>, E> {
        let Some(slot) = self.find(key) else {
            return Lookup::Absent;
        };
        match &self.storage.slots[slot]
            .as_ref()
            .expect("key owns a live slot")
            .state
        {
            StoredPayload::Vacant => Lookup::Vacant,
            StoredPayload::Failed(error) => Lookup::Failed(error),
            StoredPayload::Ready(backing) => Lookup::Ready(backing.clone()),
        }
    }
    /// Moves the table owner and its physical charge, leaving logical membership vacant.
    ///
    /// # Panics
    ///
    /// Panics if a known key has lost its slot, which is a bug in this crate.
    pub fn take(&mut self, key: &K) -> Lookup<'_, Backing<'cleanup, T>, E> {
        let Some(slot) = self.find(key) else {
            return Lookup::Absent;
        };
        let entry = self.storage.slots[slot]
            .as_mut()
            .expect("key owns a live slot");
        if matches!(entry.state, StoredPayload::Ready(_)) {
            self.policy_bytes -= entry.policy_bytes;
            entry.policy_bytes = 0;
            let StoredPayload::Ready(backing) =
                std::mem::replace(&mut entry.state, StoredPayload::Vacant)
            else {
                unreachable!()
            };
            return Lookup::Ready(backing);
        }
        match &entry.state {
            StoredPayload::Failed(error) => Lookup::Failed(error),
            _ => Lookup::Vacant,
        }
    }
    /// Installs resident backing, returning the previous owned state.
    /// Rejection preserves inputs and membership. Escaped old owners stay valid.
    ///
    /// # Errors
    ///
    /// Hands everything back if the class isn't taken or the policy count would overflow.
    ///
    /// # Panics
    ///
    /// Panics if a known key has lost its slot, which is a bug in this crate.
    pub fn install(
        &mut self,
        key: K,
        backing: Backing<'cleanup, T>,
        policy_bytes: u64,
    ) -> InstallResult<'cleanup, K, T, E> {
        if backing.allocation_class() != AllocationClass::Resident {
            return Err(InstallError {
                key,
                backing,
                policy_bytes,
                reason: InstallErrorReason::InvalidClass,
            });
        }
        let hash = self.storage.keys.hasher().hash_one(&key);
        let slot = self.find_hashed(hash, &key).map(|(slot, _)| slot);
        let old_cost = slot.map_or(0, |slot| {
            self.storage.slots[slot]
                .as_ref()
                .expect("key owns a live slot")
                .policy_bytes
        });
        let Some(total) = (self.policy_bytes - old_cost).checked_add(policy_bytes) else {
            return Err(InstallError {
                key,
                backing,
                policy_bytes,
                reason: InstallErrorReason::PolicyOverflow,
            });
        };
        let identity = self.ensure_hashed(key, hash, slot);
        let entry = self.storage.slots[identity.token.slot]
            .as_mut()
            .expect("ensured slot is live");
        let previous = std::mem::replace(&mut entry.state, StoredPayload::Ready(backing));
        entry.policy_bytes = policy_bytes;
        self.policy_bytes = total;
        Ok((identity, previous))
    }
    /// Stores a domain failure explicitly; failure membership is never a usable hit.
    pub fn fail(&mut self, key: K, error: E) -> (Identity<K>, StoredPayload<'cleanup, T, E>) {
        let identity = self.ensure(key);
        let previous = self.replace_slot(identity.token.slot, StoredPayload::Failed(error));
        (identity, previous)
    }
    /// Detaches payload or failure without removing the identity.
    pub fn clear_payload(&mut self, key: &K) -> Option<StoredPayload<'cleanup, T, E>> {
        let slot = self.find(key)?;
        Some(self.replace_slot(slot, StoredPayload::Vacant))
    }
    fn replace_slot(
        &mut self,
        slot: usize,
        state: StoredPayload<'cleanup, T, E>,
    ) -> StoredPayload<'cleanup, T, E> {
        let entry = self.storage.slots[slot]
            .as_mut()
            .expect("key owns a live slot");
        self.policy_bytes -= entry.policy_bytes;
        entry.policy_bytes = 0;
        std::mem::replace(&mut entry.state, state)
    }
    /// Removes membership and returns its state. Independent owners survive removal.
    pub fn remove(&mut self, key: &K) -> Option<(Identity<K>, StoredPayload<'cleanup, T, E>)> {
        let entry = self.remove_entry(key)?;
        // The table and counters are coherent before arbitrary key destruction.
        drop(entry.key);
        Some((entry.identity, entry.state))
    }

    /// Detaches membership without running key or payload destructors. Layered
    /// coordinators can settle their own metadata before user cleanup can unwind.
    /// Hash/Eq callbacks still run before any membership change.
    pub(crate) fn remove_entry(&mut self, key: &K) -> Option<Entry<'cleanup, K, T, E>> {
        let hash = self.storage.keys.hasher().hash_one(key);
        let (slot, previous) = self.find_hashed(hash, key)?;
        self.storage.free.reserve(1);
        let entry = self.storage.slots[slot]
            .take()
            .expect("removed key owns a live slot");
        if let Some(previous) = previous {
            self.storage.slots[previous]
                .as_mut()
                .expect("chain predecessor is live")
                .next = entry.next;
        } else if let Some(next) = entry.next {
            self.storage.keys.insert(hash, next);
        } else {
            self.storage.keys.remove(&hash);
        }
        self.policy_bytes -= entry.policy_bytes;
        self.storage.free.push(slot);
        Some(entry)
    }
}

fn observe<'entry, K, T, E>(
    entry: Option<&'entry Entry<'_, K, T, E>>,
) -> Lookup<'entry, View<'entry, T>, E> {
    match entry.map(|entry| &entry.state) {
        None => Lookup::Absent,
        Some(StoredPayload::Vacant) => Lookup::Vacant,
        Some(StoredPayload::Failed(error)) => Lookup::Failed(error),
        Some(StoredPayload::Ready(backing)) => Lookup::Ready(backing.view()),
    }
}
