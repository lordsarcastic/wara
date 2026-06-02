use crate::libs::docker::ProxyKind;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Domain {
    pub id: Uuid,
    pub service_id: Uuid,
    pub hostname: String,
    pub proxy: ProxyKind,
    pub tls_enabled: bool,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct DomainRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub service_id: Uuid,
    pub hostname: String,
    pub proxy: String,
    pub tls_enabled: bool,
}

impl From<DomainRecord> for Domain {
    fn from(record: DomainRecord) -> Self {
        Self {
            id: record.id,
            service_id: record.service_id,
            hostname: record.hostname,
            proxy: ProxyKind::from(record.proxy.as_str()),
            tls_enabled: record.tls_enabled,
        }
    }
}
