//! The crate dependency rules of proposal 0008, enforced on the real
//! workspace, and the checker's ability to see each kind of violation.

use std::collections::HashSet;
use std::ffi::OsStr;

use xtask::crate_graph::{
    BACKEND_RULES, CRATE_RULES, Dependency, Kind, Violation, WorkspacePackage, check_workspace,
    inherits_workspace_lints, load_workspace,
};
use xtask::workspace_root;

/// The permanent test: the workspace as Cargo reports it obeys every rule.
#[test]
fn workspace_obeys_the_crate_rules() {
    let manifest = workspace_root().join("Cargo.toml");
    let packages = load_workspace(OsStr::new(env!("CARGO")), &manifest).expect("cargo metadata");
    let report: Vec<String> = check_workspace(&packages)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        report.is_empty(),
        "crate rule violations:\n{}",
        report.join("\n")
    );
}

/// A well-formed package with the given dependencies.
fn package(name: &str, dependencies: &[(&str, Kind)]) -> WorkspacePackage {
    WorkspacePackage {
        name: name.to_owned(),
        edition: "2024".to_owned(),
        rust_version: Some("1.98".to_owned()),
        publishable: false,
        inherits_workspace_lints: true,
        dependencies: dependencies
            .iter()
            .map(|&(name, kind)| Dependency {
                name: name.to_owned(),
                kind,
                features: Vec::new(),
            })
            .collect(),
        feature_enables: Vec::new(),
    }
}

/// A model of a workspace that has exactly the ruled packages, none of them
/// with dependencies.
fn compliant_workspace() -> Vec<WorkspacePackage> {
    CRATE_RULES
        .iter()
        .map(|rule| package(rule.name, &[]))
        .collect()
}

/// Replaces the package of the same name in `workspace`.
fn replace(workspace: &mut [WorkspacePackage], replacement: WorkspacePackage) {
    let slot = workspace
        .iter_mut()
        .find(|package| package.name == replacement.name)
        .expect("package in the model");
    *slot = replacement;
}

#[test]
fn compliant_workspace_has_no_violations() {
    assert_eq!(check_workspace(&compliant_workspace()), Vec::new());
}

#[test]
fn simulation_depending_on_presentation_is_forbidden() {
    let mut workspace = compliant_workspace();
    replace(
        &mut workspace,
        package(
            "engine_sim",
            &[
                ("engine_core", Kind::Normal),
                ("engine_render", Kind::Normal),
            ],
        ),
    );
    assert_eq!(
        check_workspace(&workspace),
        vec![Violation::ForbiddenDependency {
            package: "engine_sim".to_owned(),
            dependency: "engine_render".to_owned(),
            kind: Kind::Normal,
        }]
    );
}

#[test]
fn development_dependencies_obey_the_same_rules() {
    let mut workspace = compliant_workspace();
    replace(
        &mut workspace,
        package("engine_sim", &[("engine_assets", Kind::Development)]),
    );
    assert_eq!(
        check_workspace(&workspace),
        vec![Violation::ForbiddenDependency {
            package: "engine_sim".to_owned(),
            dependency: "engine_assets".to_owned(),
            kind: Kind::Development,
        }]
    );
}

#[test]
fn nothing_may_depend_on_the_composition_root() {
    let mut workspace = compliant_workspace();
    replace(
        &mut workspace,
        package("engine_map", &[("engine_app", Kind::Normal)]),
    );
    assert_eq!(
        check_workspace(&workspace),
        vec![Violation::ForbiddenDependency {
            package: "engine_map".to_owned(),
            dependency: "engine_app".to_owned(),
            kind: Kind::Normal,
        }]
    );
}

#[test]
fn backends_are_private_to_their_owner() {
    let mut workspace = compliant_workspace();
    replace(
        &mut workspace,
        package("engine_render", &[("vulkane", Kind::Normal)]),
    );
    replace(
        &mut workspace,
        package("engine_ui", &[("vulkane", Kind::Build)]),
    );
    assert_eq!(
        check_workspace(&workspace),
        vec![Violation::MisplacedBackend {
            package: "engine_ui".to_owned(),
            dependency: "vulkane".to_owned(),
            owner: "engine_render",
        }]
    );
}

#[test]
fn unruled_and_missing_packages_are_reported() {
    let mut workspace = compliant_workspace();
    workspace.retain(|package| package.name != "engine_net");
    workspace.push(package("engine_extra", &[]));
    let violations = check_workspace(&workspace);
    assert!(violations.contains(&Violation::MissingPackage {
        package: "engine_net"
    }));
    assert!(violations.contains(&Violation::UnclassifiedPackage {
        package: "engine_extra".to_owned()
    }));
    assert_eq!(violations.len(), 2);
}

#[test]
fn shared_package_settings_are_required() {
    let mut workspace = compliant_workspace();
    let mut core = package("engine_core", &[]);
    core.edition = "2021".to_owned();
    core.rust_version = None;
    core.publishable = true;
    core.inherits_workspace_lints = false;
    replace(&mut workspace, core);
    let violations = check_workspace(&workspace);
    assert!(violations.contains(&Violation::LintsNotInherited {
        package: "engine_core".to_owned()
    }));
    assert_eq!(violations.len(), 4);
}

#[test]
fn workspace_lint_inheritance_is_read_from_the_manifest() {
    let inheriting = [
        "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n",
        "[lints]\n  workspace=true   # inherited\n[dependencies]\n",
        "lints.workspace = true\n[package]\n",
        "lints = { workspace = true }\n",
    ];
    for manifest in inheriting {
        assert!(inherits_workspace_lints(manifest), "{manifest:?}");
    }
    let not_inheriting = [
        "[package]\nname = \"a\"\n",
        "[lints]\nworkspace = false\n",
        "[lints.rust]\nworkspace = true\n",
        "[dependencies]\nworkspace = true\n",
        "# [lints]\n# workspace = true\n",
        "[package]\nlints.workspace = true\n",
    ];
    for manifest in not_inheriting {
        assert!(!inherits_workspace_lints(manifest), "{manifest:?}");
    }
}

/// The rule table itself must be a sound layering: names unique, every
/// permitted dependency a ruled crate, and no cycle, so the rules can never
/// demand a graph Cargo would reject.
#[test]
fn rule_table_is_an_acyclic_layering() {
    let names: HashSet<&str> = CRATE_RULES.iter().map(|rule| rule.name).collect();
    assert_eq!(names.len(), CRATE_RULES.len(), "duplicate crate rule");
    for rule in CRATE_RULES {
        for dependency in rule.may_depend_on {
            assert!(names.contains(dependency), "`{dependency}` has no rule");
        }
    }
    // Repeatedly remove the crates whose permitted dependencies have all been
    // removed; a cycle leaves crates that can never be removed.
    let mut remaining = CRATE_RULES.to_vec();
    while !remaining.is_empty() {
        let left: Vec<&str> = remaining.iter().map(|rule| rule.name).collect();
        remaining.retain(|rule| rule.may_depend_on.iter().any(|name| left.contains(name)));
        assert!(remaining.len() < left.len(), "cycle among {left:?}");
    }
}

#[test]
fn simulation_rule_excludes_presentation_and_the_fixture() {
    let sim = CRATE_RULES
        .iter()
        .find(|rule| rule.name == "engine_sim")
        .expect("rule for engine_sim");
    assert!(!sim.may_depend_on.contains(&"engine_render"));
    assert!(!sim.may_depend_on.contains(&"fixture_quake"));
}

#[test]
fn every_backend_owner_is_a_ruled_crate() {
    for backend in BACKEND_RULES {
        assert!(
            CRATE_RULES.iter().any(|rule| rule.name == backend.owner),
            "backend `{}` owned by unknown `{}`",
            backend.dependency,
            backend.owner
        );
    }
}
