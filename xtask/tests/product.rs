//! Product version spelling and build identity, as DESIGN.md defines them.

use xtask::build_number::BuildNumber;
use xtask::product::{ProductVersion, Stage, build_identity};

#[test]
fn cargo_stages_map_to_product_suffixes() {
    let cases = [
        ("0.0.0-alpha", "0.0.0a"),
        ("0.0.1-beta", "0.0.1b"),
        ("0.0.1-rc", "0.0.1rc"),
        ("0.0.1", "0.0.1s"),
        ("12.3.45", "12.3.45s"),
    ];
    for (cargo, product) in cases {
        let version = ProductVersion::from_cargo(cargo).expect(cargo);
        assert_eq!(version.to_string(), product);
    }
}

#[test]
fn unknown_or_malformed_versions_are_rejected() {
    for cargo in [
        "0.0.0-alpha.1",
        "0.0.0-dev",
        "0.0.0+build",
        "0.0",
        "0.0.0.0",
        "01.0.0",
        "a.b.c",
        "",
    ] {
        assert!(ProductVersion::from_cargo(cargo).is_err(), "{cargo}");
    }
}

#[test]
fn workspace_version_is_a_product_version() {
    let version = ProductVersion::from_cargo(env!("CARGO_PKG_VERSION")).expect("workspace version");
    assert_eq!(version.stage, Stage::Alpha);
}

#[test]
fn identity_shows_a_six_digit_minimum_build() {
    let version = ProductVersion::from_cargo("0.0.0-alpha").expect("version");
    assert_eq!(
        build_identity(version, BuildNumber::new(1)),
        "Quake Engine 0.0.0a (Build 000001)"
    );
    assert_eq!(
        build_identity(version, BuildNumber::new(1_234_567)),
        "Quake Engine 0.0.0a (Build 1234567)"
    );
}
