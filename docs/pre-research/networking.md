# Networking

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This is the networking investigation so far. Nothing here is decided. The online requirements for the offline engine are in [stack.md](stack.md#online-feel).

## Recommendation so far

| Area | Recommendation | Depends on |
|---|---|---|
| Model | Server-authoritative snapshots, delta-compressed against the last acknowledged state. A base to improve on, see [Beyond Quake 3](#beyond-quake-3) | Nothing |
| Local player | Prediction with command replay, reusing the render tick | Nothing |
| Other entities | Interpolation buffer of about 2 ticks, tunable | Testing |
| Commands | Every packet carries the last 3, as QuakeWorld does | Nothing |
| Transport | `renet` with `renet_netcode`, behind our own interface. Decided | Nothing |
| Replication | Our own layer | Nothing |
| Hit detection | Unlagged-style server rewind using the client's exact view time, built in from the start | Nothing |
| Fairness | A fair play zone around the lobby's typical ping, see [Fair play zone](#fair-play-zone) | Tuning |
| Firing | A [weapon sub-tick](#weapon-sub-tick) for exact timing, plus a [half tick](#half-tick) with fire commands sent on click. Decided | Nothing |

## How Quake's netcode evolved

| | WinQuake (NetQuake) | QuakeWorld | Quake 3 |
|---|---|---|---|
| Prediction | None | Yes, replays unacknowledged commands | Yes |
| Entity updates | Each message is relative to a fixed baseline | Delta against an acknowledged frame | Delta against the last acknowledged snapshot |
| History | None | 64 frames (`UPDATE_BACKUP`) | [32 snapshots per client](https://fabiensanglard.net/quake3/network.php) |
| Reliable data | Stop-and-wait, one message in flight | One reliable message in flight, sharing the sequenced channel | Small reliable layer, game state in snapshots |
| Lost input | Lost | Each packet repeats the last 3 commands | Recovered through redundancy |
| Packet size | 1,024 bytes | 1,450 bytes | Split at 1,400 bytes |
| Spoofing defence | None | Challenge and `qport` for bad NAT | Challenge |
| Sending | Whenever the client can accept it | Limited by a per-client `rate` | Rate limited |

QuakeWorld and WinQuake details are from the source in `reference/quake`.

Quake 3's model is the cleanest. Everything is a delta against the last state the client confirmed, so a lost packet needs no special handling: the next delta simply covers both changes.

## Real traffic

Measured from the three stock demos, which store raw server messages. These are single-player recordings, so they show entity traffic, not a busy multiplayer game.

| Demo | Messages per second | Mean size | 99th percentile | Largest | Average |
|---|---:|---:|---:|---:|---:|
| `demo1` | 12.7 | 170 B | 357 B | 597 B | 17.3 kbit/s |
| `demo2` | 13.8 | 135 B | 286 B | 594 B | 14.8 kbit/s |
| `demo3` | 13.0 | 162 B | 274 B | 600 B | 16.9 kbit/s |

At 72 messages per second those sizes come to about 80–100 kbit/s per client, before delta compression. So bandwidth isn't a constraint at Quake's scale, and every update fits in one packet under a 1,200-byte budget. That gives room for the separate tick and send rates to start equal. Larger modern maps and busy multiplayer will send more, and that hasn't been measured.

## Replication model

| Model | How it works | Fit |
|---|---|---|
| Deterministic lockstep | Send inputs only, all peers simulate identically | Poor. Needs strict determinism across Luau and physics, stalls on late input, and has trouble with late joins |
| [State synchronization](https://gafferongames.com/post/state_synchronization/) | Both sides simulate, the server sends prioritized state | Suits large physics scenes. Pops and divergence are the cost |
| [Snapshots](https://gafferongames.com/post/snapshot_compression/) with delta compression | The server sends what changed since the last acknowledged state | Best fit. It's what Quake's line settled on, and game logic stays on the server |

Snapshot deltas are efficient: Glenn Fiedler's example took 901 cubes from 17.4 Mbps towards a 256 kbps target, with a still scene around 15 kbps. Quake's entity counts are far smaller.

The rest follows standard practice. The client [replays unacknowledged commands](https://www.gabrielgambetta.com/client-side-prediction-server-reconciliation.html), and other entities render slightly in the past from a buffer. Source [defaults to 100 ms](https://gist.github.com/CoolOppo/fe0586836de3fb2f90f9) at its lower update rates. At 72 updates a second, 2 ticks is about 28 ms, which is why a high send rate pays off.

## Transport options

Activity figures are from crates.io in November 2026.

| Option | Recent downloads | Last release | Notes |
|---|---:|---|---|
| [`renet`](https://github.com/lucaspoffo/renet) | 40,000 | Oct 2026 | Reliable and unreliable channels, fragmentation. Encryption and auth in `renet_netcode`, Steam in `renet_steam`, custom transports allowed. Message-only, with no replication |
| `quinn` (QUIC) | 142,700,000 | Sep 2026 | Very widely used, encrypted by default. Datagrams stay subject to congestion control, and it's not game focused. No game-traffic comparison turned up |
| [GameNetworkingSockets](https://github.com/ValveSoftware/GameNetworkingSockets) | n/a | n/a | Strongest feature set: lanes, P2P, AES-GCM, loss simulation. C++ with BSD-3 licence, and the `gns` Rust binding is barely used |
| `steamworks` | 153,000 | May 2026 | Reaches [Steam Datagram Relay](https://partner.steamgames.com/doc/features/multiplayer/steamdatagramrelay), which hides player IPs. Steam partner only |
| `rusty_enet` | 17,000 | Nov 2026 | Pure Rust ENet. No encryption |
| [`naia`](https://github.com/naia-lib/naia) | 750 | May 2026 | ECS-agnostic replication, but prediction is left to you, and it has little use |
| `lightyear`, `bevy_replicon` | 45,000, 76,000 | Oct 2026 | Bevy only, so they're out |
| `laminar`, `netcode` | Low | 2021, 2017 | Abandoned |
| Our own UDP | n/a | n/a | QuakeWorld's channel is 452 lines, but security would be ours to solve |

**`renet` looks like the best fit.** It's engine-agnostic at the core, gives us the channels we need, and leaves replication to us, which we want because it has to fit the slot table, Luau and prediction. Steam stays available as a transport.

**Why our own replication:** `lightyear` and `bevy_replicon` need Bevy, and `naia` is the only independent option, with a small user base and no prediction. Quake's delta encoding is simple, and QuakeWorld shows how little code it takes.

**Keep it behind an interface.** Offline play uses an in-memory transport, as the original does with `net_loop.c`, and I didn't see a memory transport in `renet`. An interface lets us swap the transport later without touching replication.

## Beyond Quake 3

Quake 3's snapshots are an adequate base, but they aren't where this should stop. Its defaults were a 20 Hz server and no lag compensation. What we'd change:

| Area | Quake 3 default | Plan | Why |
|---|---|---|---|
| Server rate | 20 Hz | 72 Hz | A fast player moves 50 units per tick at 20 Hz and 14 at 72 Hz |
| Movement step | Frame-rate dependent unless `pmove_fixed` is set | Fixed tick | Same movement for everyone |
| Hit detection | At current server positions | Rewind, see below | Aim at what you see |
| Input timing | Rounded to the frame | Timestamped within the tick | Idea from [CS2's sub-tick](https://senseos.gg/guides/counter-strike-2/subtick/), though only third-party descriptions turned up |
| Offline | Separate loopback path | Full netcode stack even offline | [Titanfall 2's single-player ran this way](https://mas-bandwidth.com/some-history-around-titanfall-and-apex-legends-multiplayer/) |
| Entity priority | Everything visible, every snapshot | Priorities under a bandwidth budget, when needed | From [the Tribes model used by Halo: Reach](https://edgegap.com/blog/game-backend-deep-dive-halo-reach-netcode-host-migration). Not needed at Quake's entity counts |

Overwatch's approach of a server command buffer plus client time dilation looks useful for jitter. I only found community write-ups of that talk, so it needs checking against the GDC session. Nothing usable turned up on Rocket League's netcode, Diabotical or Reflex Arena.

## Lag compensation

[Unlagged](https://www.ra.is/unlagged/solution.html) by Neil Toronto is the reference here. It brought backward reconciliation to Quake 3, and [InstaUnlagged](https://lvlworld.com/thread/562/4128/find) applied it to instagib. When a hitscan weapon fires, the server moves other players back to what the shooter saw, runs the trace, and restores them.

What it does right, confirmed in [its code](https://github.com/ec-/baseq3a/blob/master/code/game/g_unlagged.c):

- Stores each player's origin and box per server frame, as drawn by clients.
- Finds the two records around the shot time and **interpolates between them**, in `G_TimeShiftClient`.
- Uses a timestamp with the command, not just an average ping.
- Corrects for skipping players, so shots still land.

[Its documented weaknesses](https://www.ra.is/unlagged/faq.html) are the things to design around:

| Weakness | Plan |
|---|---|
| Players get shot around corners | Inherent to favouring the shooter. Cap the rewind. [One article](https://danieljimenezmorales.github.io/2023-10-29-the-art-of-hit-registration/) gives 250 ms for Battlefield 4 and Overwatch, though I couldn't confirm that in patch notes. [Valorant cut its limit to 140 ms](https://www.sportskeeda.com/valorant/news-valorant-patch-6-02-official-notes-server-rewind-limit-140-ms-200-ms-voice-evaluation-beta) |
| Doors and platforms aren't rewound | [Noted as unexplored](https://www.ra.is/unlagged/whatnext.html). Rewind movers too |
| Boxes don't match models when jumping | Keep boxes for the baseline, and revisit with real hitboxes later |
| Projectiles aren't compensated | See [Projectiles](#projectiles) |
| Hits show late but trails show at once | Separate predicted effects from confirmed results |
| Works best with ping gaps under about 100 ms | Fine for now without external servers |

## Trailing hitboxes

The problem is a difference between where the target is drawn on the shooter's screen and where the server tests the shot. It gets worse with speed because the same time error covers more distance. I simulated a fast strafing target, shots aimed at the exact centre of what the client draws, 50 ms ping and a 32-unit wide box:

| Rewind method | 72 Hz at 1000 u/s, worst 1% error | Centre shots hit | 20 Hz at 1000 u/s, worst 1% error | Centre shots hit |
|---|---:|---:|---:|---:|
| No rewind | 77 units | 0% | 150 units | 0% |
| Ping estimate, nearest tick | 16 units | 99.1% | 31 units | 63.7% |
| Client's time, nearest tick | 7 units | 100% | 25 units | 64.0% |
| Client's time, interpolated | 0 units | 100% | 0 units | 100% |

The time error matters a lot at speed. With a ping-based estimate at 72 Hz and 1000 u/s:

| Timing error (standard deviation) | Worst 1% error | Centre shots hit |
|---:|---:|---:|
| 2 ms | 10 units | 100% |
| 5 ms | 16 units | 99.1% |
| 10 ms | 27 units | 86.9% |
| 20 ms | 52 units | 57.9% |

With 5% snapshot loss, the interpolated method still hit 100% of centre shots at 1000 u/s, with a worst-case error of about 10 units.

**The likely causes of trailing are all timing mismatches:**

- **Rounding to ticks.** The server tests a stored tick while the client drew a blend. [A Source analysis](https://www.teamfortress.tv/52942/fix-for-interpolation-based-hitbox-desync) found interpolation time rounded to whole ticks, though that thread disagrees on how much it matters.
- **Estimating from ping.** Jitter turns directly into position error.
- **Low tick rates,** which make both worse.
- **Discrete hit tests against smooth visuals,** so the box steps while the model glides.

**The plan:**

1. **Each shot carries the exact view time** the client was rendering, as a tick plus fraction. The server checks it's within limits.
2. **Client and server share the same interpolation code,** using the same quantized positions the client received.
3. **The shot's own time is exact too,** so a fast-moving shooter's position isn't rounded to a tick. At 1000 u/s that's up to 14 units of eye position.
4. **Keep 72 Hz history and cap the rewind.**
5. **Build a debug view early,** showing the client's box beside the server's rewound box, as Unlagged's `cg_drawBBox` did. Trailing is hard to diagnose without it.

The zero error in the table is expected, because the model has client and server agree exactly. Real play adds clock error, which this design limits but doesn't remove. Animation-driven hitboxes would also need rewinding.

## Fair play zone

The idea: take the lobby's mean ping, leaving out outliers, and define a zone around it, for example 20–60 ms around 40. Players below a fixed 20 ms floor get delay added to bring them up to it. Players inside get full lag compensation. Players above it get partial compensation up to a ceiling, and nothing over the ceiling.

**This matters more for us than for most games.** With no external servers, one player hosts, and a host has zero ping. Bringing them up to the zone removes host advantage, and treating the host as an ordinary client over loopback makes that natural.

**There's research behind it.** [Zander et al. (2005)](https://dl.acm.org/doi/10.1145/1178477.1178493) balanced delay between players and improved fairness in shooters. [Riot injects latency](https://lolesports.com/en-GB/news/riot-games-tech-blog-artificial-latency-for-remote-competitors) to equalize players at a target ping for remote competition. The known cost is lost responsiveness for low-ping players, and [adaptive delay](https://web.cs.wpi.edu/~claypool/papers/adaptive-fdg-26/), applied only during fights, reportedly keeps the fairness and feels better. I only read abstracts and summaries of these papers.

### Below the zone

Prediction changes the usual cost. Added delay doesn't affect your own movement or aim, because those are predicted locally. It only makes other players slightly older on your screen and your actions land slightly later. Exact view times mean the rewind already accounts for it.

The delay goes in the server's command and snapshot queues, split between directions. It only brings players up to a fixed 20 ms floor, which is already very low, so few players are affected and a small lobby can't punish a good connection.

### Above the zone

Testing perfectly aimed shots shows the effect isn't gradual:

| Uncompensated time | 320 u/s target | 600 u/s | 1000 u/s |
|---:|---:|---:|---:|
| 15 ms | 100% | 100% | 100% |
| 20 ms | 100% | 100% | 2.5% |
| 30 ms | 100% | 4.9% | 0% |
| 40 ms | 100% | 3.0% | 0% |
| 60 ms | 8.6% | 0% | 0% |

Shots hit until the error passes half the box, 16 units, then fall off a cliff. So the free margin is 16 divided by target speed: about 50 ms against a running player, and 16 ms against a very fast one. Beyond that, players have to lead, which is a skill they can use.

A shooter at 100 ms ping with a zone top of 60:

| Policy | Hits at 320 | 600 | 1000 | Victim rewound by |
|---|---:|---:|---:|---:|
| No rewind | 0% | 0% | 0% | 0 ms |
| Hard cap at the zone | 100% | 3% | 0% | 88 ms |
| Cap plus half the excess | 100% | 100% | 2.7% | 108 ms |
| Full rewind | 100% | 100% | 100% | 128 ms |

Rewind figures include the 28 ms interpolation buffer. Cap plus half the excess gives high-ping players a usable game against normal movement while still making them lead fast targets, and it limits how far victims get pulled back. At 108 ms, a player running at 320 u/s can be hit about 35 units after reaching cover.

### Choosing the zone

The zone sits around the mean ping, with outliers left out of the calculation. Pings over 100 ms don't count, unless they're the majority of the lobby. That keeps one bad or faked connection from moving the zone, while a lobby that's genuinely high still works:

| Pings | Counted | Mean | Zone | Result |
|---|---|---:|---|---|
| 8, 18, 25, 35, 40, 48, 60, 140 | All but 140 | 33 | 20–53 | 8 gets +12, 18 gets +2, and 140 is limited by the ceiling |
| 0 (host), 25–50, 120 | All but 120 | 32 | 20–52 | Host gets +20 |
| 5, 40, 180 | 5, 40 | 22 | 20–42 | 5 gets +15, and 180 is limited by the ceiling |
| 10, 200 | 10 | 10 | 20–30 | 10 gets +10. Faking 200 ms doesn't move the zone |
| 90, 110, 120, 130 | All | 112 | 20–132 | Nobody slowed, and the ceiling rises to 132 |

**Proposed rules:**

- Use the mean of server-measured ping, smoothed, and never a client-reported value. Leave out pings over 100 ms unless they're the majority.
- The zone runs from a fixed 20 ms floor up to the mean plus 20 ms.
- Added delay only brings a player up to the floor, so it's never more than 20 ms, and that only for a host. In a lobby where nobody reaches 20 ms, such as a LAN game, the floor drops to the highest ping present, so nobody is slowed for no reason.
- Set a ceiling on compensated ping: 90 ms, or the zone top if the lobby's ping is unusually high. Normal gameplay pings top out around 80–90 ms. Anyone over the ceiling gets no compensation at all.
- Between the zone top and the ceiling, compensate half the excess.
- Recalculate slowly, about every 30 seconds, and ramp changes gently so timing never jumps mid-fight.
- Show it to players, so they know when they're being delayed or need to lead.

With a 20–60 ms zone and a 90 ms ceiling, the deepest rewind is 75 ms of ping plus the interpolation buffer, about 103 ms. Players over the ceiling get no help at all and feel their full lag:

| Ping | Compensated | Left uncompensated | In practice |
|---:|---:|---:|---|
| 60 ms | 60 ms | 0 ms | Aim at what you see |
| 75 ms | 68 ms | 7 ms | Aim at what you see |
| 90 ms | 76 ms | 14 ms | Aim at what you see, up to about 800 u/s |
| 100 ms | None | 128 ms | No rewind. Lead everything |
| 120 ms | None | 148 ms | No rewind. Lead everything |
| 150 ms | None | 178 ms | No rewind. Lead everything |

Left uncompensated includes the 28 ms buffer once rewind is off.

This is a hard edge, so smoothed ping needs a margin to stop compensation flickering on and off around the ceiling: it turns off above 90 ms and only returns below about 85 ms.

A lobby with a mean of 120 has a zone top of 140 ms, so its ceiling is 140 ms and those players are treated normally among themselves.

**All of this is server settings.** The rules above are defaults, not hardcoded behaviour, so server owners can run the game the way they want. Names are proposals:

| Cvar | Default | Controls |
|---|---:|---|
| `sv_fair_enable` | 1 | The whole system. Off means plain lag compensation |
| `sv_fair_floor` | 20 | Added delay brings players up to this ping. 0 turns it off |
| `sv_fair_width` | 20 | Zone top is the mean plus this |
| `sv_fair_ceiling` | 90 | Ping above which there's no compensation |
| `sv_fair_ceiling_margin` | 5 | How far under the ceiling ping must drop to get compensation back |
| `sv_fair_partial` | 0.5 | Share of the excess compensated between zone top and ceiling |
| `sv_fair_outlier` | 100 | Pings over this are left out of the mean, unless they're the majority |
| `sv_fair_interval` | 30 | Seconds between zone updates |
| `sv_unlag_enable` | 1 | Hitscan rewind itself |
| `sv_unlag_proj` | 0 | Share of a shooter's compensated time that projectiles are moved forward. Off by default |

Settings are server-side only. Because a host could set `sv_fair_floor 0` to keep their own zero ping, the values should be visible to players in the server info, as QuakeWorld's serverinfo does, and changes announced through the existing server cvar flag.

**Expected lobby.** A typical Quake Live lobby today runs from about 10 ms (rarely) up to about 75 ms, with most players at 20–65 ms, and anything over 100 ms is close to unplayable. For pings of 10, 20, 30, 40, 45, 55, 65 and 75, the mean is about 42 and the zone 20–62:

| Ping | Effect |
|---:|---|
| 10 ms | 10 ms added |
| 20–55 ms | None |
| 65 ms | 1 ms left uncompensated |
| 75 ms | 6 ms left uncompensated |

So in an ordinary lobby the system barely shows: nobody needs to lead, and only the lowest pings are slowed at all. It mostly matters for hosts and for the occasional bad connection. Above 100 ms we warn the player and don't design around it.

**Limits of the idea:**

- It narrows peeker's advantage but doesn't remove it.
- The ceiling exists to stop abuse: over it, lag earns nothing and costs all your compensation, so faking lag can't buy a deeper rewind. View times are also checked against measured ping.
- Leaving outliers out of the mean stops a faked ping from moving the zone or the ceiling, and smoothing stops short lag switches.
- Projectiles aren't compensated by default. See [Projectiles](#projectiles).
- The values above are starting points from a simplified model. They need tuning with real players.

## Projectiles

Rewinding doesn't fit a projectile, because it flies for a while in the present. The shooter wants it to leave instantly and go where they aimed, and the target wants a fair chance to dodge.

| Game | What it does |
|---|---|
| Stock Quake, [Team Fortress 2](https://wiki.teamfortress.com/wiki/Lag_compensation) | Nothing. Projectiles aren't compensated |
| [Unlagged](https://www.ra.is/unlagged/walkthroughs.html) | Rewinds players 50 ms during missile movement, and advances projectiles 50 ms on clients, with an option to go further |
| Quake Live | `g_lagHax` antilag. Players report it covers about 80 ms, which is anecdotal, and I found no reliable description |
| [Overwatch](https://edgegap.com/blog/game-backend-deep-dive-overwatch-2016-netcode-architecture-rollback) | Predicts rockets on the client, with [occasional false visuals](https://comic.gamedev.net/forums/topic/701388-overwatch-predicted-rockets-analysis). This is from secondhand accounts of the talk |

Quake's own numbers, from the v1.01 QuakeC: rockets and nails fly at 1,000 u/s, and grenades at 600 u/s with bounce and a 2.5 second fuse. Rocket splash damage is 120 minus half the distance, so aim errors are forgiving. Nails need a direct hit.

### Design

**Projectiles are server truth.** There's one rocket, with one path, decided by the server. It spawns on the tick the shooter's command runs, and nobody gets it moved or adjusted for them. This is the fairest option: no shooter help, and the target keeps their full dodge time. The shooter leads by their own latency, as in stock Quake and TF2, and the fair play zone keeps that gap small: about 15 units at 20 ms and 28 units at 60 ms against a running target.

**Display doesn't change results.** Nobody can literally see the server's present, because everything on screen is either old or projected. If clients simply drew snapshots, each player would see the rocket at a different age, and the target's own predicted position runs ahead too. At 40 ms ping that's about 68 ms, or 68 units of rocket travel, so rockets would visibly miss and still hit you. Lower-ping players would also see it fresher, which is its own unfairness.

So clients draw the same server rocket on the same path, at the current tick:

- **Other players' projectiles** are drawn at the present moment. A rocket's path is fixed, so this is nearly exact, and it means every player sees it in the same place. The client has the map, so rockets stop at walls, which avoids Unlagged's [rockets sticking in the floor](https://www.ra.is/unlagged/whatnext.html). Grenades can be simulated the same way.
- **Your own projectile** appears when you fire. Your command runs on a known tick, so this is the same rocket on the same path, shown a little early. It's only a visual.

Neither changes where the rocket is or what it hits.

**So the work is on the client.** The server just runs the rocket and sends it. Everything that makes projectiles feel right is client-side display, which can be tuned without touching game results.

**Paths are fixed once fired.** A rocket flies straight, and a grenade's bounce is decided by its launch state, the map and the fixed tick. So a client doesn't need updates during flight: the server sends the launch state and tick once, then an end event when it explodes. Every client computes the same path, using the same movement code and map solids, which saves bandwidth and gives other players an accurate view for dodging. [Fish-Net works the same way](https://fish-networking.gitbook.io/docs/guides/features/lag-compensation/projectiles.md).

The exceptions are things clients can't know in advance: hitting a player, hitting a moving door, or anything script-driven. The server's end event settles those, and a client that has drawn a rocket past that point corrects to the explosion.

**The harder side is the shooter.** Their rocket appears before the server has confirmed it, so the client has to reach the same launch state on its own. Stock Quake gets in the way in three places:

| Source of mismatch | Stock behaviour | Plan |
|---|---|---|
| Autoaim | `aim()` bends rocket and grenade shots at the server (`sv_aim` 0.93) | Off by default. QuakeWorld turned it off too (`sv_aim 2`) |
| Random spread | Grenades add `crandom()` to their launch velocity | Seed randomness from the command's tick, so both sides compute the same result |
| Weapon rules | Refire timing and ammo live in game scripts on the server | Shared data the client can read, such as speed, spread and refire time. Scripts still decide results |

When the server's launch arrives, a small mismatch from prediction error is blended away over a few frames.

### Fire versus draw

There's a real trade-off in display, because a rocket lives in the present while other players are drawn in the past. At 40 ms ping that gap is about 68 ms. Whatever we pick, something on screen disagrees with something else. Results aren't affected.

| Whose view | Draw at the current tick | Draw in the same time as the players on screen |
|---|---|---|
| **Your own rocket** | Leaves the moment you fire. It reaches a moving target's drawn position slightly wrong, by about 22 units for a running target | Lines up with what you see, but leaves about 68 ms after you fire, which feels laggy |
| **An enemy's rocket** | Shows the true threat, so you dodge the real rocket. It first appears about 68 units ahead of the shooter's model | Leaves the muzzle cleanly, but it's old, so it can hit you before it appears to arrive |

**Proposed defaults:**

- **Your own rocket: fire instantly, no blending.** Instant fire is what you feel most, and a 22-unit visual offset is small next to splash. The server's explosion shows the real result.
- **An enemy's rocket: start at their muzzle and catch up fast.** It's drawn from the shooter's model, then eased onto its true path over about 100 ms. That means drawing it at about 1,700 u/s for that moment. [Fish-Net uses the same idea](https://fish-networking.gitbook.io/docs/guides/features/lag-compensation/projectiles.md), consuming a share of the remaining time each frame. At very close range there's no room to catch up, so the true position wins.

**Easing covers every projectile, not just rockets.** It works by adjusting the drawn time along the projectile's known path, so the projectile never leaves that path. That means one system handles rockets, nails, grenades and their bounces, plus anything new, as long as its path is fixed once fired. Your own projectiles can use it too, easing towards the timeline of the players on your screen during flight. Amounts can be tuned per projectile type: a long grenade arc has plenty of time to ease, while a nail at close range has almost none.

**Launch delay plus easing.** Sound, muzzle flash and recoil play the instant you fire, and your command goes out immediately. Only the drawn projectile starts a touch late, 5–10 ms at most, because anything like 20 ms can be felt. That's about one frame at 144 fps. Easing then works during flight.

**The goal is to halve the gap, not close it.** Chasing full accuracy would mean visibly slower projectiles for little gain. Correcting half of a 68 ms gap, with an 8 ms launch delay and the rest eased:

| Flight time | Drawn speed when closing all of it, no delay | Drawn speed when closing half, 8 ms delay |
|---:|---:|---:|
| 200 ms | 66% | 87% |
| 400 ms | 83% | 94% |
| 800 ms | 92% | 97% |

That leaves about 11 units of visual offset against a running target, down from 22, and the speed change is small enough to be hard to notice. These are starting values for testing in play.

**To test: scaling with ping.** Correction is half of ping, so it's only 10 ms at the 20 ms floor. It's one client setting, on by default, and the player can change it whenever they like. The engine never switches it for them:

| Ping | Correction | Offset left at running speed |
|---:|---:|---:|
| 20 ms | 10 ms | 12 units |
| 40 ms | 20 ms | 15 units |
| 60 ms | 30 ms | 19 units |
| 75 ms | 37 ms | 21 units |

That halves the network part of the gap and leaves the 28 ms buffer alone.

Both are client-side and can be settings, because they change nothing about what hits. Delaying the real shot on the server wouldn't help, because it adds actual latency and changes how the weapon plays.

### Optional forward step

This isn't part of the baseline and can be built later if it's wanted. Server owners could enable `sv_unlag_proj`, which moves a projectile forward at spawn by a share of the shooter's compensated time, testing against position history during that step. It favours the shooter and costs the target dodge time, so it's **off by default**. Numbers use a target at running speed and include the 28 ms buffer:

| Ping | `sv_unlag_proj` | Extra lead needed | Rocket first appears | Dodge time lost |
|---:|---:|---:|---:|---:|
| 40 ms | 0 | 22 units | At the muzzle | 0 ms |
| 40 ms | 0.5 | 11 units | 34 units out | 34 ms |
| 40 ms | 1 | 0 units | 68 units out | 68 ms |
| 75 ms | 0 | 33 units | At the muzzle | 0 ms |
| 75 ms | 0.5 | 18 units | 48 units out | 48 ms |
| 75 ms | 1 | 2 units | 96 units out | 96 ms |

Rocket errors are forgiving anyway: 22 units off costs about 11 splash damage.

### Still open

- **Rocket jumps.** Knockback changes real movement, so display alone doesn't cover it. Without prediction it arrives a round trip late, and easing can only soften the correction. Because a rocket's path is fixed, the client can work out when its own rocket hits the world and apply the same knockback formula locally, with the server correcting it if needed. That predicts movement only, not damage. Needs testing.
- **Rejected shots.** If the server refuses a shot, the early visual has to disappear cleanly.
- **Grenade bounces** have to match closely enough between client and server.
- **None of this has been built or tested.** The table is arithmetic.

## Weapon sub-tick

Weapons keep their own timing inside the tick, instead of snapping to tick boundaries. Movement and game logic stay on the fixed 72 Hz tick. It's a natural extension of timestamping shots, and it does four things:

**Exact fire rates.** Stock refire times don't divide evenly into 72 Hz ticks, so each one rounds up to the next tick:

| Weapon | Refire | At 72 Hz ticks | Slower by |
|---|---:|---:|---:|
| Axe, shotgun | 0.5 s | 0.500 s | 0% |
| Rocket launcher | 0.8 s | 0.806 s | 0.7% |
| Super shotgun | 0.7 s | 0.708 s | 1.2% |
| Grenade launcher | 0.6 s | 0.611 s | 1.9% |
| Nailgun, lightning gun | 0.1 s | 0.111 s | 11.1% |

Keeping the next fire time as an exact value and carrying the remainder gives the true rate. Nails and lightning go from 9 shots a second back to 10.

**Exact shot timing.** Each shot uses the moment it happened, so a moving shooter's eye position and the rewind time are both exact, which feeds directly into the [trailing hitbox plan](#trailing-hitboxes). A projectile's launch state records the same time, so its path starts where it should.

**Fair ordering.** Quake processes clients in slot order, so when two players fire on the same tick, the lower slot goes first and can win a trade it shouldn't. Sorting weapon events by time within the tick removes that. Same-instant shots can both land.

**Consistent feel.** Timing no longer varies with where a click falls in the tick.

**Cost:** about one byte per fire command, for a fraction of a tick with 54 µs resolution. Game scripts see a time value instead of the tick's time.

**Limits:**

- A client could claim the earliest moment in every tick. The gain is capped at one tick, 13.9 ms, and it can't exceed what the command's arrival allows.
- Scripted weapon logic has to use the event's time, so the Luau API needs to expose it.
- The original's frame timing rounded the same way, so 10 shots a second is a small change from how stock Quake plays at 72 fps. It can be a server setting.


### Half tick

**Decided.** A half tick is a light server pass midway between full ticks, giving weapons an effective 144 Hz without a full update. It only reads waiting fire commands, runs hit tests and spawns projectiles. There's no movement, physics, entity thinking or snapshot, and it does nothing if no one fired.

| Approach | Nailgun rate error | Server wait for a shot | Extra server work |
|---|---:|---:|---|
| Whole tick only | 11.1% | Up to 13.9 ms | None |
| Half tick only | 4.2% | Up to 6.9 ms | Small |
| Timestamps only | 0% | Up to 13.9 ms | None |
| Both | 0% | Up to 6.9 ms | Small |

The two solve different problems: timestamps make timing **exact**, and the half tick makes results **sooner**. To get the benefit, the client has to send a fire command as soon as you click, in its own small packet, instead of waiting for the next tick. That removes up to another 13.9 ms on the client side, for about 10 ms saved on average across both ends.

Positions at a half tick come from the same interpolated history as rewinds, so no extra state is needed. Results go out with the next full update, or straight away as a small event if we want faster hit feedback. It costs a small packet per shot, up to 10 a second with nails, and Luau weapon code may run between ticks, so it has to rely on event time. It can be a server setting.

## Accuracy by speed

With the final rules, using the typical lobby (zone 20–62 ms, ceiling 90 ms). Same simplified model as before: a strafing target, shots aimed at the centre of what the client draws, a 32-unit box, and 2 ms of clock error on the view time.

**Hitscan, share of centre shots that hit, ignoring time spent waiting for a tick:**

| Ping | Left uncompensated | 320 u/s | 600 u/s | 800 u/s | 1000 u/s |
|---:|---:|---:|---:|---:|---:|
| 10–60 ms | 0 ms | 100% | 100% | 100% | 100% |
| 65 ms | 1.5 ms | 100% | 100% | 100% | 100% |
| 75 ms | 6.5 ms | 100% | 100% | 100% | 100% |
| 90 ms | 14 ms | 100% | 100% | 99.9% | 84.7% |
| 100 ms and up | Everything | 0% | 0% | 0% | 0% |

Worst 1% error stays at 2–5 units inside the zone, and 6–19 units at 90 ms depending on speed.

**Projectiles, mean visual offset from the target drawn on the shooter's screen, in units:**

| Ping | Correction | 320 u/s | 600 u/s | 800 u/s | 1000 u/s | Without correction at 320 / 1000 |
|---:|---:|---:|---:|---:|---:|---:|
| 20 ms | 10 ms | 12 | 22 | 30 | 37 | 15 / 47 |
| 40 ms | 20 ms | 15 | 28 | 37 | 47 | 21 / 66 |
| 60 ms | 30 ms | 18 | 34 | 45 | 56 | 27 / 84 |
| 75 ms | 38 ms | 20 | 38 | 51 | 63 | 31 / 97 |
| 90 ms | 45 ms | 22 | 42 | 56 | 70 | 35 / 110 |

This offset is visual only: the server's result doesn't change. Each unit off costs half a point of rocket splash damage at most.

### With tick waits and the half tick

The tables above leave out one thing: a shot also waits to be sent and processed. Inside the zone that's fully rewound, so it doesn't matter. Above the zone, the wait adds to the uncompensated time. These figures include it, assuming the wait isn't compensated there.

**Hitscan, share of centre shots that hit:**

| Ping | Whole tick: 320 | 600 | 800 | 1000 | Half tick and send on click: 320 | 600 | 800 | 1000 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 40–60 ms | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |
| 65 ms | 100% | 97% | 77% | 55% | 100% | 100% | 100% | 100% |
| 75 ms | 100% | 85% | 49% | 26% | 100% | 100% | 100% | 99% |
| 90 ms | 100% | 44% | 13% | 3% | 100% | 100% | 82% | 33% |
| 100 ms and up | 0% | 0% | 0% | 0% | 0% | 0% | 0% | 0% |

The half tick matters most just above the zone, and it's what keeps the whole of a normal lobby accurate at speed. Always compensating the wait would match that accuracy, but it wouldn't make results any sooner, so the half tick is the plan.

**Projectiles:** average waits fall from about 14 ms to 3.5 ms, which removes about 3 units of visual offset at 320 u/s and 10 units at 1000 u/s. At 40 ms ping that's 18 down to 15 units, and 58 down to 48.

**Fire rate:** nails and lightning go from 9 to 10 shots a second with timestamps.

## Prediction cost

Prediction won't limit frame rate. I ran a rough Quake-style movement step (friction with edge check, acceleration, sliding, stair stepping, ground check) on `box3d` rays against the hull solids of `e1m1`, `e4m3` and `dm3`, 2 million steps each:

| Measure | Result |
|---|---:|
| Mean per step | 1.1–1.6 µs |
| 99th percentile | 4.0–4.8 µs |
| Rays per step | 2.9–3.8 |
| One short projectile ray | 0.4 µs |

Per frame, at 240 fps (a 4,167 µs budget), allowing 5 µs per step:

| Work | When | Cost |
|---|---|---:|
| Render tick | Every frame | 5 µs |
| Replay after a snapshot, 75 ms ping | 72 times a second, about 7 steps | 35 µs |
| Replay at 150 ms | 72 times a second, about 12 steps | 60 µs |
| 50 active projectiles | One step each per tick | 20 µs |
| Interpolating and easing entities | Every frame | A few µs, estimated |

That's about 1–2% of the frame in the worst case. Server-side, 16 players cost under 100 µs of a 13,900 µs tick.

**Caveats:**

- Not included: Luau, `box3d` dynamics, or rendering, which will dominate.
- Occasional multi-millisecond samples appeared. They look like operating system interruptions, but I didn't confirm that.
- **This wasn't a correct mover.** The player ended up inside solid in 6–13% of checks, and stuck steps may be cheaper than real ones. `world_cast_ray_closest` ignores a ray that starts inside a shape, so real movement needs Quake's surface margin and a stuck check.

## Security

The original's history is a warning. QuakeWorld had to add challenges against spoofing, and its [2.33 notes](../../reference/quake/QW/release233_notes.txt) cover crashes from foreign and oversized packets.

- **Use authenticated, encrypted packets.** [netcode](https://github.com/TeamHypersomnia/netcode.io) uses short-lived connect tokens and protects against amplification, replay and spoofing. Tokens need a small web backend, and `renet` also has an unsecured mode for LAN and development.
- **Treat every client packet as hostile,** and put hard limits on sizes and rates. Rust removes the memory-safety side of the old bugs but not the logic side.
- **The server stays authoritative,** which limits what a cheating client can do. It doesn't stop aimbots.

## Settled for now

| Question | Answer |
|---|---|
| Hosting and external servers | Out of scope for now, part of the later server work. That includes Linux dedicated servers |
| Steam | Not planned. `renet_steam` remains an option if that changes |
| Players per game | A server setting, not a design limit |
| Game modes | Standard modes, anything beyond that is configuration |

Without Steam or external servers, player hosting means direct connections, so hosts may need port forwarding. Relay and NAT traversal belong with the later server work.

## Not tested

- `renet` hasn't been run here, on its own or against packet loss.
- QUIC datagrams weren't compared directly. The one [relevant paper](https://dl.acm.org/doi/10.1145/3696355.3699698) wasn't accessible.
- Client clock sync, jitter handling and rate control still need design work.
- Multiplayer traffic hasn't been measured.
