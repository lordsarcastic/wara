CREATE TABLE "user_api_token_records" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "token_hash" TEXT NOT NULL,
    "token_prefix" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    "revoked_at" TEXT,
    "last_used_at" TEXT,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_user_api_token_records_by_user_id" ON "user_api_token_records" ("user_id");
CREATE UNIQUE INDEX "index_user_api_token_records_by_token_hash" ON "user_api_token_records" ("token_hash");
