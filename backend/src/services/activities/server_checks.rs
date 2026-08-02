use crate::{
    errors::api::ApiError,
    libs::db::Database,
    services::servers::{ServerCheckResponse, ServerService},
};

#[derive(Clone)]
pub struct ServerCheckActivities {
    service: ServerService,
}

impl ServerCheckActivities {
    pub fn new(db: Database, secret_key: String) -> Self {
        Self {
            service: ServerService::new(db, secret_key),
        }
    }

    pub async fn check_all_servers(&self) -> Result<Vec<ServerCheckResponse>, ApiError> {
        self.service.check_all_servers().await
    }
}
