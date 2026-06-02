#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConnectivityWorkflowInput {
    pub interval_seconds: u64,
}
