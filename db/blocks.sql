CREATE TABLE IF NOT EXISTS blocks (
    num             INT8 NOT NULL,
    hash            BYTEA NOT NULL,
    parent_hash     BYTEA NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    timestamp_ms    INT8 NOT NULL,
    gas_limit       INT8 NOT NULL,
    gas_used        INT8 NOT NULL,
    miner           BYTEA NOT NULL,
    extra_data      BYTEA,
    PRIMARY KEY (timestamp, num)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'blocks') 
       AND NOT EXISTS (SELECT 1 FROM pg_class c JOIN pg_partitioned_table pt ON c.oid = pt.partrelid WHERE c.relname = 'blocks') THEN
        PERFORM create_hypertable('blocks', by_range('timestamp', INTERVAL '7 days'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_blocks_num ON blocks (num);
CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks (hash);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'blocks') THEN
        ALTER TABLE blocks SET (
            timescaledb.compress,
            timescaledb.compress_orderby = 'timestamp DESC'
        );
        PERFORM add_compression_policy('blocks', INTERVAL '30 days', if_not_exists => true);
    END IF;
END $$;
