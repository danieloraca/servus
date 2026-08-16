use crate::{Footprint, GridPosition, SolutionId};
use std::fmt;

/// The stable identity of a constructed service.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ServiceId(u64);

impl ServiceId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Infrastructure currently available to construct.
#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceKind {
    InternetGateway,
    Firewall,
    LoadBalancer,
    ApplicationServer,
    RelationalDatabase,
    KeyValueStore,
    Cache,
}

/// Data-driven balance values for one constructible infrastructure type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceProfile {
    pub build_cost: u64,
    pub operating_costs: [u64; 3],
    pub upgrade_costs: [Option<u64>; 3],
    pub upgrade_ticks: [Option<u16>; 3],
    pub capacities: [u64; 3],
    pub construction_ticks: u16,
    pub footprint: Footprint,
    pub role: ServiceRole,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceRole {
    Ingress,
    Transit,
    Application,
    PersistentStore,
    Cache,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ServiceTier {
    #[default]
    Starter,
    Scaled,
    Enterprise,
}

impl ServiceTier {
    pub const ALL: [Self; 3] = [Self::Starter, Self::Scaled, Self::Enterprise];

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Starter => Some(Self::Scaled),
            Self::Scaled => Some(Self::Enterprise),
            Self::Enterprise => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Starter => "Starter",
            Self::Scaled => "Scaled",
            Self::Enterprise => "Enterprise",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Starter => "I",
            Self::Scaled => "II",
            Self::Enterprise => "III",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for ServiceTier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl ServiceKind {
    pub const ALL: [Self; 7] = [
        Self::InternetGateway,
        Self::Firewall,
        Self::LoadBalancer,
        Self::ApplicationServer,
        Self::RelationalDatabase,
        Self::KeyValueStore,
        Self::Cache,
    ];

    const PROFILES: [ServiceProfile; 7] = [
        ServiceProfile {
            build_cost: 50,
            operating_costs: [2, 3, 5],
            upgrade_costs: [None, Some(40), Some(80)],
            upgrade_ticks: [None, Some(2), Some(3)],
            capacities: [250, 600, 1_500],
            construction_ticks: 1,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::Ingress,
        },
        ServiceProfile {
            build_cost: 125,
            operating_costs: [5, 8, 13],
            upgrade_costs: [None, Some(90), Some(180)],
            upgrade_ticks: [None, Some(2), Some(3)],
            capacities: [200, 450, 1_000],
            construction_ticks: 2,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::Transit,
        },
        ServiceProfile {
            build_cost: 75,
            operating_costs: [4, 6, 10],
            upgrade_costs: [None, Some(60), Some(120)],
            upgrade_ticks: [None, Some(2), Some(3)],
            capacities: [150, 350, 800],
            construction_ticks: 2,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::Transit,
        },
        ServiceProfile {
            build_cost: 100,
            operating_costs: [8, 13, 22],
            upgrade_costs: [None, Some(80), Some(160)],
            upgrade_ticks: [None, Some(2), Some(3)],
            capacities: [100, 225, 500],
            construction_ticks: 3,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::Application,
        },
        ServiceProfile {
            build_cost: 180,
            operating_costs: [14, 24, 42],
            upgrade_costs: [None, Some(150), Some(300)],
            upgrade_ticks: [None, Some(3), Some(4)],
            capacities: [80, 200, 480],
            construction_ticks: 4,
            footprint: Footprint::new(2, 2),
            role: ServiceRole::PersistentStore,
        },
        ServiceProfile {
            build_cost: 120,
            operating_costs: [9, 15, 25],
            upgrade_costs: [None, Some(100), Some(210)],
            upgrade_ticks: [None, Some(2), Some(3)],
            capacities: [160, 420, 1_000],
            construction_ticks: 3,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::PersistentStore,
        },
        ServiceProfile {
            build_cost: 70,
            operating_costs: [6, 10, 17],
            upgrade_costs: [None, Some(55), Some(115)],
            upgrade_ticks: [None, Some(1), Some(2)],
            capacities: [220, 550, 1_300],
            construction_ticks: 2,
            footprint: Footprint::new(1, 1),
            role: ServiceRole::Cache,
        },
    ];

    #[must_use]
    pub const fn profile(self) -> &'static ServiceProfile {
        &Self::PROFILES[self as usize]
    }

    #[must_use]
    pub const fn build_cost(self) -> u64 {
        self.profile().build_cost
    }

    #[must_use]
    pub const fn operating_cost(self) -> u64 {
        self.operating_cost_at(ServiceTier::Starter)
    }

    #[must_use]
    pub const fn operating_cost_at(self, tier: ServiceTier) -> u64 {
        self.profile().operating_costs[tier.index()]
    }

    #[must_use]
    pub const fn upgrade_cost(self, target: ServiceTier) -> Option<u64> {
        self.profile().upgrade_costs[target.index()]
    }

    #[must_use]
    pub const fn upgrade_ticks(self, target: ServiceTier) -> Option<u16> {
        self.profile().upgrade_ticks[target.index()]
    }

    #[must_use]
    pub const fn traffic_capacity_at(self, tier: ServiceTier) -> u64 {
        self.profile().capacities[tier.index()]
    }

    #[must_use]
    pub const fn traffic_capacity(self) -> u64 {
        self.traffic_capacity_at(ServiceTier::Starter)
    }

    #[must_use]
    pub const fn serves_requests(self) -> bool {
        matches!(self.profile().role, ServiceRole::Application)
    }

    #[must_use]
    pub const fn is_persistent_store(self) -> bool {
        matches!(self.profile().role, ServiceRole::PersistentStore)
    }

    #[must_use]
    pub const fn is_cache(self) -> bool {
        matches!(self.profile().role, ServiceRole::Cache)
    }

    #[must_use]
    pub const fn construction_ticks(self) -> u16 {
        self.profile().construction_ticks
    }

    #[must_use]
    pub const fn footprint(self) -> Footprint {
        self.profile().footprint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    UnderConstruction {
        ticks_remaining: u16,
    },
    Upgrading {
        target: ServiceTier,
        ticks_remaining: u16,
    },
    Operational,
    Disrupted {
        ticks_remaining: u16,
    },
}

impl fmt::Display for ServiceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnderConstruction { ticks_remaining } => {
                write!(
                    formatter,
                    "under construction ({ticks_remaining} ticks remaining)"
                )
            }
            Self::Operational => formatter.write_str("operational"),
            Self::Upgrading {
                target,
                ticks_remaining,
            } => write!(
                formatter,
                "upgrading to {target} ({ticks_remaining} ticks remaining)"
            ),
            Self::Disrupted { ticks_remaining } => {
                write!(formatter, "disrupted ({ticks_remaining} ticks remaining)")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Service {
    id: ServiceId,
    kind: ServiceKind,
    tier: ServiceTier,
    position: GridPosition,
    solution: Option<SolutionId>,
    state: ServiceState,
}

impl Service {
    pub(crate) const fn new(id: ServiceId, kind: ServiceKind, position: GridPosition) -> Self {
        let construction_ticks = kind.construction_ticks();
        let state = if construction_ticks == 0 {
            ServiceState::Operational
        } else {
            ServiceState::UnderConstruction {
                ticks_remaining: construction_ticks,
            }
        };
        Self {
            id,
            kind,
            tier: ServiceTier::Starter,
            position,
            solution: None,
            state,
        }
    }

    pub(crate) const fn new_in_solution(
        id: ServiceId,
        kind: ServiceKind,
        position: GridPosition,
        solution: SolutionId,
    ) -> Self {
        let mut service = Self::new(id, kind, position);
        service.solution = Some(solution);
        service
    }

    #[must_use]
    pub const fn id(self) -> ServiceId {
        self.id
    }

    #[must_use]
    pub const fn kind(self) -> ServiceKind {
        self.kind
    }

    #[must_use]
    pub const fn tier(self) -> ServiceTier {
        self.tier
    }

    #[must_use]
    pub const fn next_tier(self) -> Option<ServiceTier> {
        self.tier.next()
    }

    #[must_use]
    pub const fn next_upgrade_cost(self) -> Option<u64> {
        match self.next_tier() {
            Some(target) => self.kind.upgrade_cost(target),
            None => None,
        }
    }

    #[must_use]
    pub const fn position(self) -> GridPosition {
        self.position
    }

    #[must_use]
    pub const fn solution(self) -> Option<SolutionId> {
        self.solution
    }

    #[must_use]
    pub const fn state(self) -> ServiceState {
        self.state
    }

    #[must_use]
    pub const fn is_operational(self) -> bool {
        matches!(self.state, ServiceState::Operational)
    }

    #[must_use]
    pub const fn traffic_capacity(self, unbounded_capacity: u64) -> u64 {
        match self.state {
            ServiceState::UnderConstruction { .. }
            | ServiceState::Upgrading { .. }
            | ServiceState::Disrupted { .. } => 0,
            ServiceState::Operational => {
                let capacity = self.kind.traffic_capacity_at(self.tier);
                if capacity < unbounded_capacity {
                    capacity
                } else {
                    unbounded_capacity
                }
            }
        }
    }

    #[must_use]
    pub const fn current_operating_cost(self) -> u64 {
        match self.state {
            ServiceState::UnderConstruction { .. } => 0,
            ServiceState::Operational
            | ServiceState::Upgrading { .. }
            | ServiceState::Disrupted { .. } => self.kind.operating_cost_at(self.tier),
        }
    }

    pub(crate) fn advance_construction(&mut self) -> bool {
        match self.state {
            ServiceState::UnderConstruction { ticks_remaining: 1 } => {
                self.state = ServiceState::Operational;
                true
            }
            ServiceState::UnderConstruction { ticks_remaining } => {
                self.state = ServiceState::UnderConstruction {
                    ticks_remaining: ticks_remaining - 1,
                };
                false
            }
            ServiceState::Operational => false,
            ServiceState::Upgrading {
                target,
                ticks_remaining: 1,
            } => {
                self.tier = target;
                self.state = ServiceState::Operational;
                false
            }
            ServiceState::Upgrading {
                target,
                ticks_remaining,
            } => {
                self.state = ServiceState::Upgrading {
                    target,
                    ticks_remaining: ticks_remaining - 1,
                };
                false
            }
            ServiceState::Disrupted { ticks_remaining: 1 } => {
                self.state = ServiceState::Operational;
                false
            }
            ServiceState::Disrupted { ticks_remaining } => {
                self.state = ServiceState::Disrupted {
                    ticks_remaining: ticks_remaining - 1,
                };
                false
            }
        }
    }

    pub(crate) fn disrupt(&mut self, ticks: u16) -> bool {
        if ticks == 0 || !self.is_operational() {
            return false;
        }
        self.state = ServiceState::Disrupted {
            ticks_remaining: ticks,
        };
        true
    }

    pub(crate) fn start_upgrade(&mut self, target: ServiceTier, ticks: u16) -> bool {
        if !self.is_operational() || self.tier.next() != Some(target) || ticks == 0 {
            return false;
        }
        self.state = ServiceState::Upgrading {
            target,
            ticks_remaining: ticks,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_server_has_an_initial_cost_and_capacity() {
        let kind = ServiceKind::ApplicationServer;
        assert_eq!(kind.build_cost(), 100);
        assert_eq!(kind.operating_cost(), 8);
        assert_eq!(kind.traffic_capacity(), 100);
        assert!(kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 3);
    }

    #[test]
    fn internet_gateway_is_cheap_and_does_not_handle_application_requests() {
        let kind = ServiceKind::InternetGateway;
        assert_eq!(kind.build_cost(), 50);
        assert_eq!(kind.operating_cost(), 2);
        assert_eq!(kind.traffic_capacity(), 250);
        assert!(!kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 1);
        assert_eq!(kind.footprint().width(), 1);
        assert_eq!(kind.footprint().height(), 1);
    }

    #[test]
    fn load_balancer_has_less_capacity_than_two_application_servers() {
        let kind = ServiceKind::LoadBalancer;
        assert_eq!(kind.build_cost(), 75);
        assert_eq!(kind.operating_cost(), 4);
        assert_eq!(kind.traffic_capacity(), 150);
        assert!(!kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 2);
    }

    #[test]
    fn firewall_has_a_cost_construction_time_and_throughput_limit() {
        let kind = ServiceKind::Firewall;
        assert_eq!(kind.build_cost(), 125);
        assert_eq!(kind.operating_cost(), 5);
        assert_eq!(kind.traffic_capacity(), 200);
        assert!(!kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 2);
    }

    #[test]
    fn a_service_exposes_its_identity_kind_and_capacity() {
        let position = GridPosition::new(3, 4);
        let service = Service::new(ServiceId::new(7), ServiceKind::ApplicationServer, position);
        assert_eq!(service.id().value(), 7);
        assert_eq!(service.kind(), ServiceKind::ApplicationServer);
        assert_eq!(service.tier(), ServiceTier::Starter);
        assert_eq!(service.position(), position);
        assert_eq!(
            service.state(),
            ServiceState::UnderConstruction { ticks_remaining: 3 }
        );
        assert_eq!(service.traffic_capacity(500), 0);
        assert_eq!(service.current_operating_cost(), 0);
    }

    #[test]
    fn all_lists_every_constructible_service_kind() {
        assert_eq!(
            ServiceKind::ALL,
            [
                ServiceKind::InternetGateway,
                ServiceKind::Firewall,
                ServiceKind::LoadBalancer,
                ServiceKind::ApplicationServer,
                ServiceKind::RelationalDatabase,
                ServiceKind::KeyValueStore,
                ServiceKind::Cache,
            ]
        );
    }

    #[test]
    fn data_services_have_distinct_strategic_profiles() {
        let relational = ServiceKind::RelationalDatabase;
        let key_value = ServiceKind::KeyValueStore;
        let cache = ServiceKind::Cache;

        assert!(relational.build_cost() > key_value.build_cost());
        assert!(key_value.build_cost() > cache.build_cost());
        let relational_tiles = relational.footprint().width() * relational.footprint().height();
        let key_value_tiles = key_value.footprint().width() * key_value.footprint().height();
        assert!(relational_tiles > key_value_tiles);
        assert!(cache.traffic_capacity() > key_value.traffic_capacity());
        assert!(key_value.traffic_capacity() > relational.traffic_capacity());
        assert!(
            ServiceKind::ALL
                .iter()
                .all(|kind| kind.profile().construction_ticks > 0)
        );
    }

    #[test]
    fn application_server_occupies_one_tile() {
        assert_eq!(ServiceKind::ApplicationServer.footprint().width(), 1);
        assert_eq!(ServiceKind::ApplicationServer.footprint().height(), 1);
    }

    #[test]
    fn construction_progresses_until_the_service_becomes_operational() {
        let mut service = Service::new(
            ServiceId::new(7),
            ServiceKind::ApplicationServer,
            GridPosition::new(3, 4),
        );

        assert!(!service.advance_construction());
        assert_eq!(
            service.state(),
            ServiceState::UnderConstruction { ticks_remaining: 2 }
        );
        assert!(!service.advance_construction());
        assert_eq!(service.traffic_capacity(500), 0);
        assert!(service.advance_construction());
        assert_eq!(service.state(), ServiceState::Operational);
        assert_eq!(service.traffic_capacity(500), 100);
        assert_eq!(service.current_operating_cost(), 8);
        assert!(service.is_operational());
        assert!(!service.advance_construction());
    }

    #[test]
    fn service_states_have_readable_names() {
        assert_eq!(
            ServiceState::UnderConstruction { ticks_remaining: 2 }.to_string(),
            "under construction (2 ticks remaining)"
        );
        assert_eq!(ServiceState::Operational.to_string(), "operational");
        assert_eq!(
            ServiceState::Upgrading {
                target: ServiceTier::Scaled,
                ticks_remaining: 2,
            }
            .to_string(),
            "upgrading to Scaled (2 ticks remaining)"
        );
        assert_eq!(
            ServiceState::Disrupted { ticks_remaining: 2 }.to_string(),
            "disrupted (2 ticks remaining)"
        );
    }

    #[test]
    fn disruption_removes_capacity_then_recovers_automatically() {
        let mut service = Service::new(
            ServiceId::new(8),
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        );
        service.advance_construction();
        assert!(service.disrupt(2));
        assert!(!service.is_operational());
        assert_eq!(service.traffic_capacity(100), 0);
        assert_eq!(service.current_operating_cost(), 2);
        assert!(!service.advance_construction());
        assert_eq!(
            service.state(),
            ServiceState::Disrupted { ticks_remaining: 1 }
        );
        assert!(!service.advance_construction());
        assert_eq!(service.state(), ServiceState::Operational);
        assert!(!service.disrupt(0));
    }

    #[test]
    fn tiers_trade_upgrade_capital_for_more_efficient_capacity() {
        let kind = ServiceKind::ApplicationServer;
        assert_eq!(ServiceTier::Starter.next(), Some(ServiceTier::Scaled));
        assert_eq!(ServiceTier::Scaled.next(), Some(ServiceTier::Enterprise));
        assert_eq!(ServiceTier::Enterprise.next(), None);
        assert_eq!(
            ServiceTier::ALL.map(ServiceTier::short_label),
            ["I", "II", "III"]
        );
        assert_eq!(kind.traffic_capacity_at(ServiceTier::Starter), 100);
        assert_eq!(kind.traffic_capacity_at(ServiceTier::Scaled), 225);
        assert_eq!(kind.traffic_capacity_at(ServiceTier::Enterprise), 500);
        assert_eq!(kind.operating_cost_at(ServiceTier::Starter), 8);
        assert_eq!(kind.operating_cost_at(ServiceTier::Scaled), 13);
        assert_eq!(kind.operating_cost_at(ServiceTier::Enterprise), 22);
        assert_eq!(kind.upgrade_cost(ServiceTier::Starter), None);
        assert_eq!(kind.upgrade_cost(ServiceTier::Scaled), Some(80));
        assert_eq!(kind.upgrade_cost(ServiceTier::Enterprise), Some(160));
    }

    #[test]
    fn upgrading_temporarily_removes_capacity_then_activates_the_new_tier() {
        let mut service = Service::new(
            ServiceId::new(10),
            ServiceKind::ApplicationServer,
            GridPosition::new(0, 0),
        );
        for _ in 0..ServiceKind::ApplicationServer.construction_ticks() {
            service.advance_construction();
        }

        assert!(service.start_upgrade(ServiceTier::Scaled, 2));
        assert_eq!(service.tier(), ServiceTier::Starter);
        assert_eq!(service.traffic_capacity(500), 0);
        assert_eq!(service.current_operating_cost(), 8);
        assert!(!service.start_upgrade(ServiceTier::Scaled, 2));
        assert!(!service.advance_construction());
        assert_eq!(
            service.state(),
            ServiceState::Upgrading {
                target: ServiceTier::Scaled,
                ticks_remaining: 1,
            }
        );
        assert!(!service.advance_construction());
        assert_eq!(service.state(), ServiceState::Operational);
        assert_eq!(service.tier(), ServiceTier::Scaled);
        assert_eq!(service.traffic_capacity(500), 225);
        assert_eq!(service.current_operating_cost(), 13);
        assert!(!service.start_upgrade(ServiceTier::Enterprise, 0));
    }
}
