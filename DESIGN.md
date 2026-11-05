# Design principles

This is the first design document and governing authority for this engine. Technical documentation is indexed in [docs/README.md](docs/README.md). Only README.md and DESIGN.md belong at the repository root as documentation.

The name `quake-engine` is a working name.

## Founding principles

The human set these on 2026-11-03.

1. **A new engine, not a recreation of Quake.** The original is mostly a fixture: its maps and entities give the engine something real to build and test against. Maps in particular will evolve quickly.
2. **A solid baseline first.** Make what's available work with the systems being built, knowing they will change. Larger changes come after the baseline.
3. **Rust is the implementation language.** Luau covers scripting.
4. **QuakeC gets ported to Luau,** because the fixture needs it. Doors, lifts and other map entities come first.
5. **Rendering and sound are decoupled from physics.** Simulation runs on a pegged rate that doesn't depend on frame rate.
6. **Physics has a `box3d` backend,** with game movement rebuilt on top, so the platform is there if we want to change things later.
7. **Rendering is built on `vulkane`.** Rendering features are remade there.
8. **The host, worker and cache crates are part of this engine.** `engine_host`, `engine_worker` and `engine_cache` live here as repository crates and can be changed here.
9. **Online play is server-authoritative, with fair play as a goal.** Behaviour is controlled by server settings, not hardcoded.
10. **Original game data is never committed.** The engine loads it from the user's own copy.
11. **The first end goal is the port.** The finished product plays Quake, the fixture, so we know it works.
12. **Game dev encapsulation throughout,** so anything can be changed later. Each system sits behind its own boundary, and third-party libraries stay private to the system that uses them.

Decisions recorded before this workflow was adopted are in [docs/pre-research](docs/pre-research/README.md). They say what we intend to do. They aren't research, and each still goes through the workflow below.

## Workflow

- **Who reads what.** The human does not read code. They judge the final product, how well it works and how fast it runs, and they care about the speed and cost of development.
- **The sequence.** Research → Design → Proposal → Implementation → Review for bugs and fix → Commit, for everything: file structures, toolchain, features, formats. This is a measure twice, cut once repository. The implementing agent completes every stage. The depth of each stage fits the decision, but the sequence is kept and no stage is skipped.
- **Research covers how, not only what.** Choosing a direction isn't research. Research establishes how something will actually be done: the technical details, the sources, the constraints and the evidence.
- **Every item goes through the whole sequence on its own.** An item is any decision: a toolchain pin, a file or directory layout, a lint set, a tool, a crate boundary, a dependency, a file format, a feature. Items are not batched into one proposal per increment.
- **Records come first.** Each item's research, design and proposal (the selected approach, its tradeoffs, acceptance criteria and future extension path) are recorded under `docs/` and indexed in `docs/README.md` before it is implemented. Reviews are recorded there too.
- **Second review before push.** Work is committed locally. At the end of each increment the commit range is reported to the human, who decides on review, and it is pushed only after approval.
- **Research gaps are recorded, not guessed.** Open questions go in [docs/research/open-questions.md](docs/research/open-questions.md) with what is needed, and the affected work waits for the answer.
- **Agreed decisions only.** An assistant's suggestion doesn't become a requirement until the human agrees to it.
- **Bounded increments.** Each increment states what it delivers, what remains and how work continues. The scope of each piece of work must not shrink as remaining work decreases.
- **Zero errors and zero warnings.** All build, format, lint, static-analysis, test and packaging checks pass cleanly before work is complete. Fix causes. Never hide output, weaken checks or add blanket suppressions.
- **Validation by purpose.** Permanent tests, disposable tests, verification and benchmarks are distinct. Each is chosen by the question it answers and the failure it can detect, weighed against its cost.
- **Standalone ownership.** The product owns everything it ships, even when code is borrowed. A borrowed component must be justified by the design and fit that ownership.

## Versions

- **Product versions** are `major.minor.patch` with a stage suffix: `a` alpha, `b` beta, `rc` release candidate, `s` stable. A version change is an explicit release decision with comprehensive release notes for the whole commit range.
- **Packaged builds** carry a separate, monotonically increasing build number shown with six digits. Numbers are reserved before compiling and never reused.

## Commits

Commits follow Conventional Commits 1.0.0:

- `<type>[scope][!]: <imperative summary>`, with types `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore` or `revert`, and a lowercase summary with no trailing period.
- Every commit has a body giving the concrete purpose, resulting behaviour, validation and any material limitation.
- Commits are focused, independently verifiable and leave a buildable state.

## Authority

Later research, designs, proposals and implementations must be justified against these principles. They can't silently weaken them or reduce the product to fit an implementation's limits. Changes to the founding principles require an explicit decision from the human.
