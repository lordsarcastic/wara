CREATE TABLE "jwt_public_key_records" (
    "id" UUID NOT NULL,
    "key_type" TEXT NOT NULL,
    "key_use" TEXT NOT NULL,
    "algorithm" TEXT NOT NULL,
    "modulus" TEXT NOT NULL,
    "exponent" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    "revoked_at" TEXT,
    PRIMARY KEY ("id")
);
