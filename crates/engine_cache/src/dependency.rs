//! Caller-declared revisions with explicit, caller-selected propagation.
//!
//! The domain enumerates every affected dependency when advancing revisions.
//! There are no inferred graph edges, scope relationships or mutation discovery.

use std::fmt;
use std::sync::Arc;

/// Independent validity domains; advancing one does not implicitly advance another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyScope {
    /// It depends on where the data came from.
    Source,
    /// It depends on how the data is represented.
    Representation,
    /// It depends on one instance.
    Instance,
    /// It depends on one view.
    View,
    /// It depends on a device.
    Device,
}

#[derive(Debug)]
struct Token;

/// Opaque registry-scoped dependency identity, distinct from its current revision.
/// Retaining this handle does not retain any input or payload backing.
#[derive(Clone, Debug)]
pub struct Dependency {
    registry: Arc<Token>,
    identity: Arc<Token>,
    scope: DependencyScope,
}

impl Dependency {
    /// What this dependency covers.
    pub fn scope(&self) -> DependencyScope {
        self.scope
    }
}

impl PartialEq for Dependency {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for Dependency {}

#[derive(Clone, Debug)]
struct Revision {
    dependency: Dependency,
    revision: Arc<Token>,
}

/// Coherent captured revisions from one registry, including for an empty set.
///
/// Cloning preserves the same captured revisions, not current authority. This
/// snapshot does not retain backing: callers must separately retain every input.
#[derive(Clone, Debug)]
pub struct DependencySnapshot {
    registry: Arc<Token>,
    revisions: Vec<Revision>,
}

impl DependencySnapshot {
    /// How many dependencies it holds.
    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.revisions.is_empty()
    }
}

/// Why declared dependencies cannot be captured or accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyError {
    /// The dependency belongs to another registry.
    ForeignRegistry,
    /// The dependency isn't known here.
    UnknownDependency,
    /// The dependency has changed since it was read.
    Stale,
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ForeignRegistry => "dependency belongs to another registry",
            Self::UnknownDependency => "dependency was removed from its registry",
            Self::Stale => "captured dependency revision is no longer current",
        })
    }
}
impl std::error::Error for DependencyError {}

/// Single-owner, caller-serialized declared revision registry.
///
/// Operations are synchronous and invoke no user code. Publication must borrow
/// this registry through validation and commit so revisions cannot change in
/// between. Domain mutation and notification require the same caller discipline.
/// Identity and revision tokens never wrap or recycle while captured handles
/// remain alive. Revision changes affect future validity, never old backing.
///
/// Registry lookups scan declarations linearly. For `n` selected inputs and `r`
/// declarations, snapshot/advance perform O(n*r + n*n) bookkeeping, including
/// selected-set deduplication. Snapshots allocate a vector and clone shared
/// tokens; advance captures two snapshots and allocates one fresh revision token
/// per distinct input. Validation allocates nothing and performs O(n*r) lookups.
/// Registration allocates identity/revision tokens and may grow the registry
/// vector. These costs do not establish a wall-clock service bound.
#[derive(Debug)]
pub struct Dependencies {
    registry: Arc<Token>,
    revisions: Vec<Revision>,
}

impl Default for Dependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl Dependencies {
    /// A registry with nothing in it.
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Token),
            revisions: Vec::new(),
        }
    }

    /// Declares a fresh identity; scope describes meaning, not propagation edges.
    pub fn register(&mut self, scope: DependencyScope) -> Dependency {
        let dependency = Dependency {
            registry: Arc::clone(&self.registry),
            identity: Arc::new(Token),
            scope,
        };
        self.revisions.push(Revision {
            dependency: dependency.clone(),
            revision: Arc::new(Token),
        });
        dependency
    }

    /// Captures the selected set coherently, deduplicating repeated identities.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency is foreign to this registry or isn't known.
    pub fn snapshot(
        &self,
        dependencies: &[Dependency],
    ) -> Result<DependencySnapshot, DependencyError> {
        let mut revisions: Vec<Revision> = Vec::new();
        for dependency in dependencies {
            let index = self.index(dependency)?;
            if !revisions
                .iter()
                .any(|entry| entry.dependency == *dependency)
            {
                revisions.push(self.revisions[index].clone());
            }
        }
        Ok(DependencySnapshot {
            registry: Arc::clone(&self.registry),
            revisions,
        })
    }

    /// Checks all declared revisions without acquiring any payload owner.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency is foreign, unknown, or has changed.
    pub fn validate(&self, snapshot: &DependencySnapshot) -> Result<(), DependencyError> {
        if !Arc::ptr_eq(&self.registry, &snapshot.registry) {
            return Err(DependencyError::ForeignRegistry);
        }
        for captured in &snapshot.revisions {
            let index = self.index(&captured.dependency)?;
            if !Arc::ptr_eq(&self.revisions[index].revision, &captured.revision) {
                return Err(DependencyError::Stale);
            }
        }
        Ok(())
    }

    /// Advances exactly the caller-selected affected set and returns new revisions.
    ///
    /// All inputs are checked before any change. Duplicate handles advance once.
    /// Callers explicitly include dependent representations/instances/views when
    /// their domain rules require propagation. No payload is destroyed here.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency is foreign or isn't known, and advances nothing.
    pub fn advance(
        &mut self,
        dependencies: &[Dependency],
    ) -> Result<DependencySnapshot, DependencyError> {
        let captured = self.snapshot(dependencies)?;
        for entry in &captured.revisions {
            // snapshot verified every identity; no mutation removes entries here.
            let index = self.index(&entry.dependency)?;
            self.revisions[index].revision = Arc::new(Token);
        }
        self.snapshot(dependencies)
    }

    /// Removes one declaration. Re-registering its scope creates a fresh identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the dependency is foreign or isn't known.
    pub fn remove(&mut self, dependency: &Dependency) -> Result<(), DependencyError> {
        let index = self.index(dependency)?;
        self.revisions.remove(index);
        Ok(())
    }

    fn index(&self, dependency: &Dependency) -> Result<usize, DependencyError> {
        if !Arc::ptr_eq(&self.registry, &dependency.registry) {
            return Err(DependencyError::ForeignRegistry);
        }
        self.revisions
            .iter()
            .position(|entry| entry.dependency == *dependency)
            .ok_or(DependencyError::UnknownDependency)
    }
}
