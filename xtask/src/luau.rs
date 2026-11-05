//! Keeping Luau formatted with `StyLua`, as a library, so there's no tool to
//! install (proposal 0094).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use stylua_lib::{Config, IndentType, LineEndings, LuaVersion, OutputVerification, format_code};

/// Where Luau lives, from the workspace root.
const LUAU_ROOTS: [&str; 2] = ["crates", "content"];

/// Why Luau couldn't be checked, or wasn't formatted.
#[derive(Debug)]
pub enum LuauError {
    /// A file couldn't be read or written.
    Io {
        /// The file.
        path: PathBuf,
        /// The operating-system error.
        source: io::Error,
    },
    /// A file couldn't be parsed.
    Format {
        /// The file.
        path: PathBuf,
        /// What the formatter said.
        message: String,
    },
    /// These files aren't formatted.
    Unformatted(Vec<PathBuf>),
}

impl fmt::Display for LuauError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Format { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Unformatted(paths) => {
                write!(
                    formatter,
                    "{} Luau files need `cargo xtask fmt-luau`:",
                    paths.len()
                )?;
                paths
                    .iter()
                    .try_for_each(|path| write!(formatter, " {}", path.display()))
            }
        }
    }
}

impl std::error::Error for LuauError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Format { .. } | Self::Unformatted(_) => None,
        }
    }
}

/// How `docs/style.md` says Luau looks.
fn config() -> Config {
    Config {
        syntax: LuaVersion::Luau,
        indent_type: IndentType::Tabs,
        column_width: 100,
        line_endings: LineEndings::Unix,
        ..Config::default()
    }
}

/// `text` formatted, or what's wrong with it.
///
/// # Errors
///
/// Returns the formatter's message if `text` doesn't parse.
pub fn format_text(text: &str) -> Result<String, String> {
    format_code(text, config(), None, OutputVerification::None).map_err(|error| error.to_string())
}

/// Adds every `.luau` file under `directory` to `found`, skipping build output.
fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "luau")
        {
            found.push(path);
        }
    }
}

/// Every Luau file of the workspace, in a steady order.
#[must_use]
pub fn luau_files(workspace_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in LUAU_ROOTS {
        collect(&workspace_root.join(root), &mut found);
    }
    found.sort();
    found
}

/// A file's text, and that text formatted.
fn read_formatted(path: &Path) -> Result<(String, String), LuauError> {
    let text = fs::read_to_string(path).map_err(|source| LuauError::Io {
        path: path.to_owned(),
        source,
    })?;
    let formatted = format_text(&text).map_err(|message| LuauError::Format {
        path: path.to_owned(),
        message,
    })?;
    Ok((text, formatted))
}

/// Checks that every Luau file is formatted.
///
/// # Errors
///
/// Returns [`LuauError`] for the first file that can't be read or parsed, or
/// with every file that isn't formatted.
pub fn check_luau(workspace_root: &Path) -> Result<(), LuauError> {
    let mut unformatted = Vec::new();
    for path in luau_files(workspace_root) {
        let (text, formatted) = read_formatted(&path)?;
        if text != formatted {
            unformatted.push(path);
        }
    }
    if unformatted.is_empty() {
        Ok(())
    } else {
        Err(LuauError::Unformatted(unformatted))
    }
}

/// Formats every Luau file, and returns how many changed.
///
/// # Errors
///
/// Returns [`LuauError`] for the first file that can't be read, parsed or
/// written.
pub fn format_luau(workspace_root: &Path) -> Result<usize, LuauError> {
    let mut changed = 0;
    for path in luau_files(workspace_root) {
        let (text, formatted) = read_formatted(&path)?;
        if text != formatted {
            fs::write(&path, formatted).map_err(|source| LuauError::Io {
                path: path.clone(),
                source,
            })?;
            changed += 1;
        }
    }
    Ok(changed)
}
