//! Session-level PostgreSQL advisory lock so two indexers cannot share a chain.
//!
//! The lock is held on a dedicated connection for the process lifetime. Pool
//! connections are not used, because returning a pooled connection would drop
//! a session lock.

use crate::domain::AppError;
use sqlx::postgres::PgConnection;
use sqlx::{Connection, PgPool};
use tracing::info;

pub struct IndexerLock {
    conn: PgConnection,
    key: i64,
}

impl IndexerLock {
    pub async fn acquire(pool: &PgPool, chain_id: u64) -> Result<Self, AppError> {
        let key = lock_key(chain_id);
        let mut conn = pool.acquire().await.map_err(AppError::database)?.detach();
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut conn)
            .await
            .map_err(AppError::database)?;
        if !locked {
            return Err(AppError::IndexerLockHeld(chain_id));
        }
        info!(chain_id, key, "acquired PostgreSQL advisory lock");
        Ok(Self { conn, key })
    }

    pub async fn release(mut self) -> Result<(), AppError> {
        let _: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
            .bind(self.key)
            .fetch_one(&mut self.conn)
            .await
            .map_err(AppError::database)?;
        let _ = self.conn.close().await;
        Ok(())
    }
}

fn lock_key(chain_id: u64) -> i64 {
    // Namespace `chainwatch` (0x4348) in the high 16 bits so other apps can
    // share the same Postgres cluster without colliding.
    let namespaced = ((0x4348_u64) << 48) | (chain_id & 0x0000_FFFF_FFFF_FFFF);
    namespaced as i64
}
