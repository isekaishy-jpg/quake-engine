//! Owner-driven publication with retained readers and declared validity.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use crate::cache::{
    Cache, Identity, IdentityReplaceReason, InstallErrorReason, Lookup, RetentionReport,
    StoredPayload,
};
use crate::dependency::{Dependencies, DependencyError, DependencySnapshot};
use crate::ownership::{Backing, View};
use crate::policy::ByteLimit;

#[cfg(test)]
#[path = "../tests/unit/publication.rs"]
mod tests;

struct State {
    authority: Arc<()>,
    round: Option<Arc<()>>,
    validity: Option<DependencySnapshot>,
}

/// Local authority attachment, not proof of transport provenance.
/// Providers must bind this token to the actual delivery context before switching
/// authorities, or settle the old stream first.
pub struct Authority<K> {
    identity: Identity<K>,
    token: Arc<()>,
}
impl<K> Clone for Authority<K> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            token: Arc::clone(&self.token),
        }
    }
}

/// Captured owner attachment and declared dependencies for one publication round.
/// Clone only when the domain permits independent candidates: clones contend for
/// one winner. The first successful publish closes the round; cloning does not
/// start production or create a new round. Pair each ticket with its own production outcome. Publication must
/// receive an owned outcome claimed after all producer accesses have ended.
pub struct PublicationAttempt<K> {
    authority: Authority<K>,
    round: Arc<()>,
    validity: DependencySnapshot,
}
impl<K> Clone for PublicationAttempt<K> {
    fn clone(&self) -> Self {
        Self {
            authority: self.authority.clone(),
            round: Arc::clone(&self.round),
            validity: self.validity.clone(),
        }
    }
}

/// Why something wasn't published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationRejectionReason {
    /// Nothing is known of the identity.
    Missing,
    /// Authority over it has moved on.
    AuthorityChanged,
    /// A newer attempt has replaced this one.
    ObsoleteAttempt,
    /// A dependency stopped it.
    Dependency(DependencyError),
    /// The cache wouldn't take it.
    Install(InstallErrorReason),
}

/// Rejection preserves the supplied owned payload and policy cost for caller cleanup.
pub struct PublicationRejected<'cleanup, T, E> {
    payload: Result<Backing<'cleanup, T>, E>,
    policy_bytes: u64,
    reason: PublicationRejectionReason,
}
impl<'cleanup, T, E> PublicationRejected<'cleanup, T, E> {
    /// Why it wasn't published.
    pub fn reason(&self) -> PublicationRejectionReason {
        self.reason
    }
    /// What was offered, and its policy bytes, handed back.
    pub fn into_parts(self) -> (Result<Backing<'cleanup, T>, E>, u64) {
        (self.payload, self.policy_bytes)
    }
}
impl<T, E> fmt::Debug for PublicationRejected<'_, T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Single-owner cache coordinator. Exclusive mutation makes validation and install
/// one protected phase without a publication lock or delivery executor. No user
/// callback or destructor runs between successful install and authority commit.
/// Invalidating a declared dependency prevents subsequent lookup/publication but
/// does not revoke escaped readers. Domain code explicitly chooses propagation.
pub struct Publication<'cleanup, K, T, E = ()> {
    cache: Cache<'cleanup, K, T, E>,
    states: HashMap<Identity<K>, State>,
}
impl<'cleanup, K, T, E> Publication<'cleanup, K, T, E> {
    /// An empty publication.
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            states: HashMap::new(),
        }
    }
    /// How many policy bytes it holds.
    pub fn policy_bytes(&self) -> u64 {
        self.cache.policy_bytes()
    }
    /// Runs the existing bounded retention service on the caller. Eviction keeps
    /// attempt attachment and logical identity; vacant entries have no validity
    /// requirement until a new payload is successfully installed.
    pub fn maintain(&mut self, limit: ByteLimit, max_slots: usize, force: bool) -> RetentionReport {
        self.cache.maintain(limit, max_slots, force)
    }
    /// Consumes validity bookkeeping. The returned raw cache no longer checks it.
    pub fn into_cache(self) -> Cache<'cleanup, K, T, E> {
        self.cache
    }
    /// Current authority over `identity`, if it's known.
    pub fn authority(&self, identity: &Identity<K>) -> Option<Authority<K>> {
        if !self.cache.contains_identity(identity) {
            return None;
        }
        self.states.get(identity).map(|state| Authority {
            identity: identity.clone(),
            token: Arc::clone(&state.authority),
        })
    }
    /// Starts a fresh round without erasing the existing usable payload.
    /// Capturing current inputs and revisions is the caller's responsibility;
    /// this operation replaces the attachment even if the supplied snapshot is
    /// stale. Declared validity is checked when publishing the owned outcome.
    /// This is not a join operation. With `ProductionMap::begin_shared`, call
    /// this only for `Started` and before provider submission; a `Joined` caller
    /// must preserve the existing round. Starting a round does not replace or
    /// cancel the production map's associated work. Never attach a fresh ticket
    /// to an old outcome to bypass its captured dependency validity.
    pub fn begin_attempt(
        &mut self,
        identity: &Identity<K>,
        validity: DependencySnapshot,
    ) -> Option<PublicationAttempt<K>> {
        if !self.cache.contains_identity(identity) {
            return None;
        }
        let state = self.states.get_mut(identity)?;
        let round = Arc::new(());
        state.round = Some(Arc::clone(&round));
        Some(PublicationAttempt {
            authority: Authority {
                identity: identity.clone(),
                token: Arc::clone(&state.authority),
            },
            round,
            validity,
        })
    }
    fn validate_stored(
        &self,
        identity: &Identity<K>,
        dependencies: &Dependencies,
    ) -> Result<(), DependencyError> {
        if matches!(
            self.cache.lookup_identity(identity),
            Lookup::Absent | Lookup::Vacant
        ) {
            return Ok(());
        }
        if let Some(validity) = self
            .states
            .get(identity)
            .and_then(|state| state.validity.as_ref())
        {
            dependencies.validate(validity)?;
        }
        Ok(())
    }
    /// What's published for `identity`, as it stands.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency of what's published has changed.
    pub fn lookup(
        &self,
        identity: &Identity<K>,
        dependencies: &Dependencies,
    ) -> Result<Lookup<'_, View<'_, T>, E>, DependencyError> {
        self.validate_stored(identity, dependencies)?;
        Ok(self.cache.lookup_identity(identity))
    }
    /// What's published for `identity`, as a share that keeps it alive.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency of what's published has changed.
    pub fn share(
        &self,
        identity: &Identity<K>,
        dependencies: &Dependencies,
    ) -> Result<Lookup<'_, Backing<'cleanup, T>, E>, DependencyError> {
        self.validate_stored(identity, dependencies)?;
        Ok(self.cache.share_identity(identity))
    }
    /// Clears payload and attempt attachment while preserving delivery authority.
    /// Untagged updates vouched for by the caller may repopulate this membership.
    pub fn clear(&mut self, identity: &Identity<K>) -> Option<StoredPayload<'cleanup, T, E>> {
        let state = self.states.get_mut(identity)?;
        let previous = self
            .cache
            .replace_identity(identity, StoredPayload::Vacant, 0)
            .ok()?;
        state.round = None;
        state.validity = None;
        Some(previous)
    }
    /// Replaces the source/delivery authority and clears payload. Old tagged
    /// deliveries and attempt tickets are rejected; old backing readers survive.
    ///
    /// # Panics
    ///
    /// Panics if a cleared member has gone, which is a bug in this crate.
    pub fn replace_authority(
        &mut self,
        identity: &Identity<K>,
    ) -> Option<StoredPayload<'cleanup, T, E>> {
        let authority = Arc::new(());
        let previous = self.clear(identity)?;
        self.states
            .get_mut(identity)
            .expect("cleared membership exists")
            .authority = authority;
        Some(previous)
    }
    /// Publishes `outcome` under `authority`, if that authority still holds.
    ///
    /// # Errors
    ///
    /// Hands the outcome back if the identity is gone, authority has moved on, a newer
    /// attempt replaced this one, or a dependency or the cache turned it down.
    pub fn publish(
        &mut self,
        attempt: &PublicationAttempt<K>,
        dependencies: &Dependencies,
        payload: Result<Backing<'cleanup, T>, E>,
        policy_bytes: u64,
    ) -> Result<StoredPayload<'cleanup, T, E>, PublicationRejected<'cleanup, T, E>> {
        let identity = &attempt.authority.identity;
        let rejection = match self.states.get(identity) {
            _ if !self.cache.contains_identity(identity) => {
                Some(PublicationRejectionReason::Missing)
            }
            None => Some(PublicationRejectionReason::Missing),
            Some(state) if !Arc::ptr_eq(&state.authority, &attempt.authority.token) => {
                Some(PublicationRejectionReason::AuthorityChanged)
            }
            Some(state)
                if !state
                    .round
                    .as_ref()
                    .is_some_and(|round| Arc::ptr_eq(round, &attempt.round)) =>
            {
                Some(PublicationRejectionReason::ObsoleteAttempt)
            }
            _ => None,
        };
        if let Some(reason) = rejection {
            return Err(PublicationRejected {
                payload,
                policy_bytes,
                reason,
            });
        }
        self.install(
            identity,
            dependencies,
            &attempt.validity,
            payload,
            policy_bytes,
        )
    }
    /// Applies an authoritative update independently of attempt completion.
    /// `None` means the caller vouches for the current delivery authority; `engine_cache`
    /// cannot distinguish old untagged transport messages or invent provenance.
    /// A successful update revokes pending attempt attachment. Rejection changes
    /// neither payload nor attachment and returns the supplied outcome intact.
    /// The validity stamp is borrowed so a rejected update can retry with its
    /// original revisions; do not recapture newer revisions for an old payload.
    ///
    /// # Errors
    ///
    /// Hands the outcome back if the identity is gone, authority has moved on, or a
    /// dependency or the cache turned it down.
    pub fn authoritative(
        &mut self,
        identity: &Identity<K>,
        authority: Option<&Authority<K>>,
        dependencies: &Dependencies,
        validity: &DependencySnapshot,
        payload: Result<Backing<'cleanup, T>, E>,
        policy_bytes: u64,
    ) -> Result<StoredPayload<'cleanup, T, E>, PublicationRejected<'cleanup, T, E>> {
        let rejection = match self.states.get(identity) {
            _ if !self.cache.contains_identity(identity) => {
                Some(PublicationRejectionReason::Missing)
            }
            None => Some(PublicationRejectionReason::Missing),
            Some(state)
                if authority.is_some_and(|authority| {
                    authority.identity != *identity
                        || !Arc::ptr_eq(&authority.token, &state.authority)
                }) =>
            {
                Some(PublicationRejectionReason::AuthorityChanged)
            }
            _ => None,
        };
        if let Some(reason) = rejection {
            return Err(PublicationRejected {
                payload,
                policy_bytes,
                reason,
            });
        }
        self.install(identity, dependencies, validity, payload, policy_bytes)
    }
    fn install(
        &mut self,
        identity: &Identity<K>,
        dependencies: &Dependencies,
        validity: &DependencySnapshot,
        payload: Result<Backing<'cleanup, T>, E>,
        policy_bytes: u64,
    ) -> Result<StoredPayload<'cleanup, T, E>, PublicationRejected<'cleanup, T, E>> {
        if let Err(error) = dependencies.validate(validity) {
            return Err(PublicationRejected {
                payload,
                policy_bytes,
                reason: PublicationRejectionReason::Dependency(error),
            });
        }
        let validity = validity.clone();
        let stored = match payload {
            Ok(backing) => StoredPayload::Ready(backing),
            Err(error) => StoredPayload::Failed(error),
        };
        let previous = self
            .cache
            .replace_identity(identity, stored, policy_bytes)
            .map_err(|error| PublicationRejected {
                payload: match error.payload {
                    StoredPayload::Ready(backing) => Ok(backing),
                    StoredPayload::Failed(error) => Err(error),
                    StoredPayload::Vacant => unreachable!("production outcome is never vacant"),
                },
                policy_bytes: error.policy_bytes,
                reason: match error.reason {
                    IdentityReplaceReason::Missing => PublicationRejectionReason::Missing,
                    IdentityReplaceReason::InvalidClass => {
                        PublicationRejectionReason::Install(InstallErrorReason::InvalidClass)
                    }
                    IdentityReplaceReason::PolicyOverflow => {
                        PublicationRejectionReason::Install(InstallErrorReason::PolicyOverflow)
                    }
                },
            })?;
        let state = self
            .states
            .get_mut(identity)
            .expect("validated membership exists");
        state.validity = Some(validity);
        state.round = None;
        Ok(previous)
    }
}
impl<K, T, E> Default for Publication<'_, K, T, E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'cleanup, K: Eq + Hash, T, E> Publication<'cleanup, K, T, E> {
    /// The identity of `key`, made if it's new.
    pub fn ensure(&mut self, key: K) -> Identity<K> {
        let identity = self.cache.ensure(key);
        self.states
            .entry(identity.clone())
            .or_insert_with(|| State {
                authority: Arc::new(()),
                round: None,
                validity: None,
            });
        identity
    }
    /// Removes membership. Escaped backing and old production remain owned.
    /// Key callbacks run on the caller. Membership and publication metadata are
    /// detached before key destruction, including when that destructor panics.
    pub fn remove(&mut self, key: &K) -> Option<(Identity<K>, StoredPayload<'cleanup, T, E>)> {
        let entry = self.cache.remove_entry(key)?;
        self.states.remove(&entry.identity);
        drop(entry.key);
        Some((entry.identity, entry.state))
    }
    /// The identity of `key`, if it has one.
    pub fn identity(&self, key: &K) -> Option<Identity<K>> {
        self.cache.identity(key)
    }
}
