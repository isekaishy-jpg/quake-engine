# Mesh and prefab standard

What a mesh or a prefab has to be to go in the engine. Give this to whoever makes them, a person or a model, along with the request. See [the texture standard](texture-standard.md) for its pictures and [Meshes](meshes.md) for bringing one in.

## Delivery

- **glTF 2.0,** as a `.glb`, or a `.gltf` with its files.
- **Y up, in metres,** as glTF has it. `import_gltf` turns it to our Z up and scales it, at 32 units to a metre unless told otherwise.
- **One thing to a file,** named `<kit>_<piece>`, in lower case with underscores, such as `abbey_arch_wide`.

## Scale

A player is 56 units tall and 32 wide, and steps up 18. Things are made for that.

| Thing | Units |
|---|---|
| Doorway | 64 wide, 112 tall |
| Corridor | 128 wide, 128 tall |
| Stair step | 8 to 16 high, 16 to 24 deep |
| Railing | 40 tall |
| Ramp | No steeper than 45 degrees to walk up |
| Room height | 128 at least |

## Origin and snapping

- **The origin** is at the bottom, in the middle of what it stands on, so that a piece placed on a floor stands on it.
- **Pieces fit a grid** of 8 units, and most of 32 or 64. A kit says its grid, and every piece's size is a whole number of it.
- **Openings meet.** Pieces that join share the same section where they join, so that any end fits any other.
- **No gaps or overlaps at joins.** Faces at a join line up exactly, with nothing poking through.
- **Facing.** The front faces along +X once it's in the engine, and a piece that turns does so about its origin.

## Geometry

- Triangles and quads are fine, and glTF stores triangles. Flat polygons come back together in the bake, so flat parts stay cheap.
- **Budget, by size on screen:**

| Thing | Triangles |
|---|---|
| Small prop: candle, cup | 100 to 500 |
| Large prop: barrel, brazier | 500 to 2,000 |
| Kit piece: wall, arch | 200 to 2,000 |
| Hero: well, statue, bridge | 2,000 to 10,000 |

- **Round things are round enough.** A ring wants 32 sides or more where it's seen close. Normals do the rest.
- **Normals are given,** smoothed by hard edges where edges are meant to be hard. With none, they're worked out, smooth except at sharp turns.
- **Closed where it can be.** No stray faces, flipped faces or faces inside others. A piece that's a shell is fine if nothing ever sees its back.
- **No hidden detail.** What can't be seen isn't modelled.
- **Attach points** are empties named `attach_<name>`. Use the same name across a kit, so that one thing fits them all.
- **Levels of detail** are made for you. To make your own, add nodes named `<name>_lod1`, `<name>_lod2` and so on, with the same materials.

## Things that move

- **Four joints to a corner** at most. More are dropped, keeping the strongest.
- **256 joints** at most, counting what's above them.
- **A joint that twists far** wants a twist bone, or it loses volume.
- **Clips are named,** as scripts play them by name.
- **Shape keys are named,** for the same reason. They move corners and normals. UVs and colours don't change.
- **Colours are optional.** A colour attribute comes through, set as the active one on export, and it multiplies the material. White changes nothing, so use it for tints, or dirt in the corners, and paint a seam where two colours should meet.

## Texturing

- **One UV set,** and it can overlap and repeat, as light has its own, generated.
- **Pixels per unit** match the kit's.
- **A kit piece uses its kit sheet and trims,** by material name, as the texture standard says.
- A hero model can have a unique texture.

## Collision

Convex pieces are generated, so a well can be stepped into with nothing placed by hand. For most things that's all. An author can give boxes, which win. Things too small to matter, such as a candle, can be `only_seen`.

## Prefabs

A prefab is a script in `prefabs/` that says what goes down: brushes, meshes, entities and scripts, from parameters.

- **Parameters have defaults,** so that it works with none.
- **Primitive prefabs** are brushes, and come in two parts that snap: a shape and what dresses it.
- **Mesh prefabs** hold a mesh, and whatever goes with it: the light of a lamp, the fire of a brazier, the script of a lift.
- **A prefab that moves** carries its own script, such as a lift that moves when stood on, or by itself.
- **Light and effects go with what makes them,** so that a torch brings its flame and its light.
- **Everything has a thumbnail,** which the editor makes.

## Asking a model for a mesh

Say all of this in the request:

1. What it is, its kit, and the size in units, from the tables above.
2. Its grid and how it joins, for a kit piece.
3. The triangle budget.
4. Its materials by name, or the pictures to come with it, as the texture standard says.
5. That it comes as glTF, with its origin as above, and normals given.
