use std::collections::{HashSet, VecDeque};

use crate::{CyberAttackReport, Network, Service, ServiceId, ServiceKind, Tick};

pub const CYBER_ATTACK_INTERVAL: u64 = 8;
pub const DISRUPTION_TICKS: u16 = 2;

pub(crate) fn resolve_scheduled_attack(
    tick: Tick,
    services: &mut [Service],
    network: &Network,
) -> Option<CyberAttackReport> {
    if tick.number() == 0 || !tick.number().is_multiple_of(CYBER_ATTACK_INTERVAL) {
        return None;
    }

    let target = services
        .iter()
        .find(|service| {
            service.kind() == ServiceKind::ApplicationServer
                && service.is_operational()
                && reachable_from_gateway(services, network, service.id(), true)
        })?
        .id();
    let blocked = !reachable_from_gateway(services, network, target, false);
    if !blocked {
        let service = services
            .iter_mut()
            .find(|service| service.id() == target)
            .expect("the selected attack target exists");
        let disrupted = service.disrupt(DISRUPTION_TICKS);
        debug_assert!(disrupted, "attack targets must be operational");
    }
    Some(CyberAttackReport {
        target,
        blocked,
        disruption_ticks: if blocked { 0 } else { DISRUPTION_TICKS },
    })
}

fn reachable_from_gateway(
    services: &[Service],
    network: &Network,
    target: ServiceId,
    allow_firewalls: bool,
) -> bool {
    let mut visited = HashSet::new();
    let mut queue = services
        .iter()
        .filter(|service| {
            service.kind() == ServiceKind::InternetGateway && service.is_operational()
        })
        .map(|service| service.id())
        .collect::<VecDeque<_>>();

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) {
            continue;
        }
        if current == target {
            return true;
        }
        for link in network.links().iter().filter(|link| link.from == current) {
            let Some(next) = services.iter().find(|service| service.id() == link.to) else {
                continue;
            };
            if !next.is_operational() || (!allow_firewalls && next.kind() == ServiceKind::Firewall)
            {
                continue;
            }
            queue.push_back(next.id());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::GridPosition;

    use super::*;

    fn operational(id: u64, kind: ServiceKind) -> Service {
        let mut service = Service::new(ServiceId::new(id), kind, GridPosition::new(id as u16, 0));
        for _ in 0..kind.construction_ticks() {
            service.advance_construction();
        }
        service
    }

    fn connect(network: &mut Network, from: u64, to: u64) {
        network.add_link(ServiceId::new(from), ServiceId::new(to));
    }

    fn attack_tick() -> Tick {
        let mut tick = Tick::default();
        for _ in 0..CYBER_ATTACK_INTERVAL {
            tick.advance();
        }
        tick
    }

    #[test]
    fn attacks_only_run_on_the_schedule_and_need_a_reachable_server() {
        let mut services = vec![
            operational(1, ServiceKind::InternetGateway),
            operational(2, ServiceKind::ApplicationServer),
        ];
        let network = Network::default();
        assert_eq!(
            resolve_scheduled_attack(Tick::default(), &mut services, &network),
            None
        );
        assert_eq!(
            resolve_scheduled_attack(attack_tick(), &mut services, &network),
            None
        );
    }

    #[test]
    fn an_unprotected_path_disrupts_the_target() {
        let mut services = vec![
            operational(1, ServiceKind::InternetGateway),
            operational(2, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);

        assert_eq!(
            resolve_scheduled_attack(attack_tick(), &mut services, &network),
            Some(CyberAttackReport {
                target: ServiceId::new(2),
                blocked: false,
                disruption_ticks: DISRUPTION_TICKS,
            })
        );
        assert_eq!(
            services[1].state(),
            crate::ServiceState::Disrupted {
                ticks_remaining: DISRUPTION_TICKS
            }
        );
    }

    #[test]
    fn a_firewall_on_every_ingress_path_blocks_the_attack() {
        let mut services = vec![
            operational(1, ServiceKind::InternetGateway),
            operational(2, ServiceKind::Firewall),
            operational(3, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);

        let report = resolve_scheduled_attack(attack_tick(), &mut services, &network)
            .expect("the protected server is reachable");
        assert!(report.blocked);
        assert_eq!(report.disruption_ticks, 0);
        assert_eq!(services[2].state(), crate::ServiceState::Operational);
    }

    #[test]
    fn a_firewall_bypass_leaves_the_server_vulnerable() {
        let mut services = vec![
            operational(1, ServiceKind::InternetGateway),
            operational(2, ServiceKind::Firewall),
            operational(3, ServiceKind::ApplicationServer),
        ];
        let mut network = Network::default();
        connect(&mut network, 1, 2);
        connect(&mut network, 2, 3);
        connect(&mut network, 1, 3);

        let report = resolve_scheduled_attack(attack_tick(), &mut services, &network)
            .expect("the server is reachable");
        assert!(!report.blocked);
        assert!(matches!(
            services[2].state(),
            crate::ServiceState::Disrupted { .. }
        ));
    }
}
