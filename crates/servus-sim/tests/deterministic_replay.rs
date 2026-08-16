use servus_sim::{GameCommand, GridPosition, MapSize, ServiceKind, Simulation};

#[test]
fn identical_commands_produce_identical_simulations() {
    let map_size = MapSize::new(4, 4).expect("test map dimensions are valid");
    let mut first = Simulation::new(200, 150, map_size);
    let mut second = Simulation::new(200, 150, map_size);

    for simulation in [&mut first, &mut second] {
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: GridPosition::new(2, 1),
            })
            .expect("both simulations have enough construction credits");
        simulation.advance();
        simulation.set_requests_per_tick(80);
        simulation.advance();
    }

    assert_eq!(first, second);
}
