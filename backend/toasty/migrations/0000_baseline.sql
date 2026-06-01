CREATE TABLE "domain_records" (
    "id" UUID NOT NULL,
    "service_id" UUID NOT NULL,
    "hostname" TEXT NOT NULL,
    "proxy" TEXT NOT NULL,
    "tls_enabled" BOOLEAN NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_domain_records_by_service_id" ON "domain_records" ("service_id");
CREATE TABLE "app_service_records" (
    "id" UUID NOT NULL,
    "project_id" UUID NOT NULL,
    "environment_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "deploy_kind" TEXT NOT NULL,
    "image" TEXT,
    "compose_file" TEXT,
    "dockerfile" TEXT,
    "internal_port" INTEGER,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_app_service_records_by_project_id" ON "app_service_records" ("project_id");
CREATE INDEX "index_app_service_records_by_environment_id" ON "app_service_records" ("environment_id");
CREATE TABLE "projects" (
    "id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "description" TEXT,
    PRIMARY KEY ("id")
);
CREATE TABLE "docker_credential_records" (
    "id" UUID NOT NULL,
    "project_id" UUID NOT NULL,
    "registry" TEXT NOT NULL,
    "username" TEXT NOT NULL,
    "encrypted_password" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_docker_credential_records_by_project_id" ON "docker_credential_records" ("project_id");
CREATE TABLE "env_var_records" (
    "id" UUID NOT NULL,
    "project_id" UUID NOT NULL,
    "environment_id" UUID,
    "service_id" UUID,
    "key" TEXT NOT NULL,
    "encrypted_value" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_env_var_records_by_project_id" ON "env_var_records" ("project_id");
CREATE TABLE "project_template_records" (
    "id" UUID NOT NULL,
    "source_project_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "description" TEXT,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_project_template_records_by_source_project_id" ON "project_template_records" ("source_project_id");
CREATE TABLE "server_records" (
    "id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "host" TEXT NOT NULL,
    "port" INTEGER NOT NULL,
    "username" TEXT NOT NULL,
    "public_key" TEXT NOT NULL,
    "private_key_fingerprint" TEXT NOT NULL,
    "encrypted_private_key" TEXT NOT NULL,
    "encrypted_private_key_passphrase" TEXT,
    "default_proxy" TEXT NOT NULL,
    "docker_status" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "environments" (
    "id" UUID NOT NULL,
    "project_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_environments_by_project_id" ON "environments" ("project_id");
CREATE TABLE "deployment_records" (
    "id" UUID NOT NULL,
    "service_id" UUID NOT NULL,
    "status" TEXT NOT NULL,
    "workflow_id" TEXT NOT NULL,
    "output" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_deployment_records_by_service_id" ON "deployment_records" ("service_id");
CREATE TABLE "user_records" (
    "id" UUID NOT NULL,
    "email" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "role" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "password_hash" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_user_records_by_email" ON "user_records" ("email");
CREATE TABLE "user_invite_records" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "token_hash" TEXT NOT NULL,
    "expires_at" TEXT NOT NULL,
    "accepted_at" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_user_invite_records_by_user_id" ON "user_invite_records" ("user_id");
CREATE UNIQUE INDEX "index_user_invite_records_by_token_hash" ON "user_invite_records" ("token_hash");
