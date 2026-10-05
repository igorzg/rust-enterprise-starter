use async_trait::async_trait;

/// Status of a single dependency, reported by the adapter that owns it.
/// The core names no specific technology: the adapter supplies the
/// component name (e.g. "postgres", "redis").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ComponentStatus {
    pub name: &'static str,
    pub up: bool,
}

/// Result of a readiness check: a list of component statuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthStatus {
    pub components: Vec<ComponentStatus>,
}

impl HealthStatus {
    /// The API is ready when every dependency is up.
    pub fn ready(&self) -> bool {
        self.components.iter().all(|component| component.up)
    }
}

/// Dependency-probe port used by the `/ready` endpoint.
#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;
}
