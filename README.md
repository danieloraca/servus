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
Operational infrastructure also charges a recurring cost each tick: 2 credits for a Gateway,
5 for a Firewall, 4 for a Load Balancer, and 8 for an Application Server. The client tracks
revenue, operating profit, capital invested, unpaid costs, and infrastructure ROI; exhausting the
budget while costs are due makes the solution insolvent.
Every service begins at the Starter tier and can be upgraded through Scaled to Enterprise. Upgrades
consume capital and take the service offline while work completes, but buy increasingly efficient
capacity in the same map tile. This creates a tradeoff: upgraded infrastructure is cheaper and more
space-efficient at scale, while multiple Starter instances provide safer redundancy.

| Service | Starter capacity / opex | Scaled capacity / opex / upgrade | Enterprise capacity / opex / upgrade |
| --- | ---: | ---: | ---: |
| Gateway | 250 / 2 | 600 / 3 / 40 | 1,500 / 5 / 80 |
| Firewall | 200 / 5 | 450 / 8 / 90 | 1,000 / 13 / 180 |
| Load Balancer | 150 / 4 | 350 / 6 / 60 | 800 / 10 / 120 |
| Application Server | 100 / 8 | 225 / 13 / 80 | 500 / 22 / 160 |
| Relational Database | 80 / 14 | 200 / 24 / 150 | 480 / 42 / 300 |
| Key-Value Store | 160 / 9 | 420 / 15 / 100 | 1,000 / 25 / 210 |
| Cache | 220 / 6 | 550 / 10 / 55 | 1,300 / 17 / 115 |

The first data-services pack introduces three different scaling shapes. A Relational Database is
expensive, slow to construct, and occupies 2×2 tiles; a Key-Value Store trades richer structure for
greater throughput; a Cache is cheap and fast but represents temporary data. These generic names
map to real-world families such as Aurora/Azure SQL/Cloud SQL, DynamoDB/Cosmos DB/Firestore, and
ElastiCache/Azure Managed Redis/Memorystore. Their workload-specific persistence and caching effects
will build on the catalog in a later slice.

The Bevy client starts a playable empty-map scenario and renders infrastructure states, directed
connections, live simulation metrics, and objectives in a native window. A separate deterministic
three-tick ASCII scenario remains available for headless debugging.
The current learning scenario starts with 1,000 credits, enough to experiment with one Starter
instance of every infrastructure type while retaining some capital for connections and operations.
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

- select a Gateway, Load Balancer, Application Server, Firewall, Relational Database, Key-Value
  Store, or Cache with 1 through 7;
- click a highlighted free map tile to build;
- press C, then click a source service and destination service to create a directed connection;
- press X, then click its source and destination to remove a directed connection without a refund;
- press Escape to cancel connection mode;
- right-click a service to inspect its state, capacity, links, and current traffic;
- press U to upgrade the inspected service; cyan rings show Scaled and Enterprise tiers;
- press M to mute sound and [ or ] to adjust the volume;
- compare revenue, operating costs, capital investment, and ROI in the economics panel;
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
