# Proposal 0060: incoming blasts

Status: accepted, 2026-11-04, increment 9. It extends [proposal 0053](0053-rocket-jumps.md).

## Research

- Proposal 0053 left out other players' rockets, saying nothing told a client when they were fired. That was wrong, as the human pointed out: every client hears every launch, with its place, speed and exact time, or it couldn't draw them.
- `T_MissileTouch` does 100 to 120 to what it hits directly, and leaves that out of the blast. `T_RadiusDamage` halves damage only for the one who fired it.
- `T_Damage` applies the attacker's quad, which a client can't see in the stats of someone else.
- A grenade goes off 2.5 seconds after it's fired, or on touching a player, and the path of a bounce is already known exactly.

## Design

- A launch carries how strong it is, set by the scripts, and its fuse.
- Everything that blows up is followed, whoever fired it. It's mapped to our own commands from the time of its launch, in ticks, as our own shots are.
- A rocket that reaches our box hits us directly. `direct_damage` is the middle of its range.
- A shot that reaches another player's box goes off there, using the boxes every snapshot already holds. The box of whoever fired it doesn't count.
- A grenade goes off at its fuse, on its path, or on reaching us.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a direct hit's damage is random, so it can be out by up to 80 units a second until the server's numbers arrive. A shot fired so close that it lands within a round trip can't be predicted, because the launch hasn't reached us yet. Other players' boxes are as old as the last snapshot, so a near miss on them can be judged wrongly. Monsters' grenades in co-op aren't announced, so they aren't predicted. The server's numbers always win.
- **Acceptance criteria:** a blast near the player stays within 3 units of the server at 40 and 80 ms, including an attacker's quad and a rocket that hits someone else first, and a fuse does too. A direct hit and a grenade hit at least halve the error. Every case has a control with prediction off.
- **Future extension:** none planned.

## Review

See [review 0012](../reviews/0012-increment-9.md).
