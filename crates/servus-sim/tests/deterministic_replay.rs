use servus_sim::{GameCommand, ServiceKind, Simulation};

#[test]
fn identical_commands_produce_identical_simulations() {
    let mut first = Simulation::new(200, 150);
    let mut second = Simulation::new(200, 150);

    for simulation in [&mut first, &mut second] {
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
            })
            .expect("both simulations have enough construction credits");
        simulation.advance();
        simulation.set_requests_per_tick(80);
        simulation.advance();
    }

    assert_eq!(first, second);
}
