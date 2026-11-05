# Review 0012: increment 9

Status: review record, 2026-11-04. Reviewed: proposals 0045 to 0056, and the changes to `engine_net`, `engine_sim`, `engine_script`, `engine_server`, `engine_client`, `engine_app`, `fixture_quake` and the tests.

| # | Finding | Handling |
|---|---|---|
| 1 | A client's snapshots carried over a level change could name a baseline from the last level, which the new client doesn't have, and it would never catch up | A seat forgets what it's sent when a level starts |
| 2 | The host quitting left guests waiting for a timeout, because nothing said goodbye | Stopping leaves the game properly, and guests return to the menus at once |
| 3 | `pause` showed its plaque online, where the game can't stop for one player | It does nothing with others connected |
| 4 | `deathmatch` stayed set after hosting, so a new single player game had deathmatch rules | A new game clears it, as `maxplayers 1` does in the original |
| 5 | Loading a save while hosting left guests on a server with no game | Loading leaves the game first |
| 6 | Colours of 0 drew the skin as painted, where the original draws grey | Only the plain model keeps its painted colours |
| 7 | Both players spawning on one spot telefragged each other in a test | That's correct. The test room has spawn points now |
| 8 | Armour and ammo never came back in deathmatch, which a test caught | They're given their times |
| 9 | Remote clients could use `noclip`, `setpos`, `give` and `god` | Only the host can |
| 10 | A client could send commands faster than real time to move faster | Each seat has an allowance of game time, so bursts are refused |
| 11 | The human pointed out that 20% loss isn't a meaningful figure, and even 5% is high | 2% is the case tested, 5% the design limit, and 20% is kept as a labelled margin. The test loses packets both ways now, where it only lost them one way |
| 12 | A commit was made with `--no-verify` partway through, against the gates | It was undone, with the work kept, before anything else happened |
| 13 | Compensation ramped up from nothing when a player joined, so their first shots got no help | A newcomer starts where they should be |
| 14 | Client paths and bots' sight used the player's hull, stopping short of what a point reaches | They trace a point |
| 15 | A player collided with what they own, unlike `SV_ClipToLinks`. Their own rocket's box made the client's move start solid, which wiped a predicted knock | `solid_boxes` leaves out what they own, on both sides |
| 16 | A predicted knock counted twice, since a snapshot answering the command before it already holds it | The client drops it at that snapshot |
| 17 | Working out which command a blast lands on from estimated times came out late | It's counted in commands, from the one that fired |
| 18 | Quake's rocket numbers sat in `engine_client`, against the encapsulation rule | The fixture supplies them as a `BlastRule` |
| 19 | Exact weapon timing locked every shot to the target's 0.5 s patrol, always at the turn, which made the shot tests pass or fail for the wrong reason | The patrol is 0.61 s, and only shots at a target at full speed count |
| 20 | A guest on a good connection was told their ping was over the limit as they joined, because a stall after loading was the first measure | The first measure is the lowest of eight. Without that, a test's 20 ms client read 583 ms |
| 21 | Anything typed could pass as talk, or as a second line, once the server marked talk | The server drops control characters |

## Verified

These ran against the Steam data, read in place. Captures used throwaway user folders.

| Check | Result |
|---|---|
| `cargo xtask check` | Passes |
| Two real processes over UDP on `dm3` | Joined, chatted, colours and rankings shown, player drawn |
| A level change with a guest connected | Both moved to `dm4`, and chat went on working |
| Reliable messages over real sockets, losing packets both ways | All arrive in order at 2% and 5%, and at the 20% margin |
| A real client against a fight, at 2% and 5% loss | Within 2 units of the server |
| Traffic from the server to one client | 19 to 32 KB a second on `e1m1`, `dm3` and `e4m8`, against about 1.4 MB a second for full snapshots. This counts messages, not `renet`'s own overhead |
| Saves on every `id1` map, every monster, every script | As before: 38 maps identical, 1888 monsters and 117 maps with no script errors |
| Shots from a bot at a target at 320 units a second on `dm3`, with rewind and without | See the table below |
| Over the ceiling, at 130 ms | No shots land, against all of them at 60 ms in the same lobby |
| The box a shot is judged against, at 60 ms | 0.0 units from the shooter's screen, over 18 shots. The same measure against a view 6 ticks later gives 21.1 |
| A client's projectile paths against the server's | Rocket 0.0 units, grenade 0.00006 units over 2.4 s |
| A predicted rocket jump | 0.0 units from the server at 40 and 80 ms, against 11.3 and 32.1 without prediction |
| Fire rates | Nails reach ten a second |
| Two real processes with bots and a simulated 50 ms, 5 ms jitter and 2% loss | Joined, chatted, team talk, a level change with a guest and a bot, no false ping warning. Captured chat and the scores beside the bar |

| Ping (ms) | Jitter (ms) | Loss | Hits with rewind | Hits without |
|---|---|---|---|---|
| 0 | 0 | 0% | 99% | 99% |
| 20 | 0 | 0% | 100% | 100% |
| 40 | 0 | 0% | 100% | 100% |
| 40 | 5 | 2% | 100% | 48% |
| 60 | 0 | 0% | 100% | 48% |
| 60 | 10 | 2% | 99% | 0% |
| 60 | 10 | 5% (margin) | 99% | 0% |
| 75 | 5 | 2% | 99% | 0% |

A running target gives about 50 ms of give before a shot misses by a box, which is why low pings hit either way.

## Not verified

- Nobody has played this. Everything ran on one machine, against bots and simulated networks, so there's been no real latency, jitter or loss.
- The red and green boxes of `sv_unlag_debug` are measured, but haven't been looked at in a live fight.
- A click's sound and projectile are shown at once, but how that feels hasn't been tried, and neither has the shot switch on `e1m2`.
- The rewind and the view clock are tested, but how they feel can't be, and their numbers are starting points.
- The Multiplayer menus were only walked with a pretend host.
- Connecting through a router hasn't been tried. Hosts may need to forward UDP port 26000.

## Limitations

- Traffic isn't encrypted and anyone who can reach the port can join, which is fine for direct play between friends and not for open servers.
- The fair play zone's numbers, projectile easing and the rest of this are starting points until real players try them.
- Predicted rocket jumps leave out quad damage, and other players' rockets aren't predicted.
- A muzzle flash and recoil still wait for the server. A shot's sound and projectile don't.
