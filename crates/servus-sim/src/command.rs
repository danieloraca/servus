use std::error::Error;
use std::fmt;

use crate::{BudgetError, GridPosition, NetworkError, PlacementError, ServiceId, ServiceKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameCommand {
    BuildService {
        kind: ServiceKind,
        position: GridPosition,
    },
    ConnectServices {
        from: ServiceId,
        to: ServiceId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandOutcome {
    ServiceBuilt {
        id: ServiceId,
        kind: ServiceKind,
        position: GridPosition,
    },
    ServicesConnected {
        from: ServiceId,
        to: ServiceId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandError {
    InsufficientBudget(BudgetError),
    InvalidPlacement(PlacementError),
    InvalidNetwork(NetworkError),
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientBudget(error) => error.fmt(formatter),
            Self::InvalidPlacement(error) => error.fmt(formatter),
            Self::InvalidNetwork(error) => error.fmt(formatter),
        }
    }
}

impl Error for CommandError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InsufficientBudget(error) => Some(error),
            Self::InvalidPlacement(error) => Some(error),
            Self::InvalidNetwork(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_delegates_its_message_and_source() {
        let error = CommandError::InsufficientBudget(BudgetError {
            required: 100,
            available: 50,
        });
        assert_eq!(
            error.to_string(),
            "not enough credits: 100 required, 50 available"
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn placement_command_error_delegates_its_message_and_source() {
        let error = CommandError::InvalidPlacement(PlacementError::Occupied {
            position: GridPosition::new(1, 2),
            service_id: ServiceId::new(3),
        });
        assert_eq!(error.to_string(), "tile (1, 2) is occupied by service 3");
        assert!(error.source().is_some());
    }

    #[test]
    fn network_command_error_delegates_its_message_and_source() {
        let error = CommandError::InvalidNetwork(NetworkError::UnknownService(ServiceId::new(9)));
        assert_eq!(error.to_string(), "service 9 does not exist");
        assert!(error.source().is_some());
    }
}
