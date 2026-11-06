//! Checks a workspace model against the rules.

use std::fmt;

use super::rules::{
    BACKEND_RULES, CRATE_RULES, CrateRule, FORMAT_RULES, REQUIRED_EDITION, REQUIRED_RUST_VERSION,
};
use super::{Kind, WorkspacePackage};

/// One way a workspace breaks the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A workspace package has no rule, so its place in the graph is undecided.
    UnclassifiedPackage {
        /// The package name.
        package: String,
    },
    /// A rule names a package the workspace does not contain.
    MissingPackage {
        /// The package name.
        package: &'static str,
    },
    /// A package depends on a workspace package its rule does not permit.
    ForbiddenDependency {
        /// The depending package.
        package: String,
        /// The depended-on workspace package.
        dependency: String,
        /// The declaring section.
        kind: Kind,
    },
    /// A package depends on a backend that is private to another crate.
    MisplacedBackend {
        /// The depending package.
        package: String,
        /// The backend package.
        dependency: String,
        /// The crate that owns the backend.
        owner: &'static str,
    },
    /// A package depends on a format library it has no format for.
    MisplacedFormat {
        /// The depending package.
        package: String,
        /// The format library.
        dependency: String,
    },
    /// A test-only feature is enabled outside a development dependency.
    TestingFeatureInBuild {
        /// The package enabling it.
        package: String,
        /// The crate whose feature it is.
        dependency: String,
        /// The feature.
        feature: &'static str,
    },
    /// A package does not inherit the shared edition.
    WrongEdition {
        /// The package name.
        package: String,
        /// Its edition.
        edition: String,
    },
    /// A package does not declare the shared minimum Rust version.
    WrongRustVersion {
        /// The package name.
        package: String,
        /// Its declared version, if any.
        rust_version: Option<String>,
    },
    /// A package could be published to a registry.
    Publishable {
        /// The package name.
        package: String,
    },
    /// A package does not inherit the workspace lints, so it builds with the
    /// defaults only.
    LintsNotInherited {
        /// The package name.
        package: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclassifiedPackage { package } => {
                write!(
                    f,
                    "`{package}` has no crate rule; add one through a proposal"
                )
            }
            Self::MissingPackage { package } => {
                write!(f, "crate rule `{package}` names no workspace package")
            }
            Self::ForbiddenDependency {
                package,
                dependency,
                kind,
            } => write!(
                f,
                "`{package}` must not depend on `{dependency}` ({kind:?} dependency)"
            ),
            Self::MisplacedBackend {
                package,
                dependency,
                owner,
            } => write!(
                f,
                "`{package}` must not depend on `{dependency}`, a backend private to `{owner}`"
            ),
            Self::MisplacedFormat {
                package,
                dependency,
            } => write!(
                f,
                "`{package}` must not depend on `{dependency}`; only crates with a format of their own may"
            ),
            Self::TestingFeatureInBuild {
                package,
                dependency,
                feature,
            } => write!(
                f,
                "`{package}` enables `{dependency}/{feature}`, which only development dependencies may"
            ),
            Self::WrongEdition { package, edition } => write!(
                f,
                "`{package}` uses edition {edition}, not the workspace edition {REQUIRED_EDITION}"
            ),
            Self::WrongRustVersion {
                package,
                rust_version,
            } => write!(
                f,
                "`{package}` declares rust-version {rust_version:?}, not {REQUIRED_RUST_VERSION}"
            ),
            Self::Publishable { package } => {
                write!(f, "`{package}` is publishable; inherit `publish = false`")
            }
            Self::LintsNotInherited { package } => write!(
                f,
                "`{package}` does not inherit the workspace lints; add `[lints] workspace = true`"
            ),
        }
    }
}

/// Checks `packages`, the whole workspace, against the crate rules, the
/// backend rules and the shared package settings. Returns every violation,
/// so one run reports all of them.
#[must_use]
pub fn check_workspace(packages: &[WorkspacePackage]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for rule in CRATE_RULES {
        if !packages.iter().any(|package| package.name == rule.name) {
            violations.push(Violation::MissingPackage { package: rule.name });
        }
    }
    for package in packages {
        check_settings(package, &mut violations);
        let Some(rule) = rule_for(&package.name) else {
            violations.push(Violation::UnclassifiedPackage {
                package: package.name.clone(),
            });
            continue;
        };
        check_dependencies(package, rule, packages, &mut violations);
    }
    violations
}

/// Finds the rule for a package name.
fn rule_for(name: &str) -> Option<&'static CrateRule> {
    CRATE_RULES.iter().find(|rule| rule.name == name)
}

/// Checks the settings every package inherits from the workspace.
fn check_settings(package: &WorkspacePackage, violations: &mut Vec<Violation>) {
    if package.edition != REQUIRED_EDITION {
        violations.push(Violation::WrongEdition {
            package: package.name.clone(),
            edition: package.edition.clone(),
        });
    }
    if package.rust_version.as_deref() != Some(REQUIRED_RUST_VERSION) {
        violations.push(Violation::WrongRustVersion {
            package: package.name.clone(),
            rust_version: package.rust_version.clone(),
        });
    }
    if package.publishable {
        violations.push(Violation::Publishable {
            package: package.name.clone(),
        });
    }
    if !package.inherits_workspace_lints {
        violations.push(Violation::LintsNotInherited {
            package: package.name.clone(),
        });
    }
}

/// Checks one package's declared dependencies. Every dependency kind is held
/// to the same rule: a development dependency on a forbidden crate would still
/// couple the crates' tests to each other's internals. A package's
/// development dependency on itself (to enable its own test features) is
/// allowed.
fn check_dependencies(
    package: &WorkspacePackage,
    rule: &CrateRule,
    packages: &[WorkspacePackage],
    violations: &mut Vec<Violation>,
) {
    for dependency in &package.dependencies {
        let is_workspace = packages.iter().any(|other| other.name == dependency.name);
        let is_own_tests = dependency.name == package.name && dependency.kind == Kind::Development;
        if is_workspace && !is_own_tests && !rule.may_depend_on.contains(&dependency.name.as_str())
        {
            violations.push(Violation::ForbiddenDependency {
                package: package.name.clone(),
                dependency: dependency.name.clone(),
                kind: dependency.kind,
            });
        }
        let backend = BACKEND_RULES
            .iter()
            .find(|backend| backend.dependency == dependency.name);
        if let Some(backend) = backend
            && backend.owner != package.name
        {
            violations.push(Violation::MisplacedBackend {
                package: package.name.clone(),
                dependency: dependency.name.clone(),
                owner: backend.owner,
            });
        }
        let format = FORMAT_RULES
            .iter()
            .find(|format| format.dependency == dependency.name);
        if let Some(format) = format
            && !format.users.contains(&package.name.as_str())
        {
            violations.push(Violation::MisplacedFormat {
                package: package.name.clone(),
                dependency: dependency.name.clone(),
            });
        }
    }
}
