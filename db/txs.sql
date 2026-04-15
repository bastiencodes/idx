CREATE TABLE IF NOT EXISTS txs (
    block_num               INT8 NOT NULL,
    block_timestamp         TIMESTAMPTZ NOT NULL,
    idx                     INT4 NOT NULL,
    hash                    BYTEA NOT NULL,
    type                    INT2 NOT NULL,
    "from"                  BYTEA NOT NULL,
    "to"                    BYTEA,
    value                   TEXT NOT NULL,
    input                   BYTEA NOT NULL,
    gas_limit               INT8 NOT NULL,
    max_fee_per_gas         TEXT NOT NULL,
    max_priority_fee_per_gas TEXT NOT NULL,
    gas_used                INT8,
    nonce                   INT8 NOT NULL,
    PRIMARY KEY (block_timestamp, block_num, idx)
);

CREATE INDEX IF NOT EXISTS idx_txs_hash ON txs (hash);
CREATE INDEX IF NOT EXISTS idx_txs_block_num ON txs (block_num DESC);
DROP INDEX IF EXISTS idx_txs_block_num_asc;
CREATE INDEX IF NOT EXISTS idx_txs_from ON txs ("from", block_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_txs_to ON txs ("to", block_timestamp DESC);
DROP INDEX IF EXISTS idx_txs_calls;
DROP INDEX IF EXISTS idx_txs_selector;

-- Migration: remove legacy Tempo-only columns/indexes.
DROP INDEX IF EXISTS idx_txs_calls_partial;
ALTER TABLE txs DROP COLUMN IF EXISTS nonce_key;
ALTER TABLE txs DROP COLUMN IF EXISTS fee_token;
ALTER TABLE txs DROP COLUMN IF EXISTS fee_payer;
ALTER TABLE txs DROP COLUMN IF EXISTS calls;
ALTER TABLE txs DROP COLUMN IF EXISTS call_count;
ALTER TABLE txs DROP COLUMN IF EXISTS valid_before;
ALTER TABLE txs DROP COLUMN IF EXISTS valid_after;
ALTER TABLE txs DROP COLUMN IF EXISTS signature_type;
