use crate::{
    Budget, CommandError, CommandOutcome, GameCommand, GridMap, MapSize, Service, ServiceId, Tick,
    TickReport, Traffic,
};

const REVENUE_PER_SERVED_REQUEST: u64 = 1;

/// Complete deterministic state for the first simulation slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Simulation {
    tick: Tick,
    budget: Budget,
    traffic: Traffic,
    map: GridMap,
    services: Vec<Service>,
    next_service_id: u64,
}

impl Simulation {
    #[must_use]
    pub fn new(starting_credits: u64, requests_per_tick: u64, map_size: MapSize) -> Self {
        Self {
            tick: Tick::default(),
            budget: Budget::new(starting_credits),
            traffic: Traffic::new(requests_per_tick),
            map: GridMap::new(map_size),
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

    #[must_use]
    pub const fn map(&self) -> &GridMap {
        &self.map
    }

    pub fn set_requests_per_tick(&mut self, requests_per_tick: u64) {
        self.traffic.set_requests_per_tick(requests_per_tick);
    }

    #[must_use]
    pub fn services(&self) -> &[Service] {
        &self.services
    }

    #[must_use]
    pub fn service(&self, id: ServiceId) -> Option<&Service> {
        self.services.iter().find(|service| service.id() == id)
    }

    pub fn apply(&mut self, command: GameCommand) -> Result<CommandOutcome, CommandError> {
        match command {
            GameCommand::BuildService { kind, position } => {
                let footprint = kind.footprint();
                self.map
                    .validate_placement(position, footprint)
                    .map_err(CommandError::InvalidPlacement)?;
                self.budget
                    .spend(kind.build_cost())
                    .map_err(CommandError::InsufficientBudget)?;

                let id = ServiceId::new(self.next_service_id);
                self.next_service_id = self.next_service_id.saturating_add(1);
                self.map.occupy(position, footprint, id);
                self.services.push(Service::new(id, kind, position));

                Ok(CommandOutcome::ServiceBuilt { id, kind, position })
            }
        }
    }

    pub fn advance(&mut self) -> TickReport {
        self.tick.advance();

        let completed_services = self
            .services
            .iter_mut()
            .filter_map(|service| {
                if service.advance_construction() {
                    Some(service.id())
                } else {
                    None
                }
            })
            .collect();

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
            completed_services,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BudgetError, GridPosition, PlacementError, ServiceKind, ServiceState};

    use super::*;

    fn map_size() -> MapSize {
        MapSize::new(4, 4).expect("test map dimensions are valid")
    }

    fn position(x: u16, y: u16) -> GridPosition {
        GridPosition::new(x, y)
    }

    #[test]
    fn building_a_service_spends_credits_and_reserves_its_tile() {
        let mut simulation = Simulation::new(250, 0, map_size());
        let outcome = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(2, 3),
        });
        assert_eq!(
            outcome,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(1),
                kind: ServiceKind::ApplicationServer,
                position: position(2, 3),
            })
        );
        assert_eq!(simulation.budget().credits(), 150);
        assert_eq!(simulation.services().len(), 1);
        assert_eq!(
            simulation.service(ServiceId::new(1)),
            simulation.services().first()
        );
        assert_eq!(simulation.service(ServiceId::new(99)), None);
        assert_eq!(
            simulation.services()[0].state(),
            ServiceState::UnderConstruction { ticks_remaining: 3 }
        );
        assert_eq!(
            simulation.map().service_at(position(2, 3)),
            Some(ServiceId::new(1))
        );
    }

    #[test]
    fn failed_construction_does_not_mutate_the_simulation() {
        let mut simulation = Simulation::new(50, 0, map_size());
        let before = simulation.clone();
        let result = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(0, 0),
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
        let mut simulation = Simulation::new(200, 0, map_size());
        let first = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(0, 0),
        });
        let second = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(1, 0),
        });
        assert_eq!(
            first,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(1),
                kind: ServiceKind::ApplicationServer,
                position: position(0, 0),
            })
        );
        assert_eq!(
            second,
            Ok(CommandOutcome::ServiceBuilt {
                id: ServiceId::new(2),
                kind: ServiceKind::ApplicationServer,
                position: position(1, 0),
            })
        );
    }

    #[test]
    fn construction_must_complete_before_capacity_becomes_available() {
        let mut simulation = Simulation::new(100, 140, map_size());
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: position(0, 0),
            })
            .expect("the test has enough construction credits");
        let first = simulation.advance();
        assert_eq!(first.tick.number(), 1);
        assert_eq!(first.served, 0);
        assert_eq!(first.dropped, 140);
        assert!(first.completed_services.is_empty());

        let second = simulation.advance();
        assert_eq!(second.served, 0);
        assert!(second.completed_services.is_empty());

        let third = simulation.advance();
        assert_eq!(third.tick.number(), 3);
        assert_eq!(third.received, 140);
        assert_eq!(third.served, 100);
        assert_eq!(third.dropped, 40);
        assert_eq!(third.revenue, 100);
        assert_eq!(third.completed_services, vec![ServiceId::new(1)]);
        assert_eq!(simulation.budget().credits(), 100);
    }

    #[test]
    fn requests_are_dropped_when_there_is_no_infrastructure() {
        let mut simulation = Simulation::new(0, 30, map_size());
        let report = simulation.advance();
        assert_eq!(report.served, 0);
        assert_eq!(report.dropped, 30);
        assert_eq!(report.revenue, 0);
        assert!(report.completed_services.is_empty());
    }

    #[test]
    fn demand_can_change_between_ticks() {
        let mut simulation = Simulation::new(100, 20, map_size());
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: position(0, 0),
            })
            .expect("the test has enough construction credits");
        for _ in 0..ServiceKind::ApplicationServer.construction_ticks() {
            simulation.advance();
        }
        simulation.set_requests_per_tick(75);
        assert_eq!(simulation.traffic().requests_per_tick(), 75);
        let report = simulation.advance();
        assert_eq!(report.received, 75);
        assert_eq!(report.served, 75);
    }

    #[test]
    fn construction_outside_the_map_is_rejected_without_mutation() {
        let mut simulation = Simulation::new(200, 0, map_size());
        let before = simulation.clone();
        let result = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(4, 0),
        });
        assert_eq!(
            result,
            Err(CommandError::InvalidPlacement(
                PlacementError::OutOfBounds {
                    position: position(4, 0),
                    footprint: ServiceKind::ApplicationServer.footprint(),
                    map_size: map_size(),
                }
            ))
        );
        assert_eq!(simulation, before);
    }

    #[test]
    fn construction_cannot_overlap_an_existing_service() {
        let mut simulation = Simulation::new(200, 0, map_size());
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: position(1, 2),
            })
            .expect("the first placement is valid");
        let before_failed_command = simulation.clone();

        let result = simulation.apply(GameCommand::BuildService {
            kind: ServiceKind::ApplicationServer,
            position: position(1, 2),
        });

        assert_eq!(
            result,
            Err(CommandError::InvalidPlacement(PlacementError::Occupied {
                position: position(1, 2),
                service_id: ServiceId::new(1),
            }))
        );
        assert_eq!(simulation, before_failed_command);
    }
}
