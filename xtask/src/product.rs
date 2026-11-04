//! Product version and build identity, as DESIGN.md spells them.
//!
//! Cargo needs a SemVer version, so the workspace stores the product version
//! in SemVer form and this module converts it:
//!
//! | Stage | Cargo version | Product version |
//! | --- | --- | --- |
//! | Alpha | `0.0.0-alpha` | `0.0.0a` |
//! | Beta | `0.0.1-beta` | `0.0.1b` |
//! | Release candidate | `0.0.1-rc` | `0.0.1rc` |
//! | Stable | `0.0.1` | `0.0.1s` |

use std::fmt;

use crate::build_number::BuildNumber;

/// The product name shown in every build identity.
pub const PRODUCT_NAME: &str = "Quake Engine";

/// A release stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Alpha, suffix `a`.
    Alpha,
    /// Beta, suffix `b`.
    Beta,
    /// Release candidate, suffix `rc`.
    ReleaseCandidate,
    /// Stable, suffix `s`.
    Stable,
}

impl Stage {
    /// The product suffix of this stage.
    #[must_use]
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Alpha => "a",
            Self::Beta => "b",
            Self::ReleaseCandidate => "rc",
            Self::Stable => "s",
        }
    }
}

/// A product version: three numeric components and a stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductVersion {
    /// The major component.
    pub major: u64,
    /// The minor component.
    pub minor: u64,
    /// The patch component.
    pub patch: u64,
    /// The release stage.
    pub stage: Stage,
}

/// A Cargo version that does not encode a product version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionError {
    /// The rejected Cargo version.
    pub cargo_version: String,
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cargo version `{}` is not `major.minor.patch` with an optional `-alpha`, `-beta` or `-rc` stage",
            self.cargo_version
        )
    }
}

impl std::error::Error for VersionError {}

impl ProductVersion {
    /// Parses the Cargo spelling of a product version.
    ///
    /// Only the four stage spellings in the module table are accepted. Build
    /// metadata (`+...`) and other pre-release tags are rejected, because the
    /// build number is kept separately and other tags have no product meaning.
    ///
    /// # Errors
    ///
    /// Returns [`VersionError`] when `cargo_version` is not one of those
    /// spellings.
    pub fn from_cargo(cargo_version: &str) -> Result<Self, VersionError> {
        let error = || VersionError {
            cargo_version: cargo_version.to_owned(),
        };
        let (numbers, stage) = match cargo_version.split_once('-') {
            None => (cargo_version, Stage::Stable),
            Some((numbers, "alpha")) => (numbers, Stage::Alpha),
            Some((numbers, "beta")) => (numbers, Stage::Beta),
            Some((numbers, "rc")) => (numbers, Stage::ReleaseCandidate),
            Some(_) => return Err(error()),
        };
        let mut components = numbers.split('.').map(parse_component);
        let (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) = (
            components.next(),
            components.next(),
            components.next(),
            components.next(),
        ) else {
            return Err(error());
        };
        Ok(Self {
            major,
            minor,
            patch,
            stage,
        })
    }
}

/// Parses one SemVer numeric component: decimal digits without a leading zero,
/// except for `0` itself.
fn parse_component(text: &str) -> Option<u64> {
    let digits_only = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits_only || (text.len() > 1 && text.starts_with('0')) {
        return None;
    }
    text.parse().ok()
}

impl fmt::Display for ProductVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}{}",
            self.major,
            self.minor,
            self.patch,
            self.stage.suffix()
        )
    }
}

/// The user-facing identity of a build, for example
/// `Quake Engine 0.0.0a (Build 000001)`.
#[must_use]
pub fn build_identity(version: ProductVersion, build: BuildNumber) -> String {
    format!("{PRODUCT_NAME} {version} (Build {build})")
}
