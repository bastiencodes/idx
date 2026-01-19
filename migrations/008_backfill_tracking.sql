-- Track bidirectional backfill progress
-- backfill_num: lowest block synced going backwards (NULL = not started, 0 = complete)
-- started_at: when sync started (for resumption tracking)

ALTER TABLE sync_state ADD COLUMN IF NOT EXISTS backfill_num INT8;
ALTER TABLE sync_state ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ;

COMMENT ON COLUMN sync_state.synced_num IS 'Highest block synced (forward direction)';
COMMENT ON COLUMN sync_state.backfill_num IS 'Lowest block synced going backwards (NULL=not started, 0=complete)';
COMMENT ON COLUMN sync_state.started_at IS 'When this sync instance started';
