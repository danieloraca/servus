//! Deterministic, engine-independent simulation for Servus.

mod clock;
mod command;
mod economy;
mod infrastructure;
mod map;
mod network;
mod routing;
mod security;
mod simulation;
mod solution;
mod traffic;

pub use clock::Tick;
pub use command::{CommandError, CommandOutcome, GameCommand, UpgradeError};
pub use economy::{Budget, BudgetError, OUTAGE_PENALTY_PER_DROPPED_REQUEST};
pub use infrastructure::{
    Service, ServiceId, ServiceKind, ServiceProfile, ServiceRole, ServiceState, ServiceTier,
};
pub use map::{
    Footprint, GridMap, GridPosition, MapOccupant, MapSize, MapSizeError, PlacementError,
};
pub use network::{NETWORK_LINK_COST, Network, NetworkError, NetworkLink};
pub use routing::CACHE_HIT_PERCENT;
pub use security::{CYBER_ATTACK_INTERVAL, DISRUPTION_TICKS};
pub use simulation::Simulation;
pub use solution::{BuildingScale, FoundationKind, Solution, SolutionError, SolutionId};
pub use traffic::{CyberAttackReport, LinkTraffic, TickReport, Traffic};
