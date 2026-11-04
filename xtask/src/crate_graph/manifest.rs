//! Facts read from a member's `Cargo.toml` that `cargo metadata` omits.

/// Whether a package manifest inherits the workspace lints, in any of the
/// three TOML spellings Cargo accepts:
///
/// ```toml
/// [lints]
/// workspace = true
/// ```
///
/// or `lints.workspace = true`, or `lints = { workspace = true }`, at the top
/// level.
///
/// This is a line scan, not a TOML parser. It handles the manifests this
/// workspace writes (one key per line, `#` comments, any spacing). A spelling
/// it does not recognise reads as "not inherited", so the check fails loudly
/// rather than passing a package that opts out.
#[must_use]
pub fn inherits_workspace_lints(manifest: &str) -> bool {
    let mut table = String::new();
    for line in manifest.lines() {
        let line = strip_comment(line);
        let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.starts_with('[') {
            table = compact;
            continue;
        }
        let inherits = match table.as_str() {
            "" => compact == "lints.workspace=true" || compact == "lints={workspace=true}",
            "[lints]" => compact == "workspace=true",
            _ => false,
        };
        if inherits {
            return true;
        }
    }
    false
}

/// Removes a trailing `#` comment. Manifest values in this workspace never
/// contain `#` inside a string where a lint key could follow it.
fn strip_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(code, _comment)| code)
}
