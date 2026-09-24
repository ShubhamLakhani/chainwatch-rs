use crate::domain::AppError;
use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn connect_pool(database_url: &str) -> Result<PgPool, AppError> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(AppError::database)
}
