use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, toasty::Model)]
pub struct Workspace {
    #[key]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}
