//! Luau formatting: what it does, and that the repository keeps to it.

use xtask::luau::{check_luau, format_text};
use xtask::workspace_root;

/// Tabs indent, as the style guide says, and meaning stays.
#[test]
fn formatting_indents_with_tabs() {
    let formatted = format_text("function f()\n  return 1\nend\n").expect("it parses");
    assert_eq!(formatted, "function f()\n\treturn 1\nend\n");
}

/// Luau's own syntax is understood, not only Lua's.
#[test]
fn luau_syntax_parses() {
    let text = "local x: number = 1\nx += 1\n";
    assert_eq!(format_text(text).expect("it parses"), text);
}

/// Something that isn't Luau says so.
#[test]
fn broken_text_is_reported() {
    assert!(format_text("function (").is_err());
}

/// Every Luau file in the repository is formatted.
#[test]
fn the_repository_is_formatted() {
    check_luau(&workspace_root()).expect("formatted");
}
