use crate::{LinkTraffic, MessagingMode, Network, Service, ServiceId};

const QUEUE_STORAGE_MULTIPLIER: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QueueBacklog {
    pub queue: ServiceId,
    pub messages: u64,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct MessagingResult {
    pub published: u64,
    pub processed: u64,
    pub queued: u64,
    pub dropped: u64,
    pub link_traffic: Vec<LinkTraffic>,
}

pub(crate) fn process_messages(
    services: &[Service],
    network: &Network,
    request_traffic: &[LinkTraffic],
    backlogs: &mut Vec<QueueBacklog>,
) -> MessagingResult {
    let mut result = MessagingResult::default();
    let mut worker_capacity: Vec<(ServiceId, u64)> = services
        .iter()
        .filter(|service| service.kind().serves_requests() && service.is_operational())
        .map(|service| {
            (
                service.id(),
                service
                    .traffic_capacity(u64::MAX)
                    .saturating_sub(incoming_requests(service.id(), request_traffic)),
            )
        })
        .collect();

    for broker in services
        .iter()
        .filter(|service| service.kind().messaging_mode().is_some() && service.is_operational())
    {
        let capacity = broker.traffic_capacity(u64::MAX);
        let mut remaining_ingress = capacity;
        let mut accepted = 0_u64;

        for link in network.links().iter().filter(|link| link.to == broker.id()) {
            let Some(producer) = service(services, link.from) else {
                continue;
            };
            if !producer.kind().serves_requests() || !producer.is_operational() {
                continue;
            }
            let attempted = incoming_requests(producer.id(), request_traffic);
            let published = attempted.min(remaining_ingress);
            remaining_ingress -= published;
            accepted = accepted.saturating_add(published);
            result.dropped = result.dropped.saturating_add(attempted - published);
            push_traffic(
                &mut result.link_traffic,
                producer.id(),
                broker.id(),
                published,
            );
        }

        result.published = result.published.saturating_add(accepted);
        match broker
            .kind()
            .messaging_mode()
            .expect("filtered services have a messaging mode")
        {
            MessagingMode::Queue => process_queue(
                broker,
                accepted,
                capacity,
                services,
                network,
                backlogs,
                &mut worker_capacity,
                &mut result,
            ),
            MessagingMode::FanOut => process_fan_out(
                broker,
                accepted,
                services,
                network,
                &mut worker_capacity,
                &mut result,
            ),
            MessagingMode::Routed => process_routed(
                broker,
                accepted,
                services,
                network,
                &mut worker_capacity,
                &mut result,
            ),
        }
    }

    result.queued = backlogs.iter().map(|backlog| backlog.messages).sum();
    result
}

#[allow(clippy::too_many_arguments)]
fn process_queue(
    broker: &Service,
    accepted: u64,
    throughput: u64,
    services: &[Service],
    network: &Network,
    backlogs: &mut Vec<QueueBacklog>,
    worker_capacity: &mut [(ServiceId, u64)],
    result: &mut MessagingResult,
) {
    let backlog = queue_backlog(backlogs, broker.id());
    let storage = throughput.saturating_mul(QUEUE_STORAGE_MULTIPLIER);
    let stored = accepted.min(storage.saturating_sub(*backlog));
    result.dropped = result.dropped.saturating_add(accepted - stored);
    *backlog = backlog.saturating_add(stored);

    let mut remaining_dispatch = throughput;
    for consumer in consumers(broker.id(), services, network) {
        let available = remaining_worker_capacity(worker_capacity, consumer.id());
        let delivered = (*backlog).min(available).min(remaining_dispatch);
        *backlog -= delivered;
        remaining_dispatch -= delivered;
        consume_worker_capacity(worker_capacity, consumer.id(), delivered);
        result.processed = result.processed.saturating_add(delivered);
        push_traffic(
            &mut result.link_traffic,
            broker.id(),
            consumer.id(),
            delivered,
        );
    }
}

fn process_fan_out(
    broker: &Service,
    accepted: u64,
    services: &[Service],
    network: &Network,
    worker_capacity: &mut [(ServiceId, u64)],
    result: &mut MessagingResult,
) {
    let consumers = consumers(broker.id(), services, network);
    if consumers.is_empty() {
        result.dropped = result.dropped.saturating_add(accepted);
        return;
    }
    for consumer in consumers {
        let delivered = accepted.min(remaining_worker_capacity(worker_capacity, consumer.id()));
        consume_worker_capacity(worker_capacity, consumer.id(), delivered);
        result.processed = result.processed.saturating_add(delivered);
        result.dropped = result.dropped.saturating_add(accepted - delivered);
        push_traffic(
            &mut result.link_traffic,
            broker.id(),
            consumer.id(),
            delivered,
        );
    }
}

fn process_routed(
    broker: &Service,
    accepted: u64,
    services: &[Service],
    network: &Network,
    worker_capacity: &mut [(ServiceId, u64)],
    result: &mut MessagingResult,
) {
    let mut remaining = accepted;
    for consumer in consumers(broker.id(), services, network) {
        let delivered = remaining.min(remaining_worker_capacity(worker_capacity, consumer.id()));
        remaining -= delivered;
        consume_worker_capacity(worker_capacity, consumer.id(), delivered);
        result.processed = result.processed.saturating_add(delivered);
        push_traffic(
            &mut result.link_traffic,
            broker.id(),
            consumer.id(),
            delivered,
        );
    }
    result.dropped = result.dropped.saturating_add(remaining);
}

fn queue_backlog(backlogs: &mut Vec<QueueBacklog>, queue: ServiceId) -> &mut u64 {
    if let Some(index) = backlogs.iter().position(|backlog| backlog.queue == queue) {
        return &mut backlogs[index].messages;
    }
    backlogs.push(QueueBacklog { queue, messages: 0 });
    &mut backlogs
        .last_mut()
        .expect("the queue backlog was just inserted")
        .messages
}

fn incoming_requests(producer: ServiceId, request_traffic: &[LinkTraffic]) -> u64 {
    request_traffic
        .iter()
        .filter(|traffic| traffic.to == producer)
        .map(|traffic| traffic.requests)
        .sum()
}

fn consumers<'a>(
    broker: ServiceId,
    services: &'a [Service],
    network: &Network,
) -> Vec<&'a Service> {
    network
        .links()
        .iter()
        .filter(|link| link.from == broker)
        .filter_map(|link| service(services, link.to))
        .filter(|service| service.kind().serves_requests() && service.is_operational())
        .collect()
}

fn service(services: &[Service], id: ServiceId) -> Option<&Service> {
    services.iter().find(|service| service.id() == id)
}

fn remaining_worker_capacity(capacities: &[(ServiceId, u64)], worker: ServiceId) -> u64 {
    capacities
        .iter()
        .find(|(id, _)| *id == worker)
        .map_or(0, |(_, capacity)| *capacity)
}

fn consume_worker_capacity(capacities: &mut [(ServiceId, u64)], worker: ServiceId, consumed: u64) {
    if let Some((_, capacity)) = capacities.iter_mut().find(|(id, _)| *id == worker) {
        *capacity -= consumed;
    }
}

fn push_traffic(traffic: &mut Vec<LinkTraffic>, from: ServiceId, to: ServiceId, requests: u64) {
    if requests > 0 {
        traffic.push(LinkTraffic { from, to, requests });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GridPosition, ServiceKind};

    fn operational(id: u64, kind: ServiceKind) -> Service {
        let mut service = Service::new(ServiceId::new(id), kind, GridPosition::new(0, 0));
        while !service.is_operational() {
            service.advance_construction();
        }
        service
    }

    fn fixture(kind: ServiceKind, consumers: usize) -> (Vec<Service>, Network, Vec<LinkTraffic>) {
        let mut services = vec![
            operational(1, ServiceKind::InternetGateway),
            operational(2, ServiceKind::ApplicationServer),
            operational(3, kind),
        ];
        let mut network = Network::default();
        network.add_link(ServiceId::new(1), ServiceId::new(2));
        network.add_link(ServiceId::new(2), ServiceId::new(3));
        for offset in 0..consumers {
            let id = 4 + offset as u64;
            services.push(operational(id, ServiceKind::ApplicationServer));
            network.add_link(ServiceId::new(3), ServiceId::new(id));
        }
        let traffic = vec![LinkTraffic {
            from: ServiceId::new(1),
            to: ServiceId::new(2),
            requests: 100,
        }];
        (services, network, traffic)
    }

    #[test]
    fn queue_buffers_work_without_a_consumer_and_drains_it_later() {
        let (mut services, mut network, traffic) = fixture(ServiceKind::MessageQueue, 0);
        let mut backlogs = Vec::new();
        let first = process_messages(&services, &network, &traffic, &mut backlogs);
        assert_eq!(
            (first.published, first.processed, first.queued),
            (100, 0, 100)
        );

        services.push(operational(4, ServiceKind::ApplicationServer));
        network.add_link(ServiceId::new(3), ServiceId::new(4));
        let second = process_messages(&services, &network, &[], &mut backlogs);
        assert_eq!(
            (second.published, second.processed, second.queued),
            (0, 100, 0)
        );
    }

    #[test]
    fn topic_fans_each_event_out_to_every_consumer() {
        let (services, network, traffic) = fixture(ServiceKind::PubSubTopic, 2);
        let result = process_messages(&services, &network, &traffic, &mut Vec::new());
        assert_eq!(
            (result.published, result.processed, result.dropped),
            (100, 200, 0)
        );
        assert_eq!(result.link_traffic.len(), 3);
    }

    #[test]
    fn event_bus_routes_each_event_to_only_one_consumer() {
        let (services, network, traffic) = fixture(ServiceKind::EventBus, 2);
        let result = process_messages(&services, &network, &traffic, &mut Vec::new());
        assert_eq!(
            (result.published, result.processed, result.dropped),
            (100, 100, 0)
        );
        assert_eq!(result.link_traffic.len(), 2);
    }

    #[test]
    fn background_work_uses_only_a_consumers_remaining_capacity() {
        let (services, network, mut traffic) = fixture(ServiceKind::EventBus, 1);
        traffic.push(LinkTraffic {
            from: ServiceId::new(1),
            to: ServiceId::new(4),
            requests: 80,
        });
        let result = process_messages(&services, &network, &traffic, &mut Vec::new());
        assert_eq!(
            (result.published, result.processed, result.dropped),
            (100, 20, 80)
        );
    }

    #[test]
    fn queue_storage_is_bounded_and_overflow_is_dropped() {
        let (services, network, _) = fixture(ServiceKind::MessageQueue, 0);
        let traffic = vec![LinkTraffic {
            from: ServiceId::new(1),
            to: ServiceId::new(2),
            requests: 2_000,
        }];
        let mut backlogs = vec![QueueBacklog {
            queue: ServiceId::new(3),
            messages: 1_150,
        }];
        let result = process_messages(&services, &network, &traffic, &mut backlogs);
        assert_eq!((result.published, result.queued), (120, 1_200));
        assert_eq!(result.dropped, 1_950);
    }
}
