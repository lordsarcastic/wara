ALTER TABLE "server_records"
    ADD COLUMN "docker_version" TEXT NOT NULL DEFAULT '',
    ADD COLUMN "last_check_at" TEXT NOT NULL DEFAULT '',
    ADD COLUMN "last_check_error" TEXT NOT NULL DEFAULT '';
