CREATE TABLE IF NOT EXISTS receipts (
    block_num               INT8 NOT NULL,
    block_timestamp         TIMESTAMPTZ NOT NULL,
    tx_idx                  INT4 NOT NULL,
    tx_hash                 BYTEA NOT NULL,
    "from"                  BYTEA NOT NULL,
    "to"                    BYTEA,
    contract_address        BYTEA,
    gas_used                INT8 NOT NULL,
    cumulative_gas_used     INT8 NOT NULL,
    effective_gas_price     TEXT,
    status                  INT2,
    fee_payer               BYTEA,
    PRIMARY KEY (block_timestamp, block_num, tx_idx)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'receipts') 
       AND NOT EXISTS (SELECT 1 FROM pg_class c JOIN pg_partitioned_table pt ON c.oid = pt.partrelid WHERE c.relname = 'receipts') THEN
        PERFORM create_hypertable('receipts', by_range('block_timestamp', INTERVAL '7 days'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_receipts_tx_hash ON receipts (tx_hash);
CREATE INDEX IF NOT EXISTS idx_receipts_block_num ON receipts (block_num);
CREATE INDEX IF NOT EXISTS idx_receipts_from ON receipts ("from", block_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_receipts_fee_payer ON receipts (fee_payer, block_timestamp DESC) WHERE fee_payer IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_receipts_contract_address ON receipts (contract_address) WHERE contract_address IS NOT NULL;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'receipts') THEN
        ALTER TABLE receipts SET (
            timescaledb.compress,
            timescaledb.compress_orderby = 'block_timestamp DESC, tx_idx'
        );
        PERFORM add_compression_policy('receipts', INTERVAL '30 days', if_not_exists => true);
    END IF;
END $$;
