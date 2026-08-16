//! Deterministic, engine-independent simulation for Servus.

mod clock;
mod command;
mod economy;
mod infrastructure;
mod map;
mod simulation;
mod traffic;

pub use clock::Tick;
pub use command::{CommandError, CommandOutcome, GameCommand};
pub use economy::{Budget, BudgetError};
pub use infrastructure::{Service, ServiceId, ServiceKind, ServiceState};
pub use map::{Footprint, GridMap, GridPosition, MapSize, MapSizeError, PlacementError};
pub use simulation::Simulation;
pub use traffic::{TickReport, Traffic};
