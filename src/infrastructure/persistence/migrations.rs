use crate::domain::AppError;
use sqlx::PgPool;
use tracing::info;

pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(AppError::database)?;
    info!("database migrations applied");
    Ok(())
}
