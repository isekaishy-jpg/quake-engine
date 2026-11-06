# Proposal 0116: zones

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The human asked for regions, areas, zones and sub zones, as a named part of a map that things can hang from, such as showing an area's name as text, audio and music, and ambient light, and left how to make it work to Claude.

- MMOs and open world games name areas with volumes and show a title on entering. World of Warcraft has zones and sub zones, with the name of the smallest shown. What matters there is that a sub zone inherits what it doesn't set, and that coming back out doesn't announce the larger area again.
- The map's tree can't hold them. Leaves are cut by geometry, and a zone cares nothing for walls. A short list of convex volumes, tested by planes, costs nothing at this scale, and brushes are how an author already makes volumes.
- What a zone shows is the player's own, so it belongs on the client. Scripts want to ask too, so the map holds them on both sides.
- Light can't be rebaked as someone walks in. Light reaching the eye is what's left, which is how exposure works in any case, and it can fade.

## Design

- **`type = zone`** on an entity with brushes. They become planes in `Map::zones`, and never a model, so they're not seen or touched.
- **The inner one wins:** by `priority`, then the smallest.
- **Inheriting:** `parent` names what it's part of, and `zone_settings` fills in what it doesn't set.
- **`ZoneTracker`** on the client follows the view. It gives titles, music and sound on a change, and fades light over a second and a half. Coming back out from inside a zone doesn't show its title.
- **Sound:** a fourth ambient slot in the mixer, which fades down before another takes over.
- **Music** is a track or a file, and goes on when you leave for nowhere, as stopping sounds wrong.
- **Light** is a colour on the final pass, ahead of the player's gamma.
- **Titles** use the centre print until the engine's UI (item 16) has somewhere better.
- **Scripts:** `engine.zone_at(origin)`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a zone is checked each frame, every plane of every shape, so a level with thousands would want a grid. Levels have tens.
- **Acceptance criteria:** going in shows a title and starts music, sound and light. A sub zone wins and inherits. Coming back out shows nothing. A zone can't be seen or touched.
- **Future extension:** fog by zone, and zones in the editor's views (item 18).

## Review

See the tests in `engine_client/tests/zones.rs`.
