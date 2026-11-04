//! Splitting buffered text into commands, and commands into tokens.

/// Splits the first command from `text`, returning it and the rest.
///
/// A command ends at a newline, or at a `;` outside double quotes. This
/// matches `Cbuf_Execute`.
#[must_use]
pub fn split_commands(text: &str) -> (&str, &str) {
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ';' if !quoted => return (&text[..index], &text[index + 1..]),
            '\n' => return (&text[..index], &text[index + 1..]),
            _ => {}
        }
    }
    (text, "")
}

/// Splits one command into tokens.
///
/// Whitespace separates tokens, `//` starts a comment, and double quotes
/// group text. This matches `COM_Parse`.
#[must_use]
pub fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = command;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with("//") {
            return tokens;
        }
        if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"').unwrap_or(quoted.len());
            tokens.push(quoted[..end].to_owned());
            rest = quoted.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            tokens.push(rest[..end].to_owned());
            rest = &rest[end..];
        }
    }
}
