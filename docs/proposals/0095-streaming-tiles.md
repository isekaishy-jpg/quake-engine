# Proposal 0095: streaming tiles

Status: accepted, 2026-11-05, increment 12.

## Research

- [Proposal 0089](0089-tiles.md) loaded every tile with its map, and left streaming to the human, who asked for it. It said streaming would need `Map` split by tile, through every system.
- Measuring says otherwise. In memory, the nine tiles of `tile_world` hold:

| Part | Bytes | Who needs it |
|---|---|---|
| Walkable grids, for two hulls | 13.8 MB | The server, for paths |
| Light, directions and crisp edges | 4.5 MB | The client, for drawing |
| Shared textures | 2.1 MB | The client |
| Faces and points | 0.5 MB | The client, and sound |
| Probes | 0.2 MB | The client |
| The tree and clipping hulls | 0.1 MB | Everything |

- So collision is about 12 KB a tile, and even the largest grid, 65 by 65, holds 50 MB of it. Splitting what's small through every system buys nothing, and risks the rule that matters most. What's worth streaming is two things, each with one user.
- There's also a wall today. The light atlas stops at 8192 pixels a side, so a big enough tile map doesn't load at all.
- The human's rules stand: a player never hits an unloaded tile, for any speed or teleport, and [nothing in the world has a hard distance limit](../research/maps-materials-lighting.md). So a tile that isn't loaded still has to draw.

## Design

**What stays.** The tree, hulls, faces, entities, probes and the vertex buffer stay in memory for every tile. Collision, the server's rules and sound don't change, and a player can't reach something unloaded, because everything they can touch is always there.

**Walkable grids** are stored in chunks of 128 columns in side files. A place in a chunk is a height and eight one byte links, which name a level in the next column, so 12 bytes against 44, and links cross chunks with nothing special. A search reads the chunks it reaches, and a small cache keeps the most recent. Searches have a budget, so a goal nothing reaches can't walk the whole world.

**Light** is stored in a side file for each tile. Each face also keeps its average colour, which costs three bytes.

**The atlas** becomes a fixed image of pages. Every face has a texel of its average in a part of the atlas that stays, so a far tile draws with one colour a face, which at that range is what its detail would blur to. A tile near the view takes pages, its light is copied in, and its vertices are pointed at them. When the view leaves, they're pointed back and the pages go free. Brush models' light never streams, as they can go anywhere.

**Who loads.** The view asks for tiles within reach of it, nearest first, reads them off the main thread, and copies a few a frame. Reach starts at two tiles, which is two seconds at top speed. After a teleport nothing has to wait: the far texels are already there, and detail arrives over the next few frames.

**Shadows.** A shadow map draws only the tiles its light reaches, rather than every face in the world.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** memory still grows with a world's geometry. By the measurement above, the largest grid holds about 1.3 GB rather than about 8.8 GB. Compiling a tiled map still needs all of it in memory. A far tile's faces each have one colour, so a sharp shadow across a far floor is lost until it's near.
- **Acceptance criteria:** a tile map beyond the atlas's old limit loads and plays, memory for light and paths stays flat while walking across it, a path crosses chunks and agrees with the same path on a whole grid, a face with no detail draws in its average colour, and detail arrives and leaves without changing what's near the view.
- **Future extension:** streaming geometry, if a world ever needs more than the largest grid.

## Review

See the increment 12 review.
