CREATE TABLE IF NOT EXISTS logs (
    block_num       INT8 NOT NULL,
    block_timestamp TIMESTAMPTZ NOT NULL,
    log_idx         INT4 NOT NULL,
    tx_idx          INT4 NOT NULL,
    tx_hash         BYTEA NOT NULL,
    address         BYTEA NOT NULL,
    selector        BYTEA,
    topics          BYTEA[] NOT NULL,
    data            BYTEA NOT NULL,
    PRIMARY KEY (block_num, log_idx)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'logs') 
       AND NOT EXISTS (SELECT 1 FROM pg_class c JOIN pg_partitioned_table pt ON c.oid = pt.partrelid WHERE c.relname = 'logs') THEN
        PERFORM create_hypertable('logs', by_range('block_num', 500000));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_logs_selector ON logs (selector, block_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_address ON logs (address, block_timestamp DESC);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = 'logs') THEN
        ALTER TABLE logs SET (
            timescaledb.compress,
            timescaledb.compress_segmentby = 'selector',
            timescaledb.compress_orderby = 'block_num DESC, log_idx'
        );
        PERFORM add_compression_policy('logs', 500000, if_not_exists => true);
    END IF;
END $$;
