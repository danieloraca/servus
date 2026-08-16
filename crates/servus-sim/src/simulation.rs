use crate::{
    Budget, CommandError, CommandOutcome, GameCommand, GridMap, MapSize, NETWORK_LINK_COST,
    Network, NetworkError, OUTAGE_PENALTY_PER_DROPPED_REQUEST, Service, ServiceId, ServiceState,
    Tick, TickReport, Traffic,
};

const REVENUE_PER_SERVED_REQUEST: u64 = 1;

/// Complete deterministic state for the first simulation slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Simulation {
    tick: Tick,
    budget: Budget,
    traffic: Traffic,
    map: GridMap,
    network: Network,
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
            network: Network::default(),
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

    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
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
            GameCommand::ConnectServices { from, to } => {
                if self.service(from).is_none() {
                    return Err(CommandError::InvalidNetwork(NetworkError::UnknownService(
                        from,
                    )));
                }
                if self.service(to).is_none() {
                    return Err(CommandError::InvalidNetwork(NetworkError::UnknownService(
                        to,
                    )));
                }
                self.network
                    .validate_link(from, to)
                    .map_err(CommandError::InvalidNetwork)?;
                self.budget
                    .spend(NETWORK_LINK_COST)
                    .map_err(CommandError::InsufficientBudget)?;
                self.network.add_link(from, to);

                Ok(CommandOutcome::ServicesConnected { from, to })
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

        let cyberattack =
            crate::security::resolve_scheduled_attack(self.tick, &mut self.services, &self.network);

        let received = self.traffic.requests_per_tick();
        let routing = crate::routing::route_requests(received, &self.services, &self.network);
        let served = routing.served;
        let dropped = received - served;
        let revenue = served.saturating_mul(REVENUE_PER_SERVED_REQUEST);
        self.budget.credit(revenue);
        let disruption_active = self
            .services
            .iter()
            .any(|service| matches!(service.state(), ServiceState::Disrupted { .. }));
        let failover_active = disruption_active && served > 0;
        let assessed_penalty = if disruption_active {
            dropped.saturating_mul(OUTAGE_PENALTY_PER_DROPPED_REQUEST)
        } else {
            0
        };
        let outage_penalty = self.budget.forfeit_up_to(assessed_penalty);

        TickReport {
            tick: self.tick,
            received,
            served,
            dropped,
            revenue,
            outage_penalty,
            failover_active,
            completed_services,
            link_traffic: routing.link_traffic,
            cyberattack,
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

    fn build(simulation: &mut Simulation, kind: ServiceKind, position: GridPosition) -> ServiceId {
        match simulation
            .apply(GameCommand::BuildService { kind, position })
            .expect("test construction is affordable and valid")
        {
            CommandOutcome::ServiceBuilt { id, .. } => id,
            CommandOutcome::ServicesConnected { .. } => {
                panic!("a build command must produce a service")
            }
        }
    }

    fn build_connected_stack(simulation: &mut Simulation) -> (ServiceId, ServiceId) {
        let gateway = build(simulation, ServiceKind::InternetGateway, position(0, 0));
        let server = build(simulation, ServiceKind::ApplicationServer, position(1, 0));
        simulation
            .apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            })
            .expect("test link is affordable and valid");
        (gateway, server)
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
        let mut simulation = Simulation::new(160, 140, map_size());
        let (gateway, server) = build_connected_stack(&mut simulation);
        assert!(simulation.network().has_link(gateway, server));
        assert_eq!(simulation.budget().credits(), 0);
        let first = simulation.advance();
        assert_eq!(first.tick.number(), 1);
        assert_eq!(first.served, 0);
        assert_eq!(first.dropped, 140);
        assert_eq!(first.completed_services, vec![gateway]);

        let second = simulation.advance();
        assert_eq!(second.served, 0);
        assert!(second.completed_services.is_empty());

        let third = simulation.advance();
        assert_eq!(third.tick.number(), 3);
        assert_eq!(third.received, 140);
        assert_eq!(third.served, 100);
        assert_eq!(third.dropped, 40);
        assert_eq!(third.revenue, 100);
        assert_eq!(third.completed_services, vec![server]);
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
        let mut simulation = Simulation::new(160, 20, map_size());
        build_connected_stack(&mut simulation);
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
    fn operational_but_disconnected_servers_cannot_receive_traffic() {
        let mut simulation = Simulation::new(150, 80, map_size());
        build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(1, 0),
        );

        let mut report = simulation.advance();
        for _ in 1..ServiceKind::ApplicationServer.construction_ticks() {
            report = simulation.advance();
        }

        assert_eq!(report.served, 0);
        assert_eq!(report.dropped, 80);
        assert_eq!(
            simulation.service(server).map(|service| service.state()),
            Some(ServiceState::Operational)
        );
    }

    #[test]
    fn connecting_services_spends_credits_and_rejects_invalid_links_atomically() {
        let mut simulation = Simulation::new(170, 0, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(1, 0),
        );

        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            }),
            Ok(CommandOutcome::ServicesConnected {
                from: gateway,
                to: server,
            })
        );
        assert_eq!(simulation.budget().credits(), 10);

        let before_invalid_command = simulation.clone();
        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            }),
            Err(CommandError::InvalidNetwork(NetworkError::DuplicateLink {
                from: gateway,
                to: server,
            }))
        );
        assert_eq!(simulation, before_invalid_command);
    }

    #[test]
    fn connections_require_existing_distinct_services() {
        let mut simulation = Simulation::new(100, 0, map_size());
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(0, 0),
        );
        let unknown = ServiceId::new(99);

        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: unknown,
                to: server,
            }),
            Err(CommandError::InvalidNetwork(NetworkError::UnknownService(
                unknown
            )))
        );
        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: server,
                to: unknown,
            }),
            Err(CommandError::InvalidNetwork(NetworkError::UnknownService(
                unknown
            )))
        );
        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: server,
                to: server,
            }),
            Err(CommandError::InvalidNetwork(NetworkError::SelfConnection(
                server
            )))
        );
    }

    #[test]
    fn unaffordable_connections_leave_the_simulation_unchanged() {
        let mut simulation = Simulation::new(150, 0, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(1, 0),
        );
        let before = simulation.clone();

        assert_eq!(
            simulation.apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            }),
            Err(CommandError::InsufficientBudget(BudgetError {
                required: NETWORK_LINK_COST,
                available: 0,
            }))
        );
        assert_eq!(simulation, before);
    }

    #[test]
    fn traffic_traversal_handles_multi_hop_paths_and_cycles() {
        let mut simulation = Simulation::new(365, 250, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let load_balancer = build(&mut simulation, ServiceKind::LoadBalancer, position(1, 0));
        let first_server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(2, 0),
        );
        let second_server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(3, 0),
        );
        for (from, to) in [
            (gateway, load_balancer),
            (load_balancer, first_server),
            (load_balancer, second_server),
            (second_server, load_balancer),
        ] {
            simulation
                .apply(GameCommand::ConnectServices { from, to })
                .expect("test links are affordable and valid");
        }
        let mut report = simulation.advance();
        for _ in 1..ServiceKind::ApplicationServer.construction_ticks() {
            report = simulation.advance();
        }

        assert_eq!(report.served, 150);
        assert_eq!(report.dropped, 100);
        assert_eq!(
            report.link_traffic,
            vec![
                crate::LinkTraffic {
                    from: gateway,
                    to: load_balancer,
                    requests: 150,
                },
                crate::LinkTraffic {
                    from: load_balancer,
                    to: first_server,
                    requests: 100,
                },
                crate::LinkTraffic {
                    from: load_balancer,
                    to: second_server,
                    requests: 50,
                },
            ]
        );
    }

    #[test]
    fn scheduled_attack_disrupts_an_unprotected_server_then_it_recovers() {
        let mut simulation = Simulation::new(300, 100, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(1, 0),
        );
        simulation
            .apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            })
            .expect("test link is affordable");

        let mut report = simulation.advance();
        while report.tick.number() < crate::CYBER_ATTACK_INTERVAL - 1 {
            report = simulation.advance();
        }
        assert_eq!(report.served, 100);
        assert_eq!(report.cyberattack, None);
        let credits_before_attack = simulation.budget().credits();

        let attacked = simulation.advance();
        assert_eq!(
            attacked.cyberattack,
            Some(crate::CyberAttackReport {
                target: server,
                blocked: false,
                disruption_ticks: crate::DISRUPTION_TICKS,
            })
        );
        assert_eq!(attacked.served, 0);
        assert_eq!(attacked.outage_penalty, 100);
        assert!(!attacked.failover_active);
        assert_eq!(
            simulation.budget().credits(),
            credits_before_attack - attacked.outage_penalty
        );
        let still_disrupted = simulation.advance();
        assert_eq!(still_disrupted.served, 0);
        assert_eq!(still_disrupted.outage_penalty, 100);
        let recovered = simulation.advance();
        assert_eq!(recovered.served, 100);
        assert_eq!(recovered.outage_penalty, 0);
        assert_eq!(
            simulation.service(server).map(|service| service.state()),
            Some(ServiceState::Operational)
        );
    }

    #[test]
    fn firewall_on_the_ingress_path_blocks_the_scheduled_attack() {
        let mut simulation = Simulation::new(500, 100, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let firewall = build(&mut simulation, ServiceKind::Firewall, position(1, 0));
        let server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(2, 0),
        );
        for (from, to) in [(gateway, firewall), (firewall, server)] {
            simulation
                .apply(GameCommand::ConnectServices { from, to })
                .expect("test links are affordable");
        }

        let mut report = simulation.advance();
        while report.tick.number() < crate::CYBER_ATTACK_INTERVAL {
            report = simulation.advance();
        }
        assert_eq!(
            report.cyberattack,
            Some(crate::CyberAttackReport {
                target: server,
                blocked: true,
                disruption_ticks: 0,
            })
        );
        assert_eq!(report.served, 100);
        assert_eq!(report.outage_penalty, 0);
        assert!(!report.failover_active);
        assert_eq!(
            simulation.service(server).map(|service| service.state()),
            Some(ServiceState::Operational)
        );
    }

    #[test]
    fn redundant_server_keeps_traffic_flowing_during_a_breach() {
        let mut simulation = Simulation::new(500, 100, map_size());
        let gateway = build(
            &mut simulation,
            ServiceKind::InternetGateway,
            position(0, 0),
        );
        let load_balancer = build(&mut simulation, ServiceKind::LoadBalancer, position(1, 0));
        let first_server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(2, 0),
        );
        let second_server = build(
            &mut simulation,
            ServiceKind::ApplicationServer,
            position(3, 0),
        );
        for (from, to) in [
            (gateway, load_balancer),
            (load_balancer, first_server),
            (load_balancer, second_server),
        ] {
            simulation
                .apply(GameCommand::ConnectServices { from, to })
                .expect("test links are affordable");
        }

        let mut report = simulation.advance();
        while report.tick.number() < crate::CYBER_ATTACK_INTERVAL {
            report = simulation.advance();
        }
        assert_eq!(
            report.cyberattack,
            Some(crate::CyberAttackReport {
                target: first_server,
                blocked: false,
                disruption_ticks: crate::DISRUPTION_TICKS,
            })
        );
        assert!(report.failover_active);
        assert_eq!(report.served, 100);
        assert_eq!(report.dropped, 0);
        assert_eq!(report.outage_penalty, 0);
        assert!(matches!(
            simulation
                .service(first_server)
                .map(|service| service.state()),
            Some(ServiceState::Disrupted { .. })
        ));
        assert_eq!(
            simulation
                .service(second_server)
                .map(|service| service.state()),
            Some(ServiceState::Operational)
        );
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
