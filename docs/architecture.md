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

## Spatial simulation

The simulation owns a bounded tile grid. Infrastructure construction commands include a grid
position, and the simulation validates the service footprint against map bounds and existing
occupancy before spending credits. A rejected command leaves the complete simulation state
unchanged, which keeps command replay and future multiplayer synchronization predictable.

Construction reserves a service's complete footprint immediately. New services begin in an
`UnderConstruction` state and contribute no request capacity. At the start of each tick their
remaining construction time decreases; services reaching zero become `Operational` before that
tick's traffic is processed. The tick report includes the IDs of completed services so a client
can trigger visuals and notifications without reconstructing state changes.
