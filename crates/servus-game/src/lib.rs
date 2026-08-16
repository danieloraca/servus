use servus_content::ContentCatalog;
use servus_sim::{
    CommandError, GameCommand, GridPosition, MapSize, MapSizeError, ServiceKind, Simulation,
    TickReport,
};

pub const STARTING_CREDITS: u64 = 250;
pub const STARTING_REQUESTS_PER_TICK: u64 = 140;
pub const DEMO_MAP_WIDTH: u16 = 8;
pub const DEMO_MAP_HEIGHT: u16 = 8;
pub const DEMO_SERVER_POSITION: GridPosition = GridPosition::new(3, 4);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoResult {
    pub service_name: String,
    pub service_position: GridPosition,
    pub report: TickReport,
    pub remaining_credits: u64,
}

#[derive(Debug)]
pub enum DemoError {
    InvalidMapSize(MapSizeError),
    Command(CommandError),
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMapSize(error) => error.fmt(formatter),
            Self::Command(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DemoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidMapSize(error) => Some(error),
            Self::Command(error) => Some(error),
        }
    }
}

pub fn run_demo() -> Result<DemoResult, DemoError> {
    let catalog = ContentCatalog::builtin();
    let kind = ServiceKind::ApplicationServer;
    let service_name = catalog.service(kind).map_or_else(
        || "Unknown Service".to_owned(),
        |service| service.display_name.clone(),
    );

    let map_size =
        MapSize::new(DEMO_MAP_WIDTH, DEMO_MAP_HEIGHT).map_err(DemoError::InvalidMapSize)?;
    let mut simulation = Simulation::new(STARTING_CREDITS, STARTING_REQUESTS_PER_TICK, map_size);
    simulation
        .apply(GameCommand::BuildService {
            kind,
            position: DEMO_SERVER_POSITION,
        })
        .map_err(DemoError::Command)?;
    let report = simulation.advance();

    Ok(DemoResult {
        service_name,
        service_position: DEMO_SERVER_POSITION,
        report,
        remaining_credits: simulation.budget().credits(),
    })
}

#[cfg(test)]
mod tests {
    use servus_sim::BudgetError;

    use super::*;

    #[test]
    fn demo_builds_a_server_and_processes_one_tick() {
        let result = run_demo().expect("the demo starts with enough construction credits");
        assert_eq!(result.service_name, "Application Server");
        assert_eq!(result.service_position, GridPosition::new(3, 4));
        assert_eq!(result.report.tick.number(), 1);
        assert_eq!(result.report.received, 140);
        assert_eq!(result.report.served, 100);
        assert_eq!(result.report.dropped, 40);
        assert_eq!(result.remaining_credits, 250);
    }

    #[test]
    fn demo_errors_expose_their_message_and_source() {
        let error = DemoError::InvalidMapSize(MapSizeError {
            width: 0,
            height: 8,
        });
        assert_eq!(
            error.to_string(),
            "map dimensions must be non-zero, got 0x8"
        );
        assert!(std::error::Error::source(&error).is_some());

        let error = DemoError::Command(CommandError::InsufficientBudget(BudgetError {
            required: 100,
            available: 20,
        }));
        assert_eq!(
            error.to_string(),
            "not enough credits: 100 required, 20 available"
        );
        assert!(std::error::Error::source(&error).is_some());
    }
}
