CREATE TABLE IF NOT EXISTS erc20_tokens (
    address                 BYTEA NOT NULL PRIMARY KEY,

    -- Metadata (null until resolved, or permanently null if unimplemented)
    name                    TEXT,
    symbol                  TEXT,
    decimals                INT2,

    -- Discovery: how we first observed the token (always populated)
    first_transfer_block    INT8 NOT NULL,
    first_transfer_tx_hash  BYTEA NOT NULL,
    first_transfer_at       TIMESTAMPTZ NOT NULL,

    -- Deployment: from receipts.contract_address (null for factory-deployed
    -- contracts or deployments before index start)
    deployed_block          INT8,
    deployed_tx_hash        BYTEA,
    deployed_at             TIMESTAMPTZ,

    -- Resolution state (managed by the erc20_metadata worker)
    resolution_status       TEXT NOT NULL,
    resolution_attempts     INT4 NOT NULL DEFAULT 0,
    -- Block timestamp / number the metadata snapshot was taken at. Both come
    -- from the same atomic Multicall3 batch (getBlockNumber() and
    -- getCurrentBlockTimestamp()) as the name/symbol/decimals reads, so the
    -- metadata is known to be the chain state at exactly this block. Matches
    -- the first_transfer_at/_block and deployed_at/_block naming above.
    resolved_at             TIMESTAMPTZ,
    resolved_block          INT8
);

CREATE INDEX IF NOT EXISTS idx_erc20_tokens_first_transfer_at
    ON erc20_tokens (first_transfer_at DESC);

CREATE INDEX IF NOT EXISTS idx_erc20_tokens_pending
    ON erc20_tokens (resolution_status)
    WHERE resolution_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_erc20_tokens_symbol
    ON erc20_tokens (symbol);

COMMENT ON COLUMN erc20_tokens.first_transfer_at IS
    'Block timestamp of the first Transfer log we observed from this contract';
COMMENT ON COLUMN erc20_tokens.deployed_at IS
    'Block timestamp of the CREATE tx (null if factory-deployed or before index start)';
COMMENT ON COLUMN erc20_tokens.resolution_status IS
    'Metadata resolution state: pending | ok | partial | failed';
COMMENT ON COLUMN erc20_tokens.resolved_at IS
    'Block timestamp of the chain state the metadata was read at (from Multicall3.getCurrentBlockTimestamp())';
COMMENT ON COLUMN erc20_tokens.resolved_block IS
    'Block number the metadata reads ran at, reported by Multicall3.getBlockNumber() in the same atomic batch';
