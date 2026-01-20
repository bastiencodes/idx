-- Migrate sync_state to support multiple chains (one row per chain_id)
-- Previously: single row with id=1 constraint
-- Now: one row per chain_id

-- Drop the single-row constraint
ALTER TABLE sync_state DROP CONSTRAINT IF EXISTS sync_state_single_row;

-- Drop the id column and make chain_id the primary key
ALTER TABLE sync_state DROP CONSTRAINT IF EXISTS sync_state_pkey;
ALTER TABLE sync_state DROP COLUMN IF EXISTS id;
ALTER TABLE sync_state ADD PRIMARY KEY (chain_id);

-- Add index for lookups
CREATE INDEX IF NOT EXISTS idx_sync_state_chain_id ON sync_state (chain_id);
