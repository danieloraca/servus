# Servus

Servus is an infrastructure strategy game in its earliest prototype stage.

The repository is a Cargo workspace split into:

- `servus-sim`: deterministic, engine-independent simulation;
- `servus-content`: game definitions and validation;
- `servus-game`: the executable and, later, engine integration.

The current vertical slice supports deterministic ticks, positional infrastructure construction,
map bounds and occupancy, traffic capacity, dropped requests, and revenue.
Infrastructure reserves map space while under construction and contributes capacity only after
its construction timer completes.
The executable renders a three-tick ASCII map showing that lifecycle without requiring a graphical
engine.
Incoming traffic now requires a directed, fully operational path from an Internet Gateway to an
Application Server. Network links have their own construction cost and are shown in the terminal
view.
A Load Balancer can distribute traffic to several servers, but its own throughput can become the
solution's bottleneck.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p servus-game
```
