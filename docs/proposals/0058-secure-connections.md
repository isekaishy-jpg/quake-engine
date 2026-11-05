# Proposal 0058: secure connections

Status: accepted, 2026-11-04, increment 9. It replaces the unsecured mode of [proposal 0046](0046-the-network.md).

## Research

- `netcode`'s secure mode encrypts every packet with random keys from a connect token, and stops replays. It expects a backend to hand out tokens. Its unsecured mode uses keys of all zeros, so anyone can read it.
- Direct play has no backend, so the only thing both sides can share is a password, if that. A plain Diffie-Hellman exchange gives keys, but a password mixed into one can be guessed offline by anyone who talks to the host once.
- SPAKE2 is made for this: a wrong guess learns nothing, and each guess needs a live exchange. The [`spake2`](https://crates.io/crates/spake2) crate is RustCrypto's. Its README says it has never had an independent audit, though the curve code under it is widely used.
- Tokens are about 2 KB, more than fits in one packet, and a small request for a big answer would let someone aim traffic at a third party.
- `NetcodeServerTransport` owns its socket, so nothing can share the port. `renetcode` underneath it can be driven directly.

## Design

- The host is its own token issuer. It makes a new private key each time it starts hosting.
- Client and host run SPAKE2 with the password, which is empty for an open game. The token goes across sealed with ChaCha20-Poly1305 under the result, and `netcode` takes over from there.
- It all happens on the game's own UDP port, by reading the socket in `engine_net` instead of through `NetcodeServerTransport`, so hosts forward nothing new.
- A request is 1200 bytes, always more than the answer. The token comes in two parts, each asked for and sent again if lost.
- An address can start one exchange a second, everyone together sixteen, and 64 are remembered. An answer repeats part of the request, so nobody can answer without seeing it.
- `sv_password` guards a game and `password` joins one. Neither is saved. The Multiplayer menus have a place for each.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** on its own this says nothing about who the host is, which [proposal 0059](0059-lasting-identities.md) adds. `spake2` is unaudited. A password guessed online costs a second a try.
- **Acceptance criteria:** the right password gets in, and a wrong one is refused with a reason, in tests and between real processes. A client with no token never gets in. Nothing readable crosses the wire, a changed packet is dropped and sent again, an answer is never bigger than its request, and 40 guesses in under a second get one answer. All of it holds through a lossy link.
- **Future extension:** tokens from a real backend, with the later server work.

## Review

See [review 0012](../reviews/0012-increment-9.md).
