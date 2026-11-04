# Proposal 0047: deathmatch and co-op

Status: accepted, 2026-11-04, increment 9.

## Research

- `client.qc`, `items.qc` and `player.qc` have rules for two kinds of game that the port left out:
  - `respawn`, with `CopyToBodyQue` leaving the body behind, and spawn points from `info_player_deathmatch` and `info_player_coop`, avoiding what's already there.
  - `CheckRules`, `NextLevel`, `DumpScore`, `fraglimit` and `timelimit`.
  - Items coming back, and weapons staying, in deathmatch.
  - `ClientObituary`'s kills by players, and frags.
  - `spawn_filter` drops what's marked not for deathmatch.
- Other players need drawing, which the first person view never did. `player.qc` has every frame, and `CL_NewTranslation` recolours the skin by shirt and pants.
- `sbar.c` has `Sbar_DrawFrags`, `Sbar_DeathmatchOverlay` and `Sbar_MiniDeathmatchOverlay`, reading names, frags and colours that `svc_updatename`, `svc_updatefrags` and `svc_updatecolors` send.
- `menu.c` has Multiplayer, Setup, and Game Options. Its LAN screens were for IPX and modems.

## Design

- The rest of those QuakeC files is ported, with the translator's habits, keeping original names.
- A player's frames are a full port of `player.qc`, replacing the short list kept for first person.
- `ServerMessage::Scores` sends a list of names, frags and colours when it changes. `Hud` draws them.
- Skins are recoloured as a model is loaded for a player's colours, into its own texture, since there are only as many as players.
- `say` and `messagemode` give chat. `messagemode` opens the console's line for one message.
- Multiplayer in the menus has Join, Host and Setup: a typed address, a map and rules, and a name and colours.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** teamplay's friendly fire rules come through in `T_Damage`, but there's no team menu beyond colours, as in the original.
- **Acceptance criteria:** tests on the real scripts cover frags, a fraglimit ending a level, items coming back, weapons staying, spawn points being picked and telefrags. The menus are walked with a pretend host. A capture shows another player, in their colours.
- **Future extension:** a server browser, with the later server work.

## Review

See [review 0012](../reviews/0012-increment-9.md).
