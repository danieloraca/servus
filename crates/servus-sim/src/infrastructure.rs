use crate::{Footprint, GridPosition};

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
    ApplicationServer,
}

impl ServiceKind {
    pub const ALL: [Self; 1] = [Self::ApplicationServer];

    #[must_use]
    pub const fn build_cost(self) -> u64 {
        match self {
            Self::ApplicationServer => 100,
        }
    }

    #[must_use]
    pub const fn request_capacity(self) -> u64 {
        match self {
            Self::ApplicationServer => 100,
        }
    }

    #[must_use]
    pub const fn footprint(self) -> Footprint {
        match self {
            Self::ApplicationServer => Footprint::new(1, 1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Service {
    id: ServiceId,
    kind: ServiceKind,
    position: GridPosition,
}

impl Service {
    pub(crate) const fn new(id: ServiceId, kind: ServiceKind, position: GridPosition) -> Self {
        Self { id, kind, position }
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
    pub const fn request_capacity(self) -> u64 {
        self.kind.request_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_server_has_an_initial_cost_and_capacity() {
        let kind = ServiceKind::ApplicationServer;
        assert_eq!(kind.build_cost(), 100);
        assert_eq!(kind.request_capacity(), 100);
    }

    #[test]
    fn a_service_exposes_its_identity_kind_and_capacity() {
        let position = GridPosition::new(3, 4);
        let service = Service::new(ServiceId::new(7), ServiceKind::ApplicationServer, position);
        assert_eq!(service.id().value(), 7);
        assert_eq!(service.kind(), ServiceKind::ApplicationServer);
        assert_eq!(service.position(), position);
        assert_eq!(service.request_capacity(), 100);
    }

    #[test]
    fn all_lists_every_constructible_service_kind() {
        assert_eq!(ServiceKind::ALL, [ServiceKind::ApplicationServer]);
    }

    #[test]
    fn application_server_occupies_one_tile() {
        assert_eq!(ServiceKind::ApplicationServer.footprint().width(), 1);
        assert_eq!(ServiceKind::ApplicationServer.footprint().height(), 1);
    }
}
