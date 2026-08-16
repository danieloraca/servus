use servus_content::ContentCatalog;
use servus_sim::{CommandError, GameCommand, ServiceKind, Simulation, TickReport};

pub const STARTING_CREDITS: u64 = 250;
pub const STARTING_REQUESTS_PER_TICK: u64 = 140;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoResult {
    pub service_name: String,
    pub report: TickReport,
    pub remaining_credits: u64,
}

pub fn run_demo() -> Result<DemoResult, CommandError> {
    let catalog = ContentCatalog::builtin();
    let kind = ServiceKind::ApplicationServer;
    let service_name = catalog.service(kind).map_or_else(
        || "Unknown Service".to_owned(),
        |service| service.display_name.clone(),
    );

    let mut simulation = Simulation::new(STARTING_CREDITS, STARTING_REQUESTS_PER_TICK);
    simulation.apply(GameCommand::BuildService { kind })?;
    let report = simulation.advance();

    Ok(DemoResult {
        service_name,
        report,
        remaining_credits: simulation.budget().credits(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_builds_a_server_and_processes_one_tick() {
        let result = run_demo().expect("the demo starts with enough construction credits");
        assert_eq!(result.service_name, "Application Server");
        assert_eq!(result.report.tick.number(), 1);
        assert_eq!(result.report.received, 140);
        assert_eq!(result.report.served, 100);
        assert_eq!(result.report.dropped, 40);
        assert_eq!(result.remaining_credits, 250);
    }
}
