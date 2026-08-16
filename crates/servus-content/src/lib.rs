//! Game definitions and validation kept separate from simulation behaviour.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use servus_sim::ServiceKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDefinition {
    pub kind: ServiceKind,
    pub display_name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCatalog {
    services: Vec<ServiceDefinition>,
}

impl ContentCatalog {
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            services: vec![
                ServiceDefinition {
                    kind: ServiceKind::InternetGateway,
                    display_name: "Internet Gateway".to_owned(),
                    description: "Provides an entry point for incoming internet traffic."
                        .to_owned(),
                },
                ServiceDefinition {
                    kind: ServiceKind::Firewall,
                    display_name: "Firewall".to_owned(),
                    description: "Blocks attacks when every ingress path passes through it."
                        .to_owned(),
                },
                ServiceDefinition {
                    kind: ServiceKind::LoadBalancer,
                    display_name: "Load Balancer".to_owned(),
                    description: "Distributes incoming traffic across downstream services."
                        .to_owned(),
                },
                ServiceDefinition {
                    kind: ServiceKind::ApplicationServer,
                    display_name: "Application Server".to_owned(),
                    description: "Runs application code and handles incoming requests.".to_owned(),
                },
            ],
        }
    }

    pub fn new(services: Vec<ServiceDefinition>) -> Result<Self, ContentError> {
        let mut seen = HashSet::new();

        for definition in &services {
            if definition.display_name.trim().is_empty() {
                return Err(ContentError::EmptyServiceName(definition.kind));
            }
            if !seen.insert(definition.kind) {
                return Err(ContentError::DuplicateService(definition.kind));
            }
        }

        Ok(Self { services })
    }

    #[must_use]
    pub fn services(&self) -> &[ServiceDefinition] {
        &self.services
    }

    #[must_use]
    pub fn service(&self, kind: ServiceKind) -> Option<&ServiceDefinition> {
        self.services
            .iter()
            .find(|definition| definition.kind == kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentError {
    EmptyServiceName(ServiceKind),
    DuplicateService(ServiceKind),
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyServiceName(kind) => write!(formatter, "service {kind:?} has an empty name"),
            Self::DuplicateService(kind) => write!(formatter, "service {kind:?} is defined twice"),
        }
    }
}

impl Error for ContentError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(kind: ServiceKind, display_name: &str) -> ServiceDefinition {
        ServiceDefinition {
            kind,
            display_name: display_name.to_owned(),
            description: "Test service".to_owned(),
        }
    }

    #[test]
    fn builtin_content_defines_every_service_kind() {
        let catalog = ContentCatalog::builtin();
        for kind in ServiceKind::ALL {
            assert!(
                catalog.service(kind).is_some(),
                "missing content for {kind:?}"
            );
        }
    }

    #[test]
    fn valid_custom_content_is_accepted() {
        let services = vec![definition(
            ServiceKind::ApplicationServer,
            "Application Server",
        )];
        let catalog = ContentCatalog::new(services.clone());
        assert_eq!(catalog.map(|catalog| catalog.services), Ok(services));
    }

    #[test]
    fn empty_service_names_are_rejected() {
        let result = ContentCatalog::new(vec![definition(ServiceKind::ApplicationServer, "   ")]);
        assert_eq!(
            result,
            Err(ContentError::EmptyServiceName(
                ServiceKind::ApplicationServer
            ))
        );
    }

    #[test]
    fn duplicate_services_are_rejected() {
        let result = ContentCatalog::new(vec![
            definition(ServiceKind::ApplicationServer, "First"),
            definition(ServiceKind::ApplicationServer, "Second"),
        ]);
        assert_eq!(
            result,
            Err(ContentError::DuplicateService(
                ServiceKind::ApplicationServer
            ))
        );
    }

    #[test]
    fn content_errors_have_readable_messages() {
        assert_eq!(
            ContentError::DuplicateService(ServiceKind::ApplicationServer).to_string(),
            "service ApplicationServer is defined twice"
        );
    }
}
