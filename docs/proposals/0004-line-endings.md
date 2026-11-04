# Proposal 0004: line endings

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- The first commits printed Git's warning that LF would be replaced by CRLF. populous-reborn traced the same warning to `core.autocrlf=true` in the system Git config.
- Git's `eol` attribute overrides `core.autocrlf` for the paths it matches, and `text=auto` lets Git detect binaries.
- The Sol crates arrived with mixed endings from their own repositories.

| Option | For | Against |
|---|---|---|
| `* text=auto eol=lf` in `.gitattributes` | Same bytes everywhere, travels with the repository | Editors must not convert to CRLF |
| Per-machine `core.autocrlf=false` | No file | Doesn't travel with clones |

## Design

`.gitattributes` holds `* text=auto eol=lf`, plus `binary` for images, sound, Quake's data formats and the binary libraries in `vendor`.

## Proposal

- **Selected approach:** the attributes above.
- **Tradeoffs:** a file saved with CRLF shows a Git warning, and the index still gets LF.
- **Acceptance criteria:** `git ls-files --eol` shows `i/lf w/lf` for text files.
- **Future extension:** new binary formats get a line in the change that introduces them.

## Review

`git ls-files --eol` shows the expected attributes on tracked documents. Scripts that write repository files must write LF on Windows.
