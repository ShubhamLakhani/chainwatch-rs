//! PostgreSQL persistence with transactional checkpoints.

use crate::application::ports::{EventStore, IndexedBlockRecord, RollbackStats};
use crate::domain::{
    AppError, BlockHeader, Checkpoint, DecodedTransfer, EventListFilter, IndexedEvent,
    PersistOutcome,
};
use crate::infrastructure::persistence::rows::{CheckpointRow, IndexedBlockRow, IndexedEventRow};
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use tracing::{debug, info};

#[derive(Clone)]
pub struct Repository {
    pool: PgPool,
}

impl Repository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl EventStore for Repository {
    async fn ping(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::database)
    }

    async fn get_checkpoint(&self, chain_id: i64) -> Result<Option<Checkpoint>, AppError> {
        let row = sqlx::query_as::<_, CheckpointRow>(
            "SELECT chain_id, last_indexed_block, last_indexed_block_hash, updated_at
             FROM indexer_checkpoints
             WHERE chain_id = $1",
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database)?;
        Ok(row.map(|row| Checkpoint {
            chain_id: row.chain_id,
            last_indexed_block: row.last_indexed_block,
            last_indexed_block_hash: row.last_indexed_block_hash,
            updated_at: row.updated_at,
        }))
    }

    async fn get_indexed_block(
        &self,
        chain_id: i64,
        block_number: u64,
    ) -> Result<Option<IndexedBlockRecord>, AppError> {
        let row = sqlx::query_as::<_, IndexedBlockRow>(
            "SELECT chain_id, block_number, block_hash, parent_hash, indexed_at
             FROM indexed_blocks
             WHERE chain_id = $1 AND block_number = $2",
        )
        .bind(chain_id)
        .bind(block_number as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database)?;
        Ok(row.map(|row| IndexedBlockRecord {
            chain_id: row.chain_id,
            block_number: row.block_number,
            block_hash: row.block_hash,
            parent_hash: row.parent_hash,
            indexed_at: row.indexed_at,
        }))
    }

    async fn count_events(&self, chain_id: i64) -> Result<i64, AppError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM indexed_events WHERE chain_id = $1")
                .bind(chain_id)
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::database)?;
        Ok(count.0)
    }

    async fn persist_block(
        &self,
        chain_id: i64,
        header: &BlockHeader,
        events: &[DecodedTransfer],
    ) -> Result<PersistOutcome, AppError> {
        let mut tx: Transaction<'_, Postgres> =
            self.pool.begin().await.map_err(AppError::database)?;
        let outcome = persist_block_in_tx(&mut tx, chain_id, header, events).await?;
        tx.commit().await.map_err(AppError::database)?;
        if outcome.duplicate_events > 0 {
            debug!(
                block = header.number,
                duplicates = outcome.duplicate_events,
                "duplicate events ignored during persistence"
            );
        }
        Ok(outcome)
    }

    async fn rollback_after(
        &self,
        chain_id: i64,
        ancestor_block: u64,
        ancestor_hash: &str,
    ) -> Result<RollbackStats, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database)?;

        let events = sqlx::query(
            "DELETE FROM indexed_events
             WHERE chain_id = $1 AND block_number > $2",
        )
        .bind(chain_id)
        .bind(ancestor_block as i64)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database)?;

        let blocks = sqlx::query(
            "DELETE FROM indexed_blocks
             WHERE chain_id = $1 AND block_number > $2",
        )
        .bind(chain_id)
        .bind(ancestor_block as i64)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database)?;

        sqlx::query(
            "INSERT INTO indexer_checkpoints
                (chain_id, last_indexed_block, last_indexed_block_hash, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (chain_id) DO UPDATE SET
                last_indexed_block = EXCLUDED.last_indexed_block,
                last_indexed_block_hash = EXCLUDED.last_indexed_block_hash,
                updated_at = NOW()",
        )
        .bind(chain_id)
        .bind(ancestor_block as i64)
        .bind(ancestor_hash)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database)?;

        tx.commit().await.map_err(AppError::database)?;

        let stats = RollbackStats {
            deleted_events: events.rows_affected(),
            deleted_blocks: blocks.rows_affected(),
            ancestor_block,
        };
        info!(
            chain_id,
            ancestor_block,
            ancestor_hash,
            deleted_events = stats.deleted_events,
            deleted_blocks = stats.deleted_blocks,
            "reorg rollback committed"
        );
        Ok(stats)
    }

    async fn reset_chain(&self, chain_id: i64) -> Result<RollbackStats, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database)?;
        let events = sqlx::query("DELETE FROM indexed_events WHERE chain_id = $1")
            .bind(chain_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database)?;
        let blocks = sqlx::query("DELETE FROM indexed_blocks WHERE chain_id = $1")
            .bind(chain_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database)?;
        sqlx::query("DELETE FROM indexer_checkpoints WHERE chain_id = $1")
            .bind(chain_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database)?;
        tx.commit().await.map_err(AppError::database)?;
        info!(
            chain_id,
            deleted_events = events.rows_affected(),
            deleted_blocks = blocks.rows_affected(),
            "reset all indexed data for chain after irreconcilable genesis mismatch"
        );
        Ok(RollbackStats {
            deleted_events: events.rows_affected(),
            deleted_blocks: blocks.rows_affected(),
            ancestor_block: 0,
        })
    }

    async fn list_events(&self, filter: EventListFilter) -> Result<Vec<IndexedEvent>, AppError> {
        let rows = sqlx::query_as::<_, IndexedEventRow>(
            "SELECT id, chain_id, contract_address, transaction_hash, log_index,
                    sender_address, recipient_address, amount, block_number, block_hash,
                    block_timestamp, created_at
             FROM indexed_events
             WHERE chain_id = $1
               AND ($2::text IS NULL OR sender_address = $2 OR recipient_address = $2)
               AND ($3::text IS NULL OR contract_address = $3)
             ORDER BY block_number DESC, log_index DESC
             LIMIT $4 OFFSET $5",
        )
        .bind(filter.chain_id)
        .bind(filter.address)
        .bind(filter.contract)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database)?;
        Ok(rows.into_iter().map(IndexedEvent::from).collect())
    }

    async fn events_by_tx(
        &self,
        chain_id: i64,
        transaction_hash: &str,
    ) -> Result<Vec<IndexedEvent>, AppError> {
        let rows = sqlx::query_as::<_, IndexedEventRow>(
            "SELECT id, chain_id, contract_address, transaction_hash, log_index,
                    sender_address, recipient_address, amount, block_number, block_hash,
                    block_timestamp, created_at
             FROM indexed_events
             WHERE chain_id = $1 AND transaction_hash = $2
             ORDER BY log_index ASC",
        )
        .bind(chain_id)
        .bind(transaction_hash)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database)?;
        Ok(rows.into_iter().map(IndexedEvent::from).collect())
    }
}

async fn persist_block_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: i64,
    header: &BlockHeader,
    events: &[DecodedTransfer],
) -> Result<PersistOutcome, AppError> {
    let mut inserted = 0u64;
    let mut duplicates = 0u64;

    for event in events {
        let result = sqlx::query(
            "INSERT INTO indexed_events (
                chain_id, contract_address, transaction_hash, log_index,
                sender_address, recipient_address, amount, block_number, block_hash,
                block_timestamp
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
             ON CONFLICT (chain_id, transaction_hash, log_index) DO NOTHING",
        )
        .bind(chain_id)
        .bind(format!("{:#x}", event.contract))
        .bind(format!("{:#x}", event.transaction_hash))
        .bind(event.log_index as i32)
        .bind(format!("{:#x}", event.sender))
        .bind(format!("{:#x}", event.recipient))
        .bind(event.amount.to_string())
        .bind(event.block_number as i64)
        .bind(format!("{:#x}", event.block_hash))
        .bind(event.block_timestamp)
        .execute(&mut **tx)
        .await
        .map_err(AppError::database)?;

        if result.rows_affected() == 0 {
            duplicates += 1;
        } else {
            inserted += 1;
        }
    }

    sqlx::query(
        "INSERT INTO indexed_blocks (chain_id, block_number, block_hash, parent_hash, indexed_at)
         VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (chain_id, block_number) DO UPDATE SET
            block_hash = EXCLUDED.block_hash,
            parent_hash = EXCLUDED.parent_hash,
            indexed_at = NOW()",
    )
    .bind(chain_id)
    .bind(header.number as i64)
    .bind(header.hash_hex())
    .bind(header.parent_hash_hex())
    .execute(&mut **tx)
    .await
    .map_err(AppError::database)?;

    sqlx::query(
        "INSERT INTO indexer_checkpoints
            (chain_id, last_indexed_block, last_indexed_block_hash, updated_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (chain_id) DO UPDATE SET
            last_indexed_block = EXCLUDED.last_indexed_block,
            last_indexed_block_hash = EXCLUDED.last_indexed_block_hash,
            updated_at = NOW()",
    )
    .bind(chain_id)
    .bind(header.number as i64)
    .bind(header.hash_hex())
    .execute(&mut **tx)
    .await
    .map_err(AppError::database)?;

    Ok(PersistOutcome {
        inserted_events: inserted,
        duplicate_events: duplicates,
    })
}
