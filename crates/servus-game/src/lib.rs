mod render;

pub use render::render_simulation;

use servus_content::ContentCatalog;
use servus_sim::{
    CommandError, CommandOutcome, GameCommand, GridPosition, MapSize, MapSizeError, ServiceId,
    ServiceKind, Simulation, TickReport,
};

pub const STARTING_CREDITS: u64 = 400;
pub const STARTING_REQUESTS_PER_TICK: u64 = 200;
pub const DEMO_MAP_WIDTH: u16 = 8;
pub const DEMO_MAP_HEIGHT: u16 = 8;
pub const DEMO_GATEWAY_POSITION: GridPosition = GridPosition::new(1, 4);
pub const DEMO_LOAD_BALANCER_POSITION: GridPosition = GridPosition::new(3, 4);
pub const DEMO_SERVER_ONE_POSITION: GridPosition = GridPosition::new(5, 3);
pub const DEMO_SERVER_TWO_POSITION: GridPosition = GridPosition::new(5, 5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoPlacement {
    pub name: String,
    pub position: GridPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoFrame {
    pub report: TickReport,
    pub view: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoResult {
    pub placements: Vec<DemoPlacement>,
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
    let load_balancer_kind = ServiceKind::LoadBalancer;

    let map_size =
        MapSize::new(DEMO_MAP_WIDTH, DEMO_MAP_HEIGHT).map_err(DemoError::InvalidMapSize)?;
    let mut simulation = Simulation::new(STARTING_CREDITS, STARTING_REQUESTS_PER_TICK, map_size);
    let gateway_id = build_service(&mut simulation, gateway_kind, DEMO_GATEWAY_POSITION)?;
    let load_balancer_id = build_service(
        &mut simulation,
        load_balancer_kind,
        DEMO_LOAD_BALANCER_POSITION,
    )?;
    let server_one_id = build_service(&mut simulation, server_kind, DEMO_SERVER_ONE_POSITION)?;
    let server_two_id = build_service(&mut simulation, server_kind, DEMO_SERVER_TWO_POSITION)?;
    for (from, to) in [
        (gateway_id, load_balancer_id),
        (load_balancer_id, server_one_id),
        (load_balancer_id, server_two_id),
    ] {
        simulation
            .apply(GameCommand::ConnectServices { from, to })
            .map_err(DemoError::Command)?;
    }
    let mut frames = Vec::new();
    for _ in 0..server_kind.construction_ticks() {
        let report = simulation.advance();
        let view = render_simulation(&simulation, Some(&report));
        frames.push(DemoFrame { report, view });
    }

    Ok(DemoResult {
        placements: vec![
            DemoPlacement {
                name: service_name(&catalog, gateway_kind),
                position: DEMO_GATEWAY_POSITION,
            },
            DemoPlacement {
                name: service_name(&catalog, load_balancer_kind),
                position: DEMO_LOAD_BALANCER_POSITION,
            },
            DemoPlacement {
                name: service_name(&catalog, server_kind),
                position: DEMO_SERVER_ONE_POSITION,
            },
            DemoPlacement {
                name: service_name(&catalog, server_kind),
                position: DEMO_SERVER_TWO_POSITION,
            },
        ],
        frames,
        remaining_credits: simulation.budget().credits(),
    })
}

fn service_name(catalog: &ContentCatalog, kind: ServiceKind) -> String {
    catalog.service(kind).map_or_else(
        || "Unknown Service".to_owned(),
        |service| service.display_name.clone(),
    )
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
    fn demo_shows_a_load_balancer_bottleneck() {
        let result = run_demo().expect("the demo starts with enough construction credits");
        assert_eq!(result.placements.len(), 4);
        assert_eq!(result.placements[0].name, "Internet Gateway");
        assert_eq!(result.placements[1].name, "Load Balancer");
        assert_eq!(result.placements[2].name, "Application Server");
        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.frames[0].report.tick.number(), 1);
        assert_eq!(result.frames[0].report.served, 0);
        assert!(result.frames[0].view.contains("3 |.....a..|"));
        assert!(result.frames[0].view.contains("4 |.G.l....|"));
        assert!(
            result.frames[0]
                .view
                .contains("Links: 1 -> 2, 2 -> 3, 2 -> 4")
        );
        assert_eq!(result.frames[1].report.served, 0);
        assert!(result.frames[1].view.contains("4 |.G.L....|"));
        assert_eq!(result.frames[2].report.served, 150);
        assert_eq!(result.frames[2].report.dropped, 50);
        assert_eq!(result.frames[2].report.completed_services.len(), 2);
        assert!(result.frames[2].view.contains("3 |.....A..|"));
        assert!(result.frames[2].view.contains("5 |.....A..|"));
        assert_eq!(result.remaining_credits, 195);
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
