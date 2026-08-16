# Servus

Servus is an infrastructure strategy game in its earliest prototype stage.

The repository is a Cargo workspace split into:

- `servus-sim`: deterministic, engine-independent simulation;
- `servus-content`: game definitions and validation;
- `servus-game`: the Bevy client and terminal demo.

The current vertical slice supports deterministic ticks, positional infrastructure construction,
map bounds and occupancy, traffic capacity, dropped requests, and revenue.
Infrastructure reserves map space while under construction and contributes capacity only after
its construction timer completes.
The Bevy client starts a playable empty-map scenario and renders infrastructure states, directed
connections, live simulation metrics, and objectives in a native window. A separate deterministic
three-tick ASCII scenario remains available for headless debugging.
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
cargo run -p servus-game --bin servus-ascii
```

In the graphical client:

- select a Gateway, Load Balancer, Application Server, or Firewall with 1, 2, 3, or 4;
- click a highlighted free map tile to build;
- press C, then click a source service and destination service to create a directed connection;
- press Escape to cancel connection mode;
- right-click a service to inspect its state, capacity, links, and current traffic;
- watch yellow request markers follow the exact traffic routed through each link;
- use - and + to decrease or increase incoming demand by 50 requests per tick;
- move the camera with WASD or the arrow keys and zoom with the mouse wheel;
- press Space to pause or resume simulation ticks;
- complete all objectives to reach victory, or press R to restart the scenario.

Cyberattacks arrive every eight ticks. An exposed application server is disrupted for two ticks.
A firewall blocks the attack only when every directed path from an Internet Gateway to the target
passes through an operational firewall; bypass connections remain vulnerable.
During a breach, redundant application servers can keep requests flowing through a load balancer.
Each request dropped while infrastructure is disrupted costs one credit, so partial capacity limits
the damage and full failover avoids it.
