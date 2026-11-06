# Proposal 0128: online numbers, on the networks we can make

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

Interpolation delay, the rewind limit and the 5% loss limit were starting points, to be judged by playing over the internet. That needs people on real connections. What can be done from here is to find out what those numbers do on networks that behave like real ones, and that was missing a piece: `Conditioned` lost packets one at a time, and [proposal 0049](0049-bots-and-bad-networks.md) left bursts for later. Real networks lose them in runs.

So what's drawn was measured over 300 seconds of each network. A frame is **starved** when the view has caught up with the newest snapshot, and everyone stands still until the next arrives.

With the fixed delay of two ticks, or 28 ms, which covers one lost snapshot and no jitter:

| Jitter | No loss | 2% | 2% in bursts | 5% in long bursts |
|---|---|---|---|---|
| 0 ms | 0.00% | 0.01% | 0.74% | 3.03% |
| 20 ms | 0.00% | 0.38% | 0.93% | 3.13% |
| 40 ms | 1.60% | 1.77% | 2.04% | 3.50% |
| 80 ms | 4.18% | 4.22% | 4.26% | 5.12% |

At 120 frames a second, 2% is a stutter about twice a second. A bad connection isn't helped by a delay chosen for a good one, and a good one shouldn't pay for a bad one. So it has to follow the connection, as a jitter buffer does.

## Design

- **`ViewClock` follows the worst wait lately.** Each snapshot, it notes how long past a tick it was waited for, whether from jitter or from what was lost, and keeps the longest, halving over about six seconds. It draws a tick plus that behind the newest snapshot. It never goes under two ticks or over `MOST_VIEW_DELAY`, 100 ms.
- The clock already eases at up to 10% to stay on course, so the delay changing can't be seen as a jump.
- **`Conditions::burst`** is the chance that what follows a loss is lost too, with `net_fake_burst` beside the other `net_fake` settings.
- **The server allows for it.** A shooter's rewind was limited to what's compensated plus 3 ticks, which assumed the fixed delay. It's now `MOST_VIEW_DELAY` and a tick. Without this, hits fell to 92% in bursts, as shots were judged too recently. Bigger slack gives nothing away: a client claiming to draw further back than it is only gets to shoot at the past.

Measured again:

| Jitter | No loss | 2% | 2% in bursts | 5% in long bursts |
|---|---|---|---|---|
| 0 ms | 0.00% | 0.00% | 0.31% | 0.49% |
| 20 ms | 0.00% | 0.03% | 0.26% | 0.44% |
| 40 ms | 0.00% | 0.00% | 0.16% | 0.37% |
| 80 ms | 0.00% | 0.00% | 0.05% | 0.29% |

And what it costs, as the average delay:

| Jitter | No loss | 2% | 2% in bursts | 5% in long bursts |
|---|---|---|---|---|
| 0 ms | 19 ms | 22 ms | 38 ms | 70 ms |
| 20 ms | 24 ms | 28 ms | 42 ms | 73 ms |
| 40 ms | 39 ms | 41 ms | 48 ms | 77 ms |
| 80 ms | 64 ms | 64 ms | 66 ms | 85 ms |

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** after a bad moment others are drawn a little further back for some seconds. It's the price of them not stopping.
- **Acceptance criteria:** a clean connection stays at two ticks, a jittery one buys itself time, 2% loss rarely starves, what's drawn moves evenly, and shots still land.
- **Future extension:** none planned.

The other starting points stand. The 250 ms rewind limit is above all of this, and the 5% limit stays a stress margin, as the [design says](0046-the-network.md).

## Review

Bots over `Conditioned` networks now run with bursts in all of the network tests. With rewind, hits land 99% to 100% of the time at every ordinary ping, up to 75 ms, and 2% and 5% loss, and 0% to 41% without it at 60 ms and over. A full game of bots runs for minutes with no errors.

**Real people on a real network still need a person.** `net_fake_latency`, `net_fake_jitter`, `net_fake_loss` and `net_fake_burst` give any of these networks on one machine in the meantime.
