use std::fmt::Write;

use servus_sim::{GridPosition, Service, ServiceKind, ServiceState, Simulation, TickReport};

const APPLICATION_SERVER_BUILDING: char = 'a';
const APPLICATION_SERVER_OPERATIONAL: char = 'A';
const INTERNET_GATEWAY_BUILDING: char = 'g';
const INTERNET_GATEWAY_OPERATIONAL: char = 'G';
const LOAD_BALANCER_BUILDING: char = 'l';
const LOAD_BALANCER_OPERATIONAL: char = 'L';
const EMPTY_TILE: char = '.';
const INVALID_OCCUPANT: char = '?';

/// Render public simulation state as a compact, deterministic terminal view.
#[must_use]
pub fn render_simulation(simulation: &Simulation, report: Option<&TickReport>) -> String {
    let size = simulation.map().size();
    let row_label_width = (size.height() - 1).to_string().len();
    let mut output = String::new();

    writeln!(
        output,
        "Tick: {} | Credits: {}",
        simulation.tick().number(),
        simulation.budget().credits()
    )
    .expect("writing to a String cannot fail");

    output.push_str(&" ".repeat(row_label_width + 2));
    for x in 0..size.width() {
        output.push(digit(x));
    }
    output.push('\n');

    write_border(&mut output, row_label_width, size.width());
    for y in 0..size.height() {
        write!(output, "{y:>row_label_width$} |").expect("writing to a String cannot fail");
        for x in 0..size.width() {
            let position = GridPosition::new(x, y);
            let symbol = simulation
                .map()
                .service_at(position)
                .map_or(EMPTY_TILE, |id| {
                    simulation
                        .service(id)
                        .map_or(INVALID_OCCUPANT, service_symbol)
                });
            output.push(symbol);
        }
        output.push_str("|\n");
    }
    write_border(&mut output, row_label_width, size.width());

    output.push_str(
        "Legend: g/G=Gateway, l/L=Load Balancer, a/A=App Server (building/operational)\n",
    );
    write_network_links(&mut output, simulation);
    if let Some(report) = report {
        writeln!(
            output,
            "Traffic: received={} | served={} | dropped={} | revenue={}",
            report.received, report.served, report.dropped, report.revenue
        )
        .expect("writing to a String cannot fail");
        write_completed_services(&mut output, report);
    } else {
        output.push_str("Traffic: awaiting first tick\n");
    }

    output
}

fn service_symbol(service: &Service) -> char {
    match (service.kind(), service.state()) {
        (ServiceKind::InternetGateway, ServiceState::UnderConstruction { .. }) => {
            INTERNET_GATEWAY_BUILDING
        }
        (ServiceKind::InternetGateway, ServiceState::Operational) => INTERNET_GATEWAY_OPERATIONAL,
        (ServiceKind::LoadBalancer, ServiceState::UnderConstruction { .. }) => {
            LOAD_BALANCER_BUILDING
        }
        (ServiceKind::LoadBalancer, ServiceState::Operational) => LOAD_BALANCER_OPERATIONAL,
        (ServiceKind::ApplicationServer, ServiceState::UnderConstruction { .. }) => {
            APPLICATION_SERVER_BUILDING
        }
        (ServiceKind::ApplicationServer, ServiceState::Operational) => {
            APPLICATION_SERVER_OPERATIONAL
        }
    }
}

fn write_network_links(output: &mut String, simulation: &Simulation) {
    output.push_str("Links: ");
    if simulation.network().links().is_empty() {
        output.push_str("none\n");
        return;
    }

    for (index, link) in simulation.network().links().iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(output, "{} -> {}", link.from.value(), link.to.value())
            .expect("writing to a String cannot fail");
    }
    output.push('\n');
}

fn digit(value: u16) -> char {
    char::from(b'0' + u8::try_from(value % 10).expect("a decimal digit fits in u8"))
}

fn write_border(output: &mut String, row_label_width: usize, width: u16) {
    output.push_str(&" ".repeat(row_label_width + 1));
    output.push('+');
    output.push_str(&"-".repeat(usize::from(width)));
    output.push_str("+\n");
}

fn write_completed_services(output: &mut String, report: &TickReport) {
    output.push_str("Completed: ");
    if report.completed_services.is_empty() {
        output.push_str("none\n");
        return;
    }

    for (index, id) in report.completed_services.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(output, "{}", id.value()).expect("writing to a String cannot fail");
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use servus_sim::{GameCommand, MapSize};

    use super::*;

    fn simulation(width: u16, height: u16, credits: u64) -> Simulation {
        Simulation::new(
            credits,
            25,
            MapSize::new(width, height).expect("test map dimensions are valid"),
        )
    }

    #[test]
    fn empty_map_is_rendered_with_coordinates_and_status() {
        let simulation = simulation(3, 2, 100);

        assert_eq!(
            render_simulation(&simulation, None),
            concat!(
                "Tick: 0 | Credits: 100\n",
                "   012\n",
                "  +---+\n",
                "0 |...|\n",
                "1 |...|\n",
                "  +---+\n",
                "Legend: g/G=Gateway, l/L=Load Balancer, a/A=App Server (building/operational)\n",
                "Links: none\n",
                "Traffic: awaiting first tick\n",
            )
        );
    }

    #[test]
    fn construction_and_operation_use_different_symbols() {
        let mut simulation = simulation(3, 2, 100);
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: GridPosition::new(1, 0),
            })
            .expect("test placement and budget are valid");

        let first = simulation.advance();
        let building = render_simulation(&simulation, Some(&first));
        assert!(building.contains("0 |.a.|"));
        assert!(building.contains("Completed: none"));

        simulation.advance();
        let third = simulation.advance();
        let operational = render_simulation(&simulation, Some(&third));
        assert!(operational.contains("0 |.A.|"));
        assert!(operational.contains("Completed: 1"));
    }

    #[test]
    fn coordinate_headers_repeat_decimal_digits_for_wide_maps() {
        let simulation = simulation(12, 1, 0);
        assert!(render_simulation(&simulation, None).contains("   012345678901\n"));
    }

    #[test]
    fn load_balancer_symbol_changes_when_construction_completes() {
        let mut simulation = simulation(3, 2, 75);
        simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::LoadBalancer,
                position: GridPosition::new(1, 0),
            })
            .expect("test load balancer is affordable and valid");

        assert!(render_simulation(&simulation, None).contains("0 |.l.|"));
        simulation.advance();
        simulation.advance();
        assert!(render_simulation(&simulation, None).contains("0 |.L.|"));
    }

    #[test]
    fn gateways_and_directed_links_are_rendered() {
        let mut simulation = simulation(3, 2, 170);
        let gateway = simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::InternetGateway,
                position: GridPosition::new(0, 0),
            })
            .expect("test gateway is affordable and valid");
        let server = simulation
            .apply(GameCommand::BuildService {
                kind: ServiceKind::ApplicationServer,
                position: GridPosition::new(2, 0),
            })
            .expect("test server is affordable and valid");
        let servus_sim::CommandOutcome::ServiceBuilt { id: gateway, .. } = gateway else {
            panic!("a build command must produce a service");
        };
        let servus_sim::CommandOutcome::ServiceBuilt { id: server, .. } = server else {
            panic!("a build command must produce a service");
        };
        simulation
            .apply(GameCommand::ConnectServices {
                from: gateway,
                to: server,
            })
            .expect("test link is affordable and valid");

        let building = render_simulation(&simulation, None);
        assert!(building.contains("0 |g.a|"));
        assert!(building.contains("Links: 1 -> 2"));

        simulation.advance();
        let gateway_operational = render_simulation(&simulation, None);
        assert!(gateway_operational.contains("0 |G.a|"));
    }
}
