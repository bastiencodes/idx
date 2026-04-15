CREATE TABLE IF NOT EXISTS txs (
    block_num               Int64,
    block_timestamp         DateTime64(3, 'UTC'),
    idx                     Int32,
    hash                    String,
    `type`                  Int16,
    `from`                  String,
    `to`                    Nullable(String),
    value                   String,
    input                   String,
    gas_limit               Int64,
    max_fee_per_gas         String,
    max_priority_fee_per_gas String,
    gas_used                Nullable(Int64),
    nonce                   Int64
) ENGINE = ReplacingMergeTree()
PARTITION BY toYYYYMM(block_timestamp)
ORDER BY (block_num, idx);

-- Migration: remove legacy Tempo-only columns.
ALTER TABLE txs DROP COLUMN IF EXISTS nonce_key;
ALTER TABLE txs DROP COLUMN IF EXISTS fee_token;
ALTER TABLE txs DROP COLUMN IF EXISTS fee_payer;
ALTER TABLE txs DROP COLUMN IF EXISTS calls;
ALTER TABLE txs DROP COLUMN IF EXISTS call_count;
ALTER TABLE txs DROP COLUMN IF EXISTS valid_before;
ALTER TABLE txs DROP COLUMN IF EXISTS valid_after;
ALTER TABLE txs DROP COLUMN IF EXISTS signature_type;
