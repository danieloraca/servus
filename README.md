# Servus

Servus is an infrastructure strategy game in its earliest prototype stage.

The repository is a Cargo workspace split into:

- `servus-sim`: deterministic, engine-independent simulation;
- `servus-content`: game definitions and validation;
- `servus-game`: the executable and, later, engine integration.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p servus-game
```
# servus
