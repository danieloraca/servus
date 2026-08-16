mod render;

pub use render::render_simulation;

use servus_content::ContentCatalog;
use servus_sim::{
    CommandError, CommandOutcome, GameCommand, GridPosition, MapSize, MapSizeError, ServiceId,
    ServiceKind, ServiceState, Simulation, TickReport,
};

pub const STARTING_CREDITS: u64 = 250;
pub const STARTING_REQUESTS_PER_TICK: u64 = 140;
pub const DEMO_MAP_WIDTH: u16 = 8;
pub const DEMO_MAP_HEIGHT: u16 = 8;
pub const DEMO_GATEWAY_POSITION: GridPosition = GridPosition::new(1, 4);
pub const DEMO_SERVER_POSITION: GridPosition = GridPosition::new(3, 4);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoFrame {
    pub report: TickReport,
    pub view: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoResult {
    pub gateway_name: String,
    pub gateway_position: GridPosition,
    pub service_name: String,
    pub service_position: GridPosition,
    pub service_state: ServiceState,
    pub frames: Vec<DemoFrame>,
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
    let server_kind = ServiceKind::ApplicationServer;
    let gateway_kind = ServiceKind::InternetGateway;
    let service_name = catalog.service(server_kind).map_or_else(
        || "Unknown Service".to_owned(),
        |service| service.display_name.clone(),
    );
    let gateway_name = catalog.service(gateway_kind).map_or_else(
        || "Unknown Service".to_owned(),
        |service| service.display_name.clone(),
    );

    let map_size =
        MapSize::new(DEMO_MAP_WIDTH, DEMO_MAP_HEIGHT).map_err(DemoError::InvalidMapSize)?;
    let mut simulation = Simulation::new(STARTING_CREDITS, STARTING_REQUESTS_PER_TICK, map_size);
    let gateway_id = build_service(&mut simulation, gateway_kind, DEMO_GATEWAY_POSITION)?;
    let server_id = build_service(&mut simulation, server_kind, DEMO_SERVER_POSITION)?;
    simulation
        .apply(GameCommand::ConnectServices {
            from: gateway_id,
            to: server_id,
        })
        .map_err(DemoError::Command)?;
    let mut frames = Vec::new();
    for _ in 0..server_kind.construction_ticks() {
        let report = simulation.advance();
        let view = render_simulation(&simulation, Some(&report));
        frames.push(DemoFrame { report, view });
    }
    let service_state = simulation
        .service(server_id)
        .expect("a successful build command creates its service")
        .state();

    Ok(DemoResult {
        gateway_name,
        gateway_position: DEMO_GATEWAY_POSITION,
        service_name,
        service_position: DEMO_SERVER_POSITION,
        service_state,
        frames,
        remaining_credits: simulation.budget().credits(),
    })
}

fn build_service(
    simulation: &mut Simulation,
    kind: ServiceKind,
    position: GridPosition,
) -> Result<ServiceId, DemoError> {
    let outcome = simulation
        .apply(GameCommand::BuildService { kind, position })
        .map_err(DemoError::Command)?;
    match outcome {
        CommandOutcome::ServiceBuilt { id, .. } => Ok(id),
        CommandOutcome::ServicesConnected { .. } => {
            unreachable!("a build command cannot produce a connection outcome")
        }
    }
}

#[cfg(test)]
mod tests {
    use servus_sim::BudgetError;

    use super::*;

    #[test]
    fn demo_shows_construction_becoming_operational() {
        let result = run_demo().expect("the demo starts with enough construction credits");
        assert_eq!(result.gateway_name, "Internet Gateway");
        assert_eq!(result.gateway_position, GridPosition::new(1, 4));
        assert_eq!(result.service_name, "Application Server");
        assert_eq!(result.service_position, GridPosition::new(3, 4));
        assert_eq!(result.service_state, ServiceState::Operational);
        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.frames[0].report.tick.number(), 1);
        assert_eq!(result.frames[0].report.served, 0);
        assert!(result.frames[0].view.contains("4 |.G.a....|"));
        assert!(result.frames[0].view.contains("Links: 1 -> 2"));
        assert_eq!(result.frames[1].report.served, 0);
        assert_eq!(result.frames[2].report.served, 100);
        assert_eq!(result.frames[2].report.dropped, 40);
        assert_eq!(result.frames[2].report.completed_services.len(), 1);
        assert!(result.frames[2].view.contains("4 |.G.A....|"));
        assert_eq!(result.remaining_credits, 190);
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
