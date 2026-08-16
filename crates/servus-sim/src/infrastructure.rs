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
    LoadBalancer,
    ApplicationServer,
}

impl ServiceKind {
    pub const ALL: [Self; 3] = [
        Self::InternetGateway,
        Self::LoadBalancer,
        Self::ApplicationServer,
    ];

    #[must_use]
    pub const fn build_cost(self) -> u64 {
        match self {
            Self::InternetGateway => 50,
            Self::LoadBalancer => 75,
            Self::ApplicationServer => 100,
        }
    }

    #[must_use]
    pub const fn traffic_capacity(self) -> Option<u64> {
        match self {
            Self::InternetGateway => None,
            Self::LoadBalancer => Some(150),
            Self::ApplicationServer => Some(100),
        }
    }

    #[must_use]
    pub const fn serves_requests(self) -> bool {
        matches!(self, Self::ApplicationServer)
    }

    #[must_use]
    pub const fn construction_ticks(self) -> u16 {
        match self {
            Self::InternetGateway => 1,
            Self::LoadBalancer => 2,
            Self::ApplicationServer => 3,
        }
    }

    #[must_use]
    pub const fn footprint(self) -> Footprint {
        match self {
            Self::InternetGateway => Footprint::new(1, 1),
            Self::LoadBalancer => Footprint::new(1, 1),
            Self::ApplicationServer => Footprint::new(1, 1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    UnderConstruction { ticks_remaining: u16 },
    Operational,
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
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Service {
    id: ServiceId,
    kind: ServiceKind,
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
            ServiceState::UnderConstruction { .. } => 0,
            ServiceState::Operational => match self.kind.traffic_capacity() {
                Some(capacity) if capacity < unbounded_capacity => capacity,
                Some(_) => unbounded_capacity,
                None => unbounded_capacity,
            },
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_server_has_an_initial_cost_and_capacity() {
        let kind = ServiceKind::ApplicationServer;
        assert_eq!(kind.build_cost(), 100);
        assert_eq!(kind.traffic_capacity(), Some(100));
        assert!(kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 3);
    }

    #[test]
    fn internet_gateway_is_cheap_and_does_not_handle_application_requests() {
        let kind = ServiceKind::InternetGateway;
        assert_eq!(kind.build_cost(), 50);
        assert_eq!(kind.traffic_capacity(), None);
        assert!(!kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 1);
        assert_eq!(kind.footprint().width(), 1);
        assert_eq!(kind.footprint().height(), 1);
    }

    #[test]
    fn load_balancer_has_less_capacity_than_two_application_servers() {
        let kind = ServiceKind::LoadBalancer;
        assert_eq!(kind.build_cost(), 75);
        assert_eq!(kind.traffic_capacity(), Some(150));
        assert!(!kind.serves_requests());
        assert_eq!(kind.construction_ticks(), 2);
    }

    #[test]
    fn a_service_exposes_its_identity_kind_and_capacity() {
        let position = GridPosition::new(3, 4);
        let service = Service::new(ServiceId::new(7), ServiceKind::ApplicationServer, position);
        assert_eq!(service.id().value(), 7);
        assert_eq!(service.kind(), ServiceKind::ApplicationServer);
        assert_eq!(service.position(), position);
        assert_eq!(
            service.state(),
            ServiceState::UnderConstruction { ticks_remaining: 3 }
        );
        assert_eq!(service.traffic_capacity(500), 0);
    }

    #[test]
    fn all_lists_every_constructible_service_kind() {
        assert_eq!(
            ServiceKind::ALL,
            [
                ServiceKind::InternetGateway,
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
    }
}
