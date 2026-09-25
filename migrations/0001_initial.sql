-- Initial schema for chainwatch-rs.
-- Amounts are stored as decimal text so uint256 values never lose precision.

CREATE TABLE IF NOT EXISTS indexed_events (
    id                  BIGSERIAL PRIMARY KEY,
    chain_id            BIGINT NOT NULL,
    contract_address    TEXT NOT NULL,
    transaction_hash    TEXT NOT NULL,
    log_index           INTEGER NOT NULL,
    sender_address      TEXT NOT NULL,
    recipient_address   TEXT NOT NULL,
    amount              TEXT NOT NULL,
    block_number        BIGINT NOT NULL,
    block_hash          TEXT NOT NULL,
    block_timestamp     TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT indexed_events_identity UNIQUE (chain_id, transaction_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_indexed_events_block_number
    ON indexed_events (chain_id, block_number);

CREATE INDEX IF NOT EXISTS idx_indexed_events_tx
    ON indexed_events (transaction_hash);

CREATE INDEX IF NOT EXISTS idx_indexed_events_contract
    ON indexed_events (contract_address);

CREATE INDEX IF NOT EXISTS idx_indexed_events_sender
    ON indexed_events (sender_address);

CREATE INDEX IF NOT EXISTS idx_indexed_events_recipient
    ON indexed_events (recipient_address);

CREATE TABLE IF NOT EXISTS indexed_blocks (
    chain_id        BIGINT NOT NULL,
    block_number    BIGINT NOT NULL,
    block_hash      TEXT NOT NULL,
    parent_hash     TEXT NOT NULL,
    indexed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (chain_id, block_number)
);

CREATE INDEX IF NOT EXISTS idx_indexed_blocks_hash
    ON indexed_blocks (chain_id, block_hash);

CREATE TABLE IF NOT EXISTS indexer_checkpoints (
    chain_id                    BIGINT PRIMARY KEY,
    last_indexed_block          BIGINT NOT NULL,
    last_indexed_block_hash     TEXT NOT NULL,
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
