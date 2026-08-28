-- This is free and unencumbered software released into the public domain.

DROP TABLE IF EXISTS "bitcache_meta";
DROP TABLE IF EXISTS "bitcache_data";
DROP TABLE IF EXISTS "bitcache_blob";
DROP TABLE IF EXISTS "bitcache_config";

CREATE TABLE IF NOT EXISTS "bitcache_config" (
    "key" text NOT NULL,
    "val" numeric NOT NULL,
    PRIMARY KEY ("key"),
    CHECK (length("key") > 0)
);

INSERT INTO "bitcache_config" ("key", "val") VALUES ('schema', 1);

CREATE TABLE IF NOT EXISTS "bitcache_blob" (
    "id" integer PRIMARY KEY AUTOINCREMENT,
    "blake3" blob NOT NULL,
    UNIQUE ("blake3"),
    CHECK (length("blake3") = 32)
);

CREATE TABLE IF NOT EXISTS "bitcache_data" (
    "id" integer NOT NULL REFERENCES "bitcache_blob"("id"),
    "encoding" integer NULL,      -- NULL denotes no encoding
    "data" blob NOT NULL,
    PRIMARY KEY ("id")
); -- WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS "bitcache_meta" (
    "id" integer NOT NULL REFERENCES "bitcache_blob"("id"),
    "created" integer NOT NULL,   -- milliseconds since the epoch
    "updated" integer NOT NULL,   -- milliseconds since the epoch
    "accessed" integer NULL,      -- milliseconds since the epoch; NULL until first retrieval
    "expires" integer NULL,       -- milliseconds since the epoch; NULL denotes no expiry
    "media_type" text NULL,       -- NULL denotes "application/octet-stream"
    PRIMARY KEY ("id")
); -- WITHOUT ROWID;
