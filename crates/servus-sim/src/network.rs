use std::error::Error;
use std::fmt;

use crate::ServiceId;

pub const NETWORK_LINK_COST: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkLink {
    pub from: ServiceId,
    pub to: ServiceId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Network {
    links: Vec<NetworkLink>,
}

impl Network {
    #[must_use]
    pub fn links(&self) -> &[NetworkLink] {
        &self.links
    }

    #[must_use]
    pub fn has_link(&self, from: ServiceId, to: ServiceId) -> bool {
        self.links
            .iter()
            .any(|link| link.from == from && link.to == to)
    }

    pub(crate) fn validate_link(&self, from: ServiceId, to: ServiceId) -> Result<(), NetworkError> {
        if from == to {
            return Err(NetworkError::SelfConnection(from));
        }
        if self.has_link(from, to) {
            return Err(NetworkError::DuplicateLink { from, to });
        }
        Ok(())
    }

    pub(crate) fn add_link(&mut self, from: ServiceId, to: ServiceId) {
        self.links.push(NetworkLink { from, to });
    }

    pub(crate) fn outgoing(&self, from: ServiceId) -> impl Iterator<Item = ServiceId> + '_ {
        self.links
            .iter()
            .filter(move |link| link.from == from)
            .map(|link| link.to)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkError {
    UnknownService(ServiceId),
    SelfConnection(ServiceId),
    DuplicateLink { from: ServiceId, to: ServiceId },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownService(id) => write!(formatter, "service {} does not exist", id.value()),
            Self::SelfConnection(id) => {
                write!(formatter, "service {} cannot connect to itself", id.value())
            }
            Self::DuplicateLink { from, to } => write!(
                formatter,
                "network link {} -> {} already exists",
                from.value(),
                to.value()
            ),
        }
    }
}

impl Error for NetworkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directed_links_can_be_added_and_queried() {
        let mut network = Network::default();
        let first = ServiceId::new(1);
        let second = ServiceId::new(2);

        assert_eq!(network.validate_link(first, second), Ok(()));
        network.add_link(first, second);

        assert!(network.has_link(first, second));
        assert!(!network.has_link(second, first));
        assert_eq!(network.outgoing(first).collect::<Vec<_>>(), vec![second]);
        assert_eq!(
            network.links(),
            &[NetworkLink {
                from: first,
                to: second,
            }]
        );
    }

    #[test]
    fn self_connections_and_duplicate_links_are_rejected() {
        let mut network = Network::default();
        let first = ServiceId::new(1);
        let second = ServiceId::new(2);

        assert_eq!(
            network.validate_link(first, first),
            Err(NetworkError::SelfConnection(first))
        );
        network.add_link(first, second);
        assert_eq!(
            network.validate_link(first, second),
            Err(NetworkError::DuplicateLink {
                from: first,
                to: second,
            })
        );
        assert_eq!(network.validate_link(second, first), Ok(()));
    }

    #[test]
    fn network_errors_have_readable_messages() {
        assert_eq!(
            NetworkError::UnknownService(ServiceId::new(7)).to_string(),
            "service 7 does not exist"
        );
        assert_eq!(
            NetworkError::SelfConnection(ServiceId::new(3)).to_string(),
            "service 3 cannot connect to itself"
        );
        assert_eq!(
            NetworkError::DuplicateLink {
                from: ServiceId::new(1),
                to: ServiceId::new(2),
            }
            .to_string(),
            "network link 1 -> 2 already exists"
        );
    }

    #[test]
    fn a_network_link_costs_ten_credits() {
        assert_eq!(NETWORK_LINK_COST, 10);
    }
}
