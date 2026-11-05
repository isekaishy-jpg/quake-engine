# Proposal 0059: lasting identities

Status: accepted, 2026-11-04, increment 9. It builds on [proposal 0058](0058-secure-connections.md).

## Research

- With no password, nothing said who a host was, so someone in the middle could pose as one. The human asked why players and servers don't each have a token of their own.
- A shared secret token has a catch: on a first connection you can't yet tell who you're talking to, so you could hand it to an impostor. Signatures don't have that problem, because the private half never crosses the wire. SSH works this way.
- `ed25519-dalek` is a widely used signature crate, and 2.2 uses the curve code already here through `spake2`.
- The human's design for changing keys: give out the old key with the new one until there's nobody left to give it to.

## Design

- Each player and each host has a key pair, made once and kept in the user folder as `player.key` and `host.key`.
- A player signs what they said in the exchange, and the host signs what both sides said. Someone in the middle can only read anything by changing those messages, and then the signatures fail.
- A client remembers each host's key by address in `known_hosts.txt`. Next time it has to be the same key, or lead to it, or the client refuses.
- The first visit has nothing to compare with, so a host has a code, shown when it starts hosting and by `hostcode`. `connect address code`, or the Join menu, checks the host against it. The code is 80 bits of the key.
- `rotate_host_key` changes the key. The old key signs a link to the new one, and a client follows links from the key it knows. Each link notes every player known at the time, and one by one they're crossed off as they join. When nobody's left, the link is dropped. A host keeps eight at most.
- The host learns a player's key inside their token. `status` shows the start of it, and `kick`, `ban`, `unban` and `bans` act on it. Bans are kept in `banned_players.txt` and turned away before a token is made.
- `forget_host` clears a remembered host, for an address that's now someone else's.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a first visit with no code is taken on trust, as SSH's is. A rotation is for keeping keys fresh and can't rescue a stolen key, because a thief can sign links too: that takes a new identity and a new code. Keys are kept by address, so a host whose address changes is met afresh. A banned player can make a new key by deleting theirs, so a ban stops the same player coming straight back, not someone determined. A password is still the way to keep a game private. A player who stays away through more than eight rotations needs the code again.
- **Acceptance criteria:** a different host at a remembered address is refused, and so is the wrong code. A rotated key is followed through every link, an old code still works, and a link not signed by the old key counts for nothing. A link stays until the last known player has it, then goes. A changed request gets no answer. A banned player is refused, and that lasts through a restart. Real processes do all of this with real files.
- **Future extension:** reserved names and stats that follow a player.

## Review

See [review 0012](../reviews/0012-increment-9.md).
