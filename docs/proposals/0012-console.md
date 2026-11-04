# Proposal 0012: the console, commands and cvars

Status: accepted, 2026-11-03, increment 1.

## Research

From `reference/quake/WinQuake/cmd.c` and `cvar.c`:

- `Cbuf_Execute` splits buffered text into commands at newlines, and at `;` outside quotes. `wait` stops execution until the next frame.
- `Cbuf_AddText` appends and `Cbuf_InsertText` prepends, which `exec` and aliases use so their commands run next.
- `Cmd_TokenizeString` uses `COM_Parse`: whitespace separates tokens, `//` starts a comment, and double quotes group text.
- `Cmd_ExecuteString` looks up commands, then aliases, then cvars. A cvar with one argument prints, and with two it sets.
- Names are case-insensitive.
- Aliases in the original can loop forever through the buffer.
- Config files like `quake.rc` and `default.cfg` need this behaviour to run.
- `COM_Parse` also makes each of `{ } ( ) ' :` its own token outside quotes, which breaks `host:port`. A scan of the shipped `quake.rc` and `default.cfg` found no unquoted use of these characters, so nothing depends on it.

## Design

`engine_console` has no third-party dependencies and doesn't know Quake's commands.

- `Console<C>` owns the buffer, commands, aliases, cvars and output lines. `C` is the application's context, passed to command handlers, so the console needs no globals and no knowledge of other systems.
- Commands are `fn(&mut Console<C>, &mut C, &Args)`.
- `add` appends text, `insert` prepends it, and `execute` runs commands until the buffer empties or `wait` is hit.
- Cvars have a name, string value, default and flags for saved and server. Registering a cvar twice is a programming error.
- Output is collected as lines, and the application decides where they go.
- A limit on commands per `execute` stops an alias loop freezing the engine, and it clears the buffer and reports the problem.
- Built-ins are `alias`, `echo`, `wait`, `cmdlist` and `cvarlist`.

## Proposal

- **Selected approach:** the above, matching Quake's parsing, except that punctuation doesn't split tokens.
- **Tradeoffs:** handlers are plain functions, not closures, so state goes through `C`. That keeps ownership simple.
- **Acceptance criteria:** tests cover tokenizing, separators in quotes, comments, aliases, `wait`, insertion order, cvar get and set, case-insensitive names and the loop limit. The real `quake.rc` runs.
- **Future extension:** command completion, history, key bindings and the drawn console arrive with the UI.

## Review

See [review 0003](../reviews/0003-increment-1.md).
