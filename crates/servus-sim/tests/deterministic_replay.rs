use servus_sim::{CommandOutcome, GameCommand, GridPosition, MapSize, ServiceKind, Simulation};

#[test]
fn identical_commands_produce_identical_simulations() {
    let map_size = MapSize::new(4, 4).expect("test map dimensions are valid");
    let mut first = Simulation::new(260, 150, map_size);
    let mut second = Simulation::new(260, 150, map_size);

    for simulation in [&mut first, &mut second] {
        let gateway = simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::InternetGateway,
                position: GridPosition::new(1, 1),
            })
            .expect("both simulations have enough construction credits");
        let server = simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: GridPosition::new(2, 1),
            })
            .expect("both simulations have enough construction credits");
        let CommandOutcome::ServiceBuilt { id: gateway, .. } = gateway else {
            panic!("a build command must produce a service");
        };
        let CommandOutcome::ServiceBuilt { id: server, .. } = server else {
            panic!("a build command must produce a service");
        };
        simulation
            .apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            })
            .expect("both simulations can create the same link");
        simulation.advance();
        simulation.set_requests_per_tick(80);
        simulation.advance();
        simulation.advance();
    }

    assert_eq!(first, second);
}
