# Proposal 0131: containers and file ids

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), items 3 and 5.

## Research

The human renamed the container from `.epak` to `.pkt`, and asked for fids, which are file ids, and a FlatBuffers manifest, with the schemas left to Claude.

[The pre-research](../pre-research/stack.md#asset-id-research) settled most of this before the workflow began, and this follows it:

- Assigned `u32` ids, which survive renames and format changes. Path hashes were rejected: they change on a rename, and 32 bits collide quickly.
- A text table as the source of truth, as binary can't be merged, and assigned ids are where merges go wrong.
- A FlatBuffers index through `planus`, with explicit field ids so that it can grow, and entries of id, path, offset and length, sorted by id, with the pack's SHA-256.
- Ids are never reused, and never invented at load, as those wouldn't be stable.

`planus` and `sha2` were both in the rules. `sha2` belonged to `engine_net` alone, and two crates need it now.

## Design

- **A `.pkt` file** is 16 bytes of `PKT\0`, the format and the index's length, then the index, then the files. Offsets count from the end of the index, so that writing the index can't move them.
- **`schema/pkt.fbs`** holds `Manifest`, with its format, SHA-256 and entries, and `Entry`, with its fid, path, offset, stored length and size. `pkt_generated.rs` comes from `planus rust`, with its `serde` lines removed as nothing here uses them, and its lints relaxed in `lib.rs`.
- **`FileIds`** reads and writes the table, as `next;2420` and then `1;compiled/textures/brick.png.etex`. A path keeps its id, and one that's gone keeps its row, so nothing takes its number.
- **`content/fids.txt`** is the table, in the repository. `engine-compile <content> --pack <file>` adds new files to it.
- **`FileSystem::read_id`** reads by id, through `Archive::path_of_id`. Paths stay the way things are named in content, as an id means nothing to read.
- **`PackArchive::verify`** checks the hash. A load doesn't, as reading all of a container for it costs more than it's worth at every start, and each file is still checked as it's opened.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** ids exist for packed files. Quake's paks have none, as they're the fixture's and aren't changed.
- **Acceptance criteria:** a container holds and reads what its folder did, ids survive being written and read, an id is never reused, damaged indexes are refused and never fall over, and the game plays from one.
- **Future extension:** mods choosing ids that don't collide, which the pre-research left open, is for when there are mods.

## Review

- All of `content` went into a container: 2,419 files, 670 of them shared, from 586 MB to 102 MB, in 13 seconds. Ember Keep then loaded and ran from it.
- `no_changed_byte_in_an_index_breaks_opening` flips bits through a whole index, which the pre-research said still needed doing.
- The hash finds a changed byte that opening doesn't, and that's tested.
