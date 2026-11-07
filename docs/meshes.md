# Meshes

A mesh is what a brush isn't: anything round, or made in a modelling tool. See [proposal 0109](proposals/0109-meshes.md).

## Bringing one in

Models come in as glTF 2.0, in a `.gltf` or `.glb` file.

```bash
target/release/engine-tool content - import_gltf "{\"path\":\"well.glb\",\"name\":\"well\",\"scale\":32}"
```

That writes `meshes/well.emesh`, a material for each of the model's, and its pictures. `scale` is how many world units a metre is. glTF's Y up becomes our Z up. Metal, roughness, normal, glow and alpha all come across, and a colour with no picture becomes a small plain picture.

## Placing one

```text
mesh { source = well origin = 200 -180 0 angles = 0 45 0 scale = 1.4 }
```

Or give an entity `mesh = well`, and it's that entity's model.

Light maps are laid out for it when the level is baked, so there's no second set of UVs to make. For something small, `light = probe` lights the whole of it from one place, which costs almost nothing, and shares its triangles with every other copy:

```text
mesh { source = barrel origin = 40 12 0 light = probe }
```

## Copies

A placement with `light = probe` is an instance. The mesh is on the GPU once, and each part is one draw for all the copies you can see, so a yard of a hundred barrels costs what one does, plus their places. A copy keeps everything else: it casts shadows, takes moving lights, and is collided with. Use a light map of its own for something large, where light has to fall across it. See [proposal 0140](proposals/0140-meshes-at-run-time.md).

## Levels of detail

`import_gltf` makes coarser levels for a model, each with about half the triangles of the last, if it can do that without changing its shape much. Nodes named `<name>_lod1`, `<name>_lod2` and so on are used instead, if a model has them. For a mesh that's already in, `make_levels` does the same:

```bash
target/release/engine-tool content - make_levels "{\"name\":\"well\"}"
```

A copy goes to a coarser level when it fills less of the view's height: a quarter, a tenth and 3%, or what a `levels { sizes = [...] }` block in the file says. That's the same at any resolution. Only copies have levels, so a mesh with its own light map draws its finest.

## Attach points

A place on a model that things can be put at or held to: where a flame sits on a brazier, or a muzzle on a gun. In Blender, add an empty called `attach_<name>`. It's a place facing along its +X, as a model does once it's in.

Name a placement, and its places can be found by `<placement>.<name>`:

```text
mesh { source = brazier name = left origin = 0 0 0 }
entity { type = effect effect = campfire attach = left.bowl }
```

A map or a prefab can have a place with no model:

```text
point { name = gate origin = 100 0 32 angles = 0 90 0 }
```

An entity with `attach = <place>` starts there. A place that isn't there stops the level compiling, as it's a mistake. In a prefab, a copy finds its own places first, so two braziers don't share a flame.

To follow something that moves, add `attach_to`, with the name of an entity whose model has that place. Things and effects both go with it:

```text
entity { type = func_door name = lift mesh = cage }
entity { type = effect effect = campfire attach = lamp attach_to = lift }
```

Scripts have `engine.place(name)`, `engine.attach_point(entity, name)`, which both give an origin and angles, or nothing, and `engine.attach(entity, parent, name)` and `engine.detach(entity)`. What's held moves at the end of a tick, after everything else, so it doesn't lag. If what holds it goes, it stays where it was.

## Things that move

A model with an armature comes in with its bones and its clips. Name it on the level, as a level has to know what it can show:

```text
entity world { type = worldspawn meshes = "arm guard" }
```

Then a script shows it, and plays a clip by name:

```lua
engine.precache_model("mesh:arm")
engine.set_model(id, "mesh:arm")
engine.set_clip(id, "wave")
```

`engine.set_clip` says if there's such a clip, and with no name it goes back to rest. Asking for what's playing leaves it playing. A clip goes round, and changing clip blends over a fifth of a second.

A shape key comes in as a morph target, by its name. A clip can move it, and a script can set it:

```lua
engine.set_morph(id, "smile", 0.5)
```

That's over what a clip says. A weight of 0 gives it back to the clip. Eight can be set on a thing at once, and it says if there's no room. A mesh placed in a level, which doesn't move, shows the weights its file gave it.

An empty under a bone is an attach point that goes with the bone. It's the server that works out where it is, so what's held to it is right for everyone.

Skinning is linear, as glTF defines it, with four joints to a corner. A joint that twists a long way loses volume, so give it a twist bone. See [proposal 0140](proposals/0140-meshes-at-run-time.md).

## What's in a `.emesh`

A document, binary from import, and text if a script writes it. Each `part` has a `material`, `points`, `uvs` and `triangles`, and can have `normals`, one to a corner. With none, they're worked out from the shape: smooth, except where two triangles turn by more than 40 degrees. Each `hull` is a box for collision.

## Collision

Convex pieces are fitted to a mesh for you, so a well can be stepped into and its walls stop you. A `hull` box in the file replaces that, if you'd rather choose. A placement with `only_seen = 1` has none.
