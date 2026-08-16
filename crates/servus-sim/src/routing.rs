use std::collections::VecDeque;

use crate::{LinkTraffic, Network, Service, ServiceId, ServiceKind};

pub(crate) struct RoutingResult {
    pub served: u64,
    pub link_traffic: Vec<LinkTraffic>,
}

#[derive(Clone, Debug)]
struct FlowEdge {
    to: usize,
    reverse: usize,
    residual: u64,
    initial_capacity: u64,
}

/// Calculate how many incoming requests can reach request-serving infrastructure.
#[cfg(test)]
pub(crate) fn served_requests(demand: u64, services: &[Service], network: &Network) -> u64 {
    route_requests(demand, services, network).served
}

pub(crate) fn route_requests(
    demand: u64,
    services: &[Service],
    network: &Network,
) -> RoutingResult {
    if demand == 0 || services.is_empty() {
        return RoutingResult {
            served: 0,
            link_traffic: Vec::new(),
        };
    }

    let source = 0;
    let sink = 1;
    let node_count = 2 + services.len() * 2;
    let mut graph = vec![Vec::new(); node_count];

    for (index, service) in services.iter().enumerate() {
        let input = service_input(index);
        let output = service_output(index);
        add_edge(&mut graph, input, output, service.traffic_capacity(demand));

        if service.kind() == ServiceKind::InternetGateway {
            add_edge(&mut graph, source, input, demand);
        }
        if service.kind().serves_requests() {
            add_edge(&mut graph, output, sink, demand);
        }
    }

    let mut tracked_links = Vec::new();
    for link in network.links() {
        let Some(from) = service_index(services, link.from) else {
            continue;
        };
        let Some(to) = service_index(services, link.to) else {
            continue;
        };
        let from_node = service_output(from);
        let edge_index = add_edge(&mut graph, from_node, service_input(to), demand);
        tracked_links.push((*link, from_node, edge_index));
    }

    let served = maximum_flow(&mut graph, source, sink, demand);
    let link_traffic = tracked_links
        .into_iter()
        .filter_map(|(link, from_node, edge_index)| {
            let edge = &graph[from_node][edge_index];
            let requests = edge.initial_capacity - edge.residual;
            (requests > 0).then_some(LinkTraffic {
                from: link.from,
                to: link.to,
                requests,
            })
        })
        .collect();
    RoutingResult {
        served,
        link_traffic,
    }
}

fn maximum_flow(graph: &mut [Vec<FlowEdge>], source: usize, sink: usize, limit: u64) -> u64 {
    let mut total = 0_u64;

    while total < limit {
        let Some(parent) = augmenting_path(graph, source, sink) else {
            break;
        };
        let mut path_capacity = limit - total;
        let mut node = sink;
        while node != source {
            let (previous, edge_index) = parent[node].expect("path nodes have a parent edge");
            path_capacity = path_capacity.min(graph[previous][edge_index].residual);
            node = previous;
        }

        node = sink;
        while node != source {
            let (previous, edge_index) = parent[node].expect("path nodes have a parent edge");
            let reverse = graph[previous][edge_index].reverse;
            graph[previous][edge_index].residual -= path_capacity;
            graph[node][reverse].residual =
                graph[node][reverse].residual.saturating_add(path_capacity);
            node = previous;
        }
        total += path_capacity;
    }

    total
}

fn augmenting_path(
    graph: &[Vec<FlowEdge>],
    source: usize,
    sink: usize,
) -> Option<Vec<Option<(usize, usize)>>> {
    let mut parent = vec![None; graph.len()];
    let mut queue = VecDeque::from([source]);
    parent[source] = Some((source, usize::MAX));

    while let Some(from) = queue.pop_front() {
        for (edge_index, edge) in graph[from].iter().enumerate() {
            if edge.residual == 0 || parent[edge.to].is_some() {
                continue;
            }
            parent[edge.to] = Some((from, edge_index));
            if edge.to == sink {
                return Some(parent);
            }
            queue.push_back(edge.to);
        }
    }

    None
}

fn add_edge(graph: &mut [Vec<FlowEdge>], from: usize, to: usize, capacity: u64) -> usize {
    let forward_index = graph[from].len();
    let reverse_index = graph[to].len();
    graph[from].push(FlowEdge {
        to,
        reverse: reverse_index,
        residual: capacity,
        initial_capacity: capacity,
    });
    graph[to].push(FlowEdge {
        to: from,
        reverse: forward_index,
        residual: 0,
        initial_capacity: 0,
    });
    forward_index
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
        assert_eq!(
            route_requests(140, &services, &network).link_traffic,
            vec![LinkTraffic {
                from: ServiceId::new(1),
                to: ServiceId::new(2),
                requests: 100,
            }]
        );
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
        assert_eq!(
            route_requests(200, &services, &network).link_traffic,
            vec![
                LinkTraffic {
                    from: ServiceId::new(1),
                    to: ServiceId::new(2),
                    requests: 150,
                },
                LinkTraffic {
                    from: ServiceId::new(2),
                    to: ServiceId::new(3),
                    requests: 100,
                },
                LinkTraffic {
                    from: ServiceId::new(2),
                    to: ServiceId::new(4),
                    requests: 50,
                },
            ]
        );
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
        assert_eq!(
            route_requests(200, &services, &network).link_traffic,
            vec![
                LinkTraffic {
                    from: ServiceId::new(1),
                    to: ServiceId::new(2),
                    requests: 100,
                },
                LinkTraffic {
                    from: ServiceId::new(2),
                    to: ServiceId::new(3),
                    requests: 100,
                },
            ]
        );
    }

    #[test]
    fn zero_demand_or_an_empty_service_list_serves_nothing() {
        let gateway = operational_service(1, ServiceKind::InternetGateway);
        assert_eq!(served_requests(0, &[gateway], &Network::default()), 0);
        assert_eq!(served_requests(100, &[], &Network::default()), 0);
        assert!(
            route_requests(0, &[gateway], &Network::default())
                .link_traffic
                .is_empty()
        );
    }

    #[test]
    fn opposite_directed_links_keep_independent_flow_totals() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 1);

        assert_eq!(
            route_requests(80, &services, &network).link_traffic,
            vec![LinkTraffic {
                from: ServiceId::new(1),
                to: ServiceId::new(2),
                requests: 80,
            }]
        );
    }
}
