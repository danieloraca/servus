# Architecture

Servus keeps simulation behaviour independent from presentation and platform code.

```text
servus-game ------> servus-content ------> servus-sim
     |                                      ^
     +--------------------------------------+
```

The simulation advances in fixed ticks and changes through explicit commands. It must remain
deterministic for a given initial state and command sequence. This supports repeatable tests,
saves, replays, balancing tools, and a possible authoritative multiplayer server.

Game content depends on public simulation types and validates definitions before they reach the
simulation. Rendering, input, audio, and engine integration belong only in `servus-game`.
