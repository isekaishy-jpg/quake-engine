# Proposal 0010: the main loop and fixed tick

Status: accepted, 2026-11-03, increment 1.

## Research

- DESIGN.md decouples rendering and sound from physics, and the [pre-research record](../pre-research/stack.md#fixed-tick) settled on 72 Hz.
- [Fix Your Timestep](https://gafferongames.com/post/fix_your_timestep/) gives the accumulator pattern, and clamps frame time so a stall can't cause a catch-up spiral.
- `solapp` offers `frame` callbacks and registered services. `SAServicePoint::Maintenance` is documented as "ordinary host maintenance, including idle and minimized windows", and `PreUpdate` runs before a frame. A service reports `WaitUntil(deadline)`, which schedules its next visit.
- Frames only arrive when a redraw is admitted, so a tick driven by `frame` would be at risk of stopping when minimized. That was an open question, and [review 0003](../reviews/0003-increment-1.md) answers it.
- `Duration::from_nanos(1e9 / 72)` truncates, so an accumulator built on a fixed interval drifts slightly. Counting ticks from total elapsed time with integer maths doesn't.

## Design

- `engine_core::time` holds `Tick`, `TickRate` and `TickClock`. They're pure and have no platform or I/O.
- `TickClock::advance(elapsed)` adds elapsed time, returns how many ticks are due, and limits catch-up. If more than a quarter second is due, the excess is dropped.
- `TickClock` also reports the fraction into the next tick, for interpolation and the render tick, and the time until the next tick.
- The simulation tick runs in a platform service registered for `PreUpdate` and `Maintenance`, so it doesn't depend on frames.
- The frame callback does presentation only.
- The clock only advances when ticks run, so a frame works out its own fraction with `fraction_after`, from the time since the clock last advanced. Review 0005 records what went wrong without it.

## Proposal

- **Selected approach:** the above, at 72 Hz.
- **Tradeoffs:** a service visit isn't exact, so ticks can arrive slightly late and in small bursts. The accumulator keeps the average right.
- **Acceptance criteria:** tests show exact tick counts over time with no drift, a capped catch-up, and a correct fraction. In the running engine, about 72 ticks a second are counted with the window visible and minimized.
- **Future extension:** the half tick and render tick use the same clock.

## Review

See [review 0003](../reviews/0003-increment-1.md).
