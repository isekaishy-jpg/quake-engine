# Proposal 0145: shader graphs and compute

Status: proposed, 2026-11-07, the [work list](../research/work-list.md), items 13 and 14, from [the research](../research/shader-graphs.md).

## Design

### A graph is surface code (13)

- A graph is `shaders/<name>.egraph`, and a material names it as it would a shader: `shader = <name>`. With both, the WGSL wins, so a person can take a graph's code and go on by hand.
- `engine_shaders::graph` turns one into WGSL. That text then goes where any shader's goes, so the renderer doesn't know graphs exist.
- A node has an id, a kind, and inputs that name other nodes or give a number. An input left out has a default.
- **Types:** a number, or two, three or four of them. A number goes wider by filling, and more goes narrower by dropping the end. Each kind says what it gives from what it's given.
- **Outputs** say which hook a node lands in: `vertex` for `shader_vertex`, `uv` for `shader_uv`, `base`, `normal` and `emissive` for `shader_surface`, and `color` for `shader_post`.
- **Kinds:**
  - What's there: `uv`, `world`, `normal`, `base`, `color`, `time`, `param`, `image`.
  - Values: `number`, `vector`.
  - Sums: `add`, `subtract`, `multiply`, `divide`, `mix`, `power`, `min`, `max`, `clamp`, `step`, `smoothstep`, `dot`, `sin`, `cos`, `abs`, `fract`, `floor`, `length`, `normalize`, `one_minus`.
  - Parts: `split` by `x`, `y`, `z` or `w`, and `join`.
  - `noise`, as land has it.
  - `code`, which is an expression with up to four inputs, `a` to `d`, and a type it says it gives.
- **What's wrong** is said by node: a kind that doesn't exist, an input that names nothing, a loop, or something a hook can't read, like `uv` in `vertex`. A graph that's wrong is a broken shader: green, with the reason in the console.
- `check_shaders` covers graphs, and `engine-tool` gets `write_graph`, `describe_graph` for the code it makes, and a list of kinds.

### One node view (13)

- `engine_editor::graph_view` is a canvas of boxes, ports and curves that knows nothing of shaders. It takes a list of what each kind has, so particles can use it.
- A **Graph** tab has the kinds to add, the view, a node's values, what's wrong, and the code it made. **Save** writes the file and builds the map again, so the level's the preview.

### A field is what compute works on (14)

- A level has `field` blocks: `field { name = ripples shader = rain size = 256 rate = 30 shader_params = 0.9 }`.
- The shader has one hook: `fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32>`. It reads the last step with `field_read(at)`, which keeps to the edges, and has `shader_param` and `shader_time` as a surface has.
- A field's two images of `rgba16float`, in turn: a step reads one and writes the other, and what's read has always finished. It steps `rate` times a second, by the clock, and no more than a few times in a frame.
- A material gives one to its shader with `shader_image_a = field:ripples`. To a surface shader or a graph it's a picture.
- Compiled ahead with the rest. A broken one doesn't step, and says so.
- Compute for particles (item 90) uses the same way of running a shader, at stage 10.

### Other approaches

- **Selected approach:** the above.
- **Tradeoffs:**
  - A graph has no branches. A `step` and a `mix` do most of that, and `code` the rest.
  - A cell can read anywhere but only writes itself. That's what keeps it safe.
- **Rejected:**
  - Graphs in the renderer, as data: every surface would pay, and nothing else would share.
  - `egui-snarl`: it's on `egui` 0.36.
  - Compute over any buffers, set up by a level: nothing asks for it, and a hook keeps out races and barriers.
- **Acceptance criteria:**
  - A graph draws in the game the same as the same code by hand.
  - A wrong graph is green, and says what's wrong and where.
  - A graph can be made in the editor, and with `engine-tool`, and its code seen.
  - A field steps at its rate, and a surface reads it, in the game.
  - What a field costs is measured.

## Review

Filled in as each part lands.
