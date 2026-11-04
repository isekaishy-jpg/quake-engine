# Proposal 0013: the filesystem and packs

Status: accepted, 2026-11-03, increment 1.

## Research

From `reference/quake/WinQuake/common.c`:

- A pak starts with `PACK`, then the directory's offset and length. Each directory entry is a 56-byte name, an offset and a length, 64 bytes in all.
- `COM_AddGameDirectory` adds the directory to the search path, then `pak0.pak`, `pak1.pak` and so on until one is missing. Each goes in front, so paks override loose files in the same directory.
- `-game` adds another directory in front of `id1`.

From the survey of a local install: file names in the Steam copy are upper-case (`PAK0.PAK`), so opening `pak0.pak` needs a case-insensitive match on systems that care. Paths inside paks are lower-case.

Encapsulation: pak is a Quake format, so under [proposal 0001](0001-architecture.md) it belongs in `fixture_quake`, while the search path is an engine service.

`assets_manager` handles typed assets and caching. Nothing here needs either yet, because this increment only reads bytes and lists files. It comes in with the first typed assets, on top of this layer.

## Design

**`engine_assets`** holds what any game needs:

- `Archive`, a trait for anything that holds files, with `label`, `paths`, `contains` and `read`.
- `DirectoryArchive`, for loose files.
- `FileSystem`, an ordered search path. `mount` adds an archive at the front, and `read`, `exists`, `list` and `search_path` work across all of them.
- `normalize_path`, which lower-cases, uses forward slashes and rejects `..` and absolute paths.

**`fixture_quake`** holds what's specific to Quake:

- `PakArchive`, which reads the directory at open and file contents on demand.
- `mount_game`, which follows Quake's search order.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** reading on demand opens the pak for each file. That's simple, and it's fine until profiling says otherwise.
- **Acceptance criteria:** tests build small paks and check parsing, rejection of bad files, search order and path rules. The engine lists and reads the real `id1` paks, and all tests run without game data.
- **Future extension:** file ids and pack indexes build on `Archive`. `assets_manager` plugs in as a source over `FileSystem`.

## Review

See [review 0003](../reviews/0003-increment-1.md).
