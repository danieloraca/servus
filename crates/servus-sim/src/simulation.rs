use crate::{
    Budget, CommandError, CommandOutcome, GameCommand, Service, ServiceId, Tick, TickReport,
    Traffic,
};

const REVENUE_PER_SERVED_REQUEST: u64 = 1;

/// Complete deterministic state for the first simulation slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Simulation {
    tick: Tick,
    budget: Budget,
    traffic: Traffic,
    services: Vec<Service>,
    next_service_id: u64,
}

impl Simulation {
    #[must_use]
    pub fn new(starting_credits: u64, requests_per_tick: u64) -> Self {
        Self {
            tick: Tick::default(),
            budget: Budget::new(starting_credits),
            traffic: Traffic::new(requests_per_tick),
            services: Vec::new(),
            next_service_id: 1,
        }
    }

    #[must_use]
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    #[must_use]
    pub const fn budget(&self) -> Budget {
        self.budget
    }

    #[must_use]
    pub const fn traffic(&self) -> Traffic {
        self.traffic
    }

    pub fn set_requests_per_tick(&mut self, requests_per_tick: u64) {
        self.traffic.set_requests_per_tick(requests_per_tick);
    }

    #[must_use]
    pub fn services(&self) -> &[Service] {
        &self.services
    }

    pub fn apply(&mut self, command: GameCommand) -> Result<CommandOutcome, CommandError> {
        match command {
            GameCommand::BuildService { kind } => {
                self.budget
                    .spend(kind.build_cost())
                    .map_err(CommandError::InsufficientBudget)?;

                let id = ServiceId::new(self.next_service_id);
                self.next_service_id = self.next_service_id.saturating_add(1);
                self.services.push(Service::new(id, kind));

                Ok(CommandOutcome::ServiceBuilt { id, kind })
            }
        }
    }

    pub fn advance(&mut self) -> TickReport {
        self.tick.advance();

        let received = self.traffic.requests_per_tick();
        let capacity = self.services.iter().fold(0_u64, |total, service| {
            total.saturating_add(service.request_capacity())
        });
        let served = received.min(capacity);
        let dropped = received - served;
        let revenue = served.saturating_mul(REVENUE_PER_SERVED_REQUEST);
        self.budget.credit(revenue);

        TickReport {
            tick: self.tick,
            received,
            served,
            dropped,
            revenue,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BudgetError, ServiceKind};

    use super::*;

    #[test]
    fn building_a_service_spends_credits_and_adds_capacity() {
        let mut simulation = Simulation::new(250, 0);
        let outcome = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
        });
        assert_eq!(
            outcome,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(1),
                kind: ServiceKind::ApplicationServer,
            })
        );
        assert_eq!(simulation.budget().credits(), 150);
        assert_eq!(simulation.services().len(), 1);
    }

    #[test]
    fn failed_construction_does_not_mutate_the_simulation() {
        let mut simulation = Simulation::new(50, 0);
        let before = simulation.clone();
        let result = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
        });
        assert_eq!(
            result,
            Err(CommandError::InsufficientBudget(BudgetError {
                required: 100,
                available: 50,
            }))
        );
        assert_eq!(simulation, before);
    }

    #[test]
    fn constructed_services_receive_stable_unique_ids() {
        let mut simulation = Simulation::new(200, 0);
        let first = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
        });
        let second = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
        });
        assert_eq!(
            first,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(1),
                kind: ServiceKind::ApplicationServer,
            })
        );
        assert_eq!(
            second,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(2),
                kind: ServiceKind::ApplicationServer,
            })
        );
    }

    #[test]
    fn a_tick_serves_requests_up_to_available_capacity() {
        let mut simulation = Simulation::new(100, 140);
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
            })
            .expect("the test has enough construction credits");
        let report = simulation.advance();
        assert_eq!(report.tick.number(), 1);
        assert_eq!(report.received, 140);
        assert_eq!(report.served, 100);
        assert_eq!(report.dropped, 40);
        assert_eq!(report.revenue, 100);
        assert_eq!(simulation.budget().credits(), 100);
    }

    #[test]
    fn requests_are_dropped_when_there_is_no_infrastructure() {
        let mut simulation = Simulation::new(0, 30);
        let report = simulation.advance();
        assert_eq!(report.served, 0);
        assert_eq!(report.dropped, 30);
        assert_eq!(report.revenue, 0);
    }

    #[test]
    fn demand_can_change_between_ticks() {
        let mut simulation = Simulation::new(100, 20);
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
            })
            .expect("the test has enough construction credits");
        assert_eq!(simulation.advance().received, 20);
        simulation.set_requests_per_tick(75);
        assert_eq!(simulation.traffic().requests_per_tick(), 75);
        assert_eq!(simulation.advance().received, 75);
    }
}
