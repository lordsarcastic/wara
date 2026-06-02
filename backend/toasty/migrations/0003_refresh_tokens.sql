CREATE TABLE "user_refresh_token_records" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "created_at" TEXT NOT NULL,
    "expires_at" TEXT NOT NULL,
    "revoked_at" TEXT,
    PRIMARY KEY ("id")
);

CREATE INDEX "index_user_refresh_token_records_by_user_id" ON "user_refresh_token_records" ("user_id");
