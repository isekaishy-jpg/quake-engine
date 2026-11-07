# Proposal 0145: shader graphs and compute

Status: done, 2026-11-07, the [work list](../research/work-list.md), items 13 and 14, from [the research](../research/shader-graphs.md).

## Design

### A graph is surface code (13)

- A graph is `shaders/<name>.egraph`, and a material names it as it would a shader: `shader = <name>`. With both, the WGSL wins, so a person can take a graph's code and go on by hand.
- `engine_compile` turns one into WGSL. That text then goes where any shader's goes, so the renderer doesn't know graphs exist.
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

### Graphs (13)

- **The same picture.** A graph and the same look written by hand came out the same in the game, with no pixel apart. It takes the same way from there, so that's what was meant to happen.
- **The compiler's in `engine_compile`, not `engine_shaders`.** A game has to read a graph as it loads, to know which compiled shader it's asking for, and that crate already reads files. `engine_shaders` is left with what compiles.
- **Every kind is compiled by the real compiler,** on its own, as a test. That found that `smoothstep` won't take numbers with no type, so every number a graph writes has one.
- **A wrong graph** is green in the game, and the console gives the node and the reason, as the graph's own words reach the compiler's error. `write_graph` refuses one, and `check_shaders` finds one put there by hand.
- **The view** was tried with the mouse's own events through a new `mouse` line command. That found what no test of it had: a drag's only reported after the pointer has moved, so a port it began on had been left, and every curve panned the canvas instead. It takes hold of where the press began now. Boxes moved and curves joined after that.
- **Nodes added** step away from each other, as they all landed in one place at first.
- **Every test here was seen failing** with its bug put back, thirteen in all. One of those was too weak the first time, a loop check that had only been moved, so it was done again with the check gone, and the test blew its stack as it should.

### Fields (14)

- **Tried in the game:** a wave simulation in about ten lines of `shader_field`, with drops falling at random, seen rippling across a floor through `shader_image_a = field:ripples`.
- **What it costs,** at 60 steps a second, on average for a frame, from the GPU's clock: 256 by 256 is 0.010 ms, 1024 is 0.054, and 2048 is 0.201. A step is about three of those at that frame rate.
- **Two images, and a pass to keep what's made.** A step reads one and writes the other. A surface reads one image, as its set of bindings is made once. `vulkane` has no copy between images, so a second small pass does it, which the costs above include.
- **Fields can be graphs,** which the research said they should: an output called `field`, with `cell` and `last` to read from, so that "reach as far as compute" holds for graphs too.
- **`shader_noise` moved** to a file of its own, as surfaces and fields both want it.
- **Adding a field** is `add_field`, or the World tab, with a name and a shader.
- **Every test here was seen failing** with its bug put back. Reading past the edge had no test until a bug put there passed, so it has one now.

The acceptance criteria hold, apart from how the Graph tab feels in use, which needs a person.
