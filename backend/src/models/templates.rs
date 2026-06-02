use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct WorkspaceTemplate {
    pub id: Uuid,
    pub source_workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct WorkspaceTemplateRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub source_workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl From<WorkspaceTemplateRecord> for WorkspaceTemplate {
    fn from(record: WorkspaceTemplateRecord) -> Self {
        Self {
            id: record.id,
            source_workspace_id: record.source_workspace_id,
            name: record.name,
            description: record.description,
        }
    }
}
