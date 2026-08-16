use std::collections::VecDeque;

use crate::{Network, Service, ServiceId, ServiceKind};

/// Calculate how many incoming requests can reach request-serving infrastructure.
pub(crate) fn served_requests(demand: u64, services: &[Service], network: &Network) -> u64 {
    if demand == 0 || services.is_empty() {
        return 0;
    }

    let source = 0;
    let sink = 1;
    let node_count = 2 + services.len() * 2;
    let mut residual = vec![vec![0_u64; node_count]; node_count];

    for (index, service) in services.iter().enumerate() {
        let input = service_input(index);
        let output = service_output(index);
        add_edge(
            &mut residual,
            input,
            output,
            service.traffic_capacity(demand),
        );

        if service.kind() == ServiceKind::InternetGateway {
            add_edge(&mut residual, source, input, demand);
        }
        if service.kind().serves_requests() {
            add_edge(&mut residual, output, sink, demand);
        }
    }

    for link in network.links() {
        let Some(from) = service_index(services, link.from) else {
            continue;
        };
        let Some(to) = service_index(services, link.to) else {
            continue;
        };
        add_edge(
            &mut residual,
            service_output(from),
            service_input(to),
            demand,
        );
    }

    maximum_flow(&mut residual, source, sink, demand)
}

fn maximum_flow(residual: &mut [Vec<u64>], source: usize, sink: usize, limit: u64) -> u64 {
    let mut total = 0_u64;

    while total < limit {
        let Some(parent) = augmenting_path(residual, source, sink) else {
            break;
        };
        let mut path_capacity = limit - total;
        let mut node = sink;
        while node != source {
            let previous = parent[node];
            path_capacity = path_capacity.min(residual[previous][node]);
            node = previous;
        }

        node = sink;
        while node != source {
            let previous = parent[node];
            residual[previous][node] -= path_capacity;
            residual[node][previous] = residual[node][previous].saturating_add(path_capacity);
            node = previous;
        }
        total += path_capacity;
    }

    total
}

fn augmenting_path(residual: &[Vec<u64>], source: usize, sink: usize) -> Option<Vec<usize>> {
    let mut parent = vec![usize::MAX; residual.len()];
    let mut queue = VecDeque::from([source]);
    parent[source] = source;

    while let Some(from) = queue.pop_front() {
        for (to, capacity) in residual[from].iter().copied().enumerate() {
            if capacity == 0 || parent[to] != usize::MAX {
                continue;
            }
            parent[to] = from;
            if to == sink {
                return Some(parent);
            }
            queue.push_back(to);
        }
    }

    None
}

fn add_edge(residual: &mut [Vec<u64>], from: usize, to: usize, capacity: u64) {
    residual[from][to] = residual[from][to].saturating_add(capacity);
}

fn service_index(services: &[Service], id: ServiceId) -> Option<usize> {
    services.iter().position(|service| service.id() == id)
}

const fn service_input(index: usize) -> usize {
    2 + index * 2
}

const fn service_output(index: usize) -> usize {
    service_input(index) + 1
}

#[cfg(test)]
mod tests {
    use crate::{GridPosition, ServiceKind};

    use super::*;

    fn operational_service(id: u64, kind: ServiceKind) -> Service {
        let mut service = Service::new(ServiceId::new(id), kind, GridPosition::new(id as u16, 0));
        for _ in 0..kind.construction_ticks() {
            service.advance_construction();
        }
        service
    }

    fn connect(network: &mut Network, from: u64, to: u64) {
        network.add_link(ServiceId::new(from), ServiceId::new(to));
    }

    #[test]
    fn direct_gateway_to_server_path_serves_up_to_server_capacity() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);

        assert_eq!(served_requests(140, &services, &network), 100);
    }

    #[test]
    fn disconnected_and_building_services_do_not_carry_traffic() {
        let gateway = operational_service(1, ServiceKind::InternetGateway);
        let building_server = Service::new(
            ServiceId::new(2),
            ServiceKind::ApplicationServer,
            GridPosition::new(1, 0),
        );
        let mut network = Network::default();
        connect(&mut network, 1, 2);

        assert_eq!(served_requests(100, &[gateway], &network), 0);
        assert_eq!(
            served_requests(100, &[gateway, building_server], &network),
            0
        );
    }

    #[test]
    fn load_balancer_limits_combined_downstream_capacity() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::LoadBalancer),
            operational_service(3, ServiceKind::ApplicationServer),
            operational_service(4, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);
        connect(&mut network, 2, 4);

        assert_eq!(served_requests(200, &services, &network), 150);
    }

    #[test]
    fn routing_handles_cycles_without_counting_capacity_twice() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::LoadBalancer),
            operational_service(3, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);
        connect(&mut network, 3, 2);

        assert_eq!(served_requests(200, &services, &network), 100);
    }

    #[test]
    fn zero_demand_or_an_empty_service_list_serves_nothing() {
        let gateway = operational_service(1, ServiceKind::InternetGateway);
        assert_eq!(served_requests(0, &[gateway], &Network::default()), 0);
        assert_eq!(served_requests(100, &[], &Network::default()), 0);
    }
}
