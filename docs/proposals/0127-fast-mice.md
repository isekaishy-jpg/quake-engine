# Proposal 0127: mice that report 8,000 times a second

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The open question was whether one message for each raw event keeps up with an 8 kHz mouse, to be answered by measuring on real hardware, with a dedicated input thread as the fallback.

Following a report through the code answers most of it without the hardware. Each one, on the main thread:

1. Makes a one-item `Vec`.
2. Goes through `receive_input_batch`, and so all of input delivery.
3. Then runs `advance`: pumping the display, reaping windows, pumping services, and preparing a redraw.

So at 8 kHz the whole host would step 8,000 times a second, all to add up two numbers that are read once a frame. At 200 frames a second that's 40 steps for each frame, and it's all on the thread that has to start the next frame.

A thread wouldn't help. Raw input arrives as `WM_INPUT` messages on the thread that registered for it, and Windows only keeps so many. What matters is emptying them cheaply, and the work to cut is ours, not the delivery.

## Design

**`MotionGather` adds up raw movement and hands it over as one.**

- Movement from the same device to the same window joins what's held.
- It's delivered when the message queue empties, just before the host runs, so it costs no latency: a frame sees everything that had arrived.
- Anything else flushes it first, so a click never overtakes the movement before it. That's every window event, and any other device event.
- `device_event` no longer runs `advance`. `about_to_wait` does, once the queue's empty, as it did already.

Sums are exact, in `f64`, and nothing's dropped. What's lost is only that the game can't see each report apart, and it never needed to: the look just adds them up.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** none for play. Something that wanted each report's own time would need them kept, and nothing does.
- **Acceptance criteria:** a second of an 8 kHz mouse is one delivery with nothing lost, and movement stays ahead of what comes after it.
- **Future extension:** none planned.

## Review

Those are tested, along with movement for another window not being added to this one. All 173 of the host's tests pass.

**A real 8 kHz mouse still needs a person.** The `stats` console command prints motion events and look angles, and `show_input` shows what arrives. With this, the motion count goes up by about the frame rate rather than the report rate. If the look ever lags the hand on one, that's the place to start.
