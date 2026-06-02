use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::{db::Database, docker::ProxyKind},
    models::domains::{Domain, DomainRecord},
    services::app_services::AppServiceService,
};

#[derive(Debug, Clone)]
pub struct CreateDomainInput {
    pub service_id: Uuid,
    pub hostname: String,
    pub proxy: Option<ProxyKind>,
    pub tls_enabled: Option<bool>,
}

#[derive(Clone)]
pub struct DomainService {
    db: Database,
}

impl DomainService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list_domains(&self, service_id: Uuid) -> Result<Vec<Domain>, ApiError> {
        let mut db = self.db.handle()?;
        let records =
            Query::<List<DomainRecord>>::filter(DomainRecord::fields().service_id().eq(service_id))
                .exec(&mut db)
                .await
                .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(Domain::from).collect())
    }

    pub async fn create_domain(&self, input: CreateDomainInput) -> Result<Domain, ApiError> {
        AppServiceService::new(self.db.clone())
            .get_service(input.service_id)
            .await?;
        let mut db = self.db.handle()?;
        let record = toasty::create!(DomainRecord {
            id: Uuid::now_v7(),
            service_id: input.service_id,
            hostname: input.hostname,
            proxy: input.proxy.unwrap_or(ProxyKind::Nginx).as_str().to_string(),
            tls_enabled: input.tls_enabled.unwrap_or(true),
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(Domain::from(record))
    }

    pub async fn get_domain(&self, id: Uuid) -> Result<Domain, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<DomainRecord>>::filter(DomainRecord::fields().id().eq(id))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        record.map(Domain::from).ok_or(ApiError::NotFound("domain"))
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
