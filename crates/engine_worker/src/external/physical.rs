//! Owned resources retained across access by an external provider or device.

use super::PhysicalRetention;
use crate::execution::context::ControlCallbackGuard;
use crate::scheduler::SpawnError;
use crate::scheduler::reservation::ByteLease;
use crate::task::Retained;
use std::sync::{Arc, Mutex};

/// An admitted resource that has not been exposed to a foreign accessor.
///
/// The provider may return it safely if submission fails before access starts.
/// Activation is the explicit boundary after which release needs proof from the
/// provider. `T` must be transferable because final cleanup may run on another
/// thread after the originating runtime has gone away.
/// Destruction rejects passive runtime waits until its retained work is released.
pub struct ExternalPrepared<T: Send + 'static> {
    resource: PreparedResource<T>,
    bytes: Option<ByteLease>,
    retention: PhysicalRetention,
}

/// First field of a prepared operation: resource cleanup precedes byte and
/// lifetime-accounting release, including unwinding through implicit Drop.
struct PreparedResource<T>(Option<Box<T>>);

impl<T> PreparedResource<T> {
    fn into_inner(mut self) -> Box<T> {
        self.0.take().expect("prepared resource")
    }
}

impl<T> Drop for PreparedResource<T> {
    fn drop(&mut self) {
        if let Some(resource) = self.0.take() {
            let _guard = ControlCallbackGuard::enter();
            drop(resource);
        }
    }
}

impl<T: Send + 'static> ExternalPrepared<T> {
    pub(crate) fn new(resource: T, bytes: Option<ByteLease>, retention: PhysicalRetention) -> Self {
        Self {
            resource: PreparedResource(Some(Box::new(resource))),
            bytes,
            retention,
        }
    }

    /// Borrows the resource while no foreign access is permitted.
    ///
    /// # Panics
    ///
    /// Panics if the handle has lost what it holds, which is a bug in this crate.
    pub fn get(&self) -> &T {
        self.resource.0.as_deref().expect("prepared resource")
    }

    /// Mutates the resource while no foreign access is permitted.
    ///
    /// # Panics
    ///
    /// Panics if the handle has lost what it holds, which is a bug in this crate.
    pub fn get_mut(&mut self) -> &mut T {
        self.resource.0.as_deref_mut().expect("prepared resource")
    }

    /// Transfers a resource that was never activated into ordinary retained
    /// ownership. Its declared byte charge follows the returned value.
    pub fn into_retained(self) -> Retained<T> {
        let Self {
            resource,
            bytes,
            retention,
        } = self;
        let retained = Retained::new(*resource.into_inner(), bytes);
        drop(retention);
        retained
    }

    /// Registers active physical access and fixes the resource's address.
    /// Rejection returns the complete prepared resource to the caller.
    ///
    /// # Errors
    ///
    /// Hands the resource back, still prepared, with why, if it can't be activated.
    pub fn activate(self) -> Result<ExternalAccess<T>, ExternalActivationRejected<T>> {
        if let Err(reason) = self.retention.activate() {
            return Err(ExternalActivationRejected {
                reason,
                prepared: self,
            });
        }

        let inner = Arc::new(PhysicalCapsule {
            state: Mutex::new(PhysicalState {
                resource: Some(self.resource.into_inner()),
                bytes: self.bytes,
                retention: Some(Arc::new(self.retention)),
                anchor: None,
            }),
        });
        // The capsule owns one strong reference to itself before the ticket can
        // expose an address. Losing the ticket cannot free a foreign live region.
        inner.state.lock().unwrap_or_else(|e| e.into_inner()).anchor = Some(Arc::clone(&inner));
        Ok(ExternalAccess { inner: Some(inner) })
    }
}

/// Activation rejection with the original, still unexposed resource.
pub struct ExternalActivationRejected<T: Send + 'static> {
    /// Why it wasn't activated.
    pub reason: SpawnError,
    /// The resource, still prepared.
    pub prepared: ExternalPrepared<T>,
}

impl<T: Send + 'static> std::fmt::Debug for ExternalActivationRejected<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalActivationRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Exclusive release authority for a physically active external resource.
///
/// Dropping this ticket records an orphan and retains its resource, byte charge,
/// and physical registration. A provider must retain the ticket until it can
/// prove all foreign access has ended, including access by a device or callbacks.
/// Logical completion, cancellation, and timeout are not release proofs.
pub struct ExternalAccess<T: Send + 'static> {
    inner: Option<Arc<PhysicalCapsule<T>>>,
}

struct PhysicalCapsule<T: Send + 'static> {
    state: Mutex<PhysicalState<T>>,
}

struct PhysicalState<T: Send + 'static> {
    resource: Option<Box<T>>,
    bytes: Option<ByteLease>,
    retention: Option<Arc<PhysicalRetention>>,
    anchor: Option<Arc<PhysicalCapsule<T>>>,
}

impl<T: Send + 'static> ExternalAccess<T> {
    /// Returns the stable address of the active resource for an external adapter.
    ///
    /// # Safety
    ///
    /// The adapter must prevent moves, reallocation, and conflicting aliases of
    /// every foreign-accessed region of `T`. The pointer must only be used while
    /// this physical operation remains active. In particular, a `Box<T>` fixes
    /// the address of `T` but does not fix allocations referenced by `T`.
    /// Repeated pointer requests must not create conflicting Rust aliases while
    /// the provider accesses the resource.
    ///
    /// # Panics
    ///
    /// Panics if the handle has lost what it holds, which is a bug in this crate.
    pub unsafe fn as_mut_ptr(&self) -> *mut T {
        let inner = self.inner.as_ref().expect("active external ticket");
        let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
        std::ptr::from_mut(
            state
                .resource
                .as_deref_mut()
                .expect("active external resource"),
        )
    }

    /// Transfers the resource and its declared byte charge into CPU-visible
    /// retained ownership after physical release has been proved.
    ///
    /// # Safety
    ///
    /// Every foreign or device accessor, including callbacks and queued uses,
    /// must have ended. The provider must establish all memory visibility needed
    /// before Rust reads or destroys `T`. A failure, cancellation, timeout, or
    /// unrelated completion counter does not establish this condition.
    ///
    /// # Panics
    ///
    /// Panics if the handle has lost what it holds, which is a bug in this crate.
    pub unsafe fn acknowledge_release(mut self) -> Retained<T> {
        let inner = self.inner.take().expect("active external ticket");
        let (resource, bytes, retention, anchor) = inner.take_release();
        let retained = Retained::new(*resource, bytes);
        // The physical count retires only after the retained handoff exists.
        drop(retention);
        drop(anchor);
        retained
    }

    /// Destroys the resource after physical release has been proved.
    /// Destruction participates in runtime cleanup: passive waits and terminal
    /// joins are rejected. A destructor panic propagates after accounting settles.
    ///
    /// # Safety
    ///
    /// The same obligations as [`Self::acknowledge_release`] apply. The resource
    /// destructor must be legal on this thread; thread-affine destruction belongs
    /// in a provider-owned service instead.
    ///
    /// # Panics
    ///
    /// Panics if the handle has lost what it holds, which is a bug in this crate.
    pub unsafe fn release(mut self) {
        let _guard = ControlCallbackGuard::enter();
        let inner = self.inner.take().expect("active external ticket");
        let (resource, bytes, retention, anchor) = inner.take_release();
        // A user destructor can panic. Finish the accounting transition only
        // after its stack has unwound, then resume the original panic.
        let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(resource)));
        drop(bytes);
        drop(retention);
        drop(anchor);
        if let Err(panic) = cleanup {
            std::panic::resume_unwind(panic);
        }
    }
}

impl<T: Send + 'static> Drop for ExternalAccess<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
            let retention = state.retention.as_ref().map(Arc::clone);
            drop(state);
            if let Some(retention) = retention {
                retention.mark_orphaned();
            }
            // The capsule's self-anchor intentionally survives ticket loss.
        }
    }
}

impl<T: Send + 'static> PhysicalCapsule<T> {
    fn take_release(
        &self,
    ) -> (
        Box<T>,
        Option<ByteLease>,
        Arc<PhysicalRetention>,
        Arc<PhysicalCapsule<T>>,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        (
            state.resource.take().expect("active external resource"),
            state.bytes.take(),
            state
                .retention
                .take()
                .expect("active physical registration"),
            state.anchor.take().expect("active physical anchor"),
        )
    }
}
