ALTER TABLE "projects" RENAME TO "workspaces";

ALTER TABLE "environments" RENAME COLUMN "project_id" TO "workspace_id";
ALTER INDEX "index_environments_by_project_id" RENAME TO "index_environments_by_workspace_id";

ALTER TABLE "app_service_records" RENAME COLUMN "project_id" TO "workspace_id";
ALTER INDEX "index_app_service_records_by_project_id" RENAME TO "index_app_service_records_by_workspace_id";

ALTER TABLE "docker_credential_records" RENAME COLUMN "project_id" TO "workspace_id";
ALTER INDEX "index_docker_credential_records_by_project_id" RENAME TO "index_docker_credential_records_by_workspace_id";

ALTER TABLE "env_var_records" RENAME COLUMN "project_id" TO "workspace_id";
ALTER INDEX "index_env_var_records_by_project_id" RENAME TO "index_env_var_records_by_workspace_id";

ALTER TABLE "project_template_records" RENAME TO "workspace_template_records";
ALTER TABLE "workspace_template_records" RENAME COLUMN "source_project_id" TO "source_workspace_id";
ALTER INDEX "index_project_template_records_by_source_project_id" RENAME TO "index_workspace_template_records_by_source_workspace_id";

CREATE TABLE "workspace_user_role_records" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "workspace_id" UUID NOT NULL,
    "role" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_workspace_user_role_records_by_user_id" ON "workspace_user_role_records" ("user_id");
CREATE INDEX "index_workspace_user_role_records_by_workspace_id" ON "workspace_user_role_records" ("workspace_id");

UPDATE "user_records" SET "role" = 'super_admin' WHERE "role" = 'admin';
