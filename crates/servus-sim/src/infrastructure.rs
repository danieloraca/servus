use crate::{Footprint, GridPosition};
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceKind {
    InternetGateway,
    Firewall,
    LoadBalancer,
    ApplicationServer,
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
}

impl fmt::Display for ServiceTier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl ServiceKind {
    pub const ALL: [Self; 4] = [
        Self::InternetGateway,
        Self::Firewall,
        Self::LoadBalancer,
        Self::ApplicationServer,
    ];

    #[must_use]
    pub const fn build_cost(self) -> u64 {
        match self {
            Self::InternetGateway => 50,
            Self::Firewall => 125,
            Self::LoadBalancer => 75,
            Self::ApplicationServer => 100,
        }
    }

    #[must_use]
    pub const fn operating_cost(self) -> u64 {
        self.operating_cost_at(ServiceTier::Starter)
    }

    #[must_use]
    pub const fn operating_cost_at(self, tier: ServiceTier) -> u64 {
        match (self, tier) {
            (Self::InternetGateway, ServiceTier::Starter) => 2,
            (Self::InternetGateway, ServiceTier::Scaled) => 3,
            (Self::InternetGateway, ServiceTier::Enterprise) => 5,
            (Self::Firewall, ServiceTier::Starter) => 5,
            (Self::Firewall, ServiceTier::Scaled) => 8,
            (Self::Firewall, ServiceTier::Enterprise) => 13,
            (Self::LoadBalancer, ServiceTier::Starter) => 4,
            (Self::LoadBalancer, ServiceTier::Scaled) => 6,
            (Self::LoadBalancer, ServiceTier::Enterprise) => 10,
            (Self::ApplicationServer, ServiceTier::Starter) => 8,
            (Self::ApplicationServer, ServiceTier::Scaled) => 13,
            (Self::ApplicationServer, ServiceTier::Enterprise) => 22,
        }
    }

    #[must_use]
    pub const fn upgrade_cost(self, target: ServiceTier) -> Option<u64> {
        match (self, target) {
            (_, ServiceTier::Starter) => None,
            (Self::InternetGateway, ServiceTier::Scaled) => Some(40),
            (Self::InternetGateway, ServiceTier::Enterprise) => Some(80),
            (Self::Firewall, ServiceTier::Scaled) => Some(90),
            (Self::Firewall, ServiceTier::Enterprise) => Some(180),
            (Self::LoadBalancer, ServiceTier::Scaled) => Some(60),
            (Self::LoadBalancer, ServiceTier::Enterprise) => Some(120),
            (Self::ApplicationServer, ServiceTier::Scaled) => Some(80),
            (Self::ApplicationServer, ServiceTier::Enterprise) => Some(160),
        }
    }

    #[must_use]
    pub const fn upgrade_ticks(self, target: ServiceTier) -> Option<u16> {
        match target {
            ServiceTier::Starter => None,
            ServiceTier::Scaled => Some(2),
            ServiceTier::Enterprise => Some(3),
        }
    }

    #[must_use]
    pub const fn traffic_capacity_at(self, tier: ServiceTier) -> u64 {
        match (self, tier) {
            (Self::InternetGateway, ServiceTier::Starter) => 250,
            (Self::InternetGateway, ServiceTier::Scaled) => 600,
            (Self::InternetGateway, ServiceTier::Enterprise) => 1_500,
            (Self::Firewall, ServiceTier::Starter) => 200,
            (Self::Firewall, ServiceTier::Scaled) => 450,
            (Self::Firewall, ServiceTier::Enterprise) => 1_000,
            (Self::LoadBalancer, ServiceTier::Starter) => 150,
            (Self::LoadBalancer, ServiceTier::Scaled) => 350,
            (Self::LoadBalancer, ServiceTier::Enterprise) => 800,
            (Self::ApplicationServer, ServiceTier::Starter) => 100,
            (Self::ApplicationServer, ServiceTier::Scaled) => 225,
            (Self::ApplicationServer, ServiceTier::Enterprise) => 500,
        }
    }

    #[must_use]
    pub const fn traffic_capacity(self) -> u64 {
        self.traffic_capacity_at(ServiceTier::Starter)
    }

    #[must_use]
    pub const fn serves_requests(self) -> bool {
        match self {
            Self::ApplicationServer => true,
            Self::InternetGateway | Self::Firewall | Self::LoadBalancer => false,
        }
    }

    #[must_use]
    pub const fn construction_ticks(self) -> u16 {
        match self {
            Self::InternetGateway => 1,
            Self::Firewall => 2,
            Self::LoadBalancer => 2,
            Self::ApplicationServer => 3,
        }
    }

    #[must_use]
    pub const fn footprint(self) -> Footprint {
        match self {
            Self::InternetGateway => Footprint::new(1, 1),
            Self::Firewall => Footprint::new(1, 1),
            Self::LoadBalancer => Footprint::new(1, 1),
            Self::ApplicationServer => Footprint::new(1, 1),
        }
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
            state,
        }
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
                ServiceKind::ApplicationServer
            ]
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
