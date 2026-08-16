use std::collections::VecDeque;

use crate::{LinkTraffic, Network, Service, ServiceId, ServiceKind};

pub const CACHE_HIT_PERCENT: u64 = 50;

pub(crate) struct RoutingResult {
    pub served: u64,
    pub link_traffic: Vec<LinkTraffic>,
    pub database_requests: u64,
    pub cache_hits: u64,
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
            database_requests: 0,
            cache_hits: 0,
        };
    }

    let stateful_apps: Vec<ServiceId> = services
        .iter()
        .filter(|service| {
            service.kind().serves_requests()
                && services.iter().any(|store| {
                    store.kind().is_persistent_store()
                        && is_reachable(service.id(), store.id(), services, network)
                })
        })
        .map(|service| service.id())
        .collect();

    let source = 0;
    let sink = 1;
    let node_count = 2 + services.len() * 2;
    let mut graph = vec![Vec::new(); node_count];

    for (index, service) in services.iter().enumerate() {
        let input = service_input(index);
        let output = service_output(index);
        let mut capacity = service.traffic_capacity(demand);
        if service.kind().is_persistent_store()
            && store_has_active_cache(service.id(), &stateful_apps, services, network)
        {
            capacity = capacity.saturating_mul(2);
        }
        add_edge(&mut graph, input, output, capacity);

        if service.kind() == ServiceKind::InternetGateway {
            add_edge(&mut graph, source, input, demand);
        }
        if service.kind().serves_requests() && !stateful_apps.contains(&service.id()) {
            add_edge(&mut graph, output, sink, demand);
        }
        if service.kind().is_persistent_store()
            && stateful_apps
                .iter()
                .any(|app| is_reachable(*app, service.id(), services, network))
        {
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
        if services[from].kind().messaging_mode().is_some()
            || services[to].kind().messaging_mode().is_some()
        {
            continue;
        }
        let from_node = service_output(from);
        let edge_index = add_edge(&mut graph, from_node, service_input(to), demand);
        tracked_links.push((*link, from, to, from_node, edge_index));
    }

    let served = maximum_flow(&mut graph, source, sink, demand);
    let mut database_requests = 0_u64;
    let mut cache_hits = 0_u64;
    let link_traffic = tracked_links
        .into_iter()
        .filter_map(|(link, from, to, from_node, edge_index)| {
            let edge = &graph[from_node][edge_index];
            let routed = edge.initial_capacity - edge.residual;
            let to_store = services[to].kind().is_persistent_store();
            let from_cache = services[from].kind().is_cache();
            let requests = if to_store && from_cache {
                let misses = routed.saturating_mul(100 - CACHE_HIT_PERCENT).div_ceil(100);
                cache_hits = cache_hits.saturating_add(routed - misses);
                misses
            } else {
                routed
            };
            if to_store {
                database_requests = database_requests.saturating_add(requests);
            }
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
        database_requests,
        cache_hits,
    }
}

fn store_has_active_cache(
    store: ServiceId,
    stateful_apps: &[ServiceId],
    services: &[Service],
    network: &Network,
) -> bool {
    services.iter().any(|cache| {
        cache.kind().is_cache()
            && cache.is_operational()
            && network
                .links()
                .iter()
                .any(|link| link.from == cache.id() && link.to == store)
            && stateful_apps
                .iter()
                .any(|app| is_reachable(*app, cache.id(), services, network))
    })
}

fn is_reachable(
    from: ServiceId,
    target: ServiceId,
    services: &[Service],
    network: &Network,
) -> bool {
    let mut visited = vec![from];
    let mut queue = VecDeque::from([from]);
    while let Some(current) = queue.pop_front() {
        for link in network.links().iter().filter(|link| link.from == current) {
            if service(services, current)
                .is_some_and(|service| service.kind().messaging_mode().is_some())
                || service(services, link.to)
                    .is_some_and(|service| service.kind().messaging_mode().is_some())
            {
                continue;
            }
            if link.to == target {
                return true;
            }
            if services.iter().any(|service| service.id() == link.to) && !visited.contains(&link.to)
            {
                visited.push(link.to);
                queue.push_back(link.to);
            }
        }
    }
    false
}

fn service(services: &[Service], id: ServiceId) -> Option<&Service> {
    services.iter().find(|service| service.id() == id)
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

    #[test]
    fn relational_database_limits_a_stateful_application() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
            operational_service(3, ServiceKind::RelationalDatabase),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);

        let result = route_requests(100, &services, &network);
        assert_eq!(result.served, 80);
        assert_eq!(result.database_requests, 80);
        assert_eq!(result.cache_hits, 0);
        assert_eq!(
            result.link_traffic,
            vec![
                LinkTraffic {
                    from: ServiceId::new(1),
                    to: ServiceId::new(2),
                    requests: 80,
                },
                LinkTraffic {
                    from: ServiceId::new(2),
                    to: ServiceId::new(3),
                    requests: 80,
                },
            ]
        );
    }

    #[test]
    fn cache_serves_half_of_stateful_reads_before_the_database() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
            operational_service(3, ServiceKind::Cache),
            operational_service(4, ServiceKind::RelationalDatabase),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);
        connect(&mut network, 3, 4);

        let result = route_requests(100, &services, &network);
        assert_eq!(result.served, 100);
        assert_eq!(result.database_requests, 50);
        assert_eq!(result.cache_hits, 50);
        assert_eq!(
            result.link_traffic.last(),
            Some(&LinkTraffic {
                from: ServiceId::new(3),
                to: ServiceId::new(4),
                requests: 50,
            })
        );
    }

    #[test]
    fn configured_stateful_app_stops_when_its_database_is_not_operational() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
            Service::new(
                ServiceId::new(3),
                ServiceKind::RelationalDatabase,
                GridPosition::new(3, 0),
            ),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);

        let result = route_requests(100, &services, &network);
        assert_eq!(result.served, 0);
        assert_eq!(result.database_requests, 0);
        assert_eq!(result.cache_hits, 0);
    }

    #[test]
    fn database_cannot_serve_internet_traffic_without_an_application() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::RelationalDatabase),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);

        assert_eq!(route_requests(100, &services, &network).served, 0);
    }

    #[test]
    fn messaging_links_do_not_become_synchronous_request_paths() {
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::MessageQueue),
            operational_service(3, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);

        let result = route_requests(100, &services, &network);
        assert_eq!(result.served, 0);
        assert!(result.link_traffic.is_empty());
    }

    #[test]
    fn cache_outage_breaks_a_solution_that_requires_its_cache_path() {
        let mut cache = operational_service(3, ServiceKind::Cache);
        assert!(cache.disrupt(2));
        let services = vec![
            operational_service(1, ServiceKind::InternetGateway),
            operational_service(2, ServiceKind::ApplicationServer),
            cache,
            operational_service(4, ServiceKind::RelationalDatabase),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);
        connect(&mut network, 3, 4);

        let result = route_requests(100, &services, &network);
        assert_eq!(result.served, 0);
        assert_eq!(result.database_requests, 0);
        assert_eq!(result.cache_hits, 0);
    }
}
